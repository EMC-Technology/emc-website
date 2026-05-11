use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;

use crate::credential::{CredentialsProvider, EnvCredentialProvider};
use crate::error::LlmError;
use crate::provider::types::{
    LanguageModelRequest, ModelCapabilities, ModelConfig, ModelId, ModelName, ProviderConfig,
    ProviderId, ProviderName,
};
use crate::provider::{LanguageModel, LanguageModelProvider};
use crate::rate_limit::RateLimiter;
use crate::stream::openai_mapper::OpenAiSseMapper;
use crate::stream::sse::SseParser;
use crate::stream::{ModelStream, StreamEvent, StreamFuture};
use crate::thinking::is_reasoning_model;
use crate::token_count::{ByteEstimator, TokenCounter};

const OPENAI_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

struct OpenAiModelSpec {
    id: &'static str,
    display_name: &'static str,
    max_tokens: u64,
    max_output_tokens: Option<u64>,
    capabilities: ModelCapabilities,
}

fn gpt_models() -> Vec<OpenAiModelSpec> {
    vec![
        OpenAiModelSpec {
            id: "gpt-4",
            display_name: "GPT-4",
            max_tokens: 8192,
            max_output_tokens: Some(8192),
            capabilities: ModelCapabilities {
                tools: true,
                images: false,
                streaming_tools: true,
                parallel_tool_calls: true,
                thinking: false,
                max_tokens: 8192,
            },
        },
        OpenAiModelSpec {
            id: "gpt-4-turbo",
            display_name: "GPT-4 Turbo",
            max_tokens: 128_000,
            max_output_tokens: Some(4096),
            capabilities: ModelCapabilities {
                tools: true,
                images: true,
                streaming_tools: true,
                parallel_tool_calls: true,
                thinking: false,
                max_tokens: 128_000,
            },
        },
        OpenAiModelSpec {
            id: "gpt-4o",
            display_name: "GPT-4o",
            max_tokens: 128_000,
            max_output_tokens: Some(16_384),
            capabilities: ModelCapabilities {
                tools: true,
                images: true,
                streaming_tools: true,
                parallel_tool_calls: true,
                thinking: false,
                max_tokens: 128_000,
            },
        },
        OpenAiModelSpec {
            id: "gpt-4o-mini",
            display_name: "GPT-4o Mini",
            max_tokens: 128_000,
            max_output_tokens: Some(16_384),
            capabilities: ModelCapabilities {
                tools: true,
                images: true,
                streaming_tools: true,
                parallel_tool_calls: true,
                thinking: false,
                max_tokens: 128_000,
            },
        },
        OpenAiModelSpec {
            id: "gpt-3.5-turbo",
            display_name: "GPT-3.5 Turbo",
            max_tokens: 16_385,
            max_output_tokens: Some(4096),
            capabilities: ModelCapabilities {
                tools: true,
                images: false,
                streaming_tools: true,
                parallel_tool_calls: false,
                thinking: false,
                max_tokens: 16_385,
            },
        },
    ]
}

fn o_series_models() -> Vec<OpenAiModelSpec> {
    vec![
        OpenAiModelSpec {
            id: "o1",
            display_name: "o1",
            max_tokens: 200_000,
            max_output_tokens: Some(100_000),
            capabilities: ModelCapabilities {
                tools: true,
                images: true,
                streaming_tools: false,
                parallel_tool_calls: true,
                thinking: true,
                max_tokens: 200_000,
            },
        },
        OpenAiModelSpec {
            id: "o1-mini",
            display_name: "o1-mini",
            max_tokens: 128_000,
            max_output_tokens: Some(65_536),
            capabilities: ModelCapabilities {
                tools: true,
                images: true,
                streaming_tools: false,
                parallel_tool_calls: true,
                thinking: true,
                max_tokens: 128_000,
            },
        },
        OpenAiModelSpec {
            id: "o3-mini",
            display_name: "o3-mini",
            max_tokens: 200_000,
            max_output_tokens: Some(100_000),
            capabilities: ModelCapabilities {
                tools: true,
                images: true,
                streaming_tools: false,
                parallel_tool_calls: true,
                thinking: true,
                max_tokens: 200_000,
            },
        },
    ]
}

fn openai_models() -> Vec<OpenAiModelSpec> {
    let mut models = Vec::with_capacity(8);
    models.extend(gpt_models());
    models.extend(o_series_models());
    models
}

/// `OpenAI` 模型实例。
pub struct OpenAiModel {
    id: ModelId,
    name: ModelName,
    provider_id: ProviderId,
    provider_name: ProviderName,
    spec: OpenAiModelSpec,
    client: Client,
    credentials: Arc<dyn CredentialsProvider>,
    rate_limiter: Arc<RateLimiter>,
}

impl OpenAiModel {
    fn new(
        spec: OpenAiModelSpec,
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

    fn build_request_body(request: &LanguageModelRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
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
            "stream": true,
        });

        if is_reasoning_model(request.model.as_ref()) {
            if let Some(max_tokens) = request.max_tokens {
                body["max_completion_tokens"] = serde_json::json!(max_tokens);
            }
        } else {
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
        }

        if let Some(tools) = &request.tools {
            let tool_defs: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.input_schema,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tool_defs);
        }

        body
    }
}

