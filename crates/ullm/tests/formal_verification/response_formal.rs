#![cfg(test)]
//! Response 模块形式化验证测试
//!
//! ## 不变量 (Invariants)
//!
//! 1. `CompletionResponse::from_stream` 正确聚合文本
//! 2. `CompletionResponse::has_tool_calls` 和 `has_thinking` 正确判断
//! 3. Stream 错误正确传播

use crate::response::{CompletionResponse, ThinkingContent};
use crate::provider::content_block::MessageContent;
use crate::provider::types::{StopReason, TokenUsage};
use crate::stream::{ModelStream, StreamEvent};
use crate::error::LlmError;
use futures_util::stream::{self, StreamExt};

fn make_stream(events: Vec<Result<StreamEvent, LlmError>>) -> ModelStream {
    Box::pin(stream::iter(events))
}

#[test]
fn test_miri_completion_response_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CompletionResponse>();
    assert_send_sync::<ThinkingContent>();
}

#[tokio::test]
async fn test_kani_from_stream_text_concatenation() {
    let events = vec![
        Ok(StreamEvent::Text("Hello".into())),
        Ok(StreamEvent::Text(" ".into())),
        Ok(StreamEvent::Text("World".into())),
        Ok(StreamEvent::Stop(StopReason::EndTurn)),
    ];
    let stream = make_stream(events);
    let response = CompletionResponse::from_stream(stream).await.unwrap();

    assert_eq!(
        response.text(),
        Some("Hello World"),
        "THEOREM: text events MUST be concatenated in order"
    );
}

#[tokio::test]
async fn test_kani_from_stream_tool_calls_aggregation() {
    let events = vec![
        Ok(StreamEvent::ToolUse(crate::tool::ToolCall {
            id: "call_1".into(),
            name: "tool_a".into(),
            arguments: "{}".into(),
        })),
        Ok(StreamEvent::ToolUse(crate::tool::ToolCall {
            id: "call_2".into(),
            name: "tool_b".into(),
            arguments: "{}".into(),
        })),
        Ok(StreamEvent::Stop(StopReason::ToolCall)),
    ];
    let stream = make_stream(events);
    let response = CompletionResponse::from_stream(stream).await.unwrap();

    assert_eq!(
        response.tool_calls.len(),
        2,
        "THEOREM: all tool_use events MUST be aggregated"
    );
    assert!(
        response.has_tool_calls(),
        "THEOREM: has_tool_calls() MUST return true when tool calls present"
    );
}

#[tokio::test]
async fn test_kani_from_stream_thinking_aggregation() {
    let events = vec![
        Ok(StreamEvent::Thinking {
            text: "Let me think...".into(),
            signature: None,
        }),
        Ok(StreamEvent::Thinking {
            text: "...more thoughts".into(),
            signature: Some("sig_abc".into()),
        }),
        Ok(StreamEvent::Text("The answer".into())),
        Ok(StreamEvent::Stop(StopReason::EndTurn)),
    ];
    let stream = make_stream(events);
    let response = CompletionResponse::from_stream(stream).await.unwrap();

    assert!(
        response.has_thinking(),
        "THEOREM: has_thinking() MUST return true when thinking events present"
    );
    assert!(
        response.thinking.is_some(),
        "THEOREM: thinking content MUST be aggregated"
    );
    assert!(
        response.thinking.as_ref().unwrap().text.contains("Let me think..."),
        "THEOREM: thinking text MUST be concatenated"
    );
}

#[tokio::test]
async fn test_kani_from_stream_error_propagation() {
    let events = vec![
        Ok(StreamEvent::Started),
        Ok(StreamEvent::Text("Hello".into())),
        Err(LlmError::StreamError("stream interrupted".into())),
    ];
    let stream = make_stream(events);
    let result = CompletionResponse::from_stream(stream).await;

    assert!(
        result.is_err(),
        "THEOREM: stream errors MUST be propagated"
    );
    assert!(
        matches!(result.unwrap_err(), LlmError::StreamError(_)),
        "THEOREM: error type MUST be StreamError"
    );
}

#[tokio::test]
async fn test_kani_from_stream_error_event() {
    let events = vec![
        Ok(StreamEvent::Text("Hello".into())),
        Ok(StreamEvent::Error("something went wrong".into())),
    ];
    let stream = make_stream(events);
    let result = CompletionResponse::from_stream(stream).await;

    assert!(
        result.is_err(),
        "THEOREM: Error stream event MUST cause failure"
    );
}

#[tokio::test]
async fn test_kani_from_stream_usage_merge() {
    let events = vec![
        Ok(StreamEvent::UsageUpdate(TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            ..Default::default()
        })),
        Ok(StreamEvent::UsageUpdate(TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 10,
            ..Default::default()
        })),
        Ok(StreamEvent::Stop(StopReason::EndTurn)),
    ];
    let stream = make_stream(events);
    let response = CompletionResponse::from_stream(stream).await.unwrap();

    assert_eq!(
        response.usage.prompt_tokens, 10,
        "THEOREM: usage updates MUST be merged (prompt_tokens accumulated)"
    );
    assert_eq!(
        response.usage.completion_tokens, 15,
        "THEOREM: usage updates MUST be merged (completion_tokens accumulated)"
    );
}

