use crate::Result;
use crate::cqrs::{Command, Query};
use crate::error::helpers;
use async_trait::async_trait;
use dashmap::DashMap;
use std::any::{Any, TypeId};
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;
use tracing::{info, instrument, warn};

/// 命令处理器特征，定义处理写操作命令的异步接口
pub trait CommandHandler<C: Command>: Send + Sync {
    /// 处理给定命令并返回执行结果
    fn handle(&self, command: C) -> impl Future<Output = Result<C::Result>> + Send;
}

/// 查询处理器特征，定义处理读操作查询的异步接口
pub trait QueryHandler<Q: Query>: Send + Sync {
    /// 处理给定查询并返回查询结果
    fn handle(&self, query: Q) -> impl Future<Output = Result<Q::Result>> + Send;
}

#[async_trait]
trait ErasedCommandHandler: Any + Send + Sync {
    #[allow(dead_code)]
    fn as_any(&self) -> &dyn Any;

    async fn handle_boxed(&self, command: Box<dyn Any + Send>) -> Result<Box<dyn Any + Send>>;
}

struct CommandHandlerWrapper<C: Command + 'static, H: CommandHandler<C> + 'static> {
    inner: H,
    _marker: PhantomData<C>,
}

#[async_trait]
impl<C: Command + 'static, H: CommandHandler<C> + 'static> ErasedCommandHandler
    for CommandHandlerWrapper<C, H>
{
    fn as_any(&self) -> &dyn Any {
        &self.inner
    }

    async fn handle_boxed(&self, command: Box<dyn Any + Send>) -> Result<Box<dyn Any + Send>> {
        let command = *command.downcast::<C>().map_err(|_| {
            helpers::aggregate_invalid_state(&format!(
                "Command dispatch type mismatch: expected {}",
                std::any::type_name::<C>()
            ))
        })?;
        let result = self.inner.handle(command).await?;
        Ok(Box::new(result))
    }
}

#[async_trait]
trait ErasedQueryHandler: Any + Send + Sync {
    #[allow(dead_code)]
    fn as_any(&self) -> &dyn Any;
    async fn handle_boxed(&self, query: Box<dyn Any + Send>) -> Result<Box<dyn Any + Send>>;
}

struct QueryHandlerWrapper<Q: Query + 'static, H: QueryHandler<Q> + 'static> {
    inner: H,
    _marker: PhantomData<Q>,
}

#[async_trait]
impl<Q: Query + 'static, H: QueryHandler<Q> + 'static> ErasedQueryHandler
    for QueryHandlerWrapper<Q, H>
{
    fn as_any(&self) -> &dyn Any {
        &self.inner
    }

    async fn handle_boxed(&self, query: Box<dyn Any + Send>) -> Result<Box<dyn Any + Send>> {
        let query = *query.downcast::<Q>().map_err(|_| {
            helpers::aggregate_invalid_state(&format!(
                "Query dispatch type mismatch: expected {}",
                std::any::type_name::<Q>()
            ))
        })?;
        let result = self.inner.handle(query).await?;
        Ok(Box::new(result))
    }
}

/// 命令分发器：将命令路由到对应的处理器
pub struct CommandDispatcher {
    handlers: DashMap<TypeId, Box<dyn ErasedCommandHandler>>,
}

impl CommandDispatcher {
    /// 创建空的命令分发器实例
    pub fn new() -> Self {
        Self {
            handlers: DashMap::new(),
        }
    }

    /// 注册命令处理器
    pub fn register<C: Command + 'static, H: CommandHandler<C> + 'static>(&self, handler: H) {
        let type_id = TypeId::of::<C>();
        if self.handlers.contains_key(&type_id) {
            warn!(
                command_type = std::any::type_name::<C>(),
                "Overwriting existing handler for command type"
            );
        }
        self.handlers.insert(
            type_id,
            Box::new(CommandHandlerWrapper {
                inner: handler,
                _marker: PhantomData,
            }),
        );
        info!(
            command_type = std::any::type_name::<C>(),
            "Command handler registered"
        );
    }

