use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::provider::types::{
    LanguageModelRequest, Message, ModelCapabilities, ModelConfig, ModelId, ModelName,
    ProviderConfig, ProviderId, ProviderName, StopReason,
};
use crate::provider::{LanguageModel, LanguageModelProvider};
use crate::stream::{ModelStream, StreamEvent, StreamFuture};
use crate::token_count::{ByteEstimator, TokenCounter};

const CODEGEEX_API_BASE: &str = "https://api.codegeex.cn";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Serialize)]
struct CodeGeeXRequest {
    prompt: String,
    max_tokens: u32,
    temperature: f32,
    top_p: f32,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct CodeGeeXResponse {
    code: i32,
    message: String,
    data: Option<CodeGeeXData>,
}

#[derive(Debug, Deserialize)]
struct CodeGeeXData {
    content: String,
    #[allow(dead_code)]
    tokens: Option<u32>,
}

/// `CodeGeeX` 模型实例。
pub struct CodeGeeXModel {
    id: ModelId,
    name: ModelName,
    provider_id: ProviderId,
    provider_name: ProviderName,
    client: Client,
    api_base: String,
}

impl CodeGeeXModel {
    /// 创建新的 `CodeGeeX` 模型实例。
    #[must_use]
    pub fn new(
        model_id: &str,
        display_name: &str,
        provider_id: ProviderId,
        provider_name: ProviderName,
        client: Client,
        api_base: String,
    ) -> Self {
        Self {
            id: ModelId::new(model_id),
            name: ModelName::new(display_name),
            provider_id,
            provider_name,
            client,
            api_base,
        }
    }

    fn messages_to_prompt(messages: &[Message]) -> String {
        messages
            .iter()
            .map(|m| {
                format!(
                    "{}: {}",
                    m.role.as_str().to_lowercase(),
                    m.content.to_text_lossy()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[async_trait]
impl LanguageModel for CodeGeeXModel {
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
        8192
    }
    fn max_output_tokens(&self) -> Option<u64> {
        Some(4096)
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
            let prompt = Self::messages_to_prompt(&request.messages);

            let codegee_request = CodeGeeXRequest {
                prompt,
                max_tokens: request.max_tokens.unwrap_or(2048),
                temperature: request.temperature.unwrap_or(0.7),
                top_p: request.top_p.unwrap_or(0.9),
                stream: false,
            };

            let url = format!("{}/chat/completions", self.api_base);
            let response = self
                .client
                .post(&url)
                .json(&codegee_request)
                .send()
                .await
                .map_err(LlmError::from)?;

            let status = response.status();
            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                return Err(LlmError::from_http_status(status, &body_text));
            }

            let codegee_response: CodeGeeXResponse = response
                .json()
                .await
                .map_err(|e| LlmError::Network(e.to_string()))?;

            if codegee_response.code != 0 {
                return Err(LlmError::InvalidRequest {
                    message: codegee_response.message,
                });
            }

            let content = codegee_response.data.map(|d| d.content).unwrap_or_default();

            let stream: ModelStream = Box::pin(futures_util::stream::iter(vec![
                Ok(StreamEvent::Text(content)),
                Ok(StreamEvent::Stop(StopReason::EndTurn)),
            ]));

            Ok(stream)
        })
    }
}

/// `CodeGeeX` 供应商，管理模型列表与认证。
pub struct CodeGeeXProvider {
    client: Client,
    api_base: String,
    config: ProviderConfig,
}

impl CodeGeeXProvider {
    /// # Panics
    ///
    /// 若 HTTP 客户端构建失败则 panic（通常表示 TLS 后端不可用）。
    #[must_use]
    pub fn new() -> Self {
        let client =
            crate::build_http_client(DEFAULT_TIMEOUT).expect("HTTP client init should not fail");

        let config = ProviderConfig {
            id: ProviderId::new("codegeex"),
            name: ProviderName::new("CodeGeeX"),
            api_url: CODEGEEX_API_BASE.to_string(),
            api_key_env: None,
            models: vec![ModelConfig {
                id: ModelId::from("codegeex-4"),
                display_name: "CodeGeeX 4".to_string(),
                max_tokens: 8192,
                max_output_tokens: Some(4096),
                capabilities: ModelCapabilities {
                    tools: false,
                    images: false,
                    streaming_tools: false,
                    parallel_tool_calls: false,
                    thinking: false,
                    max_tokens: 8192,
                },
            }],
            rate_limit: None,
            retry: None,
            extra_headers: HashMap::new(),
            max_request_body_bytes: None,
        };

        Self {
            client,
            api_base: CODEGEEX_API_BASE.to_string(),
            config,
        }
    }

