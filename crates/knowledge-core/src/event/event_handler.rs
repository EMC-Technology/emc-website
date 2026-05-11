//! 事件处理器（Event Handler）
//!
//! 本模块定义事件驱动架构中的消费者端：
//! - `Handler`: 异步事件处理器 trait
//! - 具体处理器实现：文档/节点/搜索/嵌入
//! - `AsyncEventHandler`: 包装器，将 handler 绑定到 EventBus receiver
//!
//! # 设计模式
//!
//! 采用 **Branch by Abstraction** 策略：
//! - 新代码通过 Handler trait + EventBus 通信
//! - 旧代码保持直接函数调用（通过 feature flag 控制）
//! - 渐进式迁移，两者可共存

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use super::event_bus::EventBus;
use super::types::{KnowledgeEvent, SystemEvent};

/// 异步事件处理器 trait
///
/// 所有事件消费者必须实现此 trait。框架负责从 EventBus 接收事件，
/// 分发给对应的 `handle()` 方法。
///
/// # 类型参数
///
/// - `E`: 事件类型（通常为 `KnowledgeEvent`）///
/// # 实现注意事项
///
/// - `handle()` 应尽快返回，避免阻塞事件循环
/// - 对于耗时操作（如 DB 写入、Embedding 计算），应在内部 spawn 子任务
/// - 错误应通过 `Result` 返回而非 panic
#[async_trait]
pub trait Handler<E: SystemEvent>: Send + Sync {
    /// 处理器名称（用于日志标识和 metrics 标签）
    fn name(&self) -> &str;

    /// 判断此处理器是否关心给定事件
    ///
    /// 返回 `false` 时框架会跳过此处理器，避免不必要的类型匹配开销。
    fn accepts(&self, event: &E) -> bool {
        let _ = event;
        true
    }

    /// 处理事件的核心方法
    ///
    /// # Errors
    ///
    /// 返回处理失败的具体原因。框架会根据配置决定是否重试。
    async fn handle(&self, event: E) -> std::result::Result<HandleResult, HandleError>;
}

/// 事件处理结果
#[derive(Debug)]
pub struct HandleResult {
    pub success: bool,
    pub message: String,
    pub duration_us: u128,
}

/// 事件处理错误
#[derive(Debug, thiserror::Error)]
pub enum HandleError {
    #[error("事件类型不匹配")]
    TypeMismatch,
    #[error("处理失败: {0}")]
    ProcessingFailed(String),
    #[error("{0}")]
    Other(String),
}

impl HandleResult {
    pub fn ok(message: impl Into<String>, duration_us: u128) -> Self {
        Self {
            success: true,
            message: message.into(),
            duration_us,
        }
    }

    pub fn err(message: impl Into<String>, duration_us: u128) -> Self {
        Self {
            success: false,
            message: message.into(),
            duration_us,
        }
    }
}

// ============================================================================
// 文档事件处理器
// ============================================================================

/// 文档生命周期事件处理器
///
/// 监听 DocumentIngested / DocumentParsed / DocumentIndexed / DocumentDeleted 事件，
/// 触发后续处理流程（如自动解析、索引更新、级联清理）。
pub struct DocumentEventHandler {
    name: String,
}

impl DocumentEventHandler {
    pub fn new() -> Self {
        Self {
            name: "document-event-handler".to_string(),
        }
    }

    async fn handle_ingested(
        &self,
        event: KnowledgeEvent,
    ) -> std::result::Result<HandleResult, HandleError> {
        let start = Instant::now();
        if let KnowledgeEvent::DocumentIngested(ref doc_event) = event {
            debug!(
                document_id = %doc_event.document_id,
                file_path = %doc_event.file_path,
                file_size = doc_event.file_size,
                "[DocumentEventHandler] 文档已摄入，触发解析流程"
            );
            Ok(HandleResult::ok(
                format!("文档 {} 摄入处理完成", doc_event.document_id),
                start.elapsed().as_micros(),
            ))
        } else {
            Err(HandleError::TypeMismatch)
        }
    }

