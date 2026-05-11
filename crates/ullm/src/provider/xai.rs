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
use crate::token_count::{ByteEstimator, TokenCounter};

const XAI_API_BASE: &str = "https://api.x.ai/v1";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

struct XaiModelSpec {
    id: &'static str,
    display_name: &'static str,
    max_tokens: u64,
    max_output_tokens: Option<u64>,
    capabilities: ModelCapabilities,
}

fn xai_models() -> Vec<XaiModelSpec> {
    vec![
        XaiModelSpec {
            id: "grok-3",
            display_name: "Grok 3",
            max_tokens: 131_072,
            max_output_tokens: Some(8192),
            capabilities: ModelCapabilities {
                tools: true,
                images: false,
                streaming_tools: true,
                parallel_tool_calls: false,
                thinking: false,
                max_tokens: 131_072,
            },
        },
        XaiModelSpec {
            id: "grok-3-mini",
            display_name: "Grok 3 Mini",
            max_tokens: 131_072,
            max_output_tokens: Some(8192),
            capabilities: ModelCapabilities {
                tools: true,
                images: false,
                streaming_tools: true,
                parallel_tool_calls: false,
                thinking: true,
                max_tokens: 131_072,
            },
        },
    ]
}

/// xAI 模型实例。
pub struct XaiModel {
    id: ModelId,
    name: ModelName,
    provider_id: ProviderId,
    provider_name: ProviderName,
    spec: XaiModelSpec,
    client: Client,
    credentials: Arc<dyn CredentialsProvider>,
    rate_limiter: Arc<RateLimiter>,
}

impl XaiModel {
    fn new(
        spec: XaiModelSpec,
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
impl LanguageModel for XaiModel {
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
            let api_key = self.credentials.get_api_key("xai").await?;
            let _permit = self.rate_limiter.acquire_owned().await?;

            let body = Self::build_request_body(&request);
            let url = format!("{XAI_API_BASE}/chat/completions");

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

/// xAI 供应商，管理模型列表与认证。
pub struct XaiProvider {
    client: Client,
    credentials: Arc<dyn CredentialsProvider>,
    rate_limiter: Arc<RateLimiter>,
    config: ProviderConfig,
}

impl XaiProvider {
    /// 创建新的 xAI Provider 实例。
    ///
    /// # Panics
    ///
    /// 当 HTTP 客户端构建失败时 panic。
    #[must_use]
    pub fn new() -> Self {
        let client =
            crate::build_http_client(DEFAULT_TIMEOUT).expect("HTTP client init should not fail");

        let credentials: Arc<dyn CredentialsProvider> = Arc::new(EnvCredentialProvider::new());
        let rate_limiter = Arc::new(RateLimiter::new(5));

        let model_configs: Vec<ModelConfig> = xai_models()
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
            id: ProviderId::new("xai"),
            name: ProviderName::new("xAI"),
            api_url: XAI_API_BASE.to_string(),
            api_key_env: Some("XAI_API_KEY".to_string()),
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
}

impl Default for XaiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LanguageModelProvider for XaiProvider {
    fn id(&self) -> &ProviderId {
        &self.config.id
    }
    fn name(&self) -> &ProviderName {
        &self.config.name
    }

    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
        xai_models()
            .into_iter()
            .map(|spec| {
                Arc::new(XaiModel::new(
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
            tokio::runtime::Handle::current().block_on(self.credentials.get_api_key("xai"))
        })
        .is_ok()
    }

    async fn authenticate(&self) -> Result<(), LlmError> {
        self.credentials.get_api_key("xai").await?;
        Ok(())
    }

    async fn reset_credentials(&self) -> Result<(), LlmError> {
        self.credentials.invalidate("xai").await;
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

    fn make_provider() -> XaiProvider {
        let creds: Arc<dyn CredentialsProvider> = Arc::new(ConfigCredentialProvider::new(
            vec![("xai".to_string(), "test-key".to_string())]
                .into_iter()
                .collect(),
        ));
        XaiProvider::new().with_credentials(creds)
    }

    #[test]
    fn test_xai_provider_new() {
        let provider = XaiProvider::new();
        assert_eq!(provider.id().as_ref(), "xai");
        assert_eq!(provider.name().as_ref(), "xAI");
    }

    #[test]
    fn test_xai_provider_default() {
        let provider = XaiProvider::default();
        assert_eq!(provider.id().as_ref(), "xai");
    }

    #[test]
    fn test_xai_provider_configuration() {
        let provider = make_provider();
        let config = provider.configuration();
        assert_eq!(config.api_url, "https://api.x.ai/v1");
        assert_eq!(config.api_key_env, Some("XAI_API_KEY".to_string()));
    }

    #[test]
    fn test_xai_provided_models() {
        let provider = make_provider();
        let models = provider.provided_models();
        assert_eq!(models.len(), 2);
        let grok3 = models.iter().find(|m| m.id().as_str() == "grok-3").unwrap();
        assert!(grok3.supports_tools());
        assert!(!grok3.supports_thinking());
        let grok3_mini = models
            .iter()
            .find(|m| m.id().as_str() == "grok-3-mini")
            .unwrap();
        assert!(grok3_mini.supports_thinking());
    }

    #[tokio::test]
    async fn test_xai_model_count_tokens() {
        let provider = make_provider();
        let model = provider.provided_models().into_iter().next().unwrap();
        let request = LanguageModelRequest::new(model.id().as_str(), vec![Message::user("hello")]);
        let count = model.count_tokens(&request).await.unwrap();
        assert!(count > 0);
    }

    #[tokio::test]
    async fn test_xai_authenticate_with_config_creds() {
        let provider = make_provider();
        let result = provider.authenticate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_xai_reset_credentials() {
        let provider = make_provider();
        let result = provider.reset_credentials().await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_xai_is_authenticated_with_config_creds() {
        let provider = make_provider();
        assert!(provider.is_authenticated());
    }
}
