use async_trait::async_trait;
use reqwest::Client;

use crate::build_http_client;
use crate::credential::portal_auth::{PortalAuthStrategy, PortalAuthToken, PortalAuthType};
use crate::error::LlmError;

/// GitHub Copilot 代理认证策略，通过 GitHub Copilot 内部 Token 端点获取认证。
pub struct CopilotProxyAuth {
    client: Client,
}

impl CopilotProxyAuth {
    /// # Errors
    ///
    /// 当 HTTP 客户端构建失败时返回 `LlmError::HttpClientInit`。
    pub fn new() -> Result<Self, LlmError> {
        let client = build_http_client(std::time::Duration::from_secs(30))?;
        Ok(Self { client })
    }
}

fn copilot_config_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let app_data = std::env::var("APPDATA").ok()?;
        Some(std::path::PathBuf::from(app_data).join("github-copilot"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").ok()?;
        Some(
            std::path::PathBuf::from(home)
                .join(".config")
                .join("github-copilot"),
        )
    }
}

fn read_copilot_token() -> Option<String> {
    let dir = copilot_config_dir()?;
    let hosts_file = dir.join("hosts.json");
    let content = std::fs::read_to_string(&hosts_file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("github.com")
        .and_then(|v| v.get("oauth_token"))
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string)
}

#[async_trait]
impl PortalAuthStrategy for CopilotProxyAuth {
    fn name(&self) -> &'static str {
        "copilot-proxy"
    }

    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError> {
        if let Some(oauth_token) = read_copilot_token() {
            let response = self
                .client
                .get("https://api.github.com/copilot_internal/v2/token")
                .header("Authorization", format!("token {oauth_token}"))
                .header("User-Agent", "AI-IDE")
                .send()
                .await
                .map_err(|e| LlmError::Network(format!("Copilot token request failed: {e}")))?;

            if !response.status().is_success() {
                return Err(LlmError::AuthenticationError {
                    message: format!("Copilot token request failed: HTTP {}", response.status()),
                });
            }

            let body: serde_json::Value = response.json().await.map_err(|e| {
                LlmError::Other(format!("Failed to parse Copilot token response: {e}"))
            })?;

            let token = body.get("token").and_then(|v| v.as_str()).ok_or_else(|| {
                LlmError::AuthenticationError {
                    message: "No token in Copilot response".into(),
                }
            })?;

            let expires_at = body
                .get("expires_at")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_else(|| {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    now + 1800
                });

            return Ok(PortalAuthToken {
                access_token: token.to_string(),
                refresh_token: Some(oauth_token),
                expires_at: Some(expires_at),
                token_type: PortalAuthType::Bearer {
                    header_name: "Authorization".into(),
                },
            });
        }

        Err(LlmError::AuthenticationError {
            message: "Copilot auth: no cached OAuth token found. Install and sign in to GitHub Copilot extension first.".into(),
        })
    }

    async fn refresh(&self, oauth_token: &str) -> Result<PortalAuthToken, LlmError> {
        let response = self
            .client
            .get("https://api.github.com/copilot_internal/v2/token")
            .header("Authorization", format!("token {oauth_token}"))
            .header("User-Agent", "AI-IDE")
            .send()
            .await
            .map_err(|e| LlmError::Network(format!("Copilot token refresh failed: {e}")))?;

        if !response.status().is_success() {
            return Err(LlmError::AuthenticationError {
                message: format!("Copilot token refresh failed: HTTP {}", response.status()),
            });
        }

        let body: serde_json::Value = response.json().await.map_err(|e| {
            LlmError::Other(format!(
                "Failed to parse Copilot token refresh response: {e}"
            ))
        })?;

        let token = body.get("token").and_then(|v| v.as_str()).ok_or_else(|| {
            LlmError::AuthenticationError {
                message: "No token in Copilot refresh response".into(),
            }
        })?;

        let expires_at = body
            .get("expires_at")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_else(|| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                now + 1800
            });

        Ok(PortalAuthToken {
            access_token: token.to_string(),
            refresh_token: Some(oauth_token.to_string()),
            expires_at: Some(expires_at),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copilot_proxy_auth_name() {
        let auth = CopilotProxyAuth::new().expect("HTTP client build should not fail in test");
        assert_eq!(auth.name(), "copilot-proxy");
    }

    #[test]
    fn test_copilot_proxy_auth_type() {
        let auth = CopilotProxyAuth::new().expect("HTTP client build should not fail in test");
        match auth.auth_type() {
            PortalAuthType::Bearer { header_name } => {
                assert_eq!(header_name, "Authorization");
            }
            _ => panic!("expected Bearer auth type"),
        }
    }

    #[test]
    fn test_copilot_proxy_auth_new_returns_ok() {
        let result = CopilotProxyAuth::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_copilot_config_dir_returns_some() {
        let result = copilot_config_dir();
        assert!(result.is_some() || result.is_none());
    }

    #[test]
    fn test_read_copilot_token_no_config() {
        let result = read_copilot_token();
        assert!(result.is_none() || result.is_some());
    }

    #[tokio::test]
    async fn test_copilot_authenticate_no_token() {
        let auth = CopilotProxyAuth::new().expect("HTTP client build should not fail in test");
        let result = auth.authenticate().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::AuthenticationError { message } => {
                assert!(message.contains("no cached OAuth token") || message.contains("Copilot"));
            }
            other => panic!("expected AuthenticationError, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_copilot_refresh_network_error() {
        let auth = CopilotProxyAuth::new().expect("HTTP client build should not fail in test");
        let result = auth.refresh("invalid_token").await;
        assert!(result.is_err());
    }
}