    async fn handle_parsed(
        &self,
        event: KnowledgeEvent,
    ) -> std::result::Result<HandleResult, HandleError> {
        let start = Instant::now();
        if let KnowledgeEvent::DocumentParsed(ref parsed_event) = event {
            debug!(
                document_id = %parsed_event.document_id,
                block_count = parsed_event.block_count,
                token_count = parsed_event.token_count,
                "[DocumentEventHandler] 文档已解析，触发索引构建"
            );
            Ok(HandleResult::ok(
                format!(
                    "文档 {} 解析完成 ({} blocks, {} tokens)",
                    parsed_event.document_id, parsed_event.block_count, parsed_event.token_count
                ),
                start.elapsed().as_micros(),
            ))
        } else {
            Err(HandleError::TypeMismatch)
        }
    }

    async fn handle_deleted(
        &self,
        event: KnowledgeEvent,
    ) -> std::result::Result<HandleResult, HandleError> {
        let start = Instant::now();
        if let KnowledgeEvent::DocumentDeleted(ref del_event) = event {
            info!(
                document_id = %del_event.document_id,
                cascaded_blocks = del_event.cascaded_blocks,
                cascaded_tokens = del_event.cascaded_tokens,
                "[DocumentEventHandler] 文档删除级联清理完成"
            );
            Ok(HandleResult::ok(
                format!("文档 {} 删除清理完成", del_event.document_id),
                start.elapsed().as_micros(),
            ))
        } else {
            Err(HandleError::TypeMismatch)
        }
    }
}

#[async_trait]
impl Handler<KnowledgeEvent> for DocumentEventHandler {
    fn name(&self) -> &str {
        &self.name
    }

    fn accepts(&self, event: &KnowledgeEvent) -> bool {
        matches!(
            event,
            KnowledgeEvent::DocumentIngested(_)
                | KnowledgeEvent::DocumentParsed(_)
                | KnowledgeEvent::DocumentIndexed(_)
                | KnowledgeEvent::DocumentDeleted(_)
        )
    }

    async fn handle(
        &self,
        event: KnowledgeEvent,
    ) -> std::result::Result<HandleResult, HandleError> {
        match &event {
            KnowledgeEvent::DocumentIngested(_) => self.handle_ingested(event).await,
            KnowledgeEvent::DocumentParsed(_) => self.handle_parsed(event).await,
            KnowledgeEvent::DocumentIndexed(_) => {
                let start = Instant::now();
                debug!("[DocumentEventHandler] 文档已索引");
                Ok(HandleResult::ok(
                    "索引确认完成",
                    start.elapsed().as_micros(),
                ))
            }
            KnowledgeEvent::DocumentDeleted(_) => self.handle_deleted(event).await,
            KnowledgeEvent::NodeCreated(_)
            | KnowledgeEvent::NodeUpdated(_)
            | KnowledgeEvent::NodeDeleted(_)
            | KnowledgeEvent::NodeLinked(_)
            | KnowledgeEvent::EdgeCreated(_)
            | KnowledgeEvent::EdgeDeleted(_)
            | KnowledgeEvent::SearchPerformed(_)
            | KnowledgeEvent::QueryExecuted(_)
            | KnowledgeEvent::EmbeddingGenerated(_)
            | KnowledgeEvent::EmbeddingCached(_)
            | KnowledgeEvent::UserAction(_)
            | KnowledgeEvent::SystemHealthCheck(_) => Err(HandleError::TypeMismatch),
        }
    }
}

// ============================================================================
// 节点事件处理器
// ============================================================================

/// 知识节点事件处理器
///
/// 监听 NodeCreated / NodeUpdated / NodeDeleted / NodeLinked 事件，
/// 维护知识图谱的一致性状态。
pub struct NodeEventHandler {
    name: String,
}