    /// 设置自定义 API 基地址。
    #[must_use]
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }
}

impl Default for CodeGeeXProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LanguageModelProvider for CodeGeeXProvider {
    fn id(&self) -> &ProviderId {
        &self.config.id
    }
    fn name(&self) -> &ProviderName {
        &self.config.name
    }

    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
        vec![Arc::new(CodeGeeXModel::new(
            "codegeex-4",
            "CodeGeeX 4",
            self.config.id.clone(),
            self.config.name.clone(),
            self.client.clone(),
            self.api_base.clone(),
        )) as Arc<dyn LanguageModel>]
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
    use crate::provider::types::Role;

    #[test]
    fn test_codegeex_provider_new() {
        let provider = CodeGeeXProvider::new();
        assert_eq!(provider.id().as_ref(), "codegeex");
        assert_eq!(provider.name().as_ref(), "CodeGeeX");
    }

    #[test]
    fn test_codegeex_provider_default() {
        let provider = CodeGeeXProvider::default();
        assert_eq!(provider.id().as_ref(), "codegeex");
    }

    #[test]
    fn test_codegeex_provider_with_api_base() {
        let provider = CodeGeeXProvider::new().with_api_base("http://custom:8080");
        assert_eq!(provider.configuration().api_url, "https://api.codegeex.cn");
    }

    #[test]
    fn test_codegeex_provider_configuration() {
        let provider = CodeGeeXProvider::new();
        let config = provider.configuration();
        assert_eq!(config.api_url, "https://api.codegeex.cn");
        assert!(config.api_key_env.is_none());
    }

    #[test]
    fn test_codegeex_provided_models() {
        let provider = CodeGeeXProvider::new();
        let models = provider.provided_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id().as_str(), "codegeex-4");
        assert!(!models[0].supports_tools());
        assert!(!models[0].supports_images());
        assert!(!models[0].supports_thinking());
        assert_eq!(models[0].max_token_count(), 8192);
        assert_eq!(models[0].max_output_tokens(), Some(4096));
    }

    #[test]
    fn test_codegeex_is_authenticated() {
        let provider = CodeGeeXProvider::new();
        assert!(provider.is_authenticated());
    }

    #[tokio::test]
    async fn test_codegeex_model_count_tokens() {
        let provider = CodeGeeXProvider::new();
        let model = provider.provided_models().into_iter().next().unwrap();
        let request = LanguageModelRequest::new("codegeex-4", vec![Message::user("hello")]);
        let count = model.count_tokens(&request).await.unwrap();
        assert!(count > 0);
    }

    #[tokio::test]
    async fn test_codegeex_authenticate() {
        let provider = CodeGeeXProvider::new();
        let result = provider.authenticate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_codegeex_reset_credentials() {
        let provider = CodeGeeXProvider::new();
        let result = provider.reset_credentials().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_codegeex_messages_to_prompt() {
        let messages = vec![
            Message::user("write a function"),
            Message {
                role: Role::Assistant,
                content: crate::provider::content_block::MessageContent::text("fn hello()"),
                name: None,
                tool_call_id: None,
            },
        ];
        let prompt = CodeGeeXModel::messages_to_prompt(&messages);
        assert!(prompt.contains("user:"));
        assert!(prompt.contains("assistant:"));
    }
}
