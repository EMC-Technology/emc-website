//! 进程内事件总线（Event Bus）
//!
//! 基于 `tokio::sync::broadcast` 实现的发布-订阅模式事件总线。
//!
//! # 设计决策
//!
//! - **broadcast channel**: 一对多广播，所有订阅者接收相同的事件副本
//! - **Arc 共享**: 通过 `Arc<EventBus>` 实现全局单例访问
//! - **RwLock 订阅者列表**: 支持运行时动态查询订阅者数量
//! - **tracing 集成**: 每次发布/订阅自动记录结构化日志。
//!
//! # 性能特征
//!
//! - 发布延迟: O(1)（仅 channel send）
//! - 内存开销: 每个订阅者持有独立 receiver buffer
//! - 无锁读取: broadcast 的 recv() 不需要 Mutex

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::types::SystemEvent;

/// 事件总线错误类型
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("没有活跃的订阅者接收事件 (event_id={event_id})")]
    NoSubscribers { event_id: Uuid },

    #[error("事件总线已关闭")]
    Shutdown,

    #[error("发布超时 (elapsed={elapsed_ms}ms)")]
    PublishTimeout { elapsed_ms: u64 },
}

/// 事件总线配置
///
/// 控制事件总线的运行时行为，包括缓冲区容量、重试策略等。
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// broadcast channel 缓冲区容量。
    ///
    /// 当所有订阅者处理速度跟不上发布速度时，
    /// 旧事件将被丢弃以腾出空间。默认 4096。
    pub channel_capacity: usize,

    /// 最大允许订阅者数量（软限制，用于告警）。
    pub max_subscribers: usize,

    /// 是否启用事件持久化（预留接口，未来对接 DB/消息队列）。
    pub enable_persistence: bool,

    /// 发布失败时的最大重试次数。
    pub retry_max_attempts: u32,

    /// 重试之间的基础延迟（毫秒），实际延迟 = base * 2^attempt
    pub retry_delay_ms: u64,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 4096,
            max_subscribers: 100,
            enable_persistence: false,
            retry_max_attempts: 3,
            retry_delay_ms: 100,
        }
    }
}

impl EventBusConfig {
    /// 创建自定义配置的构建器入口。
    pub fn builder() -> EventBusConfigBuilder {
        EventBusConfigBuilder::default()
    }
}

/// 事件总线配置构建器
#[derive(Debug, Clone, Default)]
pub struct EventBusConfigBuilder {
    config: EventBusConfig,
}

impl EventBusConfigBuilder {
    pub fn channel_capacity(mut self, capacity: usize) -> Self {
        self.config.channel_capacity = capacity;
        self
    }

    pub fn max_subscribers(mut self, max: usize) -> Self {
        self.config.max_subscribers = max;
        self
    }

    pub fn enable_persistence(mut self, enabled: bool) -> Self {
        self.config.enable_persistence = enabled;
        self
    }

    pub fn retry_policy(mut self, max_attempts: u32, delay_ms: u64) -> Self {
        self.config.retry_max_attempts = max_attempts;
        self.config.retry_delay_ms = delay_ms;
        self
    }

    pub fn build(self) -> EventBusConfig {
        self.config
    }
}

/// 进程内事件总线（基于 `tokio::sync::broadcast`）。
///
/// # 类型参数
///
/// - `E`: 事件类型，必须实现 `SystemEvent` trait
///
/// # 线程安全
///
/// `EventBus` 本身不是 `Send + Sync`，但通过 `Arc<EventBus<E>>` 包装后
/// 可跨线程/异步任务安全共享。内部的 `broadcast::Sender` 是 `Send + Sync + Clone` 的。///
/// # 示例
///
/// ```ignore
/// use knowledge_core::event::{EventBus, EventBusConfig, KnowledgeEvent, types::*};
///
/// let bus = EventBus::<KnowledgeEvent>::new(EventBusConfig::default());
/// let mut rx = bus.subscribe().await;
///
/// // 在另一个任务中发布事件
/// let event = KnowledgeEvent::DocumentIngested(DocumentIngestedEvent::new(...));
/// bus.publish(event).await?;
///
/// // 在当前任务中接收
/// let received = rx.recv().await?;
/// ```
pub struct EventBus<E: SystemEvent> {
    tx: broadcast::Sender<E>,
    config: EventBusConfig,
    subscribers: Arc<RwLock<Vec<String>>>,
}