impl NodeEventHandler {
    pub fn new() -> Self {
        Self {
            name: "node-event-handler".to_string(),
        }
    }
}

#[async_trait]
impl Handler<KnowledgeEvent> for NodeEventHandler {
    fn name(&self) -> &str {
        &self.name
    }

    fn accepts(&self, event: &KnowledgeEvent) -> bool {
        matches!(
            event,
            KnowledgeEvent::NodeCreated(_)
                | KnowledgeEvent::NodeUpdated(_)
                | KnowledgeEvent::NodeDeleted(_)
                | KnowledgeEvent::NodeLinked(_)
        )
    }

    async fn handle(
        &self,
        event: KnowledgeEvent,
    ) -> std::result::Result<HandleResult, HandleError> {
        let start = Instant::now();
        match &event {
            KnowledgeEvent::NodeCreated(e) => {
                debug!(
                    node_id = %e.node_id,
                    node_type = %e.node_type,
                    "[NodeEventHandler] 节点已创建"
                );
                Ok(HandleResult::ok(
                    format!("节点 {} 创建完成", e.node_id),
                    start.elapsed().as_micros(),
                ))
            }
            KnowledgeEvent::NodeUpdated(e) => {
                debug!(
                    node_id = %e.node_id,
                    changes = ?e.changes,
                    "[NodeEventHandler] 节点已更新"
                );
                Ok(HandleResult::ok(
                    format!("节点 {} 更新完成", e.node_id),
                    start.elapsed().as_micros(),
                ))
            }
            KnowledgeEvent::NodeDeleted(e) => {
                info!(node_id = %e.node_id, "[NodeEventHandler] 节点已删除");
                Ok(HandleResult::ok(
                    format!("节点 {} 删除完成", e.node_id),
                    start.elapsed().as_micros(),
                ))
            }
            KnowledgeEvent::NodeLinked(e) => {
                debug!(
                    from = %e.from_node_id,
                    to = %e.to_node_id,
                    relation = %e.relation_type,
                    "[NodeEventHandler] 节点链接已建立"
                );
                Ok(HandleResult::ok(
                    format!(
                        "链接 {} → {} ({}) 已建立",
                        e.from_node_id, e.to_node_id, e.relation_type
                    ),
                    start.elapsed().as_micros(),
                ))
            }
            KnowledgeEvent::DocumentIngested(_)
            | KnowledgeEvent::DocumentParsed(_)
            | KnowledgeEvent::DocumentIndexed(_)
            | KnowledgeEvent::DocumentDeleted(_)
            | KnowledgeEvent::EdgeCreated(_)
            | KnowledgeEvent::EdgeDeleted(_)
            | KnowledgeEvent::SearchPerformed(_)
            | KnowledgeEvent::QueryExecuted(_)
            | KnowledgeEvent::EmbeddingGenerated(_)
            | KnowledgeEvent::EmbeddingCached(_)
            | KnowledgeEvent::UserAction(_)
            | KnowledgeEvent::SystemHealthCheck(_) => Err(HandleError::TypeMismatch),
        }
    }
}

// ============================================================================
// 搜索事件处理器
// ============================================================================

/// 搜索与查询事件处理器
///
/// 监听 SearchPerformed / QueryExecuted 事件，
/// 触发搜索分析、热门查询统计、索引热度更新等。
pub struct SearchEventHandler {
    name: String,
}

impl SearchEventHandler {
    pub fn new() -> Self {
        Self {
            name: "search-event-handler".to_string(),
        }
    }
}

#[async_trait]
impl Handler<KnowledgeEvent> for SearchEventHandler {
    fn name(&self) -> &str {
        &self.name
    }

    fn accepts(&self, event: &KnowledgeEvent) -> bool {
        matches!(
            event,
            KnowledgeEvent::SearchPerformed(_) | KnowledgeEvent::QueryExecuted(_)
        )
    }