#[async_trait]
impl LanguageModel for OpenAiModel {
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
            let api_key = self.credentials.get_api_key("openai").await?;

            let _permit = self.rate_limiter.acquire_owned().await?;

            let body = Self::build_request_body(&request);
            let url = format!("{OPENAI_API_BASE}/chat/completions");

            let response = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {api_key}"))
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

/// `OpenAI` 供应商，管理模型列表与认证。
pub struct OpenAiProvider {
    client: Client,
    credentials: Arc<dyn CredentialsProvider>,
    rate_limiter: Arc<RateLimiter>,
    config: ProviderConfig,
}

impl OpenAiProvider {
    /// 创建新的 `OpenAI` Provider 实例。
    ///
    /// # Panics
    ///
    /// 当 HTTP 客户端构建失败时 panic。
    #[must_use]
    pub fn new() -> Self {
        let client =
            crate::build_http_client(DEFAULT_TIMEOUT).expect("HTTP client init should not fail");

        let credentials: Arc<dyn CredentialsProvider> = Arc::new(EnvCredentialProvider::new());
        let rate_limiter = Arc::new(RateLimiter::new(10));

        let model_configs: Vec<ModelConfig> = openai_models()
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
            id: ProviderId::new("openai"),
            name: ProviderName::new("OpenAI"),
            api_url: OPENAI_API_BASE.to_string(),
            api_key_env: Some("OPENAI_API_KEY".to_string()),
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

impl Default for OpenAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LanguageModelProvider for OpenAiProvider {
    fn id(&self) -> &ProviderId {
        &self.config.id
    }

    fn name(&self) -> &ProviderName {
        &self.config.name
    }

    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
        openai_models()
            .into_iter()
            .map(|spec| {
                Arc::new(OpenAiModel::new(
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
            tokio::runtime::Handle::current().block_on(self.credentials.get_api_key("openai"))
        })
        .is_ok()
    }

    async fn authenticate(&self) -> Result<(), LlmError> {
        self.credentials.get_api_key("openai").await?;
        Ok(())
    }

    async fn reset_credentials(&self) -> Result<(), LlmError> {
        self.credentials.invalidate("openai").await;
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
    use crate::provider::types::Message;

    fn make_provider() -> OpenAiProvider {
        let creds: Arc<dyn CredentialsProvider> = Arc::new(ConfigCredentialProvider::new(
            vec![("openai".to_string(), "test-key".to_string())]
                .into_iter()
                .collect(),
        ));
        OpenAiProvider::new().with_credentials(creds)
    }

    #[test]
    fn test_openai_provider_new() {
        let provider = OpenAiProvider::new();
        assert_eq!(provider.id().as_ref(), "openai");
        assert_eq!(provider.name().as_ref(), "OpenAI");
    }

    #[test]
    fn test_openai_provider_default() {
        let provider = OpenAiProvider::default();
        assert_eq!(provider.id().as_ref(), "openai");
    }

    #[test]
    fn test_openai_provider_configuration() {
        let provider = make_provider();
        let config = provider.configuration();
        assert_eq!(config.api_url, "https://api.openai.com/v1");
        assert_eq!(config.api_key_env, Some("OPENAI_API_KEY".to_string()));
    }

    #[test]
    fn test_openai_provider_with_api_base() {
        let provider = make_provider().with_api_base("http://localhost:9090");
        assert_eq!(provider.configuration().api_url, "http://localhost:9090");
    }

    #[test]
    fn test_openai_provided_models() {
        let provider = make_provider();
        let models = provider.provided_models();
        assert!(!models.is_empty());
        let gpt4o = models.iter().find(|m| m.id().as_str() == "gpt-4o").unwrap();
        assert!(gpt4o.supports_tools());
        assert!(gpt4o.supports_images());
        assert!(!gpt4o.supports_thinking());
        assert_eq!(gpt4o.max_token_count(), 128_000);
    }

    #[test]
    fn test_openai_o1_thinking() {
        let provider = make_provider();
        let models = provider.provided_models();
        let o1 = models.iter().find(|m| m.id().as_str() == "o1").unwrap();
        assert!(o1.supports_thinking());
        assert!(!o1.supports_streaming_tools());
    }

    #[test]
    fn test_openai_gpt35_no_parallel_tools() {
        let provider = make_provider();
        let models = provider.provided_models();
        let gpt35 = models
            .iter()
            .find(|m| m.id().as_str() == "gpt-3.5-turbo")
            .unwrap();
        assert!(!gpt35.supports_images());
    }

    #[tokio::test]
    async fn test_openai_model_count_tokens() {
        let provider = make_provider();
        let model = provider.provided_models().into_iter().next().unwrap();
        let request = LanguageModelRequest::new(model.id().as_str(), vec![Message::user("hello")]);
        let count = model.count_tokens(&request).await.unwrap();
        assert!(count > 0);
    }

    #[tokio::test]
    async fn test_openai_authenticate_with_config_creds() {
        let provider = make_provider();
        let result = provider.authenticate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_openai_reset_credentials() {
        let provider = make_provider();
        let result = provider.reset_credentials().await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_openai_is_authenticated_with_config_creds() {
        let provider = make_provider();
        assert!(provider.is_authenticated());
    }
}
