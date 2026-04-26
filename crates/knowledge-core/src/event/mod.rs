/// 事件驱动架构核心模块
///
/// 本模块提供进程内事件总线（Event-Driven Architecture）的完整实现：
/// - **types**: 系统事件 trait 定义与领域事件枚举
/// - **event_bus**: 基于 `tokio::sync::broadcast` 的进程内事件总线
/// - **event_handler**: 事件处理器 trait 与各领域处理器实现
///
/// # 设计目标
///
/// 1. **松耦合**: 模块间通过事件通信，消除直接依赖
/// 2. **可扩展**: 为未来微服务拆分预留接口
/// 3. **可观测**: 每个事件自动记录日志与 metrics
/// 4. **可靠性**: 至少一次投递保证 + 指数退避重试
///
/// # Feature Flag
///
/// 通过 `event-driven` feature flag 控制：
/// - 启用时：模块通过 EventBus 通信
/// - 禁用时：保持原有直接函数调用（向后兼容）
pub mod types;
pub mod event_bus;
pub mod event_handler;

pub use types::{SystemEvent, KnowledgeEvent};
pub use event_bus::{EventBus, EventBusConfig, EventError, global_event_bus, init_global_event_bus};
pub use event_handler::{
    Handler, HandleResult,
    DocumentEventHandler, NodeEventHandler, SearchEventHandler, EmbeddingEventHandler,
    AsyncEventHandler,
};
