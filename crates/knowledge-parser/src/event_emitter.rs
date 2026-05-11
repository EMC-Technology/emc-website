//! 事件发射辅助（Event-Driven 模式）
//!
//! 当 `event-driven` feature 启用时，提供通过 EventBus 发射事件的能力。
//! 禁用时，所有函数为空操作（no-op），零开销。
//!
//! # 因果上下文传播
//!
//! 所有发射函数均支持可选的因果上下文参数（`causation_id`、`correlation_id`、`triggered_by`），
//! 遵循项目规则 #7（事件元数据因果链规则）。当调用方处于 CQRS 命令处理链中时，
//! 应传递 `CausationContext` 的相关信息以确保审计追踪完整性。
//!
//! # 使用方式
//!
//! ```ignore
//! use crate::event_emitter::emit_parsed;
//!
//! // 无因果上下文（系统调度场景）
//! emit_parsed("doc-001", 10, 200, 50, None, None, None).await;
//!
//! // 带因果上下文（CQRS 命令处理链中）
//! emit_parsed("doc-001", 10, 200, 50, Some("cmd-123"), Some("corr-456"), Some("Command")).await;
//! ```

#[cfg(feature = "event-driven")]
use knowledge_core::event::{KnowledgeEvent, global_event_bus, types::DocumentParsedEvent};

/// 发射文档解析完成事件
///
/// 当 `event-driven` feature 启用时，向全局 EventBus 发布 `DocumentParsedEvent`。
/// 禁用时此函数为空操作。
#[cfg(feature = "event-driven")]
pub async fn emit_parsed(
    document_id: impl Into<String>,
    block_count: usize,
    token_count: usize,
    parse_duration_ms: u64,
    causation_id: Option<&str>,
    correlation_id: Option<&str>,
    triggered_by: Option<&str>,
) {
    let event = KnowledgeEvent::DocumentParsed(DocumentParsedEvent::new(
        document_id,
        block_count,
        token_count,
        parse_duration_ms,
        "knowledge-parser",
    ));

    if let Err(e) = global_event_bus()
        .publish_with_causation(event, causation_id, correlation_id, triggered_by)
        .await
    {
        tracing::warn!(error = %e, "DocumentParsedEvent 发射失败（可能无订阅者）");
    }
}

#[cfg(not(feature = "event-driven"))]
#[inline]
pub async fn emit_parsed(
    _document_id: impl Into<String>,
    _block_count: usize,
    _token_count: usize,
    _parse_duration_ms: u64,
    _causation_id: Option<&str>,
    _correlation_id: Option<&str>,
    _triggered_by: Option<&str>,
) {
}

#[cfg(feature = "event-driven")]
use knowledge_core::event::types::DocumentIngestedEvent;
use knowledge_core::model::SourceType;

/// 发射文档摄入事件
#[cfg(feature = "event-driven")]
pub async fn emit_ingested(
    document_id: impl Into<String>,
    file_path: impl Into<String>,
    file_size: u64,
    source_type: SourceType,
    hash: impl Into<String>,
    causation_id: Option<&str>,
    correlation_id: Option<&str>,
    triggered_by: Option<&str>,
) {
    let event = KnowledgeEvent::DocumentIngested(DocumentIngestedEvent::new(
        document_id,
        file_path,
        file_size,
        source_type,
        hash,
        "knowledge-parser",
    ));

    if let Err(e) = global_event_bus()
        .publish_with_causation(event, causation_id, correlation_id, triggered_by)
        .await
    {
        tracing::warn!(error = %e, "DocumentIngestedEvent 发射失败");
    }
}

#[cfg(not(feature = "event-driven"))]
#[inline]
pub async fn emit_ingested(
    _document_id: impl Into<String>,
    _file_path: impl Into<String>,
    _file_size: u64,
    _source_type: SourceType,
    _hash: impl Into<String>,
    _causation_id: Option<&str>,
    _correlation_id: Option<&str>,
    _triggered_by: Option<&str>,
) {
}

#[cfg(feature = "event-driven")]
use knowledge_core::event::types::NodeCreatedEvent;
use knowledge_core::event::types::NodeType;

/// 发射节点创建事件
#[cfg(feature = "event-driven")]
pub async fn emit_node_created(
    node_id: impl Into<String>,
    node_type: NodeType,
    document_id: Option<impl Into<String>>,
    causation_id: Option<&str>,
    correlation_id: Option<&str>,
    triggered_by: Option<&str>,
) {
    let event = KnowledgeEvent::NodeCreated(NodeCreatedEvent::new(
        node_id,
        node_type,
        document_id.map(|s| s.into()),
        "knowledge-parser",
    ));

    if let Err(e) = global_event_bus()
        .publish_with_causation(event, causation_id, correlation_id, triggered_by)
        .await
    {
        tracing::warn!(error = %e, "NodeCreatedEvent 发射失败");
    }
}

#[cfg(not(feature = "event-driven"))]
#[inline]
pub async fn emit_node_created(
    _node_id: impl Into<String>,
    _node_type: NodeType,
    _document_id: Option<impl Into<String>>,
    _causation_id: Option<&str>,
    _correlation_id: Option<&str>,
    _triggered_by: Option<&str>,
) {
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_emit_noop_when_feature_disabled() {
        emit_parsed("test-doc", 1, 10, 5, None, None, None).await;
        emit_ingested(
            "test-doc",
            "/test.md",
            100,
            SourceType::Markdown,
            &"a".repeat(64),
            None,
            None,
            None,
        )
        .await;
        emit_node_created(
            "node-001",
            knowledge_core::event::types::NodeType::Block,
            Some("test-doc"),
            None,
            None,
            None,
        )
        .await;
    }

    #[cfg(feature = "event-driven")]
    #[tokio::test]
    async fn test_emit_publishes_to_global_bus() {
        use knowledge_core::event::*;

        let mut rx = global_event_bus().subscribe().await;

        emit_parsed(
            "emit-test-doc",
            3,
            50,
            20,
            Some("cmd-123"),
            Some("corr-456"),
            Some("Command"),
        )
        .await;

        let received = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await;

        assert!(received.is_ok(), "应从全局总线接收到事件");
        match received.unwrap() {
            Ok(KnowledgeEvent::DocumentParsed(e)) => {
                assert_eq!(e.document_id, "emit-test-doc");
                assert_eq!(e.block_count, 3);
            }
            other => panic!("期望 DocumentParsedEvent，实际收到: {:?}", other),
        }
    }
}
