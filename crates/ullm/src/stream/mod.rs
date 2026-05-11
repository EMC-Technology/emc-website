/// Anthropic SSE 事件映射器
pub mod anthropic_mapper;
/// 流式 JSON 修复工具
pub mod json_fix;
/// `OpenAI` SSE 事件映射器
pub mod openai_mapper;
/// Server-Sent Events 解析器
pub mod sse;
/// 工具调用状态累积器
pub mod tool_call_state;

use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::provider::types::TokenUsage;
use crate::provider::types::{ModelId, StopReason};
use crate::tool::ToolCall;

/// 流式事件枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEvent {
    /// 响应元数据（ID 和模型标识）
    ResponseMeta {
        /// 响应唯一标识
        id: String,
        /// 模型标识
        model: ModelId,
    },
    /// 请求排队等待
    Queued {
        /// 队列位置
        position: usize,
    },
    /// 流式传输开始
    Started,
    /// 文本增量
    Text(String),
    /// 思维链增量
    Thinking {
        /// 思维文本
        text: String,
        /// 签名（用于验证思维链完整性）
        signature: Option<String>,
    },
    /// 已脱敏的思维链（不可见原文）
    RedactedThinking {
        /// 加密数据
        data: String,
    },
    /// 工具调用
    ToolUse(ToolCall),
    /// 工具调用 JSON 解析错误
    ToolUseJsonParseError {
        /// 调用标识
        id: String,
        /// 工具名称
        tool_name: String,
        /// 原始输入
        raw_input: String,
        /// 解析错误信息
        json_parse_error: String,
    },
    /// Token 用量更新
    UsageUpdate(TokenUsage),
    /// 流式传输停止
    Stop(StopReason),
    /// 错误事件
    Error(String),
}

impl StreamEvent {
    /// 是否为文本事件
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self, StreamEvent::Text(_))
    }

    /// 是否为思维链事件
    #[must_use]
    pub fn is_thinking(&self) -> bool {
        matches!(
            self,
            StreamEvent::Thinking { .. } | StreamEvent::RedactedThinking { .. }
        )
    }

    /// 是否为工具调用事件
    #[must_use]
    pub fn is_tool_use(&self) -> bool {
        matches!(self, StreamEvent::ToolUse(_))
    }

    /// 是否为停止事件
    #[must_use]
    pub fn is_stop(&self) -> bool {
        matches!(self, StreamEvent::Stop(_))
    }

    /// 是否为错误事件
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, StreamEvent::Error(_))
    }

    /// 提取文本内容
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            StreamEvent::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// 模型流类型（异步 Stream 的 Pin<Box> 别名）
pub type ModelStream =
    std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<StreamEvent, LlmError>> + Send>>;

/// 流式 Future 类型（返回 `ModelStream` 的异步 Future 别名）
pub type StreamFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<ModelStream, LlmError>> + Send + 'a>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_event_is_text() {
        assert!(StreamEvent::Text("hello".into()).is_text());
        assert!(!StreamEvent::Started.is_text());
    }

    #[test]
    fn test_stream_event_is_thinking() {
        assert!(
            StreamEvent::Thinking {
                text: "hmm".into(),
                signature: None
            }
            .is_thinking()
        );
        assert!(StreamEvent::RedactedThinking { data: "x".into() }.is_thinking());
        assert!(!StreamEvent::Text("hello".into()).is_thinking());
    }

    #[test]
    fn test_stream_event_is_tool_use() {
        assert!(
            StreamEvent::ToolUse(crate::tool::ToolCall {
                id: "c1".into(),
                name: "tool".into(),
                arguments: "{}".into(),
            })
            .is_tool_use()
        );
        assert!(!StreamEvent::Started.is_tool_use());
    }

    #[test]
    fn test_stream_event_is_stop() {
        assert!(StreamEvent::Stop(StopReason::EndTurn).is_stop());
        assert!(!StreamEvent::Started.is_stop());
    }

    #[test]
    fn test_stream_event_is_error() {
        assert!(StreamEvent::Error("fail".into()).is_error());
        assert!(!StreamEvent::Started.is_error());
    }

    #[test]
    fn test_stream_event_as_text() {
        assert_eq!(StreamEvent::Text("hello".into()).as_text(), Some("hello"));
        assert_eq!(StreamEvent::Started.as_text(), None);
    }

    #[test]
    fn test_stream_event_serde_roundtrip() {
        let event = StreamEvent::Text("hello".into());
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: StreamEvent = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_text());
    }
}