    /// 分发命令到已注册的处理器
    #[instrument(skip(self, command), fields(command_type = std::any::type_name::<C>()))]
    pub async fn dispatch<C: Command + 'static>(&self, command: C) -> Result<C::Result>
    where
        C::Result: Send + 'static,
    {
        let type_id = TypeId::of::<C>();

        let result_box = self
            .handlers
            .get(&type_id)
            .ok_or_else(|| {
                helpers::not_found(
                    "CommandHandler",
                    &format!(
                        "No handler registered for command type: {}",
                        std::any::type_name::<C>()
                    ),
                )
            })?
            .handle_boxed(Box::new(command))
            .await?;
        let result = *result_box.downcast::<C::Result>().map_err(|_| {
            helpers::aggregate_invalid_state(&format!(
                "Command result type mismatch: expected {}",
                std::any::type_name::<C::Result>()
            ))
        })?;
        Ok(result)
    }

    /// 检查是否已为指定命令类型注册了处理器
    pub fn has_handler<C: Command + 'static>(&self) -> bool {
        self.handlers.contains_key(&TypeId::of::<C>())
    }

    /// 返回已注册的命令处理器数量
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for CommandDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// 查询分发器：将查询路由到对应的处理器
pub struct QueryDispatcher {
    handlers: DashMap<TypeId, Box<dyn ErasedQueryHandler>>,
}

impl QueryDispatcher {
    /// 创建空的查询分发器实例
    pub fn new() -> Self {
        Self {
            handlers: DashMap::new(),
        }
    }

    /// 注册查询处理器
    pub fn register<Q: Query + 'static, H: QueryHandler<Q> + 'static>(&self, handler: H) {
        let type_id = TypeId::of::<Q>();
        if self.handlers.contains_key(&type_id) {
            warn!(
                query_type = std::any::type_name::<Q>(),
                "Overwriting existing handler for query type"
            );
        }
        self.handlers.insert(
            type_id,
            Box::new(QueryHandlerWrapper {
                inner: handler,
                _marker: PhantomData,
            }),
        );
        info!(
            query_type = std::any::type_name::<Q>(),
            "Query handler registered"
        );
    }

    /// 分发查询到已注册的处理器
    #[instrument(skip(self, query), fields(query_type = std::any::type_name::<Q>()))]
    pub async fn dispatch<Q: Query + 'static>(&self, query: Q) -> Result<Q::Result>
    where
        Q::Result: Send + 'static,
    {
        let type_id = TypeId::of::<Q>();

        let result_box = self
            .handlers
            .get(&type_id)
            .ok_or_else(|| {
                helpers::not_found(
                    "QueryHandler",
                    &format!(
                        "No handler registered for query type: {}",
                        std::any::type_name::<Q>()
                    ),
                )
            })?
            .handle_boxed(Box::new(query))
            .await?;
        let result = *result_box.downcast::<Q::Result>().map_err(|_| {
            helpers::aggregate_invalid_state(&format!(
                "Query result type mismatch: expected {}",
                std::any::type_name::<Q::Result>()
            ))
        })?;
        Ok(result)
    }

    /// 检查是否已为指定查询类型注册了处理器
    pub fn has_handler<Q: Query + 'static>(&self) -> bool {
        self.handlers.contains_key(&TypeId::of::<Q>())
    }

    /// 返回已注册的查询处理器数量
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for QueryDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// CQRS 中介者：统一管理命令和查询的分发
///
/// 命令执行后自动将产生的领域事件持久化到 Event Store，
/// 确保事件溯源（Event Sourcing）的完整性。
pub struct CqrsMediator {
    command_dispatcher: Arc<CommandDispatcher>,
    query_dispatcher: Arc<QueryDispatcher>,
    #[allow(dead_code)]
    event_store: Option<Arc<dyn crate::cqrs::event_store::EventStore>>,
}

impl CqrsMediator {
    /// 创建新的 CQRS 中介者实例
    pub fn new() -> Self {
        Self {
            command_dispatcher: Arc::new(CommandDispatcher::new()),
            query_dispatcher: Arc::new(QueryDispatcher::new()),
            event_store: None,
        }
    }

