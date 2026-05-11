use std::sync::Arc;

use async_trait::async_trait;

use crate::credential::{ApiKeyState, CredentialsProvider, EnvCredentialProvider};
use crate::error::LlmError;

/// Portal 认证类型枚举。
#[derive(Debug, Clone)]
pub enum PortalAuthType {
    /// Cookie 认证
    Cookie {
        /// Cookie 域名
        domain: String,
    },
    /// Bearer Token 认证
    Bearer {
        /// HTTP 头名称
        header_name: String,
    },
    /// 自定义认证头
    Custom {
        /// HTTP 头名称
        header_name: String,
        /// 值前缀（如 `Bearer `）
        prefix: String,
    },
}

/// Portal 认证令牌。
#[derive(Debug, Clone)]
pub struct PortalAuthToken {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌
    pub refresh_token: Option<String>,
    /// 过期时间（UNIX 秒）
    pub expires_at: Option<u64>,
    /// 令牌认证类型
    pub token_type: PortalAuthType,
}

impl PortalAuthToken {
    /// # Panics
    ///
    /// 若系统时钟设置在 UNIX 纪元之前则 panic（环境异常）。
    #[must_use]
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock set before UNIX epoch — environment is broken")
                    .as_secs();
                now >= expires
            }
            None => false,
        }
    }

    /// # Panics
    ///
    /// 若系统时钟设置在 UNIX 纪元之前则 panic（环境异常）。
    #[must_use]
    pub fn remaining_secs(&self) -> Option<u64> {
        self.expires_at.map(|expires| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock set before UNIX epoch — environment is broken")
                .as_secs();
            expires.saturating_sub(now)
        })
    }
}

/// Portal 认证策略 trait，定义认证与刷新流程。
#[async_trait]
pub trait PortalAuthStrategy: Send + Sync {
    /// 返回策略名称。
    fn name(&self) -> &'static str;
    /// 执行认证流程，获取新的访问令牌。
    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError>;
    /// 使用刷新令牌获取新的访问令牌。
    async fn refresh(&self, refresh_token: &str) -> Result<PortalAuthToken, LlmError>;
    /// 返回该策略使用的认证类型。
    fn auth_type(&self) -> PortalAuthType;
}

/// 基于 Portal 认证的凭证提供者，支持令牌缓存与自动刷新。
pub struct PortalAuthCredentialProvider {
    strategy: Arc<dyn PortalAuthStrategy>,
    token: tokio::sync::RwLock<Option<PortalAuthToken>>,
    env_fallback: EnvCredentialProvider,
}

impl PortalAuthCredentialProvider {
    /// 创建新的 Portal 认证凭证提供者。
    #[must_use]
    pub fn new(strategy: Arc<dyn PortalAuthStrategy>) -> Self {
        Self {
            strategy,
            token: tokio::sync::RwLock::new(None),
            env_fallback: EnvCredentialProvider::new(),
        }
    }

    /// 设置环境变量凭证作为回退。
    #[must_use]
    pub fn with_env_fallback(mut self, fallback: EnvCredentialProvider) -> Self {
        self.env_fallback = fallback;
        self
    }

    /// 返回认证策略名称。
    #[must_use]
    pub fn strategy_name(&self) -> &'static str {
        self.strategy.name()
    }
}

#[async_trait]
impl CredentialsProvider for PortalAuthCredentialProvider {
    async fn get_api_key(&self, provider_id: &str) -> Result<String, LlmError> {
        let cached = {
            let guard = self.token.read().await;
            guard.as_ref().and_then(|t| {
                if let Some(expires) = t.expires_at {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("system clock set before UNIX epoch — environment is broken")
                        .as_secs();
                    if now < expires {
                        Some(t.access_token.clone())
                    } else {
                        None
                    }
                } else {
                    Some(t.access_token.clone())
                }
            })
        };
        if let Some(access) = cached {
            return Ok(access);
        }

        let refresh_token = {
            let guard = self.token.read().await;
            guard.as_ref().and_then(|t| t.refresh_token.clone())
        };
        if let Some(refresh) = refresh_token {
            match self.strategy.refresh(&refresh).await {
                Ok(new_token) => {
                    let access = new_token.access_token.clone();
                    *self.token.write().await = Some(new_token);
                    return Ok(access);
                }
                Err(_) => {
                    *self.token.write().await = None;
                }
            }
        }

        if let Ok(key) = self.env_fallback.get_api_key(provider_id).await {
            return Ok(key);
        }

        let new_token = self.strategy.authenticate().await?;
        let access = new_token.access_token.clone();
        *self.token.write().await = Some(new_token);
        Ok(access)
    }

