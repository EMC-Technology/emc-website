use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;

use crate::error::LlmError;
use crate::provider::types::{
    LanguageModelRequest, ModelCapabilities, ModelId, ModelName, ProviderConfig, ProviderId,
    ProviderName,
};
use crate::provider::{LanguageModel, LanguageModelProvider};
use crate::rate_limit::RateLimiter;
use crate::stream::openai_mapper::OpenAiSseMapper;
use crate::stream::sse::SseParser;
use crate::stream::{ModelStream, StreamEvent, StreamFuture};
use crate::token_count::{ByteEstimator, TokenCounter};

const OLLAMA_DEFAULT_BASE: &str = "http://localhost:11434";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Deserialize)]
struct OllamaTagResponse {
    models: Vec<OllamaModelInfo>,
}

/// Ollama 模型信息（从 `/api/tags` 接口返回）。
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaModelInfo {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    model: String,
    #[allow(dead_code)]
    size: Option<u64>,
}

/// Ollama 模型实例。
pub struct OllamaModel {
    id: ModelId,
    name: ModelName,
    provider_id: ProviderId,
    provider_name: ProviderName,
    max_tokens: u64,
    max_output: Option<u64>,
    capabilities: ModelCapabilities,
    client: Client,
    api_base: String,
    rate_limiter: Arc<RateLimiter>,
}

impl OllamaModel {
    fn new(
        model_name: &str,
        provider_id: ProviderId,
        provider_name: ProviderName,
        client: Client,
        api_base: String,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        let max_tokens = 32_000u64;
        Self {
            id: ModelId::new(model_name),
            name: ModelName::new(model_name),
            provider_id,
            provider_name,
            max_tokens,
            max_output: Some(4096),
            capabilities: ModelCapabilities {
                tools: false,
                images: false,
                streaming_tools: false,
                parallel_tool_calls: false,
                thinking: false,
                max_tokens,
            },
            client,
            api_base,
            rate_limiter,
        }
    }

    fn build_request_body(request: &LanguageModelRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role.as_str(),
                    "content": m.content,
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "stream": true,
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(stop) = &request.stop {
            body["stop"] = serde_json::json!(stop);
        }

        body
    }
}

#[async_trait]
impl LanguageModel for OllamaModel {
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
        self.capabilities.tools
    }
    fn supports_streaming_tools(&self) -> bool {
        self.capabilities.streaming_tools
    }
    fn supports_images(&self) -> bool {
        self.capabilities.images
    }
    fn supports_thinking(&self) -> bool {
        self.capabilities.thinking
    }
    fn max_token_count(&self) -> u64 {
        self.max_tokens
    }
    fn max_output_tokens(&self) -> Option<u64> {
        self.max_output
    }

    fn count_tokens(
        &self,
        request: &LanguageModelRequest,
    ) -> crate::token_count::TokenCountFuture<'_> {
        let estimator = ByteEstimator;
        let estimate = estimator.count_tokens(
            &request
                .messages
                .iter()
                .map(|m| m.content.to_text_lossy())
                .collect::<Vec<_>>()
                .join(" "),
            request.model.as_ref(),
        );
        Box::pin(async move { Ok(estimate) })
    }

    fn stream_completion(&self, request: LanguageModelRequest) -> StreamFuture<'_> {
        Box::pin(async move {
            let _permit = self.rate_limiter.acquire_owned().await?;

            let body = Self::build_request_body(&request);
            let url = format!("{}/v1/chat/completions", self.api_base);

            let response = self
                .client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(LlmError::from)?;

            let status = response.status();
            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                return Err(LlmError::from_http_status(status, &body_text));
            }

            let byte_stream = response.bytes_stream();
            let mapper = OpenAiSseMapper;
            let parser = SseParser::new();

            let stream: ModelStream = Box::pin(
                byte_stream
                    .scan(parser, move |parser, chunk_result| {
                        let events = match chunk_result {
                            Ok(bytes) => parser.push(&bytes, &mapper),
                            Err(e) => Err(LlmError::StreamError(e.to_string())),
                        };
                        let events =
                            events.unwrap_or_else(|e| vec![StreamEvent::Error(e.to_string())]);
                        std::future::ready(Some(events))
                    })
                    .flat_map(|events| futures_util::stream::iter(events.into_iter().map(Ok))),
            );

            Ok(stream)
        })
    }
}

/// Ollama 供应商，管理本地模型列表与认证。
pub struct OllamaProvider {
    client: Client,
    api_base: String,
    rate_limiter: Arc<RateLimiter>,
    config: ProviderConfig,
}

impl OllamaProvider {
    /// 创建默认 Ollama 供应商实例（使用 `http://localhost:11434`）。
    #[must_use]
    pub fn new() -> Self {
        Self::with_api_base(OLLAMA_DEFAULT_BASE)
    }

