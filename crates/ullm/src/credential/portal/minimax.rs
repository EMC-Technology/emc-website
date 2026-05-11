use async_trait::async_trait;
use reqwest::Client;

use crate::build_http_client_with_cookies;
use crate::credential::portal_auth::{PortalAuthStrategy, PortalAuthToken, PortalAuthType};
use crate::error::LlmError;

/// `MiniMax` Portal 认证策略，通过 `MiniMax` Web 端点获取认证令牌。
pub struct MiniMaxPortalAuth {
    client: Client,
}

impl MiniMaxPortalAuth {
    /// # Errors
    ///
    /// 当 HTTP 客户端构建失败时返回 `LlmError::HttpClientInit`。
    pub fn new() -> Result<Self, LlmError> {
        let client = build_http_client_with_cookies(std::time::Duration::from_secs(30))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl PortalAuthStrategy for MiniMaxPortalAuth {
    fn name(&self) -> &'static str {
        "minimax-portal"
    }

    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError> {
        let response = self
            .client
            .get("https://hailuoai.com/api/chat")
            .send()
            .await
            .map_err(|e| LlmError::Network(format!("MiniMax portal auth request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(LlmError::AuthenticationError {
                message: format!("MiniMax portal auth failed: HTTP {}", response.status()),
            });
        }

        let body: serde_json::Value = response.json().await.map_err(|e| {
            LlmError::Other(format!("Failed to parse MiniMax portal auth response: {e}"))
        })?;

        let token = body
            .get("token")
            .or_else(|| body.get("access_token"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| LlmError::AuthenticationError {
                message: "No token found in MiniMax portal response".into(),
            })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(PortalAuthToken {
            access_token: token.to_string(),
            refresh_token: None,
            expires_at: Some(now + 3600),
            token_type: PortalAuthType::Cookie {
                domain: "hailuoai.com".into(),
            },
        })
    }

    async fn refresh(&self, _refresh_token: &str) -> Result<PortalAuthToken, LlmError> {
        Err(LlmError::AuthenticationError {
            message: "MiniMax portal auth does not support refresh".into(),
        })
    }

    fn auth_type(&self) -> PortalAuthType {
        PortalAuthType::Cookie {
            domain: "hailuoai.com".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimax_portal_auth_name() {
        let auth = MiniMaxPortalAuth::new().expect("HTTP client build should not fail in test");
        assert_eq!(auth.name(), "minimax-portal");
    }

    #[test]
    fn test_minimax_portal_auth_type() {
        let auth = MiniMaxPortalAuth::new().expect("HTTP client build should not fail in test");
        match auth.auth_type() {
            PortalAuthType::Cookie { domain } => assert_eq!(domain, "hailuoai.com"),
            _ => panic!("expected Cookie auth type"),
        }
    }
}
