use std::collections::HashMap;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::error::LlmError;

/// 凭证别名提供者
pub mod alias;
/// Portal 浏览器认证
pub mod portal;
/// Portal 认证策略与令牌管理
pub mod portal_auth;

/// API Key 状态枚举。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeyState {
    /// 凭证有效
    Valid,
    /// 凭证缺失
    Missing,
    /// 凭证无效
    Invalid,
    /// 凭证需要刷新
    NeedsRefresh,
}

/// 凭证提供者 trait，定义获取与刷新 API Key 的统一接口。
#[async_trait]
pub trait CredentialsProvider: Send + Sync {
    /// 获取指定供应商的 API Key。
    ///
    /// # Errors
    ///
    /// 若凭证缺失返回 `LlmError::MissingCredentials`；若凭证过期返回 `LlmError::ExpiredToken`。
    async fn get_api_key(&self, provider_id: &str) -> Result<String, LlmError>;
    /// 查询指定供应商的凭证状态。
    async fn state(&self, provider_id: &str) -> ApiKeyState;
    /// 使指定供应商的缓存凭证失效。
    async fn invalidate(&self, provider_id: &str);
}

/// 环境变量凭证提供者，从 `PROVIDER_API_KEY` 等环境变量读取 API Key。
pub struct EnvCredentialProvider {
    cache: RwLock<HashMap<String, String>>,
}

impl EnvCredentialProvider {
    /// 创建新的环境变量凭证提供者。
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    fn env_var_names(provider_id: &str) -> Vec<String> {
        let upper = provider_id.to_uppercase().replace('-', "_");
        vec![
            format!("{}_API_KEY", upper),
            format!("{}_API_KEY", upper.replace('_', "")),
            provider_id.replace('-', "_").to_uppercase(),
        ]
    }
}

impl Default for EnvCredentialProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CredentialsProvider for EnvCredentialProvider {
    async fn get_api_key(&self, provider_id: &str) -> Result<String, LlmError> {
        if let Some(key) = self.cache.read().get(provider_id).cloned() {
            return Ok(key);
        }
        let env_vars = Self::env_var_names(provider_id);
        for var in &env_vars {
            if let Ok(val) = std::env::var(var)
                && !val.is_empty()
            {
                self.cache
                    .write()
                    .insert(provider_id.to_string(), val.clone());
                return Ok(val);
            }
        }
        Err(LlmError::MissingCredentials {
            provider: provider_id.to_string(),
            env_vars,
        })
    }

    async fn state(&self, provider_id: &str) -> ApiKeyState {
        match self.get_api_key(provider_id).await {
            Ok(_) => ApiKeyState::Valid,
            Err(LlmError::MissingCredentials { .. }) => ApiKeyState::Missing,
            Err(
                LlmError::Network(_)
                | LlmError::Timeout(_)
                | LlmError::RateLimitExceeded { .. }
                | LlmError::ServerOverloaded
                | LlmError::AuthenticationError { .. }
                | LlmError::PermissionError { .. }
                | LlmError::ContextWindowExceeded { .. }
                | LlmError::PromptTooLarge(_)
                | LlmError::ModelUnavailable(_)
                | LlmError::InvalidRequest { .. }
                | LlmError::StreamError(_)
                | LlmError::ToolCallError { .. }
                | LlmError::ExpiredToken(_)
                | LlmError::JsonParse { .. }
                | LlmError::RetriesExhausted { .. }
                | LlmError::Config(_)
                | LlmError::RequestBodySizeExceeded { .. }
                | LlmError::Cancelled
                | LlmError::HttpClientInit(_)
                | LlmError::Other(_),
            ) => ApiKeyState::Invalid,
        }
    }

    async fn invalidate(&self, provider_id: &str) {
        self.cache.write().remove(provider_id);
    }
}

/// 配置文件凭证提供者，从预加载的 Key 映射中查找 API Key。
pub struct ConfigCredentialProvider {
    keys: RwLock<HashMap<String, String>>,
}

impl ConfigCredentialProvider {
    /// 创建新的配置文件凭证提供者。
    #[must_use]
    pub fn new(keys: HashMap<String, String>) -> Self {
        Self {
            keys: RwLock::new(keys),
        }
    }
}

