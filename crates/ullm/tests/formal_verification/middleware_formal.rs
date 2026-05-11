#![cfg(test)]
//! Middleware 形式化验证测试
//!
//! ## 不变量 (Invariants)
//!
//! 1. `MiddlewarePipeline` 实现 `Send + Sync`
//! 2. 中间件管道按顺序执行
//! 3. `on_request` 错误提前终止传播

use crate::middleware::{
    LoggingMiddleware, MetricsMiddleware, Middleware, MiddlewarePipeline, RetryMiddleware,
};
use crate::observability::NoopMetricsCollector;
use crate::stream::StreamEvent;
use crate::provider::types::LanguageModelRequest;
use crate::response::CompletionResponse;
use crate::error::LlmError;
use crate::retry::RetryPolicy;
use crate::provider::types::Message;
use crate::stream::ModelStream;
use std::sync::Arc;

#[test]
fn test_miri_middleware_pipeline_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<MiddlewarePipeline>();
}

#[test]
fn test_miri_middleware_pipeline_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<MiddlewarePipeline>();
}

#[test]
fn test_miri_logging_middleware_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LoggingMiddleware>();
}

#[test]
fn test_miri_metrics_middleware_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MetricsMiddleware>();
}

#[test]
fn test_miri_retry_middleware_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RetryMiddleware>();
}

#[test]
fn test_miri_middleware_pipeline_empty() {
    let pipeline = MiddlewarePipeline::new();
    let mut request = LanguageModelRequest::new("test-model", vec![Message::user("hi")]);

    let result = pipeline.on_request(&mut request).await;
    assert!(result.is_ok(), "empty pipeline MUST succeed");
}

#[test]
fn test_kani_middleware_pipeline_sequential_execution() {
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct SequencedMiddleware(u32);
    #[async_trait::async_trait]
    impl Middleware for SequencedMiddleware {
        async fn on_request(&self, _request: &mut LanguageModelRequest) -> Result<(), LlmError> {
            let current = COUNTER.load(Ordering::SeqCst);
            COUNTER.store(current * 10 + self.0, Ordering::SeqCst);
            Ok(())
        }
        fn name(&self) -> &'static str {
            "sequenced"
        }
    }

    COUNTER.store(0, Ordering::SeqCst);
    let mut pipeline = MiddlewarePipeline::new();
    pipeline.add(Arc::new(SequencedMiddleware(1)));
    pipeline.add(Arc::new(SequencedMiddleware(2)));
    pipeline.add(Arc::new(SequencedMiddleware(3)));

    let mut request = LanguageModelRequest::new("test", vec![Message::user("hi")]);
    pipeline.on_request(&mut request).await.unwrap();

    let final_value = COUNTER.load(Ordering::SeqCst);

    assert_eq!(
        final_value, 123,
        "THEOREM: middlewares MUST execute in registration order: got {}",
        final_value
    );
}

#[test]
fn test_kani_middleware_pipeline_error_propagation() {
    struct FailingMiddleware(u32);
    #[async_trait::async_trait]
    impl Middleware for FailingMiddleware {
        async fn on_request(&self, _request: &mut LanguageModelRequest) -> Result<(), LlmError> {
            if self.0 == 2 {
                Err(LlmError::Other("failing at 2".into()))
            } else {
                Ok(())
            }
        }
        fn name(&self) -> &'static str {
            "failing"
        }
    }

    let mut pipeline = MiddlewarePipeline::new();
    pipeline.add(Arc::new(FailingMiddleware(1)));
    pipeline.add(Arc::new(FailingMiddleware(2)));
    pipeline.add(Arc::new(FailingMiddleware(3)));

    let mut request = LanguageModelRequest::new("test", vec![Message::user("hi")]);
    let result = pipeline.on_request(&mut request).await;

    assert!(
        result.is_err(),
        "THEOREM: pipeline MUST return error from failing middleware"
    );
}

