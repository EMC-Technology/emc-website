use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;

use crate::cancel::CancellationToken;
use crate::error::LlmError;
use crate::middleware::MiddlewarePipeline;
use crate::process_bridge::{LlmEventLog, LlmRequestEvent};
use crate::provider::LanguageModel;
use crate::provider::types::LanguageModelRequest;
use crate::rate_limit::RateLimiter;
use crate::registry::ModelRegistry;
use crate::response::CompletionResponse;
use crate::retry::RetryPolicy;
use crate::stream::{ModelStream, StreamEvent};

/// 统一 LLM API 管理入口 — 应用层 (Application Layer)
///
/// 整合 `ModelRegistry` + `MiddlewarePipeline` + `RetryPolicy` + `RateLimiter` + `CancellationToken`
/// 为单一入口点，实现 API.md §1.1 分层架构中的应用层。
///
/// # Example
///
/// ```ignore
/// use ullm::{LlmApi, ModelRegistry, RetryPolicy, RateLimiter};
/// use std::time::Duration;
///
/// let mut registry = ModelRegistry::new();
/// // registry.register(provider);
///
/// let api = LlmApi::new(registry)
///     .with_retry_policy(RetryPolicy::new(3, Duration::from_secs(1)).unwrap())
///     .with_default_timeout(Duration::from_secs(60));
///
/// let response = api.complete("gpt-4o", request).await?;
/// ```
///
/// # Errors
///
/// 所有方法返回 `Result<_, LlmError>`，错误来源包括：
/// - 模型未找到 (`LlmError::ModelUnavailable`)
/// - 中间件拦截 (`Middleware::on_request` 返回错误)
/// - 限流器获取失败 (`LlmError::RateLimitExceeded`)
/// - Provider I/O 错误 (`LlmError::Network`/`LlmError::Timeout`/...)
pub struct LlmApi {
    registry: ModelRegistry,
    middleware: MiddlewarePipeline,
    retry_policy: RetryPolicy,
    rate_limiter: Option<RateLimiter>,
    default_timeout: Option<Duration>,
    default_cancel_token: Option<CancellationToken>,
    event_log: std::sync::Mutex<LlmEventLog>,
}

impl LlmApi {
    /// 创建 `LlmApi` 实例
    ///
    /// # Example
    ///
    /// ```ignore
    /// let api = LlmApi::new(registry);
    /// ```
    #[must_use]
    pub fn new(registry: ModelRegistry) -> Self {
        Self {
            registry,
            middleware: MiddlewarePipeline::new(),
            retry_policy: RetryPolicy::default(),
            rate_limiter: None,
            default_timeout: None,
            default_cancel_token: None,
            event_log: std::sync::Mutex::new(LlmEventLog::new()),
        }
    }

    /// 设置中间件管道（Builder 模式）
    #[must_use]
    pub fn with_middleware(mut self, pipeline: MiddlewarePipeline) -> Self {
        self.middleware = pipeline;
        self
    }