    async fn handle(
        &self,
        event: KnowledgeEvent,
    ) -> std::result::Result<HandleResult, HandleError> {
        let start = Instant::now();
        match &event {
            KnowledgeEvent::SearchPerformed(e) => {
                debug!(
                    query = %e.query,
                    result_count = e.result_count,
                    duration_ms = e.duration_ms,
                    "[SearchEventHandler] 搜索执行记录"
                );

                if e.duration_ms > 1000 {
                    warn!(
                        query = %e.query,
                        duration_ms = e.duration_ms,
                        "[SearchEventHandler] 慢查询警告 (>1s)"
                    );
                }

                Ok(HandleResult::ok(
                    format!(
                        "搜索 '{}' 完成 ({} 结果, {}ms)",
                        e.query, e.result_count, e.duration_ms
                    ),
                    start.elapsed().as_micros(),
                ))
            }
            KnowledgeEvent::QueryExecuted(e) => {
                debug!(
                    query_type = %e.query_type,
                    result_count = e.result_count,
                    duration_ms = e.duration_ms,
                    "[SearchEventHandler] 图查询执行记录"
                );
                Ok(HandleResult::ok(
                    format!(
                        "查询 [{}] 完成 ({} 结果, {}ms)",
                        e.query_type, e.result_count, e.duration_ms
                    ),
                    start.elapsed().as_micros(),
                ))
            }
            KnowledgeEvent::DocumentIngested(_)
            | KnowledgeEvent::DocumentParsed(_)
            | KnowledgeEvent::DocumentIndexed(_)
            | KnowledgeEvent::DocumentDeleted(_)
            | KnowledgeEvent::NodeCreated(_)
            | KnowledgeEvent::NodeUpdated(_)
            | KnowledgeEvent::NodeDeleted(_)
            | KnowledgeEvent::NodeLinked(_)
            | KnowledgeEvent::EdgeCreated(_)
            | KnowledgeEvent::EdgeDeleted(_)
            | KnowledgeEvent::EmbeddingGenerated(_)
            | KnowledgeEvent::EmbeddingCached(_)
            | KnowledgeEvent::UserAction(_)
            | KnowledgeEvent::SystemHealthCheck(_) => Err(HandleError::TypeMismatch),
        }
    }
}

// ============================================================================
// Embedding 事件处理器
// ============================================================================

/// 向量嵌入事件处理器
///
/// 监听 EmbeddingGenerated / EmbeddingCached 事件，
/// 管理 embedding 缓存失效策略和使用统计。
pub struct EmbeddingEventHandler {
    name: String,
}

impl EmbeddingEventHandler {
    pub fn new() -> Self {
        Self {
            name: "embedding-event-handler".to_string(),
        }
    }
}

#[async_trait]
impl Handler<KnowledgeEvent> for EmbeddingEventHandler {
    fn name(&self) -> &str {
        &self.name
    }

    fn accepts(&self, event: &KnowledgeEvent) -> bool {
        matches!(
            event,
            KnowledgeEvent::EmbeddingGenerated(_) | KnowledgeEvent::EmbeddingCached(_)
        )
    }

