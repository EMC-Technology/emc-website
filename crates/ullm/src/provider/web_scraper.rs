#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::credential::CredentialsProvider;
use crate::error::LlmError;
use crate::provider::types::{
    LanguageModelRequest, ModelCapabilities, ModelConfig, ModelId, ModelName, ProviderConfig,
    ProviderId, ProviderName, StopReason,
};
use crate::provider::{LanguageModel, LanguageModelProvider};
use crate::stream::{ModelStream, StreamEvent, StreamFuture};
use crate::token_count::{ByteEstimator, TokenCounter};

/// Web Scraper 目标站点枚举。
#[derive(Debug, Clone)]
pub enum WebTarget {
    /// Claude Web
    ClaudeWeb,
    /// Copilot Web
    CopilotWeb,
    /// 通义千问 Web
    QwenWeb,
    /// `MiniMax` Web
    MiniMaxWeb,
    /// Gemini Web
    GeminiWeb,
    /// 自定义 URL
    Custom {
        /// 目标 URL
        url: String,
    },
}

/// Web Scraper 模型，封装通过浏览器抓取方式访问 LLM 的逻辑。
pub struct WebScraperModel {
    id: ModelId,
    name: ModelName,
    provider_id: ProviderId,
    provider_name: ProviderName,
    target: WebTarget,
}

impl WebScraperModel {
    /// 创建新的 Web Scraper 模型实例。
    #[must_use]
    pub fn new(
        model_id: &str,
        display_name: &str,
        provider_id: ProviderId,
        provider_name: ProviderName,
        target: WebTarget,
    ) -> Self {
        Self {
            id: ModelId::new(model_id),
            name: ModelName::new(display_name),
            provider_id,
            provider_name,
            target,
        }
    }

    fn target_url(&self) -> &str {
        match &self.target {
            WebTarget::ClaudeWeb => "https://claude.ai",
            WebTarget::CopilotWeb => "https://copilot.github.com",
            WebTarget::QwenWeb => "https://tongyi.aliyun.com",
            WebTarget::MiniMaxWeb => "https://hailuoai.com",
            WebTarget::GeminiWeb => "https://gemini.google.com",
            WebTarget::Custom { url } => url,
        }
    }
}

#[async_trait]
impl LanguageModel for WebScraperModel {
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
        100_000
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

    fn stream_completion(&self, _request: LanguageModelRequest) -> StreamFuture<'_> {
        let url = self.target_url().to_string();
        Box::pin(async move {
            let stream: ModelStream = Box::pin(futures_util::stream::iter(vec![
                Ok(StreamEvent::Error(format!(
                    "WebScraper provider ({url}) is experimental and requires an active browser session"
                ))),
                Ok(StreamEvent::Stop(StopReason::Cancelled)),
            ]));
            Ok(stream)
        })
    }
}

/// Web Scraper 供应商，管理浏览器抓取方式的 LLM 访问。
pub struct WebScraperProvider {
    config: ProviderConfig,
    target: WebTarget,
    portal_auth: Option<Arc<crate::credential::portal::PortalAuthProvider>>,
}

impl WebScraperProvider {
    /// 根据目标站点创建 Web Scraper 供应商实例。
    #[must_use]
    pub fn new(target: WebTarget) -> Self {
        let (id_str, name_str, model_id, model_name) = match &target {
            WebTarget::ClaudeWeb => (
                "web-scraper-claude",
                "Web Scraper (Claude)",
                "claude-web",
                "Claude Web",
            ),
            WebTarget::CopilotWeb => (
                "web-scraper-copilot",
                "Web Scraper (Copilot)",
                "copilot-web",
                "Copilot Web",
            ),
            WebTarget::QwenWeb => (
                "web-scraper-qwen",
                "Web Scraper (Qwen)",
                "qwen-web",
                "Qwen Web",
            ),
            WebTarget::MiniMaxWeb => (
                "web-scraper-minimax",
                "Web Scraper (MiniMax)",
                "minimax-web",
                "MiniMax Web",
            ),
            WebTarget::GeminiWeb => (
                "web-scraper-gemini",
                "Web Scraper (Gemini)",
                "gemini-web",
                "Gemini Web",
            ),
            WebTarget::Custom { .. } => (
                "web-scraper-custom",
                "Web Scraper (Custom)",
                "custom-web",
                "Custom Web",
            ),
        };

        let config = ProviderConfig {
            id: ProviderId::new(id_str),
            name: ProviderName::new(name_str),
            api_url: "https://web-scraper.local".to_string(),
            api_key_env: None,
            models: vec![ModelConfig {
                id: ModelId::from(model_id),
                display_name: model_name.to_string(),
                max_tokens: 100_000,
                max_output_tokens: Some(4096),
                capabilities: ModelCapabilities {
                    tools: false,
                    images: false,
                    streaming_tools: false,
                    parallel_tool_calls: false,
                    thinking: false,
                    max_tokens: 100_000,
                },
            }],
            rate_limit: None,
            retry: None,
            extra_headers: HashMap::new(),
            max_request_body_bytes: None,
        };

        Self {
            config,
            target,
            portal_auth: None,
        }
    }

    /// 设置 Portal 认证提供者。
    #[must_use]
    pub fn with_portal_auth(
        mut self,
        auth: Arc<crate::credential::portal::PortalAuthProvider>,
    ) -> Self {
        self.portal_auth = Some(auth);
        self
    }

    /// 创建 Claude Web 供应商实例。
    #[must_use]
    pub fn claude_web() -> Self {
        Self::new(WebTarget::ClaudeWeb)
    }

    /// 创建 Copilot Web 供应商实例。
    #[must_use]
    pub fn copilot_web() -> Self {
        Self::new(WebTarget::CopilotWeb)
    }