    /// 设置重试策略（Builder 模式）
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// 设置并发限流器（Builder 模式）
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: RateLimiter) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// 设置全局默认超时（Builder 模式）
    ///
    /// 当请求未显式设置 `timeout` 时，使用此默认值。
    #[must_use]
    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = Some(timeout);
        self
    }

    /// 设置全局默认取消令牌（Builder 模式）
    ///
    /// 当请求未显式设置 `cancel` 时，使用此默认令牌。
    /// 取消令牌可用于优雅终止正在进行的流式传输。
    #[must_use]
    pub fn with_default_cancel_token(mut self, token: CancellationToken) -> Self {
        self.default_cancel_token = Some(token);
        self
    }

    /// 获取模型注册表引用
    #[must_use]
    pub fn registry(&self) -> &ModelRegistry {
        &self.registry
    }

    /// 获取模型注册表可变引用
    #[must_use]
    pub fn registry_mut(&mut self) -> &mut ModelRegistry {
        &mut self.registry
    }

    /// 获取中间件管道引用
    #[must_use]
    pub fn middleware(&self) -> &MiddlewarePipeline {
        &self.middleware
    }

    /// 获取重试策略引用
    #[must_use]
    pub fn retry_policy(&self) -> &RetryPolicy {
        &self.retry_policy
    }

    /// 获取限流器引用
    #[must_use]
    pub fn rate_limiter(&self) -> Option<&RateLimiter> {
        self.rate_limiter.as_ref()
    }

    /// 获取事件日志的快照
    ///
    /// # Errors
    ///
    /// 若内部事件日志互斥锁被毒化（持有锁的线程 panic），返回错误。
    pub fn event_log(&self) -> Result<std::sync::MutexGuard<'_, LlmEventLog>, LlmError> {
        self.event_log
            .lock()
            .map_err(|e| LlmError::Other(format!("事件日志互斥锁中毒: {e}")))
    }

    /// 按模型 ID 查找模型
    ///
    /// 委托给 `ModelRegistry::find_model`，支持别名解析。
    #[must_use]
    pub fn find_model(&self, model_id: &str) -> Option<Arc<dyn LanguageModel>> {
        self.registry.find_model(model_id)
    }

    /// Token 计数便捷方法
    ///
    /// 委托给 `LanguageModel::count_tokens`。
    ///
    /// # Errors
    ///
    /// 当模型未找到时返回 `LlmError::ModelUnavailable`。
    pub async fn count_tokens(
        &self,
        model_id: &str,
        request: &LanguageModelRequest,
    ) -> Result<u64, LlmError> {
        let model = self.resolve_model(model_id)?;
        model.count_tokens(request).await
    }

    /// 认证所有已注册的 Provider
    ///
    /// 对注册表中每个 Provider 调用 `authenticate()`，收集所有错误。
    /// 任一 Provider 认证失败不影响其他 Provider。
    pub async fn authenticate_all(&self) -> Vec<(String, LlmError)> {
        let mut errors = Vec::new();
        for provider in self.registry.providers() {
            if !provider.is_authenticated()
                && let Err(e) = provider.authenticate().await
            {
                errors.push((provider.id().to_string(), e));
            }
        }
        errors
    }

    /// 非流式补全 — 自动重试 + 中间件 + 限流 + 取消检测
    ///
    /// 底层调用 `LanguageModel::complete()`，在其上叠加：
    /// 1. 默认值注入（timeout / `cancel_token`）
    /// 2. 中间件 `on_request` 拦截
    /// 3. 限流器许可获取
    /// 4. 取消检测
    /// 5. 重试策略执行
    /// 6. 中间件 `on_response` / `on_error` 拦截
    ///
    /// # Errors
    ///
    /// - `LlmError::ModelUnavailable` — 模型未在注册表中找到
    /// - `LlmError::Cancelled` — 请求在执行前已被取消
    /// - 其他错误由 Provider I/O 或中间件产生
    ///
    /// # Panics
    ///
    /// 当内部事件日志互斥锁被毒化（另一线程在持锁期间 panic）时 panic。
    pub async fn complete(
        &self,
        model_id: &str,
        request: LanguageModelRequest,
    ) -> Result<CompletionResponse, LlmError> {
        let model = self.resolve_model(model_id);

        if let Err(ref err) = model {
            self.event_log
                .lock()
                .unwrap()
                .append(LlmRequestEvent::Failed {
                    request_id: String::new(),
                    error_code: err.classify_4d().4.to_string(),
                });
            return model.map(|_| unreachable!());
        }

        let model = model.unwrap();
        let mut req = self.apply_defaults(request);
        let rid = model_id.to_string();

        self.event_log
            .lock()
            .unwrap()
            .append(LlmRequestEvent::Created {
                triggered_by: crate::process_bridge::LlmTriggeredBy::UserRequest(rid.clone()),
                request_id: rid.clone(),
                provider: crate::ProviderKind::OpenAi,
                model: model_id.to_string(),
            });

        if let Some(ref token) = req.cancel
            && token.is_cancelled()
        {
            self.event_log
                .lock()
                .unwrap()
                .append(LlmRequestEvent::Cancelled {
                    request_id: rid.clone(),
                });
            return Err(LlmError::Cancelled);
        }

        self.middleware.on_request(&mut req).await?;

        let _permit = if let Some(ref limiter) = self.rate_limiter {
            Some(limiter.acquire_owned().await?)
        } else {
            None
        };

        let result = crate::retry::run_with_retry(&self.retry_policy, || {
            let model = model.clone();
            let req = req.clone();
            async move { model.complete(req).await }
        })
        .await;

        match result {
            Ok(response) => {
                self.event_log
                    .lock()
                    .unwrap()
                    .append(LlmRequestEvent::Completed {
                        request_id: rid,
                        total_tokens: u32::try_from(response.usage.total_tokens())
                            .unwrap_or(u32::MAX),
                    });
                self.middleware.on_response(&req, &response).await;
                Ok(response)
            }
            Err(err) => {
                self.event_log
                    .lock()
                    .unwrap()
                    .append(LlmRequestEvent::Failed {
                        request_id: rid,
                        error_code: err.classify_4d().4.to_string(),
                    });
                self.middleware.on_error(&req, &err).await;
                Err(err)
            }
        }
    }

    /// 纯文本流 — 仅产出文本增量，自动限流 + 中间件事件拦截
    ///
    /// 底层调用 `LanguageModel::stream_completion()`，过滤非文本事件，
    /// 同时对每个流事件调用中间件 `on_stream_event`。
    ///
    /// # Errors
    ///
    /// - `LlmError::ModelUnavailable` — 模型未在注册表中找到
    /// - `LlmError::Cancelled` — 请求在执行前已被取消
    /// - 其他错误由 Provider I/O 或中间件产生
    pub async fn stream_text(
        &self,
        model_id: &str,
        request: LanguageModelRequest,
    ) -> Result<Pin<Box<dyn futures_util::Stream<Item = Result<String, LlmError>> + Send>>, LlmError>
    {
        let model = self.resolve_model(model_id)?;
        let mut req = self.apply_defaults(request);

        if let Some(ref token) = req.cancel
            && token.is_cancelled()
        {
            return Err(LlmError::Cancelled);
        }

        self.middleware.on_request(&mut req).await?;

        let raw_stream = model.stream_completion(req.clone()).await?;

        let rate_limited_stream: ModelStream = if let Some(ref limiter) = self.rate_limiter {
            limiter.wrap_stream(raw_stream).await?.boxed()
        } else {
            raw_stream
        };

        let middleware = self.middleware.clone();
        let req_for_mw = req.clone();
        let text_stream = rate_limited_stream.filter_map(move |event| {
            let mw = middleware.clone();
            let req = req_for_mw.clone();
            async move {
                match event {
                    Ok(ref e) => {
                        mw.on_stream_event(&req, e).await;
                    }
                    Err(ref _err) => {}
                }
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
            }
        });

        Ok(Box::pin(text_stream))
    }

    /// 完整事件流 — 含中间件事件拦截 + 限流守卫
    ///
    /// 返回原始 `ModelStream`，对每个事件调用中间件 `on_stream_event`。
    /// 限流器许可通过 `RateLimitGuard` 持有，直到流消费完毕才释放。
    ///
    /// # Errors
    ///
    /// - `LlmError::ModelUnavailable` — 模型未在注册表中找到
    /// - `LlmError::Cancelled` — 请求在执行前已被取消
    /// - 其他错误由 Provider I/O 或中间件产生
    pub async fn stream_full(
        &self,
        model_id: &str,
        request: LanguageModelRequest,
    ) -> Result<ModelStream, LlmError> {
        let model = self.resolve_model(model_id)?;
        let mut req = self.apply_defaults(request);

        if let Some(ref token) = req.cancel
            && token.is_cancelled()
        {
            return Err(LlmError::Cancelled);
        }

        self.middleware.on_request(&mut req).await?;

        let raw_stream = model.stream_completion(req.clone()).await?;

        let rate_limited_stream: ModelStream = if let Some(ref limiter) = self.rate_limiter {
            limiter.wrap_stream(raw_stream).await?.boxed()
        } else {
            raw_stream
        };

        let middleware = self.middleware.clone();
        let req_for_mw = req.clone();
        let instrumented = rate_limited_stream.then(move |event| {
            let mw = middleware.clone();
            let req = req_for_mw.clone();
            async move {
                if let Ok(ref e) = event {
                    mw.on_stream_event(&req, e).await;
                }
                event
            }
        });

        Ok(Box::pin(instrumented))
    }

    fn resolve_model(&self, model_id: &str) -> Result<Arc<dyn LanguageModel>, LlmError> {
        self.registry.find_model(model_id).ok_or_else(|| {
            LlmError::ModelUnavailable(format!("model '{model_id}' not found in registry"))
        })
    }

    fn apply_defaults(&self, request: LanguageModelRequest) -> LanguageModelRequest {
        let mut req = request;
        if req.timeout.is_none() {
            req.timeout = self.default_timeout;
        }
        if req.cancel.is_none() {
            req.cancel.clone_from(&self.default_cancel_token);
        }
        req
    }
}

