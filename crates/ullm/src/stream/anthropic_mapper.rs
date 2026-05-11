use serde::Deserialize;

use crate::error::LlmError;
use crate::provider::types::{ModelId, StopReason, TokenUsage};
use crate::tool::ToolCall;

use super::{StreamEvent, sse::SseEventMapper};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum AnthropicSseEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageData },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: u32,
        content_block: ContentBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        index: u32,
        delta: ContentBlockDelta,
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u32 },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: MessageDeltaData,
        usage: Option<DeltaUsage>,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: ErrorData },
}

#[derive(Debug, Deserialize)]
struct MessageData {
    #[allow(dead_code)]
    id: Option<String>,
    #[allow(dead_code)]
    model: Option<String>,
    usage: Option<UsageData>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: serde_json::Value },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
enum ContentBlockDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
    #[serde(rename = "signature_delta")]
    SignatureDelta { signature: String },
}

#[derive(Debug, Deserialize)]
struct MessageDeltaData {
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeltaUsage {
    output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct UsageData {
    #[serde(rename = "input_tokens")]
    input: Option<u32>,
    #[serde(rename = "output_tokens")]
    output: Option<u32>,
    #[serde(rename = "cache_creation_input_tokens")]
    cache_creation_input: Option<u32>,
    #[serde(rename = "cache_read_input_tokens")]
    cache_read_input: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ErrorData {
    #[allow(dead_code)]
    r#type: Option<String>,
    message: Option<String>,
}

/// Anthropic SSE 事件映射器
pub struct AnthropicSseMapper;

impl SseEventMapper for AnthropicSseMapper {
    fn map_frame(&self, frame: &str) -> Result<Vec<StreamEvent>, LlmError> {
        let Some(payload) = super::sse::extract_data_lines(frame) else {
            return Ok(Vec::new());
        };

        let event: AnthropicSseEvent =
            serde_json::from_str(&payload).map_err(|e| LlmError::JsonParse {
                provider: "anthropic".into(),
                model: "unknown".into(),
                body_snippet: payload.chars().take(200).collect(),
                source: e,
            })?;

        match event {
            AnthropicSseEvent::MessageStart { message } => {
                let mut events: Vec<StreamEvent> = Vec::new();
                let meta_id = message.id.unwrap_or_default();
                let meta_model = message.model.unwrap_or_default();
                if !meta_id.is_empty() || !meta_model.is_empty() {
                    events.push(StreamEvent::ResponseMeta {
                        id: meta_id,
                        model: ModelId::new(meta_model),
                    });
                }
                if let Some(usage) = message.usage {
                    events.push(StreamEvent::UsageUpdate(TokenUsage {
                        prompt_tokens: usage.input.unwrap_or(0),
                        cache_read_tokens: usage.cache_read_input.unwrap_or(0),
                        cache_write_tokens: usage.cache_creation_input.unwrap_or(0),
                        ..Default::default()
                    }));
                }
                if events.is_empty() {
                    events.push(StreamEvent::Started);
                }
                Ok(events)
            }
            AnthropicSseEvent::ContentBlockStart {
                content_block: ContentBlock::ToolUse { id, name, .. },
                ..
            } => Ok(vec![StreamEvent::ToolUse(ToolCall {
                id,
                name,
                arguments: String::new(),
            })]),
            AnthropicSseEvent::ContentBlockStart { .. }
            | AnthropicSseEvent::ContentBlockStop { .. }
            | AnthropicSseEvent::Ping => Ok(Vec::new()),
            AnthropicSseEvent::ContentBlockDelta { delta, .. } => match delta {
                ContentBlockDelta::TextDelta { text } => Ok(vec![StreamEvent::Text(text)]),
                ContentBlockDelta::ThinkingDelta { thinking } => Ok(vec![StreamEvent::Thinking {
                    text: thinking,
                    signature: None,
                }]),
                ContentBlockDelta::InputJsonDelta { partial_json } => {
                    Ok(vec![StreamEvent::ToolUse(ToolCall {
                        id: String::new(),
                        name: String::new(),
                        arguments: partial_json,
                    })])
                }
                ContentBlockDelta::SignatureDelta { signature } => {
                    Ok(vec![StreamEvent::Thinking {
                        text: String::new(),
                        signature: Some(signature),
                    }])
                }
            },
            AnthropicSseEvent::MessageDelta { delta, usage } => {
                if let Some(reason) = delta.stop_reason {
                    let stop_reason = match reason.as_str() {
                        "end_turn" => StopReason::EndTurn,
                        "tool_use" => StopReason::ToolCall,
                        "max_tokens" => StopReason::MaxTokens,
                        "stop_sequence" => StopReason::StopSequence,
                        other => {
                            tracing::warn!(
                                reason = other,
                                "unknown Anthropic stop_reason, mapping to EndTurn"
                            );
                            StopReason::EndTurn
                        }
                    };
                    Ok(vec![StreamEvent::Stop(stop_reason)])
                } else if let Some(u) = usage {
                    Ok(vec![StreamEvent::UsageUpdate(TokenUsage {
                        completion_tokens: u.output_tokens.unwrap_or(0),
                        ..Default::default()
                    })])
                } else {
                    Ok(Vec::new())
                }
            }
            AnthropicSseEvent::MessageStop => Ok(vec![StreamEvent::Stop(StopReason::EndTurn)]),
            AnthropicSseEvent::Error { error } => Ok(vec![StreamEvent::Error(
                error
                    .message
                    .unwrap_or_else(|| "unknown stream error".into()),
            )]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::sse::SseEventMapper;

    fn first_event(events: Vec<StreamEvent>) -> Option<StreamEvent> {
        events.into_iter().next()
    }

    #[test]
    fn test_anthropic_mapper_message_start_with_usage() {
        let frame = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":25,\"output_tokens\":0}}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::UsageUpdate(usage)) => assert_eq!(usage.prompt_tokens, 25),
            other => panic!("expected UsageUpdate, got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_message_start_no_usage() {
        let frame = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Started) => {}
            other => panic!("expected Started, got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_message_start_with_meta() {
        let frame = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"model\":\"claude-3-opus\",\"usage\":{\"input_tokens\":10}}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        assert!(!result.is_empty());
        match &result[0] {
            StreamEvent::ResponseMeta { id, model } => {
                assert_eq!(id, "msg_123");
                assert_eq!(model.as_str(), "claude-3-opus");
            }
            other => panic!("expected ResponseMeta, got {other:?}"),
        }
        assert!(
            result
                .iter()
                .any(|e| matches!(e, StreamEvent::UsageUpdate(_)))
        );
    }

    #[test]
    fn test_anthropic_mapper_content_block_start_tool_use() {
        let frame = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call_1\",\"name\":\"get_weather\",\"input\":{}}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::ToolUse(call)) => {
                assert_eq!(call.id, "call_1");
                assert_eq!(call.name, "get_weather");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_content_block_start_text() {
        let frame = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_anthropic_mapper_text_delta() {
        let frame = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Text(text)) => assert_eq!(text, "Hello"),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_thinking_delta() {
        let frame = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"Let me think\"}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Thinking { text, signature }) => {
                assert_eq!(text, "Let me think");
                assert!(signature.is_none());
            }
            other => panic!("expected Thinking, got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_signature_delta() {
        let frame = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig_abc\"}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Thinking { text, signature }) => {
                assert!(text.is_empty());
                assert_eq!(signature, Some("sig_abc".into()));
            }
            other => panic!("expected Thinking with signature, got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_input_json_delta() {
        let frame = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"city\\\":\\\"SF\\\"}\"}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::ToolUse(call)) => {
                assert_eq!(call.arguments, r#"{"city":"SF"}"#);
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_message_delta_stop_reason() {
        let frame = "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Stop(StopReason::EndTurn)) => {}
            other => panic!("expected Stop(EndTurn), got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_message_delta_tool_use_stop() {
        let frame = "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Stop(StopReason::ToolCall)) => {}
            other => panic!("expected Stop(ToolCall), got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_message_delta_max_tokens() {
        let frame = "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"max_tokens\"}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Stop(StopReason::MaxTokens)) => {}
            other => panic!("expected Stop(MaxTokens), got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_message_delta_stop_sequence() {
        let frame = "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"stop_sequence\"}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Stop(StopReason::StopSequence)) => {}
            other => panic!("expected Stop(StopSequence), got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_message_delta_usage() {
        let frame = "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{},\"usage\":{\"output_tokens\":15}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::UsageUpdate(usage)) => assert_eq!(usage.completion_tokens, 15),
            other => panic!("expected UsageUpdate, got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_message_stop() {
        let frame = "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Stop(StopReason::EndTurn)) => {}
            other => panic!("expected Stop(EndTurn), got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_ping() {
        let frame = "event: ping\ndata: {\"type\":\"ping\"}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_anthropic_mapper_error() {
        let frame =
            "event: error\ndata: {\"type\":\"error\",\"error\":{\"message\":\"overloaded\"}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Error(msg)) => assert_eq!(msg, "overloaded"),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_error_no_message() {
        let frame = "event: error\ndata: {\"type\":\"error\",\"error\":{}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Error(msg)) => assert_eq!(msg, "unknown stream error"),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn test_anthropic_mapper_message_start_with_cache_usage() {
        let frame = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10,\"cache_creation_input_tokens\":5,\"cache_read_input_tokens\":3}}}\n\n";
        let mapper = AnthropicSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::UsageUpdate(usage)) => {
                assert_eq!(usage.prompt_tokens, 10);
                assert_eq!(usage.cache_write_tokens, 5);
                assert_eq!(usage.cache_read_tokens, 3);
            }
            other => panic!("expected UsageUpdate, got {other:?}"),
        }
    }
}
