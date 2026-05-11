use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;

use crate::credential::{CredentialsProvider, EnvCredentialProvider};
use crate::error::LlmError;
use crate::provider::types::{
    LanguageModelRequest, Message, ModelCapabilities, ModelConfig, ModelId, ModelName,
    ProviderConfig, ProviderId, ProviderName, Role,
};
use crate::provider::{LanguageModel, LanguageModelProvider};
use crate::rate_limit::RateLimiter;
use crate::stream::anthropic_mapper::AnthropicSseMapper;
use crate::stream::sse::SseParser;
use crate::stream::{ModelStream, StreamEvent, StreamFuture};
use crate::token_count::{ByteEstimator, TokenCounter};

const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

struct AnthropicModelSpec {
    id: &'static str,
    display_name: &'static str,
    max_tokens: u64,
    max_output_tokens: Option<u64>,
    capabilities: ModelCapabilities,
}

fn anthropic_models() -> Vec<AnthropicModelSpec> {
    vec![
        AnthropicModelSpec {
            id: "claude-opus-4-6",
            display_name: "Claude Opus 4",
            max_tokens: 200_000,
            max_output_tokens: Some(32_000),
            capabilities: ModelCapabilities {
                tools: true,
                images: true,
                streaming_tools: true,
                parallel_tool_calls: true,
                thinking: true,
                max_tokens: 200_000,
            },
        },
        AnthropicModelSpec {
            id: "claude-sonnet-4-5",
            display_name: "Claude Sonnet 4.5",
            max_tokens: 200_000,
            max_output_tokens: Some(16_384),
            capabilities: ModelCapabilities {
                tools: true,
                images: true,
                streaming_tools: true,
                parallel_tool_calls: true,
                thinking: true,
                max_tokens: 200_000,
            },
        },
        AnthropicModelSpec {
            id: "claude-haiku-3-5",
            display_name: "Claude Haiku 3.5",
            max_tokens: 200_000,
            max_output_tokens: Some(8192),
            capabilities: ModelCapabilities {
                tools: true,
                images: true,
                streaming_tools: true,
                parallel_tool_calls: true,
                thinking: false,
                max_tokens: 200_000,
            },
        },
    ]
}

/// Anthropic Claude 模型实例。
pub struct AnthropicModel {
    id: ModelId,
    name: ModelName,
    provider_id: ProviderId,
    provider_name: ProviderName,
    spec: AnthropicModelSpec,
    client: Client,
    credentials: Arc<dyn CredentialsProvider>,
    rate_limiter: Arc<RateLimiter>,
}

impl AnthropicModel {
    fn new(
        spec: AnthropicModelSpec,
        provider_id: ProviderId,
        provider_name: ProviderName,
        client: Client,
        credentials: Arc<dyn CredentialsProvider>,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        Self {
            id: ModelId::new(spec.id),
            name: ModelName::new(spec.display_name),
            provider_id,
            provider_name,
            spec,
            client,
            credentials,
            rate_limiter,
        }
    }

    fn extract_system_prompt(messages: &[Message]) -> Option<String> {
        messages
            .iter()
            .find(|m| m.role == Role::System)
            .map(|m| m.content.to_text_lossy())
    }

    fn build_request_body(&self, request: &LanguageModelRequest) -> serde_json::Value {
        let system = Self::extract_system_prompt(&request.messages);
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| {
                let mut obj = serde_json::json!({
                    "role": m.role.as_str(),
                    "content": m.content,
                });
                if let Some(name) = &m.name {
                    obj["name"] = serde_json::Value::String(name.clone());
                }
                if let Some(tool_call_id) = &m.tool_call_id {
                    obj["tool_call_id"] = serde_json::Value::String(tool_call_id.clone());
                }
                obj
            })
            .collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "stream": true,
        });

        if let Some(sys) = system {
            body["system"] = serde_json::json!(sys);
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(stop) = &request.stop {
            body["stop_sequences"] = serde_json::json!(stop);
        }

        if let Some(tools) = &request.tools {
            let tool_defs: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.input_schema,
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tool_defs);
        }

        if self.spec.capabilities.thinking {
            body["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": 10000,
            });
        }

        body
    }
}