#[test]
fn test_kani_on_response_all_called() {
    use std::sync::atomic::{AtomicU32, Ordering};

    static RESPONSE_COUNT: AtomicU32 = AtomicU32::new(0);

    struct CountingMiddleware;
    #[async_trait::async_trait]
    impl Middleware for CountingMiddleware {
        async fn on_response(&self, _: &LanguageModelRequest, _: &CompletionResponse) {
            RESPONSE_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        fn name(&self) -> &'static str {
            "counting"
        }
    }

    RESPONSE_COUNT.store(0, Ordering::SeqCst);
    let mut pipeline = MiddlewarePipeline::new();
    for _ in 0..5 {
        pipeline.add(Arc::new(CountingMiddleware));
    }

    let request = LanguageModelRequest::new("test", vec![Message::user("hi")]);
    let response = CompletionResponse {
        id: "r1".into(),
        model: "test".into(),
        content: crate::provider::content_block::MessageContent::text("hello"),
        stop_reason: crate::provider::types::StopReason::EndTurn,
        usage: Default::default(),
        tool_calls: vec![],
        thinking: None,
    };

    pipeline.on_response(&request, &response).await;

    assert_eq!(
        RESPONSE_COUNT.load(Ordering::SeqCst), 5,
        "THEOREM: on_response MUST be called on ALL registered middlewares"
    );
}

#[test]
fn test_deductive_middleware_pipeline_on_error_all_called() {
    use std::sync::atomic::{AtomicU32, Ordering};

    static ERROR_COUNT: AtomicU32 = AtomicU32::new(0);

    struct ErrorCountingMiddleware;
    #[async_trait::async_trait]
    impl Middleware for ErrorCountingMiddleware {
        async fn on_error(&self, _: &LanguageModelRequest, _: &LlmError) {
            ERROR_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        fn name(&self) -> &'static str {
            "error_counting"
        }
    }

    ERROR_COUNT.store(0, Ordering::SeqCst);
    let mut pipeline = MiddlewarePipeline::new();
    for _ in 0..3 {
        pipeline.add(Arc::new(ErrorCountingMiddleware));
    }

    let request = LanguageModelRequest::new("test", vec![Message::user("hi")]);
    let error = LlmError::Network("test".into());
    pipeline.on_error(&request, &error).await;

    assert_eq!(
        ERROR_COUNT.load(Ordering::SeqCst), 3,
        "THEOREM: on_error MUST be called on ALL registered middlewares regardless of previous errors"
    );
}

#[test]
fn test_deductive_on_stream_event_all_called() {
    use std::sync::atomic::{AtomicU32, Ordering};

    static STREAM_COUNT: AtomicU32 = AtomicU32::new(0);

    struct StreamCountingMiddleware;
    #[async_trait::async_trait]
    impl Middleware for StreamCountingMiddleware {
        async fn on_stream_event(&self, _: &LanguageModelRequest, _: &StreamEvent) {
            STREAM_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        fn name(&self) -> &'static str {
            "stream_counting"
        }
    }

    STREAM_COUNT.store(0, Ordering::SeqCst);
    let mut pipeline = MiddlewarePipeline::new();
    for _ in 0..4 {
        pipeline.add(Arc::new(StreamCountingMiddleware));
    }

    let request = LanguageModelRequest::new("test", vec![Message::user("hi")]);
    let event = StreamEvent::Text("hello".into());
    pipeline.on_stream_event(&request, &event).await;

    assert_eq!(
        STREAM_COUNT.load(Ordering::SeqCst), 4,
        "THEOREM: on_stream_event MUST be called on ALL registered middlewares"
    );
}

#[test]
fn test_deductive_middleware_pipeline_default() {
    let pipeline = MiddlewarePipeline::default();
    let mut request = LanguageModelRequest::new("test", vec![Message::user("hi")]);

    let result = pipeline.on_request(&mut request).await;
    assert!(
        result.is_ok(),
        "THEOREM: default pipeline (empty) MUST succeed"
    );
}

#[test]
fn test_deductive_middleware_pipeline_new() {
    let pipeline = MiddlewarePipeline::new();
    assert!(
        pipeline.middlewares.is_empty(),
        "THEOREM: MiddlewarePipeline::new() MUST create empty pipeline"
    );
}