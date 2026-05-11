use async_trait::async_trait;
use std::sync::Arc;
use ullm::credential::CredentialsProvider;
use ullm::credential::portal_auth::{
    PortalAuthCredentialProvider, PortalAuthStrategy, PortalAuthToken, PortalAuthType,
};
use ullm::error::LlmError;

struct FailingPortalStrategy;

#[async_trait]
impl PortalAuthStrategy for FailingPortalStrategy {
    fn name(&self) -> &'static str {
        "failing-test"
    }
    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError> {
        Err(LlmError::AuthenticationError {
            message: "intentional failure".into(),
        })
    }
    async fn refresh(&self, _refresh_token: &str) -> Result<PortalAuthToken, LlmError> {
        Err(LlmError::AuthenticationError {
            message: "refresh failed".into(),
        })
    }
    fn auth_type(&self) -> PortalAuthType {
        PortalAuthType::Bearer {
            header_name: "Authorization".into(),
        }
    }
}

#[tokio::test]
async fn test_portal_auth_failing_strategy_returns_error() {
    let strategy = Arc::new(FailingPortalStrategy);
    let provider = PortalAuthCredentialProvider::new(strategy);
    let result = provider.get_api_key("test-failing-portal").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_portal_auth_strategy_name() {
    let strategy = Arc::new(FailingPortalStrategy);
    let provider = PortalAuthCredentialProvider::new(strategy);
    assert_eq!(provider.strategy_name(), "failing-test");
}

struct ExpiringPortalStrategy;

#[async_trait]
impl PortalAuthStrategy for ExpiringPortalStrategy {
    fn name(&self) -> &'static str {
        "expiring-test"
    }
    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Ok(PortalAuthToken {
            access_token: "fresh_token".into(),
            refresh_token: None,
            expires_at: Some(now + 3600),
            token_type: PortalAuthType::Bearer {
                header_name: "Authorization".into(),
            },
        })
    }
    async fn refresh(&self, _refresh_token: &str) -> Result<PortalAuthToken, LlmError> {
        Err(LlmError::AuthenticationError {
            message: "no refresh".into(),
        })
    }
    fn auth_type(&self) -> PortalAuthType {
        PortalAuthType::Bearer {
            header_name: "Authorization".into(),
        }
    }
}

#[tokio::test]
async fn test_portal_auth_expiring_strategy_returns_token() {
    let strategy = Arc::new(ExpiringPortalStrategy);
    let provider = PortalAuthCredentialProvider::new(strategy);
    let key = provider.get_api_key("test-expiring-portal").await.unwrap();
    assert_eq!(key, "fresh_token");
}