#[async_trait]
impl CredentialsProvider for ConfigCredentialProvider {
    async fn get_api_key(&self, provider_id: &str) -> Result<String, LlmError> {
        self.keys
            .read()
            .get(provider_id)
            .cloned()
            .ok_or_else(|| LlmError::MissingCredentials {
                provider: provider_id.to_string(),
                env_vars: vec![],
            })
    }

    async fn state(&self, provider_id: &str) -> ApiKeyState {
        match self.keys.read().get(provider_id) {
            Some(_) => ApiKeyState::Valid,
            None => ApiKeyState::Missing,
        }
    }

    async fn invalidate(&self, provider_id: &str) {
        self.keys.write().remove(provider_id);
    }
}

/// OAuth 令牌集，包含访问令牌、刷新令牌和过期时间。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenSet {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌
    pub refresh_token: Option<String>,
    /// 过期时间（UNIX 秒）
    pub expires_at: Option<u64>,
}

/// OAuth 凭证提供者，管理 OAuth 令牌并回退到环境变量。
pub struct OAuthCredentialProvider {
    tokens: RwLock<HashMap<String, OAuthTokenSet>>,
    env_fallback: EnvCredentialProvider,
}

impl OAuthCredentialProvider {
    /// 创建新的 OAuth 凭证提供者。
    #[must_use]
    pub fn new() -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
            env_fallback: EnvCredentialProvider::new(),
        }
    }
}

impl Default for OAuthCredentialProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CredentialsProvider for OAuthCredentialProvider {
    async fn get_api_key(&self, provider_id: &str) -> Result<String, LlmError> {
        if let Some(token) = self.tokens.read().get(provider_id) {
            if let Some(expires) = token.expires_at {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock set before UNIX epoch — environment is broken")
                    .as_secs();
                if now >= expires {
                    return Err(LlmError::ExpiredToken(provider_id.to_string()));
                }
            }
            return Ok(token.access_token.clone());
        }
        self.env_fallback.get_api_key(provider_id).await
    }

    async fn state(&self, provider_id: &str) -> ApiKeyState {
        if let Some(token) = self.tokens.read().get(provider_id) {
            if let Some(expires) = token.expires_at {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock set before UNIX epoch — environment is broken")
                    .as_secs();
                if now >= expires {
                    return ApiKeyState::NeedsRefresh;
                }
            }
            return ApiKeyState::Valid;
        }
        self.env_fallback.state(provider_id).await
    }

