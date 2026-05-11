use std::pin::Pin;

use futures_util::StreamExt;

use crate::error::LlmError;
use crate::provider::LanguageModel;
use crate::provider::types::LanguageModelRequest;
use crate::response::CompletionResponse;
use crate::stream::StreamEvent;

impl dyn LanguageModel {
    /// 非流式补全便捷方法，内部调用 `stream_completion` 并收集完整响应。
    ///
    /// # Errors
    ///
    /// 当流式请求失败或响应中包含 `StreamEvent::Error` 时返回 `LlmError`。
    pub async fn complete(
        &self,
        request: LanguageModelRequest,
    ) -> Result<CompletionResponse, LlmError> {
        let stream = self.stream_completion(request).await?;
        CompletionResponse::from_stream(stream).await
    }

    /// 流式补全便捷方法，仅产出文本事件。
    ///
    /// # Errors
    ///
    /// 当流式请求失败或响应中包含 `StreamEvent::Error` 时返回 `LlmError`。
    pub async fn stream_text(
        &self,
        request: LanguageModelRequest,
    ) -> Result<Pin<Box<dyn futures_util::Stream<Item = Result<String, LlmError>> + Send>>, LlmError>
    {
        let stream = self.stream_completion(request).await?;
        Ok(Box::pin(stream.filter_map(|event| async move {
            match event {
                Ok(StreamEvent::Text(text)) => Some(Ok(text)),
                Ok(StreamEvent::Error(msg)) => Some(Err(LlmError::StreamError(msg))),
                Ok(
                    StreamEvent::Stop(_)
                    | StreamEvent::ResponseMeta { .. }
                    | StreamEvent::Queued { .. }
                    | StreamEvent::Started
                    | StreamEvent::Thinking { .. }
                    | StreamEvent::RedactedThinking { .. }
                    | StreamEvent::ToolUse(_)
                    | StreamEvent::ToolUseJsonParseError { .. }
                    | StreamEvent::UsageUpdate(_),
                ) => None,
                Err(e) => Some(Err(e)),
            }
        })))
    }