#[async_trait]
impl LanguageModel for AnthropicModel {
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
        self.spec.capabilities.tools
    }

    fn supports_streaming_tools(&self) -> bool {
        self.spec.capabilities.streaming_tools
    }

    fn supports_images(&self) -> bool {
        self.spec.capabilities.images
    }

    fn supports_thinking(&self) -> bool {
        self.spec.capabilities.thinking
    }

    fn max_token_count(&self) -> u64 {
        self.spec.max_tokens
    }

    fn max_output_tokens(&self) -> Option<u64> {
        self.spec.max_output_tokens
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
            let api_key = self.credentials.get_api_key("anthropic").await?;

            let _permit = self.rate_limiter.acquire_owned().await?;

            let body = self.build_request_body(&request);
            let url = format!("{ANTHROPIC_API_BASE}/messages");

            let response = self
                .client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", ANTHROPIC_API_VERSION)
                .header("content-type", "application/json")
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
            let mapper = AnthropicSseMapper;
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

/// Anthropic Claude 供应商，管理模型列表与认证。
pub struct AnthropicProvider {
    client: Client,
    credentials: Arc<dyn CredentialsProvider>,
    rate_limiter: Arc<RateLimiter>,
    config: ProviderConfig,
}

impl AnthropicProvider {
    /// # Panics
    ///
    /// 若 HTTP 客户端构建失败则 panic（通常表示 TLS 后端不可用）。
    #[must_use]
    pub fn new() -> Self {
        let client =
            crate::build_http_client(DEFAULT_TIMEOUT).expect("HTTP client init should not fail");

        let credentials: Arc<dyn CredentialsProvider> = Arc::new(EnvCredentialProvider::new());
        let rate_limiter = Arc::new(RateLimiter::new(5));

        let model_configs: Vec<ModelConfig> = anthropic_models()
            .iter()
            .map(|spec| ModelConfig {
                id: ModelId::from(spec.id),
                display_name: spec.display_name.to_string(),
                max_tokens: spec.max_tokens,
                max_output_tokens: spec.max_output_tokens,
                capabilities: spec.capabilities.clone(),
            })
            .collect();

        let config = ProviderConfig {
            id: ProviderId::new("anthropic"),
            name: ProviderName::new("Anthropic"),
            api_url: ANTHROPIC_API_BASE.to_string(),
            api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
            models: model_configs,
            rate_limit: None,
            retry: None,
            extra_headers: HashMap::new(),
            max_request_body_bytes: None,
        };

        Self {
            client,
            credentials,
            rate_limiter,
            config,
        }
    }

    /// 设置自定义凭证提供者。
    #[must_use]
    pub fn with_credentials(mut self, credentials: Arc<dyn CredentialsProvider>) -> Self {
        self.credentials = credentials;
        self
    }

    /// 设置自定义 API 基地址。
    #[must_use]
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.config.api_url = api_base.into();
        self
    }
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LanguageModelProvider for AnthropicProvider {
    fn id(&self) -> &ProviderId {
        &self.config.id
    }

    fn name(&self) -> &ProviderName {
        &self.config.name
    }

    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
        anthropic_models()
            .into_iter()
            .map(|spec| {
                Arc::new(AnthropicModel::new(
                    spec,
                    self.config.id.clone(),
                    self.config.name.clone(),
                    self.client.clone(),
                    self.credentials.clone(),
                    self.rate_limiter.clone(),
                )) as Arc<dyn LanguageModel>
            })
            .collect()
    }

    fn is_authenticated(&self) -> bool {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.credentials.get_api_key("anthropic"))
        })
        .is_ok()
    }

    async fn authenticate(&self) -> Result<(), LlmError> {
        self.credentials.get_api_key("anthropic").await?;
        Ok(())
    }

    async fn reset_credentials(&self) -> Result<(), LlmError> {
        self.credentials.invalidate("anthropic").await;
        Ok(())
    }

    fn configuration(&self) -> &ProviderConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credential::ConfigCredentialProvider;
    use crate::provider::LanguageModelProvider;

    fn make_provider() -> AnthropicProvider {
        let creds: Arc<dyn CredentialsProvider> = Arc::new(ConfigCredentialProvider::new(
            vec![("anthropic".to_string(), "test-key".to_string())]
                .into_iter()
                .collect(),
        ));
        AnthropicProvider::new().with_credentials(creds)
    }

    #[test]
    fn test_anthropic_provider_new() {
        let provider = AnthropicProvider::new();
        assert_eq!(provider.id().as_ref(), "anthropic");
        assert_eq!(provider.name().as_ref(), "Anthropic");
    }

    #[test]
    fn test_anthropic_provider_default() {
        let provider = AnthropicProvider::default();
        assert_eq!(provider.id().as_ref(), "anthropic");
    }

    #[test]
    fn test_anthropic_provider_configuration() {
        let provider = make_provider();
        let config = provider.configuration();
        assert_eq!(config.api_url, "https://api.anthropic.com/v1");
        assert_eq!(config.api_key_env, Some("ANTHROPIC_API_KEY".to_string()));
    }

    #[test]
    fn test_anthropic_provider_with_api_base() {
        let provider = make_provider().with_api_base("http://localhost:8080");
        assert_eq!(provider.configuration().api_url, "http://localhost:8080");
    }

    #[test]
    fn test_anthropic_provided_models() {
        let provider = make_provider();
        let models = provider.provided_models();
        assert!(!models.is_empty());
        let claude = models
            .iter()
            .find(|m| m.id().as_str() == "claude-opus-4-6")
            .unwrap();
        assert!(claude.supports_tools());
        assert!(claude.supports_images());
        assert!(claude.supports_thinking());
        assert!(claude.supports_streaming_tools());
        assert_eq!(claude.max_token_count(), 200_000);
        assert_eq!(claude.max_output_tokens(), Some(32_000));
    }

    #[test]
    fn test_anthropic_haiku_no_thinking() {
        let provider = make_provider();
        let models = provider.provided_models();
        let haiku = models
            .iter()
            .find(|m| m.id().as_str() == "claude-haiku-3-5")
            .unwrap();
        assert!(!haiku.supports_thinking());
    }

    #[tokio::test]
    async fn test_anthropic_model_count_tokens() {
        let provider = make_provider();
        let model = provider.provided_models().into_iter().next().unwrap();
        let request = LanguageModelRequest::new(model.id().as_str(), vec![Message::user("hello")]);
        let count = model.count_tokens(&request).await.unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_anthropic_extract_system_prompt() {
        let messages = vec![
            Message {
                role: Role::System,
                content: crate::provider::content_block::MessageContent::text("You are helpful"),
                name: None,
                tool_call_id: None,
            },
            Message::user("hi"),
        ];
        let system = AnthropicModel::extract_system_prompt(&messages);
        assert_eq!(system, Some("You are helpful".to_string()));
    }

    #[test]
    fn test_anthropic_extract_system_prompt_none() {
        let messages = vec![Message::user("hi")];
        let system = AnthropicModel::extract_system_prompt(&messages);
        assert!(system.is_none());
    }

    #[test]
    fn test_anthropic_build_request_body_basic() {
        let provider = make_provider();
        let models = provider.provided_models();
        let model = models
            .iter()
            .find(|m| m.id().as_str() == "claude-sonnet-4-5")
            .unwrap();
        let request = LanguageModelRequest::new("claude-sonnet-4-5", vec![Message::user("hello")]);
        let body = model.stream_completion(request);
        drop(body);
    }

    #[tokio::test]
    async fn test_anthropic_authenticate_with_config_creds() {
        let provider = make_provider();
        let result = provider.authenticate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_anthropic_reset_credentials() {
        let provider = make_provider();
        let result = provider.reset_credentials().await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_anthropic_is_authenticated_with_config_creds() {
        let provider = make_provider();
        assert!(provider.is_authenticated());
    }
}