    async fn handle(
        &self,
        event: KnowledgeEvent,
    ) -> std::result::Result<HandleResult, HandleError> {
        let start = Instant::now();
        match &event {
            KnowledgeEvent::EmbeddingGenerated(e) => {
                debug!(
                    entity_id = %e.entity_id,
                    entity_type = %e.entity_type,
                    dim = e.embedding_dim,
                    model = %e.model_name,
                    gen_ms = e.generation_duration_ms,
                    "[EmbeddingEventHandler] 嵌入生成完成"
                );
                Ok(HandleResult::ok(
                    format!(
                        "嵌入 {} ({}D, {}) 生成完成",
                        e.entity_id, e.embedding_dim, e.model_name
                    ),
                    start.elapsed().as_micros(),
                ))
            }
            KnowledgeEvent::EmbeddingCached(e) => {
                debug!(
                    entity_id = %e.entity_id,
                    cache_key = %e.cache_key,
                    "[EmbeddingEventHandler] 嵌入缓存命中"
                );
                Ok(HandleResult::ok(
                    format!("嵌入 {} 缓存命中", e.entity_id),
                    start.elapsed().as_micros(),
                ))
            }
            KnowledgeEvent::DocumentIngested(_)
            | KnowledgeEvent::DocumentParsed(_)
            | KnowledgeEvent::DocumentIndexed(_)
            | KnowledgeEvent::DocumentDeleted(_)
            | KnowledgeEvent::NodeCreated(_)
            | KnowledgeEvent::NodeUpdated(_)
            | KnowledgeEvent::NodeDeleted(_)
            | KnowledgeEvent::NodeLinked(_)
            | KnowledgeEvent::EdgeCreated(_)
            | KnowledgeEvent::EdgeDeleted(_)
            | KnowledgeEvent::SearchPerformed(_)
            | KnowledgeEvent::QueryExecuted(_)
            | KnowledgeEvent::UserAction(_)
            | KnowledgeEvent::SystemHealthCheck(_) => Err(HandleError::TypeMismatch),
        }
    }
}

// ============================================================================
// 异步事件处理器包装器（绑定到 EventBus）
// ============================================================================

/// 将 `Handler` 绑定到 `EventBus` 的异步运行包装器
///
/// 在独立 task 中持续从 receiver 拉取事件，
/// 通过 `accepts()` 过滤后调用 `handle()`。
///
/// # 生命周期
///
/// 包装器运行在后台 task 中，直到：
/// - EventBus 关闭（receiver 返回 `None`）
/// - 显式调用 `shutdown()` 停止
pub struct AsyncEventHandler<E: SystemEvent> {
    handler: Arc<dyn Handler<E>>,
    _rx: broadcast::Receiver<E>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl<E: SystemEvent + 'static> AsyncEventHandler<E>
where
    E: Clone,
{
    /// 创建新的异步事件处理器包装器
    ///
    /// 必须在 tokio async context 中调用。
    ///
    /// # 参数
    ///
    /// * `handler` - 实现 `Handler` trait 的处理器实例
    /// * `bus` - 事件总线引用
    ///
    /// # Errors
    ///
    /// 当不在 tokio async context 中调用时返回错误。
    pub async fn new<H>(handler: H, bus: &EventBus<E>) -> Result<Self, String>
    where
        H: Handler<E> + 'static,
    {
        let rx = bus.subscribe().await;

        Ok(Self {
            handler: Arc::new(handler),
            _rx: rx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        })
    }

    /// 启动后台消费任务
    ///
    /// 必须在 tokio async context 中调用。
    /// 返回一个 JoinHandle，可用于等待任务结束或 abort。
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        let handler = self.handler;
        let mut rx = self._rx;
        let running = self.running;

        tokio::spawn(async move {
            info!(handler_name = handler.name(), "AsyncEventHandler 已启动");

            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        if !handler.accepts(&event) {
                            continue;
                        }

                        let event_type = event.event_type();
                        let event_id = event.event_id();

                        match handler.handle(event).await {
                            Ok(result) => {
                                if result.success {
                                    debug!(
                                        handler_name = handler.name(),
                                        event_id = %event_id,
                                        event_type = event_type,
                                        duration_us = result.duration_us,
                                        msg = %result.message,
                                        "事件处理成功"
                                    );
                                } else {
                                    warn!(
                                        handler_name = handler.name(),
                                        event_id = %event_id,
                                        event_type = event_type,
                                        msg = %result.message,
                                        "事件处理失败"
                                    );
                                }
                            }
                            Err(err) => {
                                error!(
                                    handler_name = handler.name(),
                                    event_id = %event_id,
                                    event_type = event_type,
                                    error = %err,
                                    "事件处理异常"
                                );
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(lag)) => {
                        warn!(
                            handler_name = handler.name(),
                            lagged_events = lag,
                            "订阅者落后，已丢弃旧事件"
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!(handler_name = handler.name(), "EventBus 已关闭，停止消费");
                        break;
                    }
                }
            }

            info!(handler_name = handler.name(), "AsyncEventHandler 已停止");
        })
    }

