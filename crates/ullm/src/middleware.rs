use std::sync::Arc;

use async_trait::async_trait;

use crate::error::LlmError;
use crate::observability::{LogLevel, MetricsCollector};
use crate::provider::types::LanguageModelRequest;
use crate::response::CompletionResponse;
use crate::retry::RetryPolicy;
use crate::stream::StreamEvent;

/// 请求/响应中间件 trait，支持在 LLM 调用生命周期的各阶段注入自定义逻辑
#[async_trait]
pub trait Middleware: Send + Sync + 'static {
    /// 请求发出前的拦截，返回 Err 可中止请求
    async fn on_request(&self, _request: &mut LanguageModelRequest) -> Result<(), LlmError> {
        Ok(())
    }

    /// 响应返回后的回调
    async fn on_response(&self, _request: &LanguageModelRequest, _response: &CompletionResponse) {}

    /// 错误发生时的回调
    async fn on_error(&self, _request: &LanguageModelRequest, _error: &LlmError) {}

    /// 流式事件回调
    async fn on_stream_event(&self, _request: &LanguageModelRequest, _event: &StreamEvent) {}

    /// 中间件名称（用于日志/调试）
    fn name(&self) -> &'static str;
}

/// 中间件管道，按注册顺序依次调用各中间件
#[derive(Clone)]
pub struct MiddlewarePipeline {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewarePipeline {
    /// 创建空管道
    #[must_use]
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// 添加中间件到管道末尾
    pub fn add(&mut self, middleware: Arc<dyn Middleware>) {
        self.middlewares.push(middleware);
    }

    /// 依次调用管道中所有中间件的 `on_request`，任一中间件返回错误即提前终止。
    ///
    /// # Errors
    ///
    /// 当任一中间件的 `on_request` 返回错误时，将该错误向上传播。
    pub async fn on_request(&self, request: &mut LanguageModelRequest) -> Result<(), LlmError> {
        for mw in &self.middlewares {
            mw.on_request(request).await?;
        }
        Ok(())
    }

    /// 依次调用管道中所有中间件的 `on_response`
    pub async fn on_response(&self, request: &LanguageModelRequest, response: &CompletionResponse) {
        for mw in &self.middlewares {
            mw.on_response(request, response).await;
        }
    }

    /// 依次调用管道中所有中间件的 `on_error`
    pub async fn on_error(&self, request: &LanguageModelRequest, error: &LlmError) {
        for mw in &self.middlewares {
            mw.on_error(request, error).await;
        }
    }

    /// 依次调用管道中所有中间件的 `on_stream_event`
    pub async fn on_stream_event(&self, request: &LanguageModelRequest, event: &StreamEvent) {
        for mw in &self.middlewares {
            mw.on_stream_event(request, event).await;
        }
    }
}

impl Default for MiddlewarePipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// 日志中间件，按指定级别记录请求/响应/错误
pub struct LoggingMiddleware {
    level: LogLevel,
}

impl LoggingMiddleware {
    /// 创建指定日志级别的日志中间件
    #[must_use]
    pub fn new(level: LogLevel) -> Self {
        Self { level }
    }
}

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn on_request(&self, request: &mut LanguageModelRequest) -> Result<(), LlmError> {
        match self.level {
            LogLevel::Trace | LogLevel::Debug => {
                tracing::debug!(
                    model = %request.model,
                    messages = request.messages.len(),
                    stream = request.stream,
                    "LLM request"
                );
            }
            _ => {
                tracing::info!(
                    model = %request.model,
                    stream = request.stream,
                    "LLM request"
                );
            }
        }
        Ok(())
    }

    async fn on_response(&self, request: &LanguageModelRequest, response: &CompletionResponse) {
        tracing::info!(
            model = %request.model,
            stop_reason = %response.stop_reason,
            tokens = response.usage.total_tokens(),
            "LLM response"
        );
    }

    async fn on_error(&self, request: &LanguageModelRequest, error: &LlmError) {
        tracing::warn!(
            model = %request.model,
            error_class = error.safe_failure_class(),
            "LLM error"
        );
    }

    fn name(&self) -> &'static str {
        "logging"
    }
}

