//! 事件订阅与处理器注册（Event-Driven 模式）
//!
//! 当 `event-driven` feature 启用时，本模块负责：
//! - 初始化全局 EventBus（自定义配置）
//! - 注册所有领域事件处理器
//! - 启动后台消费任务
//!
//! # 使用方式
//!
//! 在应用启动时调用 `init_event_subscribers()`：
//!
//! ```ignore
//! #[cfg(feature = "event-driven")]
//! {
//!     knowledge_api::event_subscriber::init_event_subscribers().await;
//! }
//! ```

use std::sync::Arc;

#[cfg(feature = "event-driven")]
use knowledge_core::event::{
    AsyncEventHandler, DocumentEventHandler, EmbeddingEventHandler, EventBusConfig, Handler,
    NodeEventHandler, SearchEventHandler, global_event_bus, init_global_event_bus,
};

/// 初始化事件驱动架构
///
/// 执行以下步骤：
/// 1. 使用生产级配置初始化全局 EventBus
/// 2. 注册所有领域处理器并启动后台消费任务
/// 3. 返回句柄列表，用于优雅停机
#[cfg(feature = "event-driven")]
pub async fn init_event_subscribers() -> Vec<tokio::task::JoinHandle<()>> {
    let config = EventBusConfig::builder()
        .channel_capacity(8192)
        .max_subscribers(200)
        .enable_persistence(false)
        .retry_policy(5, 50)
        .build();

    if init_global_event_bus(config).is_err() {
        tracing::warn!("全局 EventBus 已被初始化，使用已有实例");
    }

    let bus = global_event_bus();
    let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    let doc_handler = AsyncEventHandler::new(DocumentEventHandler::new(), &bus)
        .await
        .map_err(|e| tracing::error!("创建文档事件处理器失败: {e}"))
        .ok();
    if let Some(h) = doc_handler {
        handles.push(h.spawn());
    }

    let node_handler = AsyncEventHandler::new(NodeEventHandler::new(), &bus)
        .await
        .map_err(|e| tracing::error!("创建节点事件处理器失败: {e}"))
        .ok();
    if let Some(h) = node_handler {
        handles.push(h.spawn());
    }

    let search_handler = AsyncEventHandler::new(SearchEventHandler::new(), &bus)
        .await
        .map_err(|e| tracing::error!("创建搜索事件处理器失败: {e}"))
        .ok();
    if let Some(h) = search_handler {
        handles.push(h.spawn());
    }

    let embed_handler = AsyncEventHandler::new(EmbeddingEventHandler::new(), &bus)
        .await
        .map_err(|e| tracing::error!("创建嵌入事件处理器失败: {e}"))
        .ok();
    if let Some(h) = embed_handler {
        handles.push(h.spawn());
    }

    tracing::info!(
        handler_count = handles.len(),
        "事件驱动架构已初始化，{} 个处理器已启动",
        handles.len()
    );

    handles
}

/// 优雅停机所有事件处理器
#[cfg(feature = "event-driven")]
pub async fn shutdown_event_subscribers(handles: Vec<tokio::task::JoinHandle<()>>) {
    for handle in handles {
        handle.abort();
    }
    tracing::info!("所有事件处理器已停机");
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_init_and_shutdown_cycle() {
        let handles = init_event_subscribers().await;
        assert!(!handles.is_empty(), "应至少注册一个处理器");

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        shutdown_event_subscribers(handles).await;
    }

    #[cfg(feature = "event-driven")]
    #[tokio::test]
    async fn test_event_flow_after_init() {
        use knowledge_core::event::*;

        let _handles = init_event_subscribers().await;
        let bus = global_event_bus();

        let test_event = KnowledgeEvent::SystemHealthCheck(types::SystemHealthEvent::new(
            "api-test",
            "ok",
            None::<String>,
            "test",
        ));
        let result = bus.publish(test_event).await;
        assert!(result.is_ok(), "初始化后发布事件应成功");
    }
}
