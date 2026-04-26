/// 命令（Command）定义：CQRS 写操作意图的类型化表示
pub mod command;
/// 查询（Query）定义：CQRS 读操作的类型化请求与视图模型
pub mod query;
/// 命令/查询处理器与分发器：将请求路由到对应处理器
pub mod handler;
/// 事件存储（Event Store）：不可变事件流的持久化与检索
pub mod event_store;
/// 聚合根（Aggregate）：领域模型的状态管理与事件溯源
pub mod aggregate;

pub use command::{Command, CreateDocumentCommand, UpdateDocumentCommand, DeleteDocumentCommand, CreateNodeCommand, LinkNodesCommand, IngestFileCommand, ReindexCommand};
pub use query::{Query, GetDocumentQuery, SearchDocumentsQuery, GetNodeQuery, ListDocumentsQuery, GetGraphStatsQuery};
pub use handler::{CommandHandler, QueryHandler, CommandDispatcher, QueryDispatcher};
pub use event_store::{EventStore, StoredEvent, EventMetadata, SurrealEventStore};
pub use aggregate::{Aggregate, AggregateRepository, DocumentAggregate, DocumentCommand, DocumentEvent, AggregateError, SnapshotStore, MemorySnapshotStore};