    /// 创建自定义 URL 供应商实例。
    pub fn custom(url: impl Into<String>) -> Self {
        Self::new(WebTarget::Custom { url: url.into() })
    }
}

#[async_trait]
impl LanguageModelProvider for WebScraperProvider {
    fn id(&self) -> &ProviderId {
        &self.config.id
    }
    fn name(&self) -> &ProviderName {
        &self.config.name
    }

    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
        let model_cfg = self.config.models.first();
        match model_cfg {
            Some(mc) => vec![Arc::new(WebScraperModel::new(
                mc.id.as_str(),
                &mc.display_name,
                self.config.id.clone(),
                self.config.name.clone(),
                self.target.clone(),
            )) as Arc<dyn LanguageModel>],
            None => Vec::new(),
        }
    }

    fn is_authenticated(&self) -> bool {
        self.portal_auth.is_some()
    }

    async fn authenticate(&self) -> Result<(), LlmError> {
        if let Some(auth) = &self.portal_auth {
            auth.get_api_key(self.config.id.as_ref()).await?;
        }
        Ok(())
    }

    async fn reset_credentials(&self) -> Result<(), LlmError> {
        if let Some(auth) = &self.portal_auth {
            auth.invalidate(self.config.id.as_ref()).await;
        }
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
    fn test_web_scraper_claude_web() {
        let provider = WebScraperProvider::claude_web();
        assert_eq!(provider.id().as_ref(), "web-scraper-claude");
        assert_eq!(provider.name().as_ref(), "Web Scraper (Claude)");
    }

    #[test]
    fn test_web_scraper_copilot_web() {
        let provider = WebScraperProvider::copilot_web();
        assert_eq!(provider.id().as_ref(), "web-scraper-copilot");
    }

    #[test]
    fn test_web_scraper_custom() {
        let provider = WebScraperProvider::custom("https://example.com");
        assert_eq!(provider.id().as_ref(), "web-scraper-custom");
        assert_eq!(provider.name().as_ref(), "Web Scraper (Custom)");
    }

    #[test]
    fn test_web_scraper_qwen_web() {
        let provider = WebScraperProvider::new(WebTarget::QwenWeb);
        assert_eq!(provider.id().as_ref(), "web-scraper-qwen");
    }

    #[test]
    fn test_web_scraper_minimax_web() {
        let provider = WebScraperProvider::new(WebTarget::MiniMaxWeb);
        assert_eq!(provider.id().as_ref(), "web-scraper-minimax");
    }

    #[test]
    fn test_web_scraper_gemini_web() {
        let provider = WebScraperProvider::new(WebTarget::GeminiWeb);
        assert_eq!(provider.id().as_ref(), "web-scraper-gemini");
    }

    #[test]
    fn test_web_scraper_provided_models() {
        let provider = WebScraperProvider::claude_web();
        let models = provider.provided_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id().as_str(), "claude-web");
        assert!(!models[0].supports_tools());
        assert!(!models[0].supports_images());
        assert!(!models[0].supports_thinking());
        assert_eq!(models[0].max_token_count(), 100_000);
    }

    #[test]
    fn test_web_scraper_is_authenticated_no_portal() {
        let provider = WebScraperProvider::claude_web();
        assert!(!provider.is_authenticated());
    }

    #[tokio::test]
    async fn test_web_scraper_authenticate_no_portal() {
        let provider = WebScraperProvider::claude_web();
        let result = provider.authenticate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_web_scraper_reset_credentials() {
        let provider = WebScraperProvider::claude_web();
        let result = provider.reset_credentials().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_web_scraper_model_count_tokens() {
        let provider = WebScraperProvider::claude_web();
        let model = provider.provided_models().into_iter().next().unwrap();
        let request = LanguageModelRequest::new("claude-web", vec![Message::user("hello")]);
        let count = model.count_tokens(&request).await.unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_web_target_urls() {
        let model = WebScraperModel::new(
            "test",
            "Test",
            ProviderId::new("test"),
            ProviderName::new("Test"),
            WebTarget::ClaudeWeb,
        );
        assert_eq!(model.target_url(), "https://claude.ai");
        let model = WebScraperModel::new(
            "test",
            "Test",
            ProviderId::new("test"),
            ProviderName::new("Test"),
            WebTarget::CopilotWeb,
        );
        assert_eq!(model.target_url(), "https://copilot.github.com");
        let model = WebScraperModel::new(
            "test",
            "Test",
            ProviderId::new("test"),
            ProviderName::new("Test"),
            WebTarget::QwenWeb,
        );
        assert_eq!(model.target_url(), "https://tongyi.aliyun.com");
        let model = WebScraperModel::new(
            "test",
            "Test",
            ProviderId::new("test"),
            ProviderName::new("Test"),
            WebTarget::MiniMaxWeb,
        );
        assert_eq!(model.target_url(), "https://hailuoai.com");
        let model = WebScraperModel::new(
            "test",
            "Test",
            ProviderId::new("test"),
            ProviderName::new("Test"),
            WebTarget::GeminiWeb,
        );
        assert_eq!(model.target_url(), "https://gemini.google.com");
        let model = WebScraperModel::new(
            "test",
            "Test",
            ProviderId::new("test"),
            ProviderName::new("Test"),
            WebTarget::Custom {
                url: "https://custom.com".into(),
            },
        );
        assert_eq!(model.target_url(), "https://custom.com");
    }

    #[test]
    fn test_web_scraper_configuration() {
        let provider = WebScraperProvider::claude_web();
        let config = provider.configuration();
        assert!(config.api_key_env.is_none());
        assert_eq!(config.models.len(), 1);
    }
}
