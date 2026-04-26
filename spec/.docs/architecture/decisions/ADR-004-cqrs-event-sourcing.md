# ADR-004: 实现 CQRS + Event Sourcing

## 状态
✅ **Accepted** (2026-04-13)

## Context (背景)

文本全结构化知识系统面临以下业务挑战：

1. **读写比例失衡**：读操作（搜索、浏览、查询）远多于写操作（创建、更新、删除），估计读写比约为 **100:1**
2. **审计合规要求**：作为企业级系统，必须能够完整追溯每一次数据变更的历史
3. **复杂业务不变量**：文档的状态变更涉及多个聚合根的一致性（如删除文档需级联清理 Block/Token/Reference）
4. **性能优化需求**：读操作需要优化的查询模型，不需要加载完整的领域对象

传统的 CRUD 架构（同一个模型同时服务于读写）导致：
- 读模型包含不必要的写相关字段和逻辑
- 无法高效地优化查询路径
- 变更历史只能通过额外的 audit_log 表间接获取

## Decision (决定)

实施 **CQRS (Command Query Responsibility Segregation)** + **Event Sourcing** 双模式架构。

### 架构概览

```
┌──────────┐     Command      ┌─────────────────┐     Events      ┌──────────────┐
│  Client  │ ──────────────→ │  Command        │ ────────────→ │   Event Store │
│          │                  │  Dispatcher     │               │  (SurrealDB)  │
└──────────┘                  └────────┬────────┘               └──────┬───────┘
                                       │                               │
                              Commands │                        StoredEvents
                                       ▼                               ▼
                              ┌─────────────────┐           ┌──────────────┐
                              │  Aggregate Root  │◀──────────│  Projector   │
                              │  (State Machine) │  Replays   │  (Read Model) │
                              └────────┬────────┘           └──────┬───────┘
                                       │                           │
                                Apply Events                 Read Models
                                       ▼                           ▼
                              ┌─────────────────┐           ┌──────────────┐
                              │  Write Database  │           │ Query Model  │
                              │  (SurrealDB)     │           │ (Optimized)  │
                              └─────────────────┘           └──────────────┘
                                             ↑                           │
                                      Writes │                    Reads    │
                                             └─────────────────────────┘
```

### 核心组件

#### Command（命令）— 写操作意图

```rust
/// 所有命令的统一 trait
pub trait Command: Send + Sync + Debug {}

/// 示例命令
pub struct CreateDocumentCommand {
    pub path: String,
    pub title: String,
    pub source_type: SourceType,
    pub content: Vec<u8>,
}

pub struct IngestFileCommand {
    pub file_path: PathBuf,
    pub source_type: Option<SourceType>,
}
```

#### Aggregate（聚合根）— 一致性边界

```rust
/// 文档聚合根 — 管理文档的完整生命周期
pub struct DocumentAggregate {
    id: String,
    version: i64,
    state: DocumentState,
    pending_events: Vec<DocumentEvent>,
}

impl DocumentAggregate {
    /// 执行命令并产生未提交事件
    pub fn execute(&mut self, cmd: DocumentCommand) -> Result<Vec<DocumentEvent>> { ... }

    /// 将已持久化的事件应用到状态机
    pub fn apply(&mut self, event: &DocumentEvent) -> Result<()> { ... }

    /// 从事件流重建聚合根状态
    pub fn rebuild_from(events: &[StoredEvent]) -> Result<Self> { ... }
}
```

#### Event Store（事件存储） — 不可变事件日志

```rust
/// 事件存储接口
#[async_trait]
pub trait EventStore: Send + Sync {
    /// 追加新事件到指定聚合根的事件流
    async fn append_events(
        &self,
        aggregate_id: &str,
        expected_version: i64,
        events: Vec<DomainEvent>,
    ) -> Result<()>;

    /// 加载聚合根的全部事件流
    async fn load_events(&self, aggregate_id: &str) -> Result<Vec<StoredEvent>>;
}
```

#### Query（查询）— 优化的读操作

```rust
/// 查询接口 — 直接从优化的读模型读取
pub trait Query: Send + Sync + Debug {}

pub struct GetDocumentQuery { pub id: String }
pub struct SearchDocumentsQuery { pub query: String, pub limit: u32 }
pub struct GetGraphStatsQuery {}  // 聚合统计查询
```

### 并发控制：乐观锁

```rust
// CommandHandler 在执行前检查版本号
let current_version = aggregate.version;
if command.expected_version != Some(current_version) {
    return Err(AggregateError::ConcurrencyConflict {
        expected: command.expected_version,
        actual: current_version,
    });
}
// 执行命令 → 产生事件 → 追加到 Event Store（原子操作）
```

## Consequences (影响)

### 正面影响

- 🟢 **读写独立优化**：读模型可以为查询场景定制索引和反范式化设计
- 🟢 **完整审计历史**：Event Sourcing 天然保留所有状态变更的时间线
- 🟢 **时间旅行调试**：可以从任意历史时间点重建系统状态
- 🟢 **业务不变量保护**：聚合根封装了一致性边界，防止非法状态转换
- 🟢 **可扩展的事件消费者**：新增 Projection（投影）不影响现有逻辑

### 负面影响

- 🔴 **最终一致性**：写操作完成后，读模型可能有短暂延迟（通常 < 100ms）
- 🔴 **事件存储增长**：Event Store 会持续增长，需要定期快照(Snapshot)压缩
- 🔴 **学习曲线**：CQRS/ES 是复杂的架构模式，团队成员需要培训
- 🔴 **代码量增加**：相比简单 CRUD，CQRS 需要更多的样板代码

### 缓解措施

- 使用 Snapshot 策略（每 N 个事件生成一次状态快照）控制事件流长度
- 对于简单的 CRUD 操作（如用户偏好设置），保持传统模式不走 CQRS
- 提供清晰的 CQRS 模板代码减少重复

## Alternatives (替代方案)

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| **传统 CRUD** | 简单、易理解 | 读写无法独立优化、无内置审计 | ❌ 不满足企业级需求 |
| **仅 CQRS (无 ES)** | 读模型优化 | 仍需额外机制记录历史 | ❌ 审计需求无法满足 |
| **仅 ES (无 CQRS)** | 完整历史 | 读操作需要重放事件流，性能差 | ❌ 读性能不足 |
| **CQRS + ES** ✅ | 读写优化 + 完整审计 | 复杂度高 | ✅ **选定方案** |

## References

- [Martin Fowler - CQRS](https://martinfowler.com/bliks/CQRS.html)
- [Martin Fowler - Event Sourcing](https://martinfowler.com/eaaDev/EventSourcing.html)
- [Microsoft - CQRS Pattern](https://learn.microsoft.com/en-us/azure/architecture/patterns/cqrs)
