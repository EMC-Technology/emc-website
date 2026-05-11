/// 聚合根（Aggregate）：领域模型的状态管理与事件溯源
pub mod aggregate;
/// 命令（Command）定义：CQRS 写操作意图的类型化表示
pub mod command;
/// 事件存储（Event Store）：不可变事件流的持久化与检索
pub mod event_store;
/// 命令/查询处理器与分发器：将请求路由到对应处理器
pub mod handler;
/// 投影（Projection）：CQRS 查询端的事件流到读模型转换
pub mod projection;
/// 查询（Query）定义：CQRS 读操作的类型化请求与视图模型
pub mod query;

pub use aggregate::{
    Aggregate, AggregateError, AggregateRepository, DocumentAggregate, DocumentCommand,
    DocumentEvent, MemorySnapshotStore, SnapshotStore,
};
pub use command::{
    Command, CreateDocumentCommand, CreateNodeCommand, DeleteDocumentCommand, IngestFileCommand,
    LinkNodesCommand, ReindexCommand, UpdateDocumentCommand,
};
pub use event_store::{
    AggregateType, EventMetadata, EventStore, EventType, StoredEvent, SurrealEventStore,
};
pub use handler::{CommandDispatcher, CommandHandler, QueryDispatcher, QueryHandler};
pub use projection::{
    DocumentProjection, DocumentSummaryView, NodeStatsProjection, NodeStatsView, Projection,
    ProjectionRegistry,
};
pub use query::{
    GetDocumentQuery, GetGraphStatsQuery, GetNodeQuery, ListDocumentsQuery, Query,
    SearchDocumentsQuery,
};
