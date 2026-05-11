/// Copilot Portal 认证实现
pub mod copilot;
/// Gemini CLI 认证实现
pub mod gemini_cli;
/// `MiniMax` Portal 认证实现
pub mod minimax;
/// 通义千问 Portal 认证实现
pub mod qwen;

pub use copilot::CopilotProxyAuth;
pub use gemini_cli::GeminiCliAuth;
pub use minimax::MiniMaxPortalAuth;
pub use qwen::QwenPortalAuth;

use std::collections::HashMap;

use async_trait::async_trait;
use parking_lot::RwLock;
use reqwest::Client;

use crate::build_http_client_with_cookies;
use crate::credential::{ApiKeyState, CredentialsProvider};
use crate::error::LlmError;

/// Portal 认证配置，定义浏览器抓取认证所需的端点与参数。
#[derive(Debug, Clone)]
pub struct PortalAuthConfig {
    /// Portal 主页 URL
    pub portal_url: String,
    /// 登录页面 URL
    pub login_url: String,
    /// Token 获取端点
    pub token_endpoint: String,
    /// 需要提取的 Cookie 名称列表
    pub cookie_names: Vec<String>,
    /// 认证头名称
    pub header_name: String,
    /// Token 刷新间隔（秒）
    pub refresh_interval_secs: u64,
}

impl PortalAuthConfig {
    /// 创建 Claude Web Portal 配置。
    #[must_use]
    pub fn claude_web() -> Self {
        Self {
            portal_url: "https://claude.ai".to_string(),
            login_url: "https://claude.ai/login".to_string(),
            token_endpoint: "https://claude.ai/api/organizations".to_string(),
            cookie_names: vec!["sessionKey".to_string()],
            header_name: "Cookie".to_string(),
            refresh_interval_secs: 3600,
        }
    }

    /// 创建 Copilot Web Portal 配置。
    #[must_use]
    pub fn copilot_web() -> Self {
        Self {
            portal_url: "https://github.com/copilot".to_string(),
            login_url: "https://github.com/login".to_string(),
            token_endpoint: "https://api.github.com/copilot_internal/v2/token".to_string(),
            cookie_names: vec!["_gh_sess".to_string(), "user_session".to_string()],
            header_name: "Cookie".to_string(),
            refresh_interval_secs: 1800,
        }
    }

    /// 创建自定义 Portal 配置。
    #[must_use]
    pub fn custom(
        portal_url: impl Into<String>,
        login_url: impl Into<String>,
        token_endpoint: impl Into<String>,
    ) -> Self {
        Self {
            portal_url: portal_url.into(),
            login_url: login_url.into(),
            token_endpoint: token_endpoint.into(),
            cookie_names: Vec::new(),
            header_name: "Cookie".to_string(),
            refresh_interval_secs: 3600,
        }
    }
}

#[derive(Debug, Clone)]
struct CachedToken {
    token: String,
    fetched_at: std::time::Instant,
    expires_in_secs: u64,
}

impl CachedToken {
    fn is_valid(&self) -> bool {
        self.fetched_at.elapsed().as_secs() < self.expires_in_secs.saturating_sub(60)
    }
}

/// Portal 认证提供者，通过浏览器端点获取认证令牌。
pub struct PortalAuthProvider {
    config: PortalAuthConfig,
    client: Client,
    cache: RwLock<HashMap<String, CachedToken>>,
    env_fallback: crate::credential::EnvCredentialProvider,
}

impl PortalAuthProvider {
    /// # Errors
    ///
    /// 当 HTTP 客户端构建失败时返回 `LlmError::HttpClientInit`。
    pub fn new(config: PortalAuthConfig) -> Result<Self, LlmError> {
        let client = build_http_client_with_cookies(std::time::Duration::from_secs(30))?;
        Ok(Self {
            config,
            client,
            cache: RwLock::new(HashMap::new()),
            env_fallback: crate::credential::EnvCredentialProvider::new(),
        })
    }

    /// 设置环境变量凭证作为回退。
    #[must_use]
    pub fn with_env_fallback(mut self, fallback: crate::credential::EnvCredentialProvider) -> Self {
        self.env_fallback = fallback;
        self
    }

    /// 返回 Portal 认证配置的引用。
    #[must_use]
    pub fn config(&self) -> &PortalAuthConfig {
        &self.config
    }

