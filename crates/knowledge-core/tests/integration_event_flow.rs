#![cfg(feature = "event-driven")]

//! 事件驱动架构集成测试
//!
//! 验证完整的端到端事件流：
//! DocumentIngested → DocumentParsed → NodeCreated → EmbeddingGenerated
//!
//! 运行方式: cargo test -p knowledge-core --features event-driven --test integration_event_flow

use std::sync::Arc;
use std::time::Duration;

use knowledge_core::event::{
    EventBus, EventBusConfig, Handler, HandleResult,
    DocumentEventHandler, NodeEventHandler, SearchEventHandler, EmbeddingEventHandler,
    AsyncEventHandler,
};
use knowledge_core::event::types::*;

/// 测试辅助：创建一个伪造的文档摄入事件
fn make_ingest_event() -> KnowledgeEvent {
    KnowledgeEvent::DocumentIngested(DocumentIngestedEvent::new(
        "doc-integration-001",
        "/integration-test/sample.md",
        2048,
        "Markdown",
        &"deadbeef".repeat(8),
        "integration-test",
    ))
}

/// 测试辅助：创建一个文档解析完成事件
fn make_parsed_event() -> KnowledgeEvent {
    KnowledgeEvent::DocumentParsed(DocumentParsedEvent::new(
        "doc-integration-001",
        5,
        120,
        35,
        "parser-service",
    ))
}

/// 测试辅助：创建一个节点创建事件
fn make_node_created_event() -> KnowledgeEvent {
    KnowledgeEvent::NodeCreated(NodeCreatedEvent::new(
        "node-integration-001",
        "Block",
        Some("doc-integration-001"),
        "graph-builder",
    ))
}

/// 测试辅助：创建一个嵌入生成事件
fn make_embedding_generated_event() -> KnowledgeEvent {
    KnowledgeEvent::EmbeddingGenerated(EmbeddingGeneratedEvent::new(
        "block-integration-001",
        "Block",
        1536,
        "text-embedding-ada-002",
        80,
        "embedding-service",
    ))
}

// ============================================================================
// 集成测试：完整事件驱动文档流程
// ============================================================================

#[tokio::test]
async fn test_event_driven_document_flow() {
    let bus: EventBus<KnowledgeEvent> = EventBus::new(EventBusConfig::default());

    // 注册所有处理器
    let doc_handler = AsyncEventHandler::new(DocumentEventHandler::new(), &bus).await.expect("创建文档处理器失败");
    let node_handler = AsyncEventHandler::new(NodeEventHandler::new(), &bus).await.expect("创建节点处理器失败");
    let search_handler = AsyncEventHandler::new(SearchEventHandler::new(), &bus).await.expect("创建搜索处理器失败");
    let embed_handler = AsyncEventHandler::new(EmbeddingEventHandler::new(), &bus).await.expect("创建嵌入处理器失败");

    let _doc_handle = doc_handler.spawn();
    let _node_handle = node_handler.spawn();
    let _search_handle = search_handler.spawn();
    let _embed_handle = embed_handler.spawn();

    tokio::time::sleep(Duration::from_millis(10)).await;

    // Step 1: 发布 DocumentIngestedEvent
    let ingest_result = bus.publish(make_ingest_event()).await;
    assert!(
        ingest_result.is_ok(),
        "DocumentIngestedEvent 应成功发布"
    );
    assert_eq!(ingest_result.unwrap(), 4, "4 个订阅者应收到");

    // Step 2: 发布 DocumentParsedEvent（模拟解析完成）
    let parsed_result = bus.publish(make_parsed_event()).await;
    assert!(parsed_result.is_ok(), "DocumentParsedEvent 应成功发布");

    // Step 3: 发布 NodeCreatedEvent（模拟图构建）
    let node_result = bus.publish(make_node_created_event()).await;
    assert!(node_result.is_ok(), "NodeCreatedEvent 应成功发布");

    // Step 4: 发布 EmbeddingGeneratedEvent（异步嵌入计算）
    let embed_result = bus.publish(make_embedding_generated_event()).await;
    assert!(embed_result.is_ok(), "EmbeddingGeneratedEvent 应成功发布");
}

// ============================================================================
// 集成测试：处理器过滤机制
// ============================================================================