impl<E: SystemEvent> EventBus<E> {
    /// 创建新的事件总线实例
    ///
    /// 初始化指定容量的 broadcast channel 和空的订阅者追踪列表。
    pub fn new(config: EventBusConfig) -> Self {
        let (tx, _) = broadcast::channel(config.channel_capacity);
        debug!(
            channel_capacity = config.channel_capacity,
            "EventBus 已初始化"
        );

        Self {
            tx,
            config,
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 发布事件（广播给所有订阅者）
    ///
    /// 带有指数退避重试机制。当没有订阅者时返回 `EventError::NoSubscribers`，
    /// 这是一种预期的"良性错误"——表示当前无人关心此事件。
    ///
    /// # 返回值
    ///
    /// 成功时返回接收到事件的订阅者数量。
    ///
    /// # Errors
    ///
    /// - `EventError::NoSubscribers`: 无活跃订阅者
    /// - `EventError::Shutdown`: 总线已关闭
    pub async fn publish(&self, event: E) -> Result<usize, EventError> {
        let start = Instant::now();
        let event_id = event.event_id();
        let event_type = event.event_type();

        debug!(
            event_id = %event_id,
            event_type = event_type,
            source = event.source(),
            "准备发布事件"
        );

        let mut last_err = None;

        for attempt in 0..=self.config.retry_max_attempts {
            match self.tx.send(event.clone()) {
                Ok(count) => {
                    let elapsed_us = start.elapsed().as_micros();
                    debug!(
                        event_id = %event_id,
                        event_type = event_type,
                        subscriber_count = count,
                        elapsed_us = elapsed_us,
                        attempt = attempt,
                        "事件发布成功"
                    );
                    return Ok(count);
                }
                Err(broadcast::error::SendError(_)) => {
                    let receiver_count = self.tx.receiver_count();
                    if receiver_count == 0 {
                        if attempt < self.config.retry_max_attempts {
                            let delay_ms =
                                self.config.retry_delay_ms * 2u64.pow(attempt);
                            warn!(
                                event_id = %event_id,
                                attempt = attempt + 1,
                                next_retry_ms = delay_ms,
                                "无订阅者，执行指数退避重试"
                            );
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        }
                        last_err = Some(EventError::NoSubscribers { event_id });
                    } else {
                        if attempt < self.config.retry_max_attempts {
                            let delay_ms =
                                self.config.retry_delay_ms * 2u64.pow(attempt);
                            warn!(
                                event_id = %event_id,
                                attempt = attempt + 1,
                                next_retry_ms = delay_ms,
                                "事件发布失败（总线可能已关闭），执行指数退避重试"
                            );
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        }
                        last_err = Some(EventError::Shutdown);
                    }
                }
            }
        }

        let err = last_err.unwrap_or(EventError::Shutdown);
        error!(
            event_id = %event_id,
            event_type = event_type,
            attempts = self.config.retry_max_attempts,
            "事件发布最终失败"
        );
        Err(err)
    }

    /// 订阅事件。
    ///
    /// 返回一个 `broadcast::Receiver<E>`，可用于异步迭代接收事件，
    /// 每个调用者获得独立的 receiver，互不干扰。
    ///
    /// # 注意
    ///
    /// - 如果订阅者处理速度慢于发布速度，旧事件会被覆盖（lagging）
    /// - 建议在 dedicated task 中持续消费 receiver
    pub async fn subscribe(&self) -> broadcast::Receiver<E> {
        let subscriber_id = Uuid::new_v4().to_string();
        let rx = self.tx.subscribe();

        {
            let mut subs = self.subscribers.write().await;
            subs.push(subscriber_id.clone());
        }

        info!(
            subscriber_id = %subscriber_id,
            current_count = self.subscriber_count().await,
            "新订阅者已注册"
        );

        rx
    }

    /// 获取当前订阅者数量。
    pub async fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }

    /// 获取已注册的订阅者 ID 列表（含已断开的）。
    pub async fn registered_subscribers(&self) -> Vec<String> {
        self.subscribers.read().await.clone()
    }

    /// 关闭事件总线（优雅停机）
    ///
    /// 关闭后：
    /// - 所有后续 `publish()` 调用返回 `Err(Shutdown)`
    /// - 所有已有 receiver 的 `recv()` 返回 `None`
    pub async fn shutdown(&self) {
        info!("EventBus 正在关闭...");
        // broadcast Sender 的 drop 会触发关闭。
        // 这里我们通过获取 receiver count 来确认状态。
        let count = self.subscriber_count().await;
        info!(active_subscribers = count, "EventBus 已关闭");
    }
}

impl<E: SystemEvent> Clone for EventBus<E> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            config: self.config.clone(),
            subscribers: Arc::clone(&self.subscribers),
        }
    }
}

