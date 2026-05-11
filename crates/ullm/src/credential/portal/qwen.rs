use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;

use crate::build_http_client_with_cookies;
use crate::credential::portal_auth::{PortalAuthStrategy, PortalAuthToken, PortalAuthType};
use crate::error::LlmError;

/// 通义千问 Portal 认证策略，通过通义千问 Web 端点获取认证令牌。
pub struct QwenPortalAuth {
    client: Client,
    #[allow(dead_code)]
    cookie_jar: Arc<reqwest::cookie::Jar>,
}

impl QwenPortalAuth {
    /// 创建新的 Qwen 门户认证实例。
    ///
    /// # Errors
    ///
    /// 当 HTTP 客户端构建失败时返回 `LlmError::HttpClientInit`。
    pub fn new() -> Result<Self, LlmError> {
        let cookie_jar = Arc::new(reqwest::cookie::Jar::default());
        let client = build_http_client_with_cookies(std::time::Duration::from_secs(30))?;
        Ok(Self { client, cookie_jar })
    }
}

#[async_trait]
impl PortalAuthStrategy for QwenPortalAuth {
    fn name(&self) -> &'static str {
        "qwen-portal"
    }

    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError> {
        let response = self
            .client
            .get("https://tongyi.aliyun.com/api/chat")
            .send()
            .await
            .map_err(|e| LlmError::Network(format!("Qwen portal auth request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(LlmError::AuthenticationError {
                message: format!("Qwen portal auth failed: HTTP {}", response.status()),
            });
        }

        let body: serde_json::Value = response.json().await.map_err(|e| {
            LlmError::Other(format!("Failed to parse Qwen portal auth response: {e}"))
        })?;

        let token = body
            .get("token")
            .or_else(|| body.get("access_token"))
            .or_else(|| body.get("data").and_then(|d| d.get("accessToken")))
            .and_then(|v| v.as_str())
            .ok_or_else(|| LlmError::AuthenticationError {
                message: "No token found in Qwen portal response".into(),
            })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(PortalAuthToken {
            access_token: token.to_string(),
            refresh_token: None,
            expires_at: Some(now + 3600),
            token_type: PortalAuthType::Custom {
                header_name: "X-Access-Token".into(),
                prefix: String::new(),
            },
        })
    }

    async fn refresh(&self, _refresh_token: &str) -> Result<PortalAuthToken, LlmError> {
        Err(LlmError::AuthenticationError {
            message: "Qwen portal auth does not support refresh".into(),
        })
    }

    fn auth_type(&self) -> PortalAuthType {
        PortalAuthType::Custom {
            header_name: "X-Access-Token".into(),
            prefix: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qwen_portal_auth_name() {
        let auth = QwenPortalAuth::new().expect("HTTP client build should not fail in test");
        assert_eq!(auth.name(), "qwen-portal");
    }

    #[test]
    fn test_qwen_portal_auth_type() {
        let auth = QwenPortalAuth::new().expect("HTTP client build should not fail in test");
        match auth.auth_type() {
            PortalAuthType::Custom {
                header_name,
                prefix,
            } => {
                assert_eq!(header_name, "X-Access-Token");
                assert_eq!(prefix, "");
            }
            _ => panic!("expected Custom auth type"),
        }
    }
}
