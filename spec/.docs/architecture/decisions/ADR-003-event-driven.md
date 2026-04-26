# ADR-003: 引入事件驱动架构

## 状态
✅ **Accepted** (2026-04-13)

## Context (背景)

文本全结构化知识系统是一个高度模块化的分布式系统，包含以下需要松耦合通信的场景：

1. **跨模块通知**：Parser 解析完成后需通知 Core 写入数据库、API 触发嵌入计算
2. **缓存一致性**：写操作完成后需通知所有缓存层失效
3. **审计日志**：任何状态变更都需要记录审计事件
4. **实时推送**：WebSocket 客户端需要实时接收数据变更通知
5. **Agent 协调**：ReAct Agent 各步骤之间需要异步协调

传统的直接函数调用方式会导致：
- 模块间强耦合（循环依赖风险）
- 难以扩展新的监听者
- 测试时需要 mock 整个调用链

## Decision (决定)

引入基于 **发布-订阅模式 (Pub/Sub)** 的进程内事件总线（EventBus），通过 `event-driven` feature flag 控制启用。

### 架构设计

```
                    ┌─────────────────┐
                    │    EventBus     │
                    │  (全局单例)     │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        ▼                    ▼                    ▼
   [Publisher]          [Subscriber]          [Subscriber]
   (事件发布者)         (事件订阅者)           (事件订阅者)
        │                    │                    │
   Parser              CacheInvalidator       AuditLogger
   Repository          EmbeddingService       WSNotifier
   CQRS Handler        SearchIndexer          MetricsCollector
```

### 核心接口定义

位于 [`event/mod.rs`](../../crates/knowledge-core/src/event/mod.rs)：

```rust
/// 系统事件枚举（所有领域事件的统一入口）
pub enum SystemEvent {
    Knowledge(KnowledgeEvent),
    // 可扩展其他领域事件...
}

/// 知识领域事件
pub enum KnowledgeEvent {
    DocumentIngested { doc_id: String },
    BlockCreated { block_id: String, doc_id: String },
    EmbeddingComputed { block_id: String },
    CacheInvalidated { key_pattern: String },
    // ...
}

/// 事件处理器 trait
#[async_trait]
pub trait EventHandler<E>: Send + Sync {
    async fn handle(&self, event: &E) -> HandleResult;
}

/// 事件总线
pub struct EventBus {
    handlers: DashMap<TypeId, Vec<Box<dyn ErasedHandler>>>,
    config: EventBusConfig,
}
```

### 设计原则

1. **进程内**：事件总线仅在单进程内工作，不涉及跨进程消息队列
2. **异步优先**：所有处理器均为 `async fn`，基于 Tokio 运行时调度
3. **类型安全**：通过泛型和 TypeId 实现编译期事件类型检查
4. **错误隔离**：单个处理器失败不影响其他处理器的执行
5. **有序分发**：同一类型的事件按注册顺序串行分发（保证因果顺序）

## Consequences (影响)

### 正面影响

- 🟢 **解耦**：Publisher 与 Subscriber 完全解耦，无需相互引用
- 🟢 **可扩展**：新增监听者只需实现 `EventHandler` trait 并注册，无需修改 Publisher
- 🟢 **可测试**：可以单独测试每个事件处理器
- 🟢 **关注点分离**：每个模块只关注自己的职责（SRP 原则）

### 负面影响

- 🔴 **调试复杂度**：事件流是隐式的，比直接函数调用更难追踪
- 🔴 **执行顺序不确定性**：虽然同类型事件有序，但跨类型事件的相对顺序不确定
- 🔴 **内存开销**：事件对象需要在堆上分配以实现动态分发
- 🔴 **Feature Gate**：增加了 feature flag 管理复杂度

### 缓解措施

- 使用 `tracing` span 为每个事件添加追踪上下文
- 事件对象包含 `correlation_id` 用于关联相关事件
- 文档化事件契约（哪些事件在什么条件下触发）

## Alternatives (替代方案)

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| **直接函数调用** | 简单直观、易于调试 | 强耦合、难以扩展 | ❌ 不满足模块化解耦需求 |
| **Channel (tokio::sync::mpsc)** | 原生异步、零依赖 | 一对一通信、不支持多订阅者 | ❌ 不满足 Pub/Sub 场景 |
| **crossbeam-channel** | 高性能多生产者多消费者 | 同步为主、不适合 async | ❌ 与 Tokio 生态不匹配 |
| **async-channel** | 异步友好 | 功能基础、无类型安全分发 | ❌ 缺少类型系统保障 |
| **自研 EventBus** ✅ | 完全控制、深度集成业务 | 需要自行实现和维护 | ✅ **选定方案** |

## References

- [Event-Driven Architecture Pattern](https://microservices.io/patterns/data/event-driven-architecture.html)
- [GoF Observer Pattern](https://en.wikipedia.org/wiki/Observer_pattern)