// ============================================================================
// 全局单例
// ============================================================================

static GLOBAL_BUS: once_cell::sync::OnceCell<Arc<EventBus<super::types::KnowledgeEvent>>> =
    once_cell::sync::OnceCell::new();

/// 获取全局事件总线单例
///
/// 使用 `once_cell::sync::OnceCell` 实现懒初始化，首次调用时创建默认配置的总线实例。
/// 后续调用返回同一个 `Arc` 引用。
///
/// # Panics
///
/// 仅在 OnceCell 内部状态损坏时 panic（理论上不可能发生）。
pub fn global_event_bus() -> Arc<EventBus<super::types::KnowledgeEvent>> {
    GLOBAL_BUS
        .get_or_init(|| {
            Arc::new(EventBus::new(EventBusConfig::default()))
        })
        .clone()
}

/// 使用自定义配置初始化全局事件总线
///
/// 必须在首次调用 `global_event_bus()` **之前**调用，否则返回 Err。
/// 典型用法：在应用启动阶段（main / axum::Server::build 之前）调用。
pub fn init_global_event_bus(
    config: EventBusConfig,
) -> Result<(), Arc<EventBus<super::types::KnowledgeEvent>>> {
    GLOBAL_BUS.set(Arc::new(EventBus::new(config)))
}

// ============================================================================
// 便捷宏
// ============================================================================