/// 指标采集中间件，记录请求延迟、Token 用量、错误率等
pub struct MetricsMiddleware {
    metrics: Arc<dyn MetricsCollector>,
    start_times: parking_lot::RwLock<std::collections::HashMap<String, std::time::Instant>>,
    next_id: parking_lot::Mutex<u64>,
}

impl MetricsMiddleware {
    /// 创建指标采集中间件
    #[must_use]
    pub fn new(metrics: Arc<dyn MetricsCollector>) -> Self {
        Self {
            metrics,
            start_times: parking_lot::RwLock::new(std::collections::HashMap::new()),
            next_id: parking_lot::Mutex::new(0),
        }
    }

    fn request_key(&self, request: &LanguageModelRequest) -> String {
        if let Some(id) = request
            .metadata
            .as_ref()
            .and_then(|m| m.request_id.as_deref())
        {
            return id.to_string();
        }
        let mut next = self.next_id.lock();
        let id = *next;
        *next += 1;
        format!("{}:{}", request.model.as_ref(), id)
    }
}

#[async_trait]
impl Middleware for MetricsMiddleware {
    async fn on_request(&self, request: &mut LanguageModelRequest) -> Result<(), LlmError> {
        let provider = request
            .metadata
            .as_ref()
            .and_then(|m| m.tags.get("provider"))
            .map_or("", std::string::String::as_str);
        self.metrics
            .record_request(provider, request.model.as_ref());
        self.start_times
            .write()
            .insert(self.request_key(request), std::time::Instant::now());
        Ok(())
    }

    async fn on_response(&self, request: &LanguageModelRequest, response: &CompletionResponse) {
        let provider = request
            .metadata
            .as_ref()
            .and_then(|m| m.tags.get("provider"))
            .map_or("", std::string::String::as_str);
        let latency_ms = self
            .start_times
            .write()
            .remove(&self.request_key(request))
            .map_or(0, |start| {
                let elapsed = start.elapsed();
                elapsed.as_secs() * 1000 + u64::from(elapsed.subsec_millis())
            });
        self.metrics.record_response(
            provider,
            request.model.as_ref(),
            latency_ms,
            &response.usage,
        );
    }

    async fn on_error(&self, request: &LanguageModelRequest, error: &LlmError) {
        let provider = request
            .metadata
            .as_ref()
            .and_then(|m| m.tags.get("provider"))
            .map_or("", std::string::String::as_str);
        self.metrics
            .record_error(provider, request.model.as_ref(), error.safe_failure_class());
    }

    async fn on_stream_event(&self, request: &LanguageModelRequest, event: &StreamEvent) {
        let provider = request
            .metadata
            .as_ref()
            .and_then(|m| m.tags.get("provider"))
            .map_or("", std::string::String::as_str);
        let event_type = match event {
            StreamEvent::ResponseMeta { .. } => "response_meta",
            StreamEvent::Queued { .. } => "queued",
            StreamEvent::Started => "started",
            StreamEvent::Text(_) => "text",
            StreamEvent::Thinking { .. } => "thinking",
            StreamEvent::RedactedThinking { .. } => "redacted_thinking",
            StreamEvent::ToolUse(_) => "tool_use",
            StreamEvent::ToolUseJsonParseError { .. } => "tool_use_json_parse_error",
            StreamEvent::UsageUpdate(_) => "usage_update",
            StreamEvent::Stop(_) => "stop",
            StreamEvent::Error(_) => "error",
        };
        self.metrics
            .record_stream_event(provider, request.model.as_ref(), event_type);
    }

    fn name(&self) -> &'static str {
        "metrics"
    }
}

/// 重试中间件，检测可重试错误并记录日志
pub struct RetryMiddleware {
    policy: RetryPolicy,
}

impl RetryMiddleware {
    /// 创建重试中间件
    #[must_use]
    pub fn new(policy: RetryPolicy) -> Self {
        Self { policy }
    }
}

#[async_trait]
impl Middleware for RetryMiddleware {
    async fn on_error(&self, _request: &LanguageModelRequest, error: &LlmError) {
        if error.is_retryable() {
            tracing::info!(
                max_attempts = self.policy.max_attempts,
                "Retryable error detected"
            );
        }
    }

    fn name(&self) -> &'static str {
        "retry"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::NoopMetricsCollector;

    #[test]
    fn test_middleware_pipeline_new() {
        let pipeline = MiddlewarePipeline::new();
        assert!(pipeline.middlewares.is_empty());
    }

