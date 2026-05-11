use serde::Deserialize;

use crate::error::LlmError;
use crate::provider::types::{ModelId, StopReason, TokenUsage};
use crate::tool::ToolCall;

use super::{StreamEvent, sse::SseEventMapper};

#[derive(Debug, Deserialize)]
struct OpenAiChunk {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    delta: OpenAiDelta,
    finish_reason: Option<String>,
    #[allow(dead_code)]
    index: u32,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct OpenAiDelta {
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<OpenAiToolCallDelta>>,
    role: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiToolCallDelta {
    #[allow(dead_code)]
    index: u32,
    id: Option<String>,
    r#type: Option<String>,
    function: Option<OpenAiFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAiFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiUsage {
    #[serde(rename = "prompt_tokens")]
    prompt: Option<u32>,
    #[serde(rename = "completion_tokens")]
    completion: Option<u32>,
    #[serde(rename = "total_tokens")]
    total: Option<u32>,
}

/// `OpenAI` SSE 事件映射器
pub struct OpenAiSseMapper;

impl SseEventMapper for OpenAiSseMapper {
    fn map_frame(&self, frame: &str) -> Result<Vec<StreamEvent>, LlmError> {
        let Some(payload) = super::sse::extract_data_lines(frame) else {
            return Ok(Vec::new());
        };

        let chunk: OpenAiChunk =
            serde_json::from_str(&payload).map_err(|e| LlmError::JsonParse {
                provider: "openai".into(),
                model: "unknown".into(),
                body_snippet: payload.chars().take(200).collect(),
                source: e,
            })?;

        let mut events: Vec<StreamEvent> = Vec::new();

        let meta_id = chunk.id.unwrap_or_default();
        let meta_model = chunk.model.unwrap_or_default();
        if !meta_id.is_empty() || !meta_model.is_empty() {
            events.push(StreamEvent::ResponseMeta {
                id: meta_id,
                model: ModelId::new(meta_model),
            });
        }

        if let Some(usage) = chunk.usage {
            events.push(StreamEvent::UsageUpdate(TokenUsage {
                prompt_tokens: usage.prompt.unwrap_or(0),
                completion_tokens: usage.completion.unwrap_or(0),
                ..Default::default()
            }));
            return Ok(events);
        }

        let Some(choice) = chunk.choices.first() else {
            return Ok(events);
        };

        if let Some(reason) = &choice.finish_reason {
            let stop_reason = match reason.as_str() {
                "stop" => StopReason::EndTurn,
                "tool_calls" => StopReason::ToolCall,
                "length" => StopReason::MaxTokens,
                "content_filter" => StopReason::Cancelled,
                other => {
                    tracing::warn!(
                        reason = other,
                        "unknown OpenAI finish_reason, mapping to EndTurn"
                    );
                    StopReason::EndTurn
                }
            };
            events.push(StreamEvent::Stop(stop_reason));
            return Ok(events);
        }

        if let Some(reasoning) = &choice.delta.reasoning_content
            && !reasoning.is_empty()
        {
            events.push(StreamEvent::Thinking {
                text: reasoning.clone(),
                signature: None,
            });
        }

        if let Some(content) = &choice.delta.content
            && !content.is_empty()
        {
            events.push(StreamEvent::Text(content.clone()));
        }

        if let Some(tool_calls) = &choice.delta.tool_calls
            && let Some(tc) = tool_calls.first()
        {
            let id = tc.id.clone().unwrap_or_default();
            let name = tc
                .function
                .as_ref()
                .and_then(|f| f.name.clone())
                .unwrap_or_default();
            let arguments = tc
                .function
                .as_ref()
                .and_then(|f| f.arguments.clone())
                .unwrap_or_default();
            events.push(StreamEvent::ToolUse(ToolCall {
                id,
                name,
                arguments,
            }));
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::sse::SseEventMapper;

    fn first_event(events: Vec<StreamEvent>) -> Option<StreamEvent> {
        events
            .into_iter()
            .find(|e| !matches!(e, StreamEvent::ResponseMeta { .. }))
    }

    #[test]
    fn test_openai_mapper_reasoning_content() {
        let frame = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"thinking...\"},\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Thinking { text, signature }) => {
                assert_eq!(text, "thinking...");
                assert_eq!(signature, None);
            }
            other => panic!("expected Thinking event, got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_reasoning_empty() {
        let frame =
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"\"},\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        assert!(
            result.is_empty()
                || result
                    .iter()
                    .all(|e| matches!(e, StreamEvent::ResponseMeta { .. }))
        );
    }

    #[test]
    fn test_openai_mapper_content() {
        let frame = "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"},\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Text(text)) => assert_eq!(text, "hello"),
            other => panic!("expected Text event, got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_stop() {
        let frame =
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\",\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Stop(StopReason::EndTurn)) => {}
            other => panic!("expected Stop(EndTurn), got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_usage() {
        let frame = "data: {\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":20,\"total_tokens\":30}}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::UsageUpdate(usage)) => {
                assert_eq!(usage.prompt_tokens, 10);
                assert_eq!(usage.completion_tokens, 20);
            }
            other => panic!("expected UsageUpdate, got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_reasoning_with_content() {
        let frame = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"thinking...\",\"content\":\"hello\"},\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Thinking { text, .. }) => assert_eq!(text, "thinking..."),
            other => panic!("expected Thinking event (reasoning before content), got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_content_without_reasoning() {
        let frame = "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"},\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Text(text)) => assert_eq!(text, "hello"),
            other => panic!("expected Text event, got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_tool_calls() {
        let frame = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"city\\\":\\\"SF\\\"}\"}}]},\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::ToolUse(call)) => {
                assert_eq!(call.id, "call_1");
                assert_eq!(call.name, "get_weather");
                assert_eq!(call.arguments, r#"{"city":"SF"}"#);
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_tool_calls_no_function() {
        let frame = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_2\"}]},\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::ToolUse(call)) => {
                assert_eq!(call.id, "call_2");
                assert_eq!(call.name, "");
                assert_eq!(call.arguments, "");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_stop_tool_calls() {
        let frame =
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\",\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Stop(StopReason::ToolCall)) => {}
            other => panic!("expected Stop(ToolCall), got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_stop_length() {
        let frame =
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\",\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Stop(StopReason::MaxTokens)) => {}
            other => panic!("expected Stop(MaxTokens), got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_stop_content_filter() {
        let frame = "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"content_filter\",\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Stop(StopReason::Cancelled)) => {}
            other => panic!("expected Stop(Cancelled), got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_stop_unknown_finish_reason() {
        let frame = "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"custom_reason\",\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::Stop(StopReason::EndTurn)) => {}
            other => panic!("expected Stop(EndTurn) for unknown finish_reason, got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_empty_content() {
        let frame = "data: {\"choices\":[{\"delta\":{\"content\":\"\"},\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        assert!(
            result.is_empty()
                || result
                    .iter()
                    .all(|e| matches!(e, StreamEvent::ResponseMeta { .. }))
        );
    }

    #[test]
    fn test_openai_mapper_no_choices() {
        let frame = "data: {\"choices\":[]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        assert!(
            result.is_empty()
                || result
                    .iter()
                    .all(|e| matches!(e, StreamEvent::ResponseMeta { .. }))
        );
    }

    #[test]
    fn test_openai_mapper_done_signal() {
        let frame = "data: [DONE]\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_openai_mapper_invalid_json() {
        let frame = "data: {invalid json}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame);
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::JsonParse { provider, .. } => assert_eq!(provider, "openai"),
            other => panic!("expected JsonParse error, got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_usage_with_none_fields() {
        let frame = "data: {\"usage\":{\"prompt_tokens\":null,\"completion_tokens\":null,\"total_tokens\":null}}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        match first_event(result) {
            Some(StreamEvent::UsageUpdate(usage)) => {
                assert_eq!(usage.prompt_tokens, 0);
                assert_eq!(usage.completion_tokens, 0);
            }
            other => panic!("expected UsageUpdate, got {other:?}"),
        }
    }

    #[test]
    fn test_openai_mapper_delta_role_only() {
        let frame = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"},\"index\":0}]}\n\n";
        let mapper = OpenAiSseMapper;
        let result = mapper.map_frame(frame).unwrap();
        assert!(
            result.is_empty()
                || result
                    .iter()
                    .all(|e| matches!(e, StreamEvent::ResponseMeta { .. }))
        );
    }
}