    async fn fetch_token_from_portal(&self, provider_id: &str) -> Result<String, LlmError> {
        let response = self
            .client
            .get(&self.config.token_endpoint)
            .send()
            .await
            .map_err(|e| LlmError::Network(format!("Portal auth request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(LlmError::AuthenticationError {
                message: format!(
                    "Portal auth failed for {provider_id}: HTTP {}",
                    response.status()
                ),
            });
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| LlmError::Other(format!("Failed to parse portal auth response: {e}")))?;

        let token = body
            .get("token")
            .or_else(|| body.get("access_token"))
            .or_else(|| body.get("api_key"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| LlmError::AuthenticationError {
                message: format!("No token found in portal response for {provider_id}"),
            })?;

        let expires_in = body
            .get("expires_in")
            .or_else(|| body.get("expires_at"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(self.config.refresh_interval_secs);

        self.cache.write().insert(
            provider_id.to_string(),
            CachedToken {
                token: token.to_string(),
                fetched_at: std::time::Instant::now(),
                expires_in_secs: expires_in,
            },
        );

        Ok(token.to_string())
    }
}

#[async_trait]
impl CredentialsProvider for PortalAuthProvider {
    async fn get_api_key(&self, provider_id: &str) -> Result<String, LlmError> {
        if let Some(cached) = self.cache.read().get(provider_id)
            && cached.is_valid()
        {
            return Ok(cached.token.clone());
        }

        match self.fetch_token_from_portal(provider_id).await {
            Ok(token) => Ok(token),
            Err(_) => self.env_fallback.get_api_key(provider_id).await,
        }
    }

    async fn state(&self, provider_id: &str) -> ApiKeyState {
        if let Some(cached) = self.cache.read().get(provider_id) {
            if cached.is_valid() {
                return ApiKeyState::Valid;
            }
            return ApiKeyState::NeedsRefresh;
        }
        match self.env_fallback.state(provider_id).await {
            ApiKeyState::Valid => ApiKeyState::Valid,
            ApiKeyState::Missing | ApiKeyState::Invalid | ApiKeyState::NeedsRefresh => {
                ApiKeyState::Missing
            }
        }
    }

    async fn invalidate(&self, provider_id: &str) {
        self.cache.write().remove(provider_id);
        self.env_fallback.invalidate(provider_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portal_auth_config_claude() {
        let config = PortalAuthConfig::claude_web();
        assert_eq!(config.portal_url, "https://claude.ai");
        assert_eq!(config.cookie_names.len(), 1);
    }

    #[test]
    fn test_portal_auth_config_copilot() {
        let config = PortalAuthConfig::copilot_web();
        assert_eq!(config.cookie_names.len(), 2);
    }

    #[test]
    fn test_portal_auth_config_custom() {
        let config = PortalAuthConfig::custom("https://example.com", "/login", "/api/token");
        assert_eq!(config.portal_url, "https://example.com");
    }

    #[test]
    fn test_cached_token_valid() {
        let cached = CachedToken {
            token: "test".to_string(),
            fetched_at: std::time::Instant::now(),
            expires_in_secs: 3600,
        };
        assert!(cached.is_valid());
    }

    #[test]
    fn test_portal_auth_provider_new() {
        let provider = PortalAuthProvider::new(PortalAuthConfig::claude_web())
            .expect("HTTP client build should not fail in test");
        assert_eq!(provider.config().portal_url, "https://claude.ai");
    }

    #[tokio::test]
    async fn test_portal_auth_provider_missing() {
        let provider = PortalAuthProvider::new(PortalAuthConfig::claude_web())
            .expect("HTTP client build should not fail in test");
        let result = provider.get_api_key("nonexistent_portal_xyz").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_cached_token_expired() {
        let cached = CachedToken {
            token: "test".to_string(),
            fetched_at: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(7200))
                .expect("Instant subtraction should not overflow"),
            expires_in_secs: 3600,
        };
        assert!(!cached.is_valid());
    }

    #[test]
    fn test_cached_token_zero_expiry() {
        let cached = CachedToken {
            token: "test".to_string(),
            fetched_at: std::time::Instant::now(),
            expires_in_secs: 0,
        };
        assert!(!cached.is_valid());
    }

    #[tokio::test]
    async fn test_portal_auth_provider_invalidate() {
        let provider = PortalAuthProvider::new(PortalAuthConfig::claude_web())
            .expect("HTTP client build should not fail in test");
        provider.invalidate("claude").await;
        assert_eq!(provider.state("claude").await, ApiKeyState::Missing);
    }

    #[tokio::test]
    async fn test_portal_auth_provider_state_missing() {
        let provider = PortalAuthProvider::new(PortalAuthConfig::claude_web())
            .expect("HTTP client build should not fail in test");
        assert_eq!(provider.state("claude").await, ApiKeyState::Missing);
    }

    #[test]
    fn test_portal_auth_config_default_fields() {
        let config = PortalAuthConfig::claude_web();
        assert!(!config.portal_url.is_empty());
        assert!(!config.cookie_names.is_empty());
    }

    #[test]
    fn test_portal_auth_config_copilot_fields() {
        let config = PortalAuthConfig::copilot_web();
        assert!(!config.portal_url.is_empty());
        assert!(!config.cookie_names.is_empty());
    }
}