    #[test]
    fn test_middleware_pipeline_default() {
        let pipeline = MiddlewarePipeline::default();
        assert!(pipeline.middlewares.is_empty());
    }

    #[tokio::test]
    async fn test_middleware_pipeline_on_request() {
        let mut pipeline = MiddlewarePipeline::new();
        pipeline.add(Arc::new(LoggingMiddleware::new(LogLevel::Info)));
        let mut request = LanguageModelRequest::new("gpt-4o", vec![]);
        let result = pipeline.on_request(&mut request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_logging_middleware() {
        let mw = LoggingMiddleware::new(LogLevel::Debug);
        assert_eq!(mw.name(), "logging");
    }

    #[tokio::test]
    async fn test_metrics_middleware() {
        let mw = MetricsMiddleware::new(Arc::new(NoopMetricsCollector));
        assert_eq!(mw.name(), "metrics");
    }

    #[test]
    fn test_retry_middleware() {
        let policy = RetryPolicy::default();
        let mw = RetryMiddleware::new(policy);
        assert_eq!(mw.name(), "retry");
    }

    #[tokio::test]
    async fn test_middleware_pipeline_on_response() {
        let mut pipeline = MiddlewarePipeline::new();
        pipeline.add(Arc::new(LoggingMiddleware::new(LogLevel::Info)));
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let response = CompletionResponse {
            id: "r1".into(),
            model: "gpt-4o".into(),
            content: crate::provider::content_block::MessageContent::text("hello"),
            stop_reason: crate::provider::types::StopReason::EndTurn,
            usage: crate::provider::types::TokenUsage::default(),
            tool_calls: vec![],
            thinking: None,
        };
        pipeline.on_response(&request, &response).await;
    }

    #[tokio::test]
    async fn test_middleware_pipeline_on_error() {
        let mut pipeline = MiddlewarePipeline::new();
        pipeline.add(Arc::new(LoggingMiddleware::new(LogLevel::Info)));
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let error = LlmError::Network("connection lost".into());
        pipeline.on_error(&request, &error).await;
    }

    #[tokio::test]
    async fn test_middleware_pipeline_on_stream_event() {
        let mut pipeline = MiddlewarePipeline::new();
        pipeline.add(Arc::new(LoggingMiddleware::new(LogLevel::Info)));
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let event = StreamEvent::Text("hello".into());
        pipeline.on_stream_event(&request, &event).await;
    }

    #[tokio::test]
    async fn test_middleware_pipeline_on_request_error_propagation() {
        use crate::middleware::Middleware;
        struct FailingMiddleware;
        #[async_trait]
        impl Middleware for FailingMiddleware {
            async fn on_request(
                &self,
                _request: &mut LanguageModelRequest,
            ) -> Result<(), LlmError> {
                Err(LlmError::InvalidRequest {
                    message: "blocked".into(),
                })
            }
            fn name(&self) -> &'static str {
                "failing"
            }
        }
        let mut pipeline = MiddlewarePipeline::new();
        pipeline.add(Arc::new(FailingMiddleware));
        let mut request = LanguageModelRequest::new("gpt-4o", vec![]);
        let result = pipeline.on_request(&mut request).await;
        assert!(matches!(result, Err(LlmError::InvalidRequest { .. })));
    }

    #[tokio::test]
    async fn test_logging_middleware_on_request_debug_level() {
        let mw = LoggingMiddleware::new(LogLevel::Debug);
        let mut request =
            LanguageModelRequest::new("gpt-4o", vec![crate::provider::types::Message::user("hi")]);
        let result = mw.on_request(&mut request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_logging_middleware_on_request_info_level() {
        let mw = LoggingMiddleware::new(LogLevel::Info);
        let mut request =
            LanguageModelRequest::new("gpt-4o", vec![crate::provider::types::Message::user("hi")]);
        let result = mw.on_request(&mut request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_logging_middleware_on_response() {
        let mw = LoggingMiddleware::new(LogLevel::Info);
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let response = CompletionResponse {
            id: "r1".into(),
            model: "gpt-4o".into(),
            content: crate::provider::content_block::MessageContent::text("hello"),
            stop_reason: crate::provider::types::StopReason::EndTurn,
            usage: crate::provider::types::TokenUsage::default(),
            tool_calls: vec![],
            thinking: None,
        };
        mw.on_response(&request, &response).await;
    }

    #[tokio::test]
    async fn test_logging_middleware_on_error() {
        let mw = LoggingMiddleware::new(LogLevel::Info);
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let error = LlmError::Network("timeout".into());
        mw.on_error(&request, &error).await;
    }

    #[tokio::test]
    async fn test_metrics_middleware_on_request() {
        let mw = MetricsMiddleware::new(Arc::new(NoopMetricsCollector));
        let mut request = LanguageModelRequest::new("gpt-4o", vec![]);
        let result = mw.on_request(&mut request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_metrics_middleware_on_response() {
        let mw = MetricsMiddleware::new(Arc::new(NoopMetricsCollector));
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let response = CompletionResponse {
            id: "r1".into(),
            model: "gpt-4o".into(),
            content: crate::provider::content_block::MessageContent::text("hello"),
            stop_reason: crate::provider::types::StopReason::EndTurn,
            usage: crate::provider::types::TokenUsage::default(),
            tool_calls: vec![],
            thinking: None,
        };
        mw.on_response(&request, &response).await;
    }

    #[tokio::test]
    async fn test_metrics_middleware_on_error() {
        let mw = MetricsMiddleware::new(Arc::new(NoopMetricsCollector));
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let error = LlmError::Network("timeout".into());
        mw.on_error(&request, &error).await;
    }

    #[tokio::test]
    async fn test_metrics_middleware_on_stream_event_text() {
        let mw = MetricsMiddleware::new(Arc::new(NoopMetricsCollector));
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let event = StreamEvent::Text("hello".into());
        mw.on_stream_event(&request, &event).await;
    }

    #[tokio::test]
    async fn test_metrics_middleware_on_stream_event_thinking() {
        let mw = MetricsMiddleware::new(Arc::new(NoopMetricsCollector));
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let event = StreamEvent::Thinking {
            text: "hmm".into(),
            signature: None,
        };
        mw.on_stream_event(&request, &event).await;
    }

    #[tokio::test]
    async fn test_metrics_middleware_on_stream_event_tool_use() {
        let mw = MetricsMiddleware::new(Arc::new(NoopMetricsCollector));
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let event = StreamEvent::ToolUse(crate::tool::ToolCall {
            id: "c1".into(),
            name: "tool".into(),
            arguments: "{}".into(),
        });
        mw.on_stream_event(&request, &event).await;
    }

    #[tokio::test]
    async fn test_metrics_middleware_on_stream_event_stop() {
        let mw = MetricsMiddleware::new(Arc::new(NoopMetricsCollector));
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let event = StreamEvent::Stop(crate::provider::types::StopReason::EndTurn);
        mw.on_stream_event(&request, &event).await;
    }

    #[tokio::test]
    async fn test_metrics_middleware_on_stream_event_error() {
        let mw = MetricsMiddleware::new(Arc::new(NoopMetricsCollector));
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let event = StreamEvent::Error("stream error".into());
        mw.on_stream_event(&request, &event).await;
    }

    #[tokio::test]
    async fn test_retry_middleware_on_error_retryable() {
        let policy = RetryPolicy::default();
        let mw = RetryMiddleware::new(policy);
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let error = LlmError::Network("timeout".into());
        mw.on_error(&request, &error).await;
    }

    #[tokio::test]
    async fn test_retry_middleware_on_error_non_retryable() {
        let policy = RetryPolicy::default();
        let mw = RetryMiddleware::new(policy);
        let request = LanguageModelRequest::new("gpt-4o", vec![]);
        let error = LlmError::AuthenticationError {
            message: "bad key".into(),
        };
        mw.on_error(&request, &error).await;
    }

    #[tokio::test]
    async fn test_middleware_pipeline_multiple_middlewares() {
        let mut pipeline = MiddlewarePipeline::new();
        pipeline.add(Arc::new(LoggingMiddleware::new(LogLevel::Info)));
        pipeline.add(Arc::new(MetricsMiddleware::new(Arc::new(
            NoopMetricsCollector,
        ))));
        let mut request = LanguageModelRequest::new("gpt-4o", vec![]);
        let result = pipeline.on_request(&mut request).await;
        assert!(result.is_ok());
    }
}