    /// 流式补全便捷方法，产出所有事件（包括工具调用）。
    ///
    /// # Errors
    ///
    /// 当流式请求失败时返回 `LlmError`。
    pub async fn stream_with_tools(
        &self,
        request: LanguageModelRequest,
        _tool_loop: &crate::tool::ToolCallLoop,
    ) -> Result<
        Pin<Box<dyn futures_util::Stream<Item = Result<StreamEvent, LlmError>> + Send>>,
        LlmError,
    > {
        let stream = self.stream_completion(request).await?;
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::StreamEvent;
    use crate::provider::types::{
        LanguageModelRequest, Message, ModelId, ModelName, ProviderId, ProviderName,
    };
    use crate::stream::ModelStream;
    use crate::stream::StreamFuture;
    use crate::token_count::TokenCountFuture;
    use futures_util::StreamExt;

    struct MockModel;

    #[async_trait::async_trait]
    impl crate::provider::LanguageModel for MockModel {
        fn id(&self) -> &ModelId {
            unimplemented!()
        }
        fn name(&self) -> &ModelName {
            unimplemented!()
        }
        fn provider_id(&self) -> &ProviderId {
            unimplemented!()
        }
        fn provider_name(&self) -> &ProviderName {
            unimplemented!()
        }
        fn supports_tools(&self) -> bool {
            false
        }
        fn supports_streaming_tools(&self) -> bool {
            false
        }
        fn supports_images(&self) -> bool {
            false
        }
        fn supports_thinking(&self) -> bool {
            false
        }
        fn max_token_count(&self) -> u64 {
            4096
        }
        fn max_output_tokens(&self) -> Option<u64> {
            None
        }
        fn count_tokens(&self, _request: &LanguageModelRequest) -> TokenCountFuture<'_> {
            Box::pin(async { Ok(0) })
        }
        fn stream_completion(&self, _request: LanguageModelRequest) -> StreamFuture<'_> {
            Box::pin(async {
                let stream: ModelStream = Box::pin(futures_util::stream::iter(vec![
                    Ok(StreamEvent::Text("Hello".into())),
                    Ok(StreamEvent::Stop(
                        crate::provider::types::StopReason::EndTurn,
                    )),
                ]));
                Ok(stream)
            })
        }
    }

    #[tokio::test]
    async fn test_dyn_complete() {
        let model: Arc<dyn crate::provider::LanguageModel> = Arc::new(MockModel);
        let request = LanguageModelRequest::new("test", vec![Message::user("hi")]);
        let response = model.complete(request).await.unwrap();
        assert_eq!(response.text(), Some("Hello"));
    }

    #[tokio::test]
    async fn test_dyn_stream_text() {
        let model: Arc<dyn crate::provider::LanguageModel> = Arc::new(MockModel);
        let request = LanguageModelRequest::new("test", vec![Message::user("hi")]).stream();
        let stream = model.stream_text(request).await.unwrap();
        let texts: Vec<String> = stream.filter_map(|r| async move { r.ok() }).collect().await;
        assert_eq!(texts, vec!["Hello"]);
    }

    #[tokio::test]
    async fn test_dyn_stream_with_tools() {
        let model: Arc<dyn crate::provider::LanguageModel> = Arc::new(MockModel);
        let request = LanguageModelRequest::new("test", vec![Message::user("hi")]).stream();
        let tool_loop = crate::tool::ToolCallLoop::new(vec![], 5);
        let stream = model.stream_with_tools(request, &tool_loop).await.unwrap();
        let events: Vec<_> = stream.collect().await;
        assert!(!events.is_empty());
    }

    struct MockModelWithError;

    #[async_trait::async_trait]
    impl crate::provider::LanguageModel for MockModelWithError {
        fn id(&self) -> &ModelId {
            unimplemented!()
        }
        fn name(&self) -> &ModelName {
            unimplemented!()
        }
        fn provider_id(&self) -> &ProviderId {
            unimplemented!()
        }
        fn provider_name(&self) -> &ProviderName {
            unimplemented!()
        }
        fn supports_tools(&self) -> bool {
            false
        }
        fn supports_streaming_tools(&self) -> bool {
            false
        }
        fn supports_images(&self) -> bool {
            false
        }
        fn supports_thinking(&self) -> bool {
            false
        }
        fn max_token_count(&self) -> u64 {
            4096
        }
        fn max_output_tokens(&self) -> Option<u64> {
            None
        }
        fn count_tokens(&self, _request: &LanguageModelRequest) -> TokenCountFuture<'_> {
            Box::pin(async { Ok(0) })
        }
        fn stream_completion(&self, _request: LanguageModelRequest) -> StreamFuture<'_> {
            Box::pin(async {
                let stream: ModelStream = Box::pin(futures_util::stream::iter(vec![
                    Ok(StreamEvent::Text("Hello".into())),
                    Ok(StreamEvent::Error("stream error".into())),
                ]));
                Ok(stream)
            })
        }
    }

    #[tokio::test]
    async fn test_dyn_complete_with_stream_error() {
        let model: Arc<dyn crate::provider::LanguageModel> = Arc::new(MockModelWithError);
        let request = LanguageModelRequest::new("test", vec![Message::user("hi")]);
        let result = model.complete(request).await;
        assert!(matches!(
            result,
            Err(crate::error::LlmError::StreamError(_))
        ));
    }

    struct MockModelWithThinkingAndTools;

    #[async_trait::async_trait]
    impl crate::provider::LanguageModel for MockModelWithThinkingAndTools {
        fn id(&self) -> &ModelId {
            unimplemented!()
        }
        fn name(&self) -> &ModelName {
            unimplemented!()
        }
        fn provider_id(&self) -> &ProviderId {
            unimplemented!()
        }
        fn provider_name(&self) -> &ProviderName {
            unimplemented!()
        }
        fn supports_tools(&self) -> bool {
            true
        }
        fn supports_streaming_tools(&self) -> bool {
            true
        }
        fn supports_images(&self) -> bool {
            false
        }
        fn supports_thinking(&self) -> bool {
            true
        }
        fn max_token_count(&self) -> u64 {
            4096
        }
        fn max_output_tokens(&self) -> Option<u64> {
            None
        }
        fn count_tokens(&self, _request: &LanguageModelRequest) -> TokenCountFuture<'_> {
            Box::pin(async { Ok(0) })
        }
        fn stream_completion(&self, _request: LanguageModelRequest) -> StreamFuture<'_> {
            Box::pin(async {
                let stream: ModelStream = Box::pin(futures_util::stream::iter(vec![
                    Ok(StreamEvent::Thinking {
                        text: "hmm".into(),
                        signature: None,
                    }),
                    Ok(StreamEvent::Text("Hello".into())),
                    Ok(StreamEvent::ToolUse(crate::tool::ToolCall {
                        id: "c1".into(),
                        name: "tool".into(),
                        arguments: "{}".into(),
                    })),
                    Ok(StreamEvent::Stop(
                        crate::provider::types::StopReason::ToolCall,
                    )),
                ]));
                Ok(stream)
            })
        }
    }

    #[tokio::test]
    async fn test_dyn_stream_text_filters_thinking_and_tools() {
        let model: Arc<dyn crate::provider::LanguageModel> =
            Arc::new(MockModelWithThinkingAndTools);
        let request = LanguageModelRequest::new("test", vec![Message::user("hi")]).stream();
        let stream = model.stream_text(request).await.unwrap();
        let texts: Vec<String> = stream.filter_map(|r| async move { r.ok() }).collect().await;
        assert_eq!(texts, vec!["Hello"]);
    }

    #[tokio::test]
    async fn test_dyn_stream_with_tools_gets_all_events() {
        let model: Arc<dyn crate::provider::LanguageModel> =
            Arc::new(MockModelWithThinkingAndTools);
        let request = LanguageModelRequest::new("test", vec![Message::user("hi")]).stream();
        let tool_loop = crate::tool::ToolCallLoop::new(vec![], 5);
        let stream = model.stream_with_tools(request, &tool_loop).await.unwrap();
        let events: Vec<_> = stream.collect().await;
        assert_eq!(events.len(), 4);
    }

    #[tokio::test]
    async fn test_dyn_complete_with_thinking_and_tools() {
        let model: Arc<dyn crate::provider::LanguageModel> =
            Arc::new(MockModelWithThinkingAndTools);
        let request = LanguageModelRequest::new("test", vec![Message::user("hi")]);
        let response = model.complete(request).await.unwrap();
        assert!(response.has_thinking());
        assert!(response.has_tool_calls());
    }
}