impl Default for LlmApi {
    fn default() -> Self {
        Self::new(ModelRegistry::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LanguageModelProvider;
    use crate::provider::types::{
        Message, ModelId, ModelName, ProviderConfig, ProviderId, ProviderName,
    };
    use crate::stream::{StreamEvent, StreamFuture};
    use crate::token_count::TokenCountFuture;

    struct MockApiModel {
        id: ModelId,
        name: ModelName,
        provider_id: ProviderId,
        provider_name: ProviderName,
    }

    #[async_trait::async_trait]
    impl LanguageModel for MockApiModel {
        fn id(&self) -> &ModelId {
            &self.id
        }
        fn name(&self) -> &ModelName {
            &self.name
        }
        fn provider_id(&self) -> &ProviderId {
            &self.provider_id
        }
        fn provider_name(&self) -> &ProviderName {
            &self.provider_name
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
                    Ok(StreamEvent::Text("Hello from API".into())),
                    Ok(StreamEvent::Stop(
                        crate::provider::types::StopReason::EndTurn,
                    )),
                ]));
                Ok(stream)
            })
        }
    }

    struct MockApiProvider {
        id: ProviderId,
        name: ProviderName,
        models: Vec<Arc<dyn LanguageModel>>,
        authenticated: std::sync::Mutex<bool>,
    }

    #[async_trait::async_trait]
    impl LanguageModelProvider for MockApiProvider {
        fn id(&self) -> &ProviderId {
            &self.id
        }
        fn name(&self) -> &ProviderName {
            &self.name
        }
        fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
            self.models.clone()
        }
        fn is_authenticated(&self) -> bool {
            self.authenticated.lock().is_ok_and(|guard| *guard)
        }
        async fn authenticate(&self) -> Result<(), LlmError> {
            let mut guard = self
                .authenticated
                .lock()
                .map_err(|e| LlmError::Other(format!("认证状态互斥锁中毒: {e}")))?;
            *guard = true;
            Ok(())
        }
        async fn reset_credentials(&self) -> Result<(), LlmError> {
            Ok(())
        }
        fn configuration(&self) -> &ProviderConfig {
            unimplemented!()
        }
    }

    fn make_api_registry() -> ModelRegistry {
        let mut registry = ModelRegistry::new();
        let pid = ProviderId::new("test-provider");
        let pname = ProviderName::new("Test Provider");
        let model: Arc<dyn LanguageModel> = Arc::new(MockApiModel {
            id: ModelId::new("test-model"),
            name: ModelName::new("Test Model"),
            provider_id: pid.clone(),
            provider_name: pname.clone(),
        });
        let provider = Arc::new(MockApiProvider {
            id: pid,
            name: pname,
            models: vec![model],
            authenticated: std::sync::Mutex::new(true),
        });
        registry.register(provider);
        registry
    }

    #[test]
    fn test_llm_api_new() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        assert!(api.rate_limiter.is_none());
        assert!(api.default_timeout.is_none());
        assert!(api.default_cancel_token.is_none());
    }

    #[test]
    fn test_llm_api_default() {
        let api = LlmApi::default();
        assert!(api.registry.find_model("anything").is_none());
    }

    #[test]
    fn test_llm_api_with_retry_policy() {
        let registry = make_api_registry();
        let policy = RetryPolicy::new(5, Duration::from_secs(2)).unwrap();
        let api = LlmApi::new(registry).with_retry_policy(policy);
        assert_eq!(api.retry_policy().max_attempts, 5);
    }

    #[test]
    fn test_llm_api_with_default_timeout() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry).with_default_timeout(Duration::from_secs(60));
        assert_eq!(api.default_timeout, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_llm_api_with_default_cancel_token() {
        let registry = make_api_registry();
        let token = CancellationToken::new();
        let api = LlmApi::new(registry).with_default_cancel_token(token);
        assert!(api.default_cancel_token.is_some());
        assert!(!api.default_cancel_token.as_ref().unwrap().is_cancelled());
    }

    #[test]
    fn test_llm_api_with_rate_limiter() {
        let registry = make_api_registry();
        let limiter = RateLimiter::new(10);
        let api = LlmApi::new(registry).with_rate_limiter(limiter);
        assert!(api.rate_limiter().is_some());
    }

    #[test]
    fn test_llm_api_find_model() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        assert!(api.find_model("test-model").is_some());
        assert!(api.find_model("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_llm_api_complete() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]);
        let response = api.complete("test-model", request).await.unwrap();
        assert_eq!(response.text(), Some("Hello from API"));
    }

    #[tokio::test]
    async fn test_llm_api_stream_text() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]).stream();
        let stream = api.stream_text("test-model", request).await.unwrap();
        let texts: Vec<String> = stream.filter_map(|r| async move { r.ok() }).collect().await;
        assert_eq!(texts, vec!["Hello from API"]);
    }

    #[tokio::test]
    async fn test_llm_api_stream_full() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]).stream();
        let stream = api.stream_full("test-model", request).await.unwrap();
        let events: Vec<_> = stream.collect().await;
        assert!(!events.is_empty());
    }

    #[tokio::test]
    async fn test_llm_api_model_not_found() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let request = LanguageModelRequest::new("nonexistent", vec![Message::user("Hi")]);
        let result = api.complete("nonexistent", request).await;
        assert!(matches!(result, Err(LlmError::ModelUnavailable(_))));
    }

    #[tokio::test]
    async fn test_llm_api_cancelled_before_start() {
        let registry = make_api_registry();
        let token = CancellationToken::new();
        token.cancel();
        let api = LlmApi::new(registry).with_default_cancel_token(token);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]);
        let result = api.complete("test-model", request).await;
        assert!(matches!(result, Err(LlmError::Cancelled)));
    }

    #[tokio::test]
    async fn test_llm_api_request_level_cancel_token() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let token = CancellationToken::new();
        token.cancel();
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")])
            .with_cancel_token(token);
        let result = api.complete("test-model", request).await;
        assert!(matches!(result, Err(LlmError::Cancelled)));
    }

    #[tokio::test]
    async fn test_llm_api_default_timeout_applied() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry).with_default_timeout(Duration::from_secs(30));
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]);
        let response = api.complete("test-model", request).await.unwrap();
        assert_eq!(response.text(), Some("Hello from API"));
    }

    #[tokio::test]
    async fn test_llm_api_default_cancel_token_applied() {
        let registry = make_api_registry();
        let token = CancellationToken::new();
        let api = LlmApi::new(registry).with_default_cancel_token(token);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]);
        let response = api.complete("test-model", request).await.unwrap();
        assert_eq!(response.text(), Some("Hello from API"));
    }

    #[tokio::test]
    async fn test_llm_api_with_rate_limiter_complete() {
        let registry = make_api_registry();
        let limiter = RateLimiter::new(5);
        let api = LlmApi::new(registry).with_rate_limiter(limiter);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]);
        let response = api.complete("test-model", request).await.unwrap();
        assert_eq!(response.text(), Some("Hello from API"));
    }

    #[tokio::test]
    async fn test_llm_api_with_middleware() {
        let registry = make_api_registry();
        let mut pipeline = MiddlewarePipeline::new();
        pipeline.add(Arc::new(crate::middleware::LoggingMiddleware::new(
            crate::observability::LogLevel::Info,
        )));
        let api = LlmApi::new(registry).with_middleware(pipeline);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]);
        let response = api.complete("test-model", request).await.unwrap();
        assert_eq!(response.text(), Some("Hello from API"));
    }

    #[tokio::test]
    async fn test_llm_api_authenticate_all() {
        let mut registry = ModelRegistry::new();
        let pid = ProviderId::new("auth-provider");
        let pname = ProviderName::new("Auth Provider");
        let model: Arc<dyn LanguageModel> = Arc::new(MockApiModel {
            id: ModelId::new("auth-model"),
            name: ModelName::new("Auth Model"),
            provider_id: pid.clone(),
            provider_name: pname.clone(),
        });
        let provider = Arc::new(MockApiProvider {
            id: pid,
            name: pname,
            models: vec![model],
            authenticated: std::sync::Mutex::new(false),
        });
        registry.register(provider);
        let api = LlmApi::new(registry);
        let errors = api.authenticate_all().await;
        assert!(errors.is_empty());
    }

    #[tokio::test]
    async fn test_llm_api_count_tokens() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]);
        let count = api.count_tokens("test-model", &request).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_llm_api_stream_text_with_rate_limiter() {
        let registry = make_api_registry();
        let limiter = RateLimiter::new(5);
        let api = LlmApi::new(registry).with_rate_limiter(limiter);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]).stream();
        let stream = api.stream_text("test-model", request).await.unwrap();
        let texts: Vec<String> = stream.filter_map(|r| async move { r.ok() }).collect().await;
        assert_eq!(texts, vec!["Hello from API"]);
    }

    #[tokio::test]
    async fn test_llm_api_stream_full_with_rate_limiter() {
        let registry = make_api_registry();
        let limiter = RateLimiter::new(5);
        let api = LlmApi::new(registry).with_rate_limiter(limiter);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]).stream();
        let stream = api.stream_full("test-model", request).await.unwrap();
        let events: Vec<_> = stream.collect().await;
        assert!(!events.is_empty());
    }

    #[tokio::test]
    async fn test_llm_api_stream_text_cancelled() {
        let registry = make_api_registry();
        let token = CancellationToken::new();
        token.cancel();
        let api = LlmApi::new(registry).with_default_cancel_token(token);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]).stream();
        let result = api.stream_text("test-model", request).await;
        assert!(matches!(result, Err(LlmError::Cancelled)));
    }

    #[tokio::test]
    async fn test_llm_api_stream_full_cancelled() {
        let registry = make_api_registry();
        let token = CancellationToken::new();
        token.cancel();
        let api = LlmApi::new(registry).with_default_cancel_token(token);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]).stream();
        let result = api.stream_full("test-model", request).await;
        assert!(matches!(result, Err(LlmError::Cancelled)));
    }

    #[tokio::test]
    async fn test_llm_api_stream_text_model_not_found() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let request = LanguageModelRequest::new("nonexistent", vec![Message::user("Hi")]).stream();
        let result = api.stream_text("nonexistent", request).await;
        assert!(matches!(result, Err(LlmError::ModelUnavailable(_))));
    }

    #[tokio::test]
    async fn test_llm_api_stream_full_model_not_found() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let request = LanguageModelRequest::new("nonexistent", vec![Message::user("Hi")]).stream();
        let result = api.stream_full("nonexistent", request).await;
        assert!(matches!(result, Err(LlmError::ModelUnavailable(_))));
    }

    #[tokio::test]
    async fn test_llm_api_count_tokens_model_not_found() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let request = LanguageModelRequest::new("nonexistent", vec![Message::user("Hi")]);
        let result = api.count_tokens("nonexistent", &request).await;
        assert!(matches!(result, Err(LlmError::ModelUnavailable(_))));
    }

    #[test]
    fn test_llm_api_registry_accessors() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        assert!(api.registry().find_model("test-model").is_some());
        let mut api_mut = LlmApi::new(ModelRegistry::new());
        api_mut.registry_mut().register(Arc::new(MockApiProvider {
            id: ProviderId::new("new"),
            name: ProviderName::new("New"),
            models: vec![],
            authenticated: std::sync::Mutex::new(true),
        }));
    }

    #[test]
    fn test_llm_api_middleware_accessor() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let _mw = api.middleware();
    }

    #[tokio::test]
    async fn test_llm_api_authenticate_all_already_authenticated() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let errors = api.authenticate_all().await;
        assert!(errors.is_empty());
    }

    #[tokio::test]
    async fn test_llm_api_complete_with_middleware_error() {
        use crate::middleware::Middleware;
        struct BlockingMiddleware;
        #[async_trait::async_trait]
        impl Middleware for BlockingMiddleware {
            async fn on_request(
                &self,
                _request: &mut LanguageModelRequest,
            ) -> Result<(), LlmError> {
                Err(LlmError::InvalidRequest {
                    message: "blocked".into(),
                })
            }
            fn name(&self) -> &'static str {
                "blocking"
            }
        }
        let registry = make_api_registry();
        let mut pipeline = MiddlewarePipeline::new();
        pipeline.add(Arc::new(BlockingMiddleware));
        let api = LlmApi::new(registry).with_middleware(pipeline);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]);
        let result = api.complete("test-model", request).await;
        assert!(matches!(result, Err(LlmError::InvalidRequest { .. })));
    }

    #[tokio::test]
    async fn test_llm_api_complete_emits_events() {
        let registry = make_api_registry();
        let api = LlmApi::new(registry);
        let request = LanguageModelRequest::new("test-model", vec![Message::user("Hi")]);
        let _response = api.complete("test-model", request).await.unwrap();
        let log = api.event_log().unwrap();
        let events = log.events();
        assert!(!events.is_empty());
        assert!(matches!(
            events[0],
            crate::process_bridge::LlmRequestEvent::Created { .. }
        ));
    }

    #[tokio::test]
    async fn test_llm_api_complete_failure_emits_failed_event() {
        let registry = ModelRegistry::new();
        let api = LlmApi::new(registry);
        let request = LanguageModelRequest::new("nonexistent", vec![Message::user("Hi")]);
        let _ = api.complete("nonexistent", request).await;
        let log = api.event_log().unwrap();
        let events = log.events();
        assert!(!events.is_empty());
    }
}
