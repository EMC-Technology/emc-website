use std::pin::Pin;

use futures_util::StreamExt;

use crate::error::LlmError;
use crate::stream::{ModelStream, StreamEvent};

/// 将模型流转换为纯文本流，过滤非文本事件
#[must_use]
pub fn stream_to_text(
    stream: ModelStream,
) -> Pin<Box<dyn futures_util::Stream<Item = Result<String, LlmError>> + Send>> {
    Box::pin(stream.filter_map(|event| async move {
        match event {
            Ok(StreamEvent::Text(text) | StreamEvent::Thinking { text, .. }) => Some(Ok(text)),
            Ok(StreamEvent::Error(msg)) => Some(Err(LlmError::StreamError(msg))),
            Ok(_) | Err(_) => None,
        }
    }))
}

/// 将模型流转换为 error-core 兼容的文本流
#[must_use]
pub fn stream_to_error_core_text(
    stream: ModelStream,
) -> Pin<Box<dyn futures_util::Stream<Item = error_core::prelude::Result<String>> + Send>> {
    Box::pin(stream.filter_map(|event| async move {
        match event {
            Ok(StreamEvent::Text(text) | StreamEvent::Thinking { text, .. }) => Some(Ok(text)),
            Ok(StreamEvent::Error(msg)) => Some(Err(LlmError::StreamError(msg).to_error_object())),
            Err(e) => Some(Err(e.to_error_object())),
            Ok(_) => None,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;

    #[test]
    fn test_stream_to_text_extracts_text_events() {
        let events = vec![
            Ok(StreamEvent::Text("hello".into())),
            Ok(StreamEvent::Text(" world".into())),
            Ok(StreamEvent::Stop(
                crate::provider::types::StopReason::EndTurn,
            )),
        ];
        let input_stream: ModelStream = Box::pin(stream::iter(events));
        let text_stream = stream_to_text(input_stream);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let texts: Vec<Result<String, LlmError>> =
            rt.block_on(async { text_stream.collect().await });

        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0].as_ref().unwrap(), "hello");
        assert_eq!(texts[1].as_ref().unwrap(), " world");
    }

    #[test]
    fn test_stream_to_text_handles_error_event() {
        let events = vec![
            Ok(StreamEvent::Text("partial".into())),
            Ok(StreamEvent::Error("boom".into())),
        ];
        let input_stream: ModelStream = Box::pin(stream::iter(events));
        let text_stream = stream_to_text(input_stream);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let texts: Vec<Result<String, LlmError>> =
            rt.block_on(async { text_stream.collect().await });

        assert_eq!(texts.len(), 2);
        assert!(texts[0].is_ok());
        assert!(matches!(texts[1], Err(LlmError::StreamError(_))));
    }
}
