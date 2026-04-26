# ADR-002: 选择 SurrealDB 作为主数据库

## 状态
✅ **Accepted** (2026-04-13)

## Context (背景)

文本全结构化知识系统的数据模型具有以下独特特征：

1. **图结构数据**：Reference 边连接任意实体（Document ↔ Block ↔ Token），需要原生图查询能力
2. **多模型混合**：既有关系型数据（文档元信息）、又有图数据（引用关系）、还有向量数据（嵌入）
3. **Schema 灵活性**：不同来源类型（Markdown/Code/Plain）的字段集略有差异
4. **Rust 原生集成**：希望避免 FFI 开销和序列化成本

传统的关系型数据库（PostgreSQL + pg_citus 图扩展）或专用图数据库（Neo4j）都无法同时满足以上所有需求。

## Decision (决定)

选择 **SurrealDB 1.3** 作为主数据库后端，存储引擎选用 **KV-RocksDB**。

### 架构定位

```
┌─────────────────────────────────────────────┐
│              knowledge-api                   │
│         (Axum HTTP/WebSocket)                │
└──────────────────┬──────────────────────────┘
                   │ SurrealDB SDK (WebSocket)
                   ▼
┌─────────────────────────────────────────────┐
│            SurrealDB 1.3                     │
│  ┌─────────┐ ┌──────────┐ ┌──────────────┐ │
│  │ Table   │ │ Relation │ │ Vector Index  │ │
│  │ (CRUD)  │ │ (Graph)  │ │ (MTREE)      │ │
│  └────┬────┘ └─────┬────┘ └──────┬───────┘ │
│       └──────────────┼────────────┘         │
│                      ▼                       │
│            KV-RocksDB Engine               │
└─────────────────────────────────────────────┘
```

### 核心选型理由

#### 1. 多模型统一 (Multi-Model)

SurrealDB 同时支持四种数据模型：

| 数据模型 | 用途 | SurrealDB 实现 |
|----------|------|----------------|
| **关系型** | Document/Block/Token 表 | 标准 `DEFINE TABLE` + SCHEMAFULL |
| **图数据库** | Reference 有向边 | `TYPE RELATION FROM ANY TO ANY` |
| **文档型** | 灵活元数据 | 内嵌 JSON 字段 |
| **向量搜索** | Block 语义检索 | MTREE 向量索引 |

单一数据库服务覆盖全部需求，避免了微服务间多数据库同步的复杂性。

#### 2. 原生 Rust SDK

```rust
// 无需通过 FFI 或 HTTP REST API，直接使用 Rust-native SDK
let db = Surreal::new::<Ws>("localhost:8000").await?;
let docs: Vec<Document> = db.select("document").await?;
```

- 零序列化开销（内部使用 `serde` 直接映射）
- 类型安全的查询构建器
- 连接池内置支持

#### 3. 图查询能力

```surql
-- 查找函数 A 的所有被调用函数（两跳 Usage 关系）
SELECT *, ->reference->token.* AS callees
FROM token
WHERE content = 'function_A'
AND ->reference.ref_type = 'Usage';
```

SurrealQL 的图遍历语法简洁且强大，避免了在应用层手动实现图算法。

#### 4. SCHEMAFULL 模式

```surql
DEFINE TABLE document SCHEMAFULL;
DEFINE FIELD hash ON document TYPE string
    ASSERT string::len($value) == 64 AND $value =~ /^[0-9a-fA-F]{64}$/;
```

- 强制字段类型约束，防止脏数据写入
- `ASSERT` 条件实现业务规则级别的验证
- 相比 Schemaless 模式提供更好的数据完整性保证

## Consequences (影响)

### 正面影响

- 🟢 **架构简化**：单一数据库替代 PostgreSQL + Neo4j + Redis(部分) 的组合
- 🟢 **性能优势**：Rust-native SDK 避免了跨进程 IPC 开销
- 🟢 **开发效率**：SurQL 图查询语法减少应用层图算法代码量
- 🟢 **部署简化**：单个二进制文件即可运行（嵌入式模式）
- 🟢 **向量索引内置**：MTREE 索引支持余弦距离语义搜索

### 负面影响

- 🔴 **生态成熟度**：相比 PostgreSQL，社区规模较小，第三方工具集成有限
- 🔴 **学习曲线**：SurQL 是领域特定语言，团队需要额外学习成本
- 🔴 **生产运维经验**：大规模生产环境下的监控、备份、故障恢复实践较少
- 🔴 **KV-RocksDB 引擎**：需要管理 RocksDB 底层存储的 compaction 和磁盘空间

### 风险缓解

- 通过 [`schema.surql`](../../crates/knowledge-core/schema.surql) 进行完整的 Schema 版本化管理
- CI 包含 SurrealDB 集成测试确保兼容性
- Docker Compose 提供标准化开发环境

### 供应链风险与替代路线（V4.0 补充）

> **供应链现状**：SurrealDB 引入 ~200-230 个传递依赖，占项目总依赖的 22%-25%。声明版本 1.3，实际锁定 1.5.6。原生编译依赖 librocksdb-sys（C++ 工具链），跨平台构建困难。
>
> **替代路线**：公司自研的内存堆图对象数据库（LightField）将以 drop-in replacement 方式替代 SurrealDB，保持 Repository trait 接口不变，仅替换底层实现。自研数据库具备 0GC 内存堆、双向相对指针、编译期内存布局（CodeFirst）、绕过文件系统直读 SSD、光通讯、内嵌形式化证明等特性，实现供应链从 200+ 传递依赖收敛至 0 外部依赖。
>
> **开源基线产品策略**：SurrealDB 作为基线产品的持久层，验证"一个数据库承载全部知识库需求"的架构可行性和 Repository trait 接口设计的合理性。自研数据库继承同一接口，用户迁移成本趋近于零。

## Alternatives (替代方案)

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| **PostgreSQL + Apache Age** | 成熟稳定、生态丰富 | 图扩展功能有限、需额外维护 | ❌ 图查询能力不足 |
| **Neo4j + PostgreSQL** | 业界最强图数据库 | 双数据库同步复杂、非 Rust 原生 | ❌ 运维复杂度过高 |
| **MongoDB + ArangoDB** | 文档+图混合 | Rust SDK 质量参差不齐 | ❌ 类型安全性不足 |
| **SQLite + 自建图层** | 零依赖、嵌入式 | 需要自行实现图算法和向量索引 | ❌ 并发性能不足 |
| **SurrealDB** ✅ | 多模型统一、Rust 原生 | 生态较新 | ✅ **选定方案** |

## References

- [SurrealDB 官方文档](https://surrealdb.com/docs)
- [SurrealDB Rust SDK](https://docs.rs/surrealdb/latest/surrealdb/)
- [SCHEMAFULL vs SCHEMALESS](https://surrealdb.com/docs/introduction/schemas)