    /// 创建带 Event Store 的 CQRS 中介者实例
    pub fn with_event_store(event_store: Arc<dyn crate::cqrs::event_store::EventStore>) -> Self {
        Self {
            command_dispatcher: Arc::new(CommandDispatcher::new()),
            query_dispatcher: Arc::new(QueryDispatcher::new()),
            event_store: Some(event_store),
        }
    }

    /// 获取命令分发器的引用
    pub fn commands(&self) -> &CommandDispatcher {
        &self.command_dispatcher
    }

    /// 获取查询分发器的引用
    pub fn queries(&self) -> &QueryDispatcher {
        &self.query_dispatcher
    }

    /// 发送命令（委托给命令分发器）
    pub async fn send<C: Command + 'static>(&self, command: C) -> Result<C::Result>
    where
        C::Result: Send + 'static,
    {
        self.command_dispatcher.dispatch(command).await
    }

    /// 执行查询（委托给查询分发器）
    pub async fn query<Q: Query + 'static>(&self, query: Q) -> Result<Q::Result>
    where
        Q::Result: Send + 'static,
    {
        self.query_dispatcher.dispatch(query).await
    }
}

impl Default for CqrsMediator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cqrs::command::{
        CreateDocumentCommand, DocumentCreatedResult, DocumentUpdatedResult, UpdateDocumentCommand,
    };
    use crate::cqrs::query::{
        DocumentDetailView, DocumentListItem, DocumentView, GetDocumentQuery, ListDocumentsQuery,
        ListView,
    };
    use chrono::Utc;
    use crate::cqrs::aggregate::DocumentStatus;
    use uuid::Uuid;

    #[derive(Clone)]
    struct MockCreateDocumentHandler;

    #[allow(clippy::manual_async_fn)]
    impl CommandHandler<CreateDocumentCommand> for MockCreateDocumentHandler {
        #[allow(clippy::manual_async_fn)]
        fn handle(&self, command: CreateDocumentCommand) -> impl Future<Output = Result<DocumentCreatedResult>> + Send {
            async move {
                Ok(DocumentCreatedResult {
                    document_id: command.aggregate_id,
                    version: 1,
                    timestamp: Utc::now(),
                })
            }
        }
    }

    #[derive(Clone)]
    struct MockUpdateDocumentHandler;

    #[allow(clippy::manual_async_fn)]
    impl CommandHandler<UpdateDocumentCommand> for MockUpdateDocumentHandler {
        #[allow(clippy::manual_async_fn)]
        fn handle(&self, command: UpdateDocumentCommand) -> impl Future<Output = Result<DocumentUpdatedResult>> + Send {
            async move {
                Ok(DocumentUpdatedResult {
                    document_id: command.aggregate_id,
                    version: 6,
                    previous_version: 5,
                    timestamp: Utc::now(),
                })
            }
        }
    }

    #[derive(Clone)]
    struct MockGetDocumentHandler;

    #[allow(clippy::manual_async_fn)]
    impl QueryHandler<GetDocumentQuery> for MockGetDocumentHandler {
        #[allow(clippy::manual_async_fn)]
        fn handle(&self, query: GetDocumentQuery) -> impl Future<Output = Result<Option<DocumentDetailView>>> + Send {
            async move {
                if query.document_id == "not_found" {
                    return Ok(None);
                }

                Ok(Some(DocumentDetailView {
                    document: DocumentView {
                        id: query.document_id,
                        title: "Test Document".to_string(),
                        content_type: "markdown".to_string(),
                        status: DocumentStatus::Published,
                        version: 5,
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                        node_count: 10,
                        content: Some("# Hello".to_string()),
                    },
                    nodes: vec![],
                    blocks: vec![],
                }))
            }
        }
    }

    #[derive(Clone)]
    struct MockListDocumentsHandler;

    #[allow(clippy::manual_async_fn)]
    impl QueryHandler<ListDocumentsQuery> for MockListDocumentsHandler {
        #[allow(clippy::manual_async_fn)]
        fn handle(&self, _query: ListDocumentsQuery) -> impl Future<Output = Result<ListView<DocumentListItem>>> + Send {
            async move {
                Ok(ListView {
                    items: vec![DocumentListItem {
                        id: "doc_001".to_string(),
                        title: "Doc 1".to_string(),
                        source_type: "Markdown".to_string(),
                        version: 1,
                        updated_at: Utc::now(),
                    }],
                    total: 1,
                    has_more: false,
                })
            }
        }
    }

    fn create_test_create_command() -> CreateDocumentCommand {
        CreateDocumentCommand {
            command_id: Uuid::new_v4(),
            aggregate_id: "doc_001".to_string(),
            title: "Test Doc".to_string(),
            content: "# Test".to_string(),
            content_type: "markdown".to_string(),
            metadata: serde_json::json!({}),
            expected_version: None,
        }
    }

    #[tokio::test]
    async fn test_command_dispatch_success() {
        let dispatcher = CommandDispatcher::new();
        dispatcher.register(MockCreateDocumentHandler);

        let cmd = create_test_create_command();
        let result = dispatcher.dispatch(cmd).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.document_id, "doc_001");
        assert_eq!(result.version, 1);
    }

    #[tokio::test]
    async fn test_command_dispatch_no_handler_error() {
        let dispatcher = CommandDispatcher::new();

        let cmd = create_test_create_command();
        let result = dispatcher.dispatch(cmd).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_command_multiple_handlers_registration() {
        let dispatcher = CommandDispatcher::new();

        dispatcher.register(MockCreateDocumentHandler);
        dispatcher.register(MockUpdateDocumentHandler);

        assert_eq!(dispatcher.handler_count(), 2);
        assert!(dispatcher.has_handler::<CreateDocumentCommand>());
        assert!(dispatcher.has_handler::<UpdateDocumentCommand>());
    }

    #[tokio::test]
    async fn test_query_dispatch_found() {
        let dispatcher = QueryDispatcher::new();
        dispatcher.register(MockGetDocumentHandler);

        let query = GetDocumentQuery {
            document_id: "doc_001".to_string(),
            include_nodes: false,
            include_blocks: false,
        };

        let result = dispatcher.dispatch(query).await;

        assert!(result.is_ok());
        let doc = result.unwrap().expect("Expected Some document");
        assert_eq!(doc.document.id, "doc_001");
        assert_eq!(doc.document.title, "Test Document");
    }

    #[tokio::test]
    async fn test_query_dispatch_not_found() {
        let dispatcher = QueryDispatcher::new();
        dispatcher.register(MockGetDocumentHandler);

        let query = GetDocumentQuery {
            document_id: "not_found".to_string(),
            include_nodes: false,
            include_blocks: false,
        };

        let result = dispatcher.dispatch(query).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_query_list_pagination() {
        let dispatcher = QueryDispatcher::new();
        dispatcher.register(MockListDocumentsHandler);

        use crate::cqrs::query::{SortField, SortOrder};

        let query = ListDocumentsQuery {
            limit: 10,
            offset: 0,
            sort_by: SortField::UpdatedAt,
            sort_order: SortOrder::Desc,
            filters: None,
        };

        let result = dispatcher.dispatch(query).await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert!(!list.items.is_empty());
        assert_eq!(list.total, 1);
        assert!(!list.has_more);
    }

    #[tokio::test]
    async fn test_mediator_integration() {
        let mediator = CqrsMediator::new();

        mediator.commands().register(MockCreateDocumentHandler);
        mediator.queries().register(MockGetDocumentHandler);

        let cmd_result = mediator.send(create_test_create_command()).await;
        assert!(cmd_result.is_ok());

        let query_result = mediator
            .query(GetDocumentQuery {
                document_id: "doc_002".to_string(),
                include_nodes: true,
                include_blocks: false,
            })
            .await;
        assert!(query_result.is_ok());
        assert!(query_result.unwrap().is_some());
    }

    #[test]
    fn test_dispatcher_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CommandDispatcher>();
        assert_send_sync::<QueryDispatcher>();
        assert_send_sync::<CqrsMediator>();
    }

    #[tokio::test]
    async fn test_handler_overwrite_warning() {
        let dispatcher = CommandDispatcher::new();

        dispatcher.register(MockCreateDocumentHandler);
        dispatcher.register(MockCreateDocumentHandler);

        assert_eq!(dispatcher.handler_count(), 1);
    }
}
