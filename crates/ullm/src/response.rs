use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::provider::content_block::MessageContent;
use crate::provider::types::{ModelId, StopReason, TokenUsage};
use crate::stream::ModelStream;
use crate::stream::StreamEvent;
use crate::tool::ToolCall;

use futures_util::StreamExt;

/// LLM 补全响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// 响应唯一标识
    pub id: String,
    /// 使用的模型标识
    pub model: ModelId,
    /// 响应内容
    pub content: MessageContent,
    /// 停止原因
    pub stop_reason: StopReason,
    /// Token 用量统计
    pub usage: TokenUsage,
    /// 工具调用列表
    pub tool_calls: Vec<ToolCall>,
    /// 思维链内容
    pub thinking: Option<ThinkingContent>,
}

/// 思维链内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingContent {
    /// 思维文本
    pub text: String,
    /// 签名（用于验证思维链完整性）
    pub signature: Option<String>,
}

impl CompletionResponse {
    /// 从流中收集完整的补全响应。
    ///
    /// # Errors
    ///
    /// 当流中发生错误事件或流处理失败时返回 `LlmError`。
    pub async fn from_stream(stream: ModelStream) -> Result<Self, LlmError> {
        let mut id = String::new();
        let mut model = ModelId::new("unknown");
        let mut text_parts: Vec<String> = Vec::new();
        let mut thinking_text = String::new();
        let mut thinking_signature: Option<String> = None;
        let mut stop_reason = StopReason::EndTurn;
        let mut usage = TokenUsage::default();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        let mut stream = stream;
        while let Some(event) = stream.next().await {
            match event {
                Ok(StreamEvent::ResponseMeta {
                    id: meta_id,
                    model: meta_model,
                }) => {
                    id = meta_id;
                    model = meta_model;
                }
                Ok(
                    StreamEvent::Started
                    | StreamEvent::Queued { .. }
                    | StreamEvent::RedactedThinking { .. }
                    | StreamEvent::ToolUseJsonParseError { .. },
                ) => {}
                Ok(StreamEvent::Text(text)) => {
                    text_parts.push(text);
                }
                Ok(StreamEvent::Thinking { text, signature }) => {
                    thinking_text.push_str(&text);
                    if signature.is_some() {
                        thinking_signature = signature;
                    }
                }
                Ok(StreamEvent::ToolUse(call)) => {
                    tool_calls.push(call);
                }
                Ok(StreamEvent::UsageUpdate(u)) => {
                    usage.merge(&u);
                }
                Ok(StreamEvent::Stop(reason)) => {
                    stop_reason = reason;
                }
                Ok(StreamEvent::Error(msg)) => {
                    return Err(LlmError::StreamError(msg));
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        let content = if text_parts.is_empty() && !tool_calls.is_empty() {
            MessageContent::text(String::new())
        } else {
            MessageContent::text(text_parts.join(""))
        };

        let thinking = if thinking_text.is_empty() {
            None
        } else {
            Some(ThinkingContent {
                text: thinking_text,
                signature: thinking_signature,
            })
        };

        Ok(Self {
            id,
            model,
            content,
            stop_reason,
            usage,
            tool_calls,
            thinking,
        })
    }

    /// 获取响应文本内容
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.content.as_text()
    }

    /// 是否包含工具调用
    #[must_use]
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    /// 是否包含思维链内容
    #[must_use]
    pub fn has_thinking(&self) -> bool {
        self.thinking.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::types::TokenUsage;
    use futures_util::stream;

    fn make_stream(events: Vec<Result<StreamEvent, LlmError>>) -> ModelStream {
        Box::pin(stream::iter(events))
    }

    #[tokio::test]
    async fn test_completion_response_from_stream() {
        let events = vec![
            Ok(StreamEvent::Started),
            Ok(StreamEvent::Text("Hello".into())),
            Ok(StreamEvent::Text(" world".into())),
            Ok(StreamEvent::UsageUpdate(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                ..Default::default()
            })),
            Ok(StreamEvent::Stop(StopReason::EndTurn)),
        ];
        let stream = make_stream(events);
        let response = CompletionResponse::from_stream(stream).await.unwrap();
        assert_eq!(response.text(), Some("Hello world"));
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert!(!response.has_tool_calls());
        assert!(!response.has_thinking());
        assert_eq!(response.usage.prompt_tokens, 10);
        assert_eq!(response.usage.completion_tokens, 5);
    }

    #[tokio::test]
    async fn test_completion_response_with_tool_calls() {
        let events = vec![
            Ok(StreamEvent::Started),
            Ok(StreamEvent::Text("Let me check".into())),
            Ok(StreamEvent::ToolUse(ToolCall {
                id: "call_1".into(),
                name: "get_weather".into(),
                arguments: r#"{"city":"SF"}"#.into(),
            })),
            Ok(StreamEvent::Stop(StopReason::ToolCall)),
        ];
        let stream = make_stream(events);
        let response = CompletionResponse::from_stream(stream).await.unwrap();
        assert!(response.has_tool_calls());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "get_weather");
        assert!(response.stop_reason.is_tool_call());
    }

    #[tokio::test]
    async fn test_completion_response_with_thinking() {
        let events = vec![
            Ok(StreamEvent::Started),
            Ok(StreamEvent::Thinking {
                text: "Let me think...".into(),
                signature: None,
            }),
            Ok(StreamEvent::Text("The answer is 42".into())),
            Ok(StreamEvent::Stop(StopReason::EndTurn)),
        ];
        let stream = make_stream(events);
        let response = CompletionResponse::from_stream(stream).await.unwrap();
        assert!(response.has_thinking());
        assert_eq!(response.thinking.as_ref().unwrap().text, "Let me think...");
        assert_eq!(response.text(), Some("The answer is 42"));
    }

    #[tokio::test]
    async fn test_completion_response_stream_error() {
        let events = vec![
            Ok(StreamEvent::Started),
            Ok(StreamEvent::Text("Hello".into())),
            Err(LlmError::StreamError("connection lost".into())),
        ];
        let stream = make_stream(events);
        let result = CompletionResponse::from_stream(stream).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_thinking_content() {
        let tc = ThinkingContent {
            text: "I need to reason about this".into(),
            signature: Some("sig123".into()),
        };
        assert_eq!(tc.text, "I need to reason about this");
        assert_eq!(tc.signature.as_deref(), Some("sig123"));
    }

    #[tokio::test]
    async fn test_completion_response_redacted_thinking() {
        let events = vec![
            Ok(StreamEvent::RedactedThinking {
                data: "encrypted".into(),
            }),
            Ok(StreamEvent::Text("Answer".into())),
            Ok(StreamEvent::Stop(StopReason::EndTurn)),
        ];
        let stream = make_stream(events);
        let response = CompletionResponse::from_stream(stream).await.unwrap();
        assert_eq!(response.text(), Some("Answer"));
        assert!(!response.has_thinking());
    }

    #[tokio::test]
    async fn test_completion_response_tool_use_json_parse_error() {
        let events = vec![
            Ok(StreamEvent::ToolUseJsonParseError {
                id: "call_1".into(),
                tool_name: "bad_tool".into(),
                raw_input: "{invalid".into(),
                json_parse_error: "expected `}`".into(),
            }),
            Ok(StreamEvent::Stop(StopReason::EndTurn)),
        ];
        let stream = make_stream(events);
        let response = CompletionResponse::from_stream(stream).await.unwrap();
        assert!(!response.has_tool_calls());
    }

    #[tokio::test]
    async fn test_completion_response_stream_error_event() {
        let events = vec![
            Ok(StreamEvent::Text("Hello".into())),
            Ok(StreamEvent::Error("something went wrong".into())),
        ];
        let stream = make_stream(events);
        let result = CompletionResponse::from_stream(stream).await;
        match result {
            Err(LlmError::StreamError(msg)) => assert_eq!(msg, "something went wrong"),
            _ => panic!("expected StreamError"),
        }
    }

    #[tokio::test]
    async fn test_completion_response_tool_calls_no_text() {
        let events = vec![
            Ok(StreamEvent::ToolUse(ToolCall {
                id: "call_1".into(),
                name: "get_weather".into(),
                arguments: r#"{"city":"SF"}"#.into(),
            })),
            Ok(StreamEvent::Stop(StopReason::ToolCall)),
        ];
        let stream = make_stream(events);
        let response = CompletionResponse::from_stream(stream).await.unwrap();
        assert!(response.has_tool_calls());
        assert_eq!(response.text(), Some(""));
    }

    #[tokio::test]
    async fn test_completion_response_thinking_with_signature() {
        let events = vec![
            Ok(StreamEvent::Thinking {
                text: "Let me think...".into(),
                signature: None,
            }),
            Ok(StreamEvent::Thinking {
                text: "...more thought".into(),
                signature: Some("sig_abc".into()),
            }),
            Ok(StreamEvent::Text("The answer".into())),
            Ok(StreamEvent::Stop(StopReason::EndTurn)),
        ];
        let stream = make_stream(events);
        let response = CompletionResponse::from_stream(stream).await.unwrap();
        assert!(response.has_thinking());
        assert_eq!(
            response.thinking.as_ref().unwrap().text,
            "Let me think......more thought"
        );
        assert_eq!(
            response.thinking.as_ref().unwrap().signature,
            Some("sig_abc".into())
        );
    }

    #[tokio::test]
    async fn test_completion_response_queued_and_started_events() {
        let events = vec![
            Ok(StreamEvent::Queued { position: 0 }),
            Ok(StreamEvent::Started),
            Ok(StreamEvent::Text("Hello".into())),
            Ok(StreamEvent::Stop(StopReason::EndTurn)),
        ];
        let stream = make_stream(events);
        let response = CompletionResponse::from_stream(stream).await.unwrap();
        assert_eq!(response.text(), Some("Hello"));
    }

    #[tokio::test]
    async fn test_completion_response_usage_update() {
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
        assert_eq!(response.usage.prompt_tokens, 10);
        assert_eq!(response.usage.completion_tokens, 15);
    }

    #[test]
    fn test_completion_response_has_tool_calls_false() {
        let response = CompletionResponse {
            id: "r1".into(),
            model: "test".into(),
            content: MessageContent::text("hello"),
            stop_reason: StopReason::EndTurn,
            usage: TokenUsage::default(),
            tool_calls: vec![],
            thinking: None,
        };
        assert!(!response.has_tool_calls());
    }

    #[test]
    fn test_completion_response_has_thinking_false() {
        let response = CompletionResponse {
            id: "r1".into(),
            model: "test".into(),
            content: MessageContent::text("hello"),
            stop_reason: StopReason::EndTurn,
            usage: TokenUsage::default(),
            tool_calls: vec![],
            thinking: None,
        };
        assert!(!response.has_thinking());
    }

    #[test]
    fn test_completion_response_serde_roundtrip() {
        let response = CompletionResponse {
            id: "r1".into(),
            model: "gpt-4o".into(),
            content: MessageContent::text("hello"),
            stop_reason: StopReason::EndTurn,
            usage: TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                ..Default::default()
            },
            tool_calls: vec![],
            thinking: Some(ThinkingContent {
                text: "thought".into(),
                signature: Some("sig".into()),
            }),
        };
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: CompletionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "r1");
        assert_eq!(deserialized.model.as_ref(), "gpt-4o");
        assert!(deserialized.has_thinking());
    }
}