#[tokio::test]
async fn test_kani_from_stream_empty_text_with_tools() {
    let events = vec![
        Ok(StreamEvent::ToolUse(crate::tool::ToolCall {
            id: "call_1".into(),
            name: "get_weather".into(),
            arguments: r#"{"city":"SF"}"#.into(),
        })),
        Ok(StreamEvent::Stop(StopReason::ToolCall)),
    ];
    let stream = make_stream(events);
    let response = CompletionResponse::from_stream(stream).await.unwrap();

    assert_eq!(
        response.text(),
        Some(""),
        "THEOREM: when only tool calls and no text, text MUST be empty string"
    );
    assert!(
        response.has_tool_calls(),
        "THEOREM: has_tool_calls() MUST be true"
    );
}

#[tokio::test]
async fn test_deductive_completion_response_text_method() {
    let response = CompletionResponse {
        id: "r1".into(),
        model: "test".into(),
        content: MessageContent::text("hello world"),
        stop_reason: StopReason::EndTurn,
        usage: Default::default(),
        tool_calls: vec![],
        thinking: None,
    };

    assert_eq!(
        response.text(),
        Some("hello world"),
        "THEOREM: text() MUST return content text"
    );
}

#[tokio::test]
async fn test_deductive_completion_response_has_tool_calls() {
    let with_tools = CompletionResponse {
        id: "r1".into(),
        model: "test".into(),
        content: MessageContent::text(""),
        stop_reason: StopReason::ToolCall,
        usage: Default::default(),
        tool_calls: vec![crate::tool::ToolCall {
            id: "c1".into(),
            name: "t".into(),
            arguments: "{}".into(),
        }],
        thinking: None,
    };

    let without_tools = CompletionResponse {
        id: "r2".into(),
        model: "test".into(),
        content: MessageContent::text("hello"),
        stop_reason: StopReason::EndTurn,
        usage: Default::default(),
        tool_calls: vec![],
        thinking: None,
    };

    assert!(
        with_tools.has_tool_calls(),
        "THEOREM: has_tool_calls() MUST return true when tool_calls is non-empty"
    );
    assert!(
        !without_tools.has_tool_calls(),
        "THEOREM: has_tool_calls() MUST return false when tool_calls is empty"
    );
}

#[tokio::test]
async fn test_deductive_completion_response_has_thinking() {
    let with_thinking = CompletionResponse {
        id: "r1".into(),
        model: "test".into(),
        content: MessageContent::text("answer"),
        stop_reason: StopReason::EndTurn,
        usage: Default::default(),
        tool_calls: vec![],
        thinking: Some(ThinkingContent {
            text: "reasoning".into(),
            signature: None,
        }),
    };

    let without_thinking = CompletionResponse {
        id: "r2".into(),
        model: "test".into(),
        content: MessageContent::text("answer"),
        stop_reason: StopReason::EndTurn,
        usage: Default::default(),
        tool_calls: vec![],
        thinking: None,
    };

    assert!(
        with_thinking.has_thinking(),
        "THEOREM: has_thinking() MUST return true when thinking is Some"
    );
    assert!(
        !without_thinking.has_thinking(),
        "THEOREM: has_thinking() MUST return false when thinking is None"
    );
}

#[tokio::test]
async fn test_deductive_from_stream_ignores_non_content_events() {
    let events = vec![
        Ok(StreamEvent::Queued { position: 1 }),
        Ok(StreamEvent::Started),
        Ok(StreamEvent::ResponseMeta {
            id: "resp_1".into(),
            model: crate::provider::types::ModelId::new("gpt-4o"),
        }),
        Ok(StreamEvent::Text("Hello".into())),
        Ok(StreamEvent::RedactedThinking { data: "encrypted".into() }),
        Ok(StreamEvent::UsageUpdate(TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            ..Default::default()
        })),
        Ok(StreamEvent::Stop(StopReason::EndTurn)),
    ];
    let stream = make_stream(events);
    let response = CompletionResponse::from_stream(stream).await.unwrap();

    assert_eq!(
        response.text(),
        Some("Hello"),
        "THEOREM: non-content events (Queued, Started, RedactedThinking, etc.) MUST be ignored for text"
    );
    assert!(
        !response.has_thinking(),
        "THEOREM: RedactedThinking MUST NOT set has_thinking()"
    );
}

#[tokio::test]
async fn test_deductive_thinking_signature_preserved() {
    let events = vec![
        Ok(StreamEvent::Thinking {
            text: "first".into(),
            signature: Some("sig_1".into()),
        }),
        Ok(StreamEvent::Thinking {
            text: "second".into(),
            signature: Some("sig_2".into()),
        }),
        Ok(StreamEvent::Stop(StopReason::EndTurn)),
    ];
    let stream = make_stream(events);
    let response = CompletionResponse::from_stream(stream).await.unwrap();

    assert_eq!(
        response.thinking.as_ref().unwrap().signature.as_deref(),
        Some("sig_2"),
        "THEOREM: last non-None signature MUST be preserved"
    );
}