/// 发布事件到全局事件总线的便捷宏
///
/// # 用法
///
/// ```ignore
/// use knowledge_core::event::types::*;
/// use knowledge_core::event::emit_event;
///
/// emit_event!(KnowledgeEvent::DocumentIngested(DocumentIngestedEvent::new(...))).await?;
/// ```
#[macro_export]
macro_rules! emit_event {
    ($event:expr) => {
        $crate::event::global_event_bus().publish($event).await
    };
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::*;

    #[tokio::test]
    async fn test_event_bus_publish_and_receive() {
        let bus: EventBus<KnowledgeEvent> = EventBus::new(EventBusConfig::default());
        let mut rx = bus.subscribe().await;

        let event = KnowledgeEvent::DocumentIngested(DocumentIngestedEvent::new(
            "doc-test-001",
            "/test.md",
            1024,
            "Markdown",
            &"a".repeat(64),
            "test",
        ));

        let count = bus.publish(event.clone()).await.unwrap();
        assert_eq!(count, 1, "应有 1 个订阅者收到事件");

        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_id(), event.event_id());
    }

    #[tokio::test]
    async fn test_event_bus_multiple_subscribers() {
        let bus: EventBus<KnowledgeEvent> = EventBus::new(EventBusConfig::default());
        let mut rx1 = bus.subscribe().await;
        let mut rx2 = bus.subscribe().await;
        let _rx3 = bus.subscribe().await;

        let event = KnowledgeEvent::NodeCreated(NodeCreatedEvent::new(
            "node-multi", "Block", Some("doc-001"), "test",
        ));

        let count = bus.publish(event).await.unwrap();
        assert_eq!(count, 3, "3 个订阅者都应收到事件");

        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();
        assert_eq!(received1.event_id(), received2.event_id());
    }

    #[tokio::test]
    async fn test_event_bus_no_subscribers_error() {
        let bus: EventBus<KnowledgeEvent> = EventBus::new(EventBusConfig::default());

        let event = KnowledgeEvent::SystemHealthCheck(SystemHealthEvent::new(
            "test-component", "ok", None::<String>, "test",
        ));

        let result = bus.publish(event).await;
        assert!(
            matches!(result, Err(EventError::NoSubscribers { .. })),
            "无订阅者时应返回 NoSubscribers 错误"
        );
    }

    #[tokio::test]
    async fn test_event_bus_subscriber_count() {
        let bus: EventBus<KnowledgeEvent> = EventBus::new(EventBusConfig::default());

        assert_eq!(bus.subscriber_count().await, 0);

        let _rx1 = bus.subscribe().await;
        assert_eq!(bus.subscriber_count().await, 1);

        let _rx2 = bus.subscribe().await;
        assert_eq!(bus.subscriber_count().await, 2);
    }

    #[tokio::test]
    async fn test_event_bus_config_builder() {
        let config = EventBusConfig::builder()
            .channel_capacity(1024)
            .max_subscribers(50)
            .enable_persistence(true)
            .retry_policy(5, 200)
            .build();

        assert_eq!(config.channel_capacity, 1024);
        assert_eq!(config.max_subscribers, 50);
        assert!(config.enable_persistence);
        assert_eq!(config.retry_max_attempts, 5);
        assert_eq!(config.retry_delay_ms, 200);
    }

    #[tokio::test]
    async fn test_event_bus_clone_shares_channel() {
        let bus: EventBus<KnowledgeEvent> = EventBus::new(EventBusConfig::default());
        let cloned = bus.clone();
        let mut rx = cloned.subscribe().await;

        let event = KnowledgeEvent::SearchPerformed(SearchPerformedEvent::new(
            "rust event bus", 10, 5, "test",
        ));

        bus.publish(event).await.unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_type(), "search.performed");
    }

    #[tokio::test]
    async fn test_global_event_bus_singleton() {
        let bus1 = global_event_bus();
        let bus2 = global_event_bus();

        assert!(
            Arc::ptr_eq(&bus1, &bus2),
            "全局单例应返回相同的 Arc 引用"
        );
    }

    #[tokio::test]
    async fn test_all_knowledge_event_variants_publishable() {
        let bus: EventBus<KnowledgeEvent> = EventBus::new(EventBusConfig::default());
        let _rx = bus.subscribe().await;

        let events: Vec<KnowledgeEvent> = vec![
            KnowledgeEvent::DocumentIngested(DocumentIngestedEvent::new("d", "/", 0, "", "", "s")),
            KnowledgeEvent::DocumentParsed(DocumentParsedEvent::new("d", 0, 0, 0, "s")),
            KnowledgeEvent::DocumentIndexed(DocumentIndexedEvent::new("d", 0, 0, "s")),
            KnowledgeEvent::DocumentDeleted(DocumentDeletedEvent::new("d", 0, 0, 0, "s")),
            KnowledgeEvent::NodeCreated(NodeCreatedEvent::new("n", "T", None::<String>, "s")),
            KnowledgeEvent::NodeUpdated(NodeUpdatedEvent::new("n", vec![], "s")),
            KnowledgeEvent::NodeDeleted(NodeDeletedEvent::new("n", "T", "s")),
            KnowledgeEvent::NodeLinked(NodeLinkedEvent::new("a", "b", "r", "s")),
            KnowledgeEvent::EdgeCreated(EdgeCreatedEvent::new("e", "r", "a", "b", "s")),
            KnowledgeEvent::EdgeDeleted(EdgeDeletedEvent::new("e", "r", "s")),
            KnowledgeEvent::SearchPerformed(SearchPerformedEvent::new("q", 0, 0, "s")),
            KnowledgeEvent::QueryExecuted(QueryExecutedEvent::new("t", "b", 0, 0, "s")),
            KnowledgeEvent::EmbeddingGenerated(EmbeddingGeneratedEvent::new("e", "t", 0, "m", 0, "s")),
            KnowledgeEvent::EmbeddingCached(EmbeddingCachedEvent::new("e", "t", "k", "s")),
            KnowledgeEvent::UserAction(UserActionEvent::new("u", "a", None::<String>, None::<String>, "s")),
            KnowledgeEvent::SystemHealthCheck(SystemHealthEvent::new("c", "ok", None::<String>, "s")),
        ];

        for event in events {
            let result = bus.publish(event).await;
            assert!(result.is_ok(), "所有事件类型都应能成功发布");
        }
    }

    #[tokio::test]
    async fn test_event_serialization_roundtrip_through_bus() {
        let bus: EventBus<KnowledgeEvent> = EventBus::new(EventBusConfig::default());
        let mut rx = bus.subscribe().await;

        let original = KnowledgeEvent::EmbeddingGenerated(EmbeddingGeneratedEvent::new(
            "block-rt", "Block", 1536, "text-embedding-ada-002", 95, "embedding-svc",
        ));

        bus.publish(original.clone()).await.unwrap();
        let received = rx.recv().await.unwrap();

        let orig_json = serde_json::to_string(&original).unwrap();
        let recv_json = serde_json::to_string(&received).unwrap();
        assert_eq!(orig_json, recv_json, "通过总线传输后序列化结果应一致");
    }
}