    async fn state(&self, provider_id: &str) -> ApiKeyState {
        match self.get_api_key(provider_id).await {
            Ok(_) => ApiKeyState::Valid,
            Err(LlmError::ExpiredToken(_)) => ApiKeyState::NeedsRefresh,
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
        *self.token.write().await = None;
        self.env_fallback.invalidate(provider_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPortalStrategy {
        auth_token: String,
        should_fail: bool,
    }

    impl MockPortalStrategy {
        fn new(auth_token: &str) -> Self {
            Self {
                auth_token: auth_token.to_string(),
                should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                auth_token: String::new(),
                should_fail: true,
            }
        }
    }

    #[async_trait]
    impl PortalAuthStrategy for MockPortalStrategy {
        fn name(&self) -> &'static str {
            "mock-portal"
        }

        async fn authenticate(&self) -> Result<PortalAuthToken, LlmError> {
            if self.should_fail {
                return Err(LlmError::AuthenticationError {
                    message: "Mock auth failed".into(),
                });
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Ok(PortalAuthToken {
                access_token: self.auth_token.clone(),
                refresh_token: Some("mock_refresh".into()),
                expires_at: Some(now + 3600),
                token_type: PortalAuthType::Bearer {
                    header_name: "Authorization".into(),
                },
            })
        }

        async fn refresh(&self, _refresh_token: &str) -> Result<PortalAuthToken, LlmError> {
            if self.should_fail {
                return Err(LlmError::AuthenticationError {
                    message: "Mock refresh failed".into(),
                });
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Ok(PortalAuthToken {
                access_token: format!("refreshed_{}", self.auth_token),
                refresh_token: Some("mock_refresh_new".into()),
                expires_at: Some(now + 3600),
                token_type: PortalAuthType::Bearer {
                    header_name: "Authorization".into(),
                },
            })
        }

        fn auth_type(&self) -> PortalAuthType {
            PortalAuthType::Bearer {
                header_name: "Authorization".into(),
            }
        }
    }

    #[tokio::test]
    async fn test_portal_auth_credential_provider_authenticate() {
        let strategy = Arc::new(MockPortalStrategy::new("test_token_123"));
        let provider = PortalAuthCredentialProvider::new(strategy);
        let key = provider.get_api_key("test-provider").await.unwrap();
        assert_eq!(key, "test_token_123");
    }

    #[tokio::test]
    async fn test_portal_auth_credential_provider_cache() {
        let strategy = Arc::new(MockPortalStrategy::new("cached_token"));
        let provider = PortalAuthCredentialProvider::new(strategy);
        let key1 = provider.get_api_key("test-provider").await.unwrap();
        let key2 = provider.get_api_key("test-provider").await.unwrap();
        assert_eq!(key1, key2);
    }

    #[tokio::test]
    async fn test_portal_auth_credential_provider_invalidate() {
        let strategy = Arc::new(MockPortalStrategy::new("token_to_invalidate"));
        let provider = PortalAuthCredentialProvider::new(strategy);
        let key = provider.get_api_key("test-provider").await.unwrap();
        assert_eq!(key, "token_to_invalidate");
        provider.invalidate("test-provider").await;
        let guard = provider.token.read().await;
        assert!(guard.is_none());
    }

    #[tokio::test]
    async fn test_portal_auth_credential_provider_failing_strategy() {
        let strategy = Arc::new(MockPortalStrategy::failing());
        let provider = PortalAuthCredentialProvider::new(strategy);
        let result = provider.get_api_key("nonexistent_xyz").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_portal_auth_credential_provider_state_valid() {
        let strategy = Arc::new(MockPortalStrategy::new("valid_token"));
        let provider = PortalAuthCredentialProvider::new(strategy);
        provider.get_api_key("test-provider").await.unwrap();
        assert_eq!(provider.state("test-provider").await, ApiKeyState::Valid);
    }

    #[test]
    fn test_portal_auth_type_bearer() {
        let auth_type = PortalAuthType::Bearer {
            header_name: "Authorization".into(),
        };
        match auth_type {
            PortalAuthType::Bearer { header_name } => assert_eq!(header_name, "Authorization"),
            _ => panic!("expected Bearer"),
        }
    }

    #[test]
    fn test_portal_auth_type_cookie() {
        let auth_type = PortalAuthType::Cookie {
            domain: "example.com".into(),
        };
        match auth_type {
            PortalAuthType::Cookie { domain } => assert_eq!(domain, "example.com"),
            _ => panic!("expected Cookie"),
        }
    }

    #[test]
    fn test_portal_auth_type_custom() {
        let auth_type = PortalAuthType::Custom {
            header_name: "X-Access-Token".into(),
            prefix: String::new(),
        };
        match auth_type {
            PortalAuthType::Custom {
                header_name,
                prefix,
            } => {
                assert_eq!(header_name, "X-Access-Token");
                assert_eq!(prefix, "");
            }
            _ => panic!("expected Custom"),
        }
    }

    #[test]
    fn test_portal_auth_token_fields() {
        let token = PortalAuthToken {
            access_token: "abc".into(),
            refresh_token: Some("def".into()),
            expires_at: Some(99999),
            token_type: PortalAuthType::Bearer {
                header_name: "Authorization".into(),
            },
        };
        assert_eq!(token.access_token, "abc");
        assert_eq!(token.refresh_token, Some("def".into()));
        assert_eq!(token.expires_at, Some(99999));
    }

    #[test]
    fn test_portal_auth_token_is_expired_no_expires_at() {
        let token = PortalAuthToken {
            access_token: "abc".into(),
            refresh_token: None,
            expires_at: None,
            token_type: PortalAuthType::Bearer {
                header_name: "Authorization".into(),
            },
        };
        assert!(!token.is_expired());
    }

    #[test]
    fn test_portal_auth_token_is_expired_future() {
        let future_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        let token = PortalAuthToken {
            access_token: "abc".into(),
            refresh_token: None,
            expires_at: Some(future_ts),
            token_type: PortalAuthType::Bearer {
                header_name: "Authorization".into(),
            },
        };
        assert!(!token.is_expired());
    }

    #[test]
    fn test_portal_auth_token_is_expired_past() {
        let token = PortalAuthToken {
            access_token: "abc".into(),
            refresh_token: None,
            expires_at: Some(1000),
            token_type: PortalAuthType::Bearer {
                header_name: "Authorization".into(),
            },
        };
        assert!(token.is_expired());
    }

    #[test]
    fn test_portal_auth_token_remaining_secs_no_expires_at() {
        let token = PortalAuthToken {
            access_token: "abc".into(),
            refresh_token: None,
            expires_at: None,
            token_type: PortalAuthType::Bearer {
                header_name: "Authorization".into(),
            },
        };
        assert!(token.remaining_secs().is_none());
    }

    #[test]
    fn test_portal_auth_token_remaining_secs_future() {
        let future_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        let token = PortalAuthToken {
            access_token: "abc".into(),
            refresh_token: None,
            expires_at: Some(future_ts),
            token_type: PortalAuthType::Bearer {
                header_name: "Authorization".into(),
            },
        };
        let remaining = token.remaining_secs().unwrap();
        assert!(remaining > 3500 && remaining <= 3600);
    }

    #[test]
    fn test_portal_auth_token_remaining_secs_past() {
        let token = PortalAuthToken {
            access_token: "abc".into(),
            refresh_token: None,
            expires_at: Some(1000),
            token_type: PortalAuthType::Bearer {
                header_name: "Authorization".into(),
            },
        };
        assert_eq!(token.remaining_secs(), Some(0));
    }

    #[tokio::test]
    async fn test_portal_auth_credential_provider_expired_with_refresh() {
        let strategy = Arc::new(MockPortalStrategy::new("refreshed_token"));
        let provider = PortalAuthCredentialProvider::new(strategy);
        let past_ts: u64 = 1000;
        {
            let mut token = provider.token.write().await;
            *token = Some(PortalAuthToken {
                access_token: "expired_old".into(),
                refresh_token: Some("refresh_tok".into()),
                expires_at: Some(past_ts),
                token_type: PortalAuthType::Bearer {
                    header_name: "Authorization".into(),
                },
            });
        }
        let key = provider.get_api_key("test-provider").await.unwrap();
        assert_eq!(key, "refreshed_refreshed_token");
    }

    #[tokio::test]
    async fn test_portal_auth_credential_provider_state_needs_refresh_then_valid() {
        let strategy = Arc::new(MockPortalStrategy::new("token"));
        let provider = PortalAuthCredentialProvider::new(strategy);
        let past_ts: u64 = 1000;
        {
            let mut token = provider.token.write().await;
            *token = Some(PortalAuthToken {
                access_token: "expired".into(),
                refresh_token: None,
                expires_at: Some(past_ts),
                token_type: PortalAuthType::Bearer {
                    header_name: "Authorization".into(),
                },
            });
        }
        assert_eq!(provider.state("test-provider").await, ApiKeyState::Valid);
    }

    #[tokio::test]
    async fn test_portal_auth_credential_provider_state_missing_then_valid() {
        let strategy = Arc::new(MockPortalStrategy::new("token"));
        let provider = PortalAuthCredentialProvider::new(strategy);
        assert_eq!(provider.state("test-provider").await, ApiKeyState::Valid);
    }
}