    /// 使用自定义 API 基地址创建 Ollama Provider 实例。
    ///
    /// # Panics
    ///
    /// 当 HTTP 客户端构建失败时 panic。
    pub fn with_api_base(api_base: impl Into<String>) -> Self {
        let api_base = api_base.into();
        let client =
            crate::build_http_client(DEFAULT_TIMEOUT).expect("HTTP client init should not fail");

        let rate_limiter = Arc::new(RateLimiter::new(5));

        let config = ProviderConfig {
            id: ProviderId::new("ollama"),
            name: ProviderName::new("Ollama"),
            api_url: api_base.clone(),
            api_key_env: None,
            models: Vec::new(),
            rate_limit: None,
            retry: None,
            extra_headers: HashMap::new(),
            max_request_body_bytes: None,
        };

        Self {
            client,
            api_base,
            rate_limiter,
            config,
        }
    }

    /// 从 Ollama API 检测可用模型列表。
    ///
    /// # Errors
    ///
    /// 当网络请求失败或 API 返回非成功状态码时返回 `LlmError`。
    pub async fn detect_models(&self) -> Result<Vec<OllamaModelInfo>, LlmError> {
        let url = format!("{}/api/tags", self.api_base);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(LlmError::from_http_status(status, &body_text));
        }

        let tag_response: OllamaTagResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        Ok(tag_response.models)
    }

    /// 根据模型名称创建 `OllamaModel` 实例。
    #[must_use]
    pub fn create_model(&self, model_name: &str) -> Arc<dyn LanguageModel> {
        Arc::new(OllamaModel::new(
            model_name,
            self.config.id.clone(),
            self.config.name.clone(),
            self.client.clone(),
            self.api_base.clone(),
            self.rate_limiter.clone(),
        ))
    }
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LanguageModelProvider for OllamaProvider {
    fn id(&self) -> &ProviderId {
        &self.config.id
    }
    fn name(&self) -> &ProviderName {
        &self.config.name
    }

    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
        Vec::new()
    }

    fn is_authenticated(&self) -> bool {
        true
    }

    async fn authenticate(&self) -> Result<(), LlmError> {
        Ok(())
    }

    async fn reset_credentials(&self) -> Result<(), LlmError> {
        Ok(())
    }

    fn configuration(&self) -> &ProviderConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LanguageModelProvider;
    use crate::provider::types::Message;

    #[test]
    fn test_ollama_provider_new() {
        let provider = OllamaProvider::new();
        assert_eq!(provider.id().as_ref(), "ollama");
        assert_eq!(provider.name().as_ref(), "Ollama");
    }

    #[test]
    fn test_ollama_provider_default() {
        let provider = OllamaProvider::default();
        assert_eq!(provider.id().as_ref(), "ollama");
    }

    #[test]
    fn test_ollama_provider_with_api_base() {
        let provider = OllamaProvider::with_api_base("http://custom:11434");
        assert_eq!(provider.configuration().api_url, "http://custom:11434");
    }

    #[test]
    fn test_ollama_provider_configuration() {
        let provider = OllamaProvider::new();
        let config = provider.configuration();
        assert_eq!(config.api_url, "http://localhost:11434");
        assert!(config.api_key_env.is_none());
    }

    #[test]
    fn test_ollama_provider_is_authenticated() {
        let provider = OllamaProvider::new();
        assert!(provider.is_authenticated());
    }

    #[test]
    fn test_ollama_provider_provided_models_empty() {
        let provider = OllamaProvider::new();
        assert!(provider.provided_models().is_empty());
    }

    #[test]
    fn test_ollama_create_model() {
        let provider = OllamaProvider::new();
        let model = provider.create_model("llama3");
        assert_eq!(model.id().as_str(), "llama3");
        assert!(!model.supports_tools());
        assert!(!model.supports_images());
        assert!(!model.supports_thinking());
        assert_eq!(model.max_token_count(), 32_000);
        assert_eq!(model.max_output_tokens(), Some(4096));
    }

    #[tokio::test]
    async fn test_ollama_model_count_tokens() {
        let provider = OllamaProvider::new();
        let model = provider.create_model("llama3");
        let request = LanguageModelRequest::new("llama3", vec![Message::user("hello")]);
        let count = model.count_tokens(&request).await.unwrap();
        assert!(count > 0);
    }

    #[tokio::test]
    async fn test_ollama_authenticate() {
        let provider = OllamaProvider::new();
        let result = provider.authenticate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ollama_reset_credentials() {
        let provider = OllamaProvider::new();
        let result = provider.reset_credentials().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_ollama_model_info_deserialize() {
        let json = r#"{"name":"llama3","model":"llama3:latest","size":4661224676}"#;
        let info: OllamaModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.name, "llama3");
        assert_eq!(info.size, Some(4_661_224_676));
    }
}
