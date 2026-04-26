# ADR-008: 多级缓存架构 (L1 + L2)

## 状态
✅ **Accepted** (2026-04-13)

## Context (背景)

文本全结构化知识系统的读负载特征如下：

1. **高频重复读取**：同一文档/块可能被多次请求（如多个 WebSocket 客户端同时查看同一文件）
2. **延迟敏感**：API P99 延迟目标 < 100ms，数据库往返可能占据大部分预算
3. **多实例部署**：生产环境会运行多个 API Pod，缓存需要在实例间共享
4. **数据一致性要求**：写操作后的缓存失效需要可控的一致性窗口

单层缓存无法同时满足低延迟（进程内缓存）和跨实例共享（分布式缓存）的需求。

## Decision (决定)

实施 **L1 + L2 两级缓存架构**：

- **L1 Cache**: Moka（异步内存缓存），进程内，亚微秒级延迟
- **L2 Cache**: Redis（分布式缓存），跨实例共享，毫秒级延迟

### 架构设计

```
Client Request
      │
      ▼
┌─────────────────┐
│   L1 Cache      │ ← Moka (Tokio 异步)
│   (Process)     │   TTL: 5min (文档), 10min (块)
│   Hit Rate: ~80%│   Max Size: 10,000 entries
└────────┬────────┘
         │ Miss
         ▼
┌─────────────────┐
│   L2 Cache      │ ← Redis (分布式)
│   (Distributed) │   TTL: 5min (文档), 10min (块)
│   Hit Rate: ~15%│   Cluster Mode: Sentinel
└────────┬────────┘
         │ Miss
         ▼
┌─────────────────┐
│   SurrealDB     │ ← 持久存储
│   (Source of    │   SCHEMAFULL 约束
│    Truth)       │
└─────────────────┘
```

### L1 Cache — Moka 配置

```rust
// 位于 cache/l1_cache.rs
use moka::future::Cache as MokaCache;

pub struct L1Cache {
    documents: MokaCache<String, DocumentDto>,     // 文档缓存
    blocks: MokaCache<String, BlockDto>,           // 块缓存
    search_results: MokaCache<String, SearchResult>, // 搜索结果缓存
}

impl L1Cache {
    pub fn new() -> Self {
        Self {
            documents: MokaCache::builder()
                .max_capacity(10_000)           // 最大条目数
                .time_to_live(Duration::from_secs(300))  // TTL 5 分钟
                .time_to_idle(Duration::from_secs(60))   // 空闲淘汰 1 分钟
                .build(),
            // ... 其他缓存类似配置
        }
    }
}
```

**Moka 选型理由**：
- 基于 Tokio runtime 的异步 API（非阻塞）
- 线程安全（内部使用 `DashMap`）
- 支持 TTL + 大小双重驱逐策略
- 零外部依赖

### L2 Cache — Redis 配置

```rust
// 位于 cache/l2_cache.rs
use redis::aio::MultiplexedConnection;

pub struct L2Cache {
    conn: MultiplexedConnection,
    prefix: String,  // 键前缀 "ks:" (Knowledge System)
}

impl L2Cache {
    /// 带 Cache-Aside 模式的读取
    pub async fn get_or_fetch<T>(&self, key: &str, fetch_fn: impl Future<Output = Result<T>>)
    -> Result<T>
    where T: Serialize + DeserializeOwned {
        if let Some(cached) = self.get(key).await? {
            return Ok(cached);
        }
        let value = fetch_fn.await?;
        self.set(key, &value, ttl).await?;
        Ok(value)
    }
}
```

**Redis 选型理由**：
- 业界标准分布式缓存，运维经验丰富
- 发布/订阅支持用于跨实例缓存失效通知
- 数据结构丰富（String、Hash、Set 用于不同场景）
- Sentinel 模式提供高可用性

### 缓存一致性机制

```
Write Operation
      │
      ▼
┌──────────────┐     EventBus Publish     ┌──────────────────┐
│ CQRS Command │ ─────────────────────→ │ Cache Invalidation │
│ (写操作)      │     "cache.invalidate"  │ Publisher          │
└──────────────┘                          └────────┬──────────┘
                                                   │
                          ┌─────────────────────────┼─────────────────────────┐
                          ▼                         ▼                         ▼
                   ┌──────────┐              ┌──────────┐              ┌──────────┐
                   │ L1 Evict │              │ L2 Delete│              │ Redis Pub│
                   │ (同步)   │              │ (同步)   │              │ /Sub     │
                   │ 本地立即  │              │ 分布式   │              │ (异步)   │
                   │ 驱逐      │              │ 删除      │              │ 通知其他  │
                   └──────────┘              └──────────┘              │ 实例     │
                                                                        └──────────┘
```

**一致性级别**：最终一致性（Eventual Consistency）
- L1 缓存：本地同步驱逐（< 1μs）
- L2 缓存：同步删除 + 异步广播（< 5ms）
- 一致性窗口：通常 < 100ms

### 缓存键命名规范

详见 [data-model.md](../data-model.md#5-缓存键命名规范) 第 5 节。

## Consequences (影响)

### 正面影响

- 🟢 **延迟大幅降低**：L1 命中时延迟 < 1ms（vs 数据库 10-50ms）
- 🟢 **数据库压力减少**：约 95% 的读请求被缓存拦截
- 🟢 **跨实例共享**：L2 缓存确保多实例数据一致性
- 🟢 **优雅降级**：Redis 故障时 L1 仍可用，服务不完全中断

### 负面影响

- 🔴 **一致复杂性**：两级缓存的失效协调增加复杂度
- 🔴 **内存占用**：L1 缓存占用进程内存（需监控）
- 🔴 **缓存穿透/雪崩风险**：需要保护机制
- 🔴 **调试困难**：缓存命中/未命中行为难以追踪

### 保护机制

| 风险 | 保护策略 |
|------|----------|
| **缓存穿透** | 布隆过滤器预判 + 空值短期缓存 |
| **缓存雪崩** | TTL 随机抖动 ±10% |
| **缓存击穿** | 热点 Key 互斥锁重建 |
| **L2 故障降级** | 自动 fallback 到直连 DB |

## Alternatives (替代方案)

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| **仅 L1 内存缓存** | 最快延迟、最简实现 | 不支持跨实例共享 | ❌ 多实例场景不足 |
| **仅 L2 Redis** | 跨实例共享 | 每次请求都有网络开销 | ❌ 延迟不够低 |
| **CDN 缓存** | 全球分发 | 仅适用于静态/半静态内容 | ❌ 动态数据不适合 |
| **L1(Moka) + L2(Redis)** ✅ | 兼顾延迟和共享 | 实现复杂度较高 | ✅ **选定方案** |

## References

- [Moka 文档](https://docs.rs/moka/latest/moka/)
- [Redis 缓存模式](https://redis.io/docs/manual/patterns/caching/)
- [Cache-Aside Pattern](https://martinfowler.com/eaaCatalog/cacheAside.html)