    async fn invalidate(&self, provider_id: &str) {
        self.tokens.write().remove(provider_id);
        self.env_fallback.invalidate(provider_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_env_credential_provider_missing() {
        let provider = EnvCredentialProvider::new();
        let result = provider.get_api_key("nonexistent_provider_xyz").await;
        assert!(matches!(result, Err(LlmError::MissingCredentials { .. })));
        assert_eq!(
            provider.state("nonexistent_provider_xyz").await,
            ApiKeyState::Missing
        );
    }

    #[tokio::test]
    async fn test_env_credential_provider_invalidate() {
        let provider = EnvCredentialProvider::new();
        provider
            .cache
            .write()
            .insert("test".to_string(), "key123".to_string());
        assert_eq!(provider.state("test").await, ApiKeyState::Valid);
        provider.invalidate("test").await;
        assert_eq!(provider.state("test").await, ApiKeyState::Missing);
    }

    #[tokio::test]
    async fn test_config_credential_provider() {
        let mut keys = HashMap::new();
        keys.insert("openai".to_string(), "sk-test123".to_string());
        let provider = ConfigCredentialProvider::new(keys);

        let key = provider.get_api_key("openai").await.unwrap();
        assert_eq!(key, "sk-test123");
        assert_eq!(provider.state("openai").await, ApiKeyState::Valid);
        assert_eq!(provider.state("anthropic").await, ApiKeyState::Missing);
    }

    #[tokio::test]
    async fn test_config_credential_provider_invalidate() {
        let mut keys = HashMap::new();
        keys.insert("openai".to_string(), "sk-test123".to_string());
        let provider = ConfigCredentialProvider::new(keys);

        provider.invalidate("openai").await;
        assert_eq!(provider.state("openai").await, ApiKeyState::Missing);
    }

    #[tokio::test]
    async fn test_oauth_credential_provider_valid() {
        let provider = OAuthCredentialProvider::new();
        let future_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        provider.tokens.write().insert(
            "google".to_string(),
            OAuthTokenSet {
                access_token: "ya29.token".to_string(),
                refresh_token: Some("refresh".to_string()),
                expires_at: Some(future_ts),
            },
        );

        let key = provider.get_api_key("google").await.unwrap();
        assert_eq!(key, "ya29.token");
        assert_eq!(provider.state("google").await, ApiKeyState::Valid);
    }

    #[tokio::test]
    async fn test_oauth_credential_provider_expired() {
        let provider = OAuthCredentialProvider::new();
        let past_ts = 1000u64;
        provider.tokens.write().insert(
            "google".to_string(),
            OAuthTokenSet {
                access_token: "expired".to_string(),
                refresh_token: None,
                expires_at: Some(past_ts),
            },
        );

        let result = provider.get_api_key("google").await;
        assert!(matches!(result, Err(LlmError::ExpiredToken(_))));
        assert_eq!(provider.state("google").await, ApiKeyState::NeedsRefresh);
    }

    #[tokio::test]
    async fn test_oauth_credential_provider_fallback() {
        let provider = OAuthCredentialProvider::new();
        let result = provider.get_api_key("nonexistent_xyz").await;
        assert!(matches!(result, Err(LlmError::MissingCredentials { .. })));
    }

    #[test]
    fn test_env_var_names() {
        let names = EnvCredentialProvider::env_var_names("openai");
        assert_eq!(names[0], "OPENAI_API_KEY");
        assert_eq!(names[2], "OPENAI");

        let names = EnvCredentialProvider::env_var_names("my-provider");
        assert_eq!(names[0], "MY_PROVIDER_API_KEY");
        assert_eq!(names[1], "MYPROVIDER_API_KEY");
        assert_eq!(names[2], "MY_PROVIDER");
    }

    #[test]
    fn test_oauth_token_set_serialize() {
        let token = OAuthTokenSet {
            access_token: "abc".to_string(),
            refresh_token: Some("def".to_string()),
            expires_at: Some(12345),
        };
        let json = serde_json::to_string(&token).unwrap();
        let deserialized: OAuthTokenSet = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.access_token, "abc");
    }

    #[tokio::test]
    async fn test_oauth_credential_provider_no_expires_at() {
        let provider = OAuthCredentialProvider::new();
        provider.tokens.write().insert(
            "custom".to_string(),
            OAuthTokenSet {
                access_token: "no_expiry".to_string(),
                refresh_token: None,
                expires_at: None,
            },
        );
        let key = provider.get_api_key("custom").await.unwrap();
        assert_eq!(key, "no_expiry");
        assert_eq!(provider.state("custom").await, ApiKeyState::Valid);
    }

    #[tokio::test]
    async fn test_oauth_credential_provider_invalidate() {
        let provider = OAuthCredentialProvider::new();
        let future_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        provider.tokens.write().insert(
            "google".to_string(),
            OAuthTokenSet {
                access_token: "ya29.token".to_string(),
                refresh_token: Some("refresh".to_string()),
                expires_at: Some(future_ts),
            },
        );
        assert_eq!(provider.state("google").await, ApiKeyState::Valid);
        provider.invalidate("google").await;
        assert_eq!(provider.state("google").await, ApiKeyState::Missing);
    }

    #[tokio::test]
    async fn test_env_credential_provider_with_cache() {
        let provider = EnvCredentialProvider::new();
        provider
            .cache
            .write()
            .insert("cached_provider".to_string(), "cached_key".to_string());
        let key = provider.get_api_key("cached_provider").await.unwrap();
        assert_eq!(key, "cached_key");
    }

    #[tokio::test]
    async fn test_config_credential_provider_missing_key() {
        let keys = HashMap::new();
        let provider = ConfigCredentialProvider::new(keys);
        let result = provider.get_api_key("nonexistent").await;
        assert!(matches!(result, Err(LlmError::MissingCredentials { .. })));
    }

    #[tokio::test]
    async fn test_config_credential_provider_invalidate_missing() {
        let keys = HashMap::new();
        let provider = ConfigCredentialProvider::new(keys);
        provider.invalidate("nonexistent").await;
        assert_eq!(provider.state("nonexistent").await, ApiKeyState::Missing);
    }
}