#[tokio::test]
async fn test_handler_filtering_mechanism() {
    let bus: EventBus<KnowledgeEvent> = EventBus::new(EventBusConfig::default());

    let doc_handler = Arc::new(DocumentEventHandler::new());
    let node_handler = Arc::new(NodeEventHandler::new());

    let mut doc_rx = bus.subscribe().await;
    let mut node_rx = bus.subscribe().await;

    // 只发布一个 NodeCreated 事件
    let node_event = make_node_created_event();
    bus.publish(node_event).await.unwrap();

    // 文档处理器应该收到但跳过（accepts 返回 false）
    let doc_received = doc_rx.recv().await;
    assert!(doc_received.is_ok());
    assert!(!doc_handler.accepts(&doc_received.unwrap()));

    // 节点处理器应该接受并处理
    let node_received = node_rx.recv().await;
    assert!(node_received.is_ok());
    assert!(node_handler.accepts(&node_received.unwrap()));
}

// ============================================================================
// 集成测试：高并发场景
// ============================================================================

#[tokio::test]
async fn test_high_concurrency_publishing() {
    let bus: EventBus<KnowledgeEvent> = EventBus::new(EventBusConfig {
        channel_capacity: 1024,
        ..EventBusConfig::default()
    });

    let mut rx = bus.subscribe().await;

    let handle = tokio::spawn(async move {
        let mut received_count = 0u32;
        while let Ok(_) = rx.recv().await {
            received_count += 1;
            if received_count >= 100 {
                break;
            }
        }
        received_count
    });

    for i in 0..100u32 {
        let event = KnowledgeEvent::SystemHealthCheck(SystemHealthEvent::new(
            format!("component-{}", i),
            "ok",
            None::<String>,
            "concurrent-test",
        ));
        let _ = bus.publish(event).await;
    }

    let count = handle.await.expect("任务不应 panic");
    assert_eq!(count, 100, "应接收到全部 100 个事件");
}

// ============================================================================
// 集合测试：事件序列化持久化兼容性
// ============================================================================

#[tokio::test]
async fn test_all_events_serialization_for_persistence() {
    let events: Vec<KnowledgeEvent> = vec![
        make_ingest_event(),
        make_parsed_event(),
        KnowledgeEvent::DocumentIndexed(DocumentIndexedEvent::new("d", 3, 10, "s")),
        KnowledgeEvent::DocumentDeleted(DocumentDeletedEvent::new("d", 3, 30, 5, "s")),
        make_node_created_event(),
        KnowledgeEvent::NodeUpdated(NodeUpdatedEvent::new("n", vec!["content".to_string()], "s")),
        KnowledgeEvent::NodeDeleted(NodeDeletedEvent::new("n", "Block", "s")),
        KnowledgeEvent::NodeLinked(NodeLinkedEvent::new("a", "b", "Usage", "s")),
        KnowledgeEvent::EdgeCreated(EdgeCreatedEvent::new("e", "Definition", "a", "b", "s")),
        KnowledgeEvent::EdgeDeleted(EdgeDeletedEvent::new("e", "Link", "s")),
        KnowledgeEvent::SearchPerformed(SearchPerformedEvent::new("rust event bus", 10, 5, "s")),
        KnowledgeEvent::QueryExecuted(QueryExecutedEvent::new("graph", "MATCH ...", 2, 15, "s")),
        make_embedding_generated_event(),
        KnowledgeEvent::EmbeddingCached(EmbeddingCachedEvent::new("e", "Block", "k", "s")),
        KnowledgeEvent::UserAction(UserActionEvent::new("user-1", "upload", Some("document"), Some("d"), "api")),
        KnowledgeEvent::SystemHealthCheck(SystemHealthEvent::new("db", "healthy", None::<String>, "monitor")),
    ];

    for (i, event) in events.iter().enumerate() {
        let json = serde_json::to_string(event)
            .unwrap_or_else(|e| panic!("事件 #{} 序列化失败: {}", i, e));

        let de_event: KnowledgeEvent = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("事件 #{} 反序列化失败: {} (json: {})", i, e, json));

        assert_eq!(
            event.event_type(),
            de_event.event_type(),
            "事件 #{} 类型不匹配",
            i
        );
        assert_eq!(
            event.event_id(),
            de_event.event_id(),
            "事件 #{} ID 不匹配",
            i
        );
    }
}

// ============================================================================
// 集成测试：全局单例初始化与使用
// ============================================================================

#[test]
fn test_global_bus_init_and_use() {
    use knowledge_core::event::{init_global_event_bus, global_event_bus};

    let result = init_global_event_bus(EventBusConfig::builder()
        .channel_capacity(2048)
        .max_subscribers(50)
        .build()
    );

    assert!(
        result.is_ok(),
        "首次初始化全局总线应成功"
    );

    let bus = global_event_bus();
    assert_eq!(bus.config.channel_capacity, 2048);

    // 重复初始化应失败
    let duplicate = init_global_event_bus(EventBusConfig::default());
    assert!(duplicate.is_err(), "重复初始化应返回 Err");
}