    /// 优雅停机
    pub fn shutdown(&self) {
        running.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::types::*;
    use super::*;
    use crate::model::SourceType;

    #[tokio::test]
    async fn test_document_handler_accepts_document_events() {
        let handler = DocumentEventHandler::new();

        assert!(handler.accepts(&KnowledgeEvent::DocumentIngested(
            DocumentIngestedEvent::new("d", "/", 0, "", "", "s")
        )));
        assert!(
            handler.accepts(&KnowledgeEvent::DocumentParsed(DocumentParsedEvent::new(
                "d", 0, 0, 0, "s"
            )))
        );
        assert!(
            handler.accepts(&KnowledgeEvent::DocumentDeleted(DocumentDeletedEvent::new(
                "d", 0, 0, 0, "s"
            )))
        );
        assert!(
            !handler.accepts(&KnowledgeEvent::NodeCreated(NodeCreatedEvent::new(
                "n",
                NodeType::Token,
                None::<String>,
                "s"
            )))
        );
    }

    #[tokio::test]
    async fn test_node_handler_accepts_node_events() {
        let handler = NodeEventHandler::new();

        assert!(
            handler.accepts(&KnowledgeEvent::NodeCreated(NodeCreatedEvent::new(
                "n",
                NodeType::Token,
                None::<String>,
                "s"
            )))
        );
        assert!(
            handler.accepts(&KnowledgeEvent::NodeLinked(NodeLinkedEvent::new(
                "a",
                "b",
                crate::model::RefType::Usage,
                "s"
            )))
        );
        assert!(
            !handler.accepts(&KnowledgeEvent::SearchPerformed(SearchPerformedEvent::new(
                "q", 0, 0, "s"
            )))
        );
    }

    #[tokio::test]
    async fn test_search_handler_detects_slow_query() {
        let handler = SearchEventHandler::new();

        let slow_event = KnowledgeEvent::SearchPerformed(SearchPerformedEvent::new(
            "complex query",
            5,
            2000,
            "test",
        ));

        let result = handler.handle(slow_event).await;
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_embedding_handler_handles_both_types() {
        let handler = EmbeddingEventHandler::new();

        let gen_event = KnowledgeEvent::EmbeddingGenerated(EmbeddingGeneratedEvent::new(
            "entity-1",
            "Block",
            1536,
            "model-001",
            50,
            "svc",
        ));
        let cache_event = KnowledgeEvent::EmbeddingCached(EmbeddingCachedEvent::new(
            "entity-1",
            "Block",
            "cache-key-001",
            "svc",
        ));

        assert!(handler.handle(gen_event).await.success);
        assert!(handler.handle(cache_event).await.success);
    }

    #[tokio::test]
    async fn test_handler_names_are_descriptive() {
        assert_eq!(DocumentEventHandler::new().name(), "document-event-handler");
        assert_eq!(NodeEventHandler::new().name(), "node-event-handler");
        assert_eq!(SearchEventHandler::new().name(), "search-event-handler");
        assert_eq!(
            EmbeddingEventHandler::new().name(),
            "embedding-event-handler"
        );
    }

    #[tokio::test]
    async fn test_handle_result_constructors() {
        let ok = HandleResult::ok("success", 100);
        assert!(ok.success);
        assert_eq!(ok.message, "success");
        assert_eq!(ok.duration_us, 100);

        let err = HandleResult::err("failure", 200);
        assert!(!err.success);
        assert_eq!(err.message, "failure");
        assert_eq!(err.duration_us, 200);
    }
}
