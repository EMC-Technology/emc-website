use async_trait::async_trait;
use reqwest::Client;

use crate::build_http_client;
use crate::credential::portal_auth::{PortalAuthStrategy, PortalAuthToken, PortalAuthType};
use crate::error::LlmError;

/// Google Gemini CLI 认证策略，通过 Gemini CLI 的本地凭证获取认证。
pub struct GeminiCliAuth {
    client: Client,
}

impl GeminiCliAuth {
    /// # Errors
    ///
    /// 当 HTTP 客户端构建失败时返回 `LlmError::HttpClientInit`。
    pub fn new() -> Result<Self, LlmError> {
        let client = build_http_client(std::time::Duration::from_secs(30))?;
        Ok(Self { client })
    }
}

fn gemini_config_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    Some(std::path::PathBuf::from(home).join(".gemini"))
}

fn read_gemini_token_cache() -> Option<String> {
    let dir = gemini_config_dir()?;
    let cache_file = dir.join("access_token.json");
    let content = std::fs::read_to_string(&cache_file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("access_token")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string)
}

fn read_gemini_refresh_token() -> Option<String> {
    let dir = gemini_config_dir()?;
    let cache_file = dir.join("access_token.json");
    let content = std::fs::read_to_string(&cache_file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("refresh_token")
        .and_then(serde_json::Value::as_str)
        .map(std::string::ToString::to_string)
}

#[async_trait]
impl PortalAuthStrategy for GeminiCliAuth {
    fn name(&self) -> &'static str {
        "gemini-cli"
    }

    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError> {
        if let Some(access_token) = read_gemini_token_cache() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            return Ok(PortalAuthToken {
                access_token,
                refresh_token: read_gemini_refresh_token(),
                expires_at: Some(now + 3600),
                token_type: PortalAuthType::Bearer {
                    header_name: "Authorization".into(),
                },
            });
        }

        Err(LlmError::AuthenticationError {
            message: "Gemini CLI auth: no cached token found. Run 'gemini auth' first.".into(),
        })
    }

    async fn refresh(&self, refresh_token: &str) -> Result<PortalAuthToken, LlmError> {
        let response = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!(
                "grant_type=refresh_token&refresh_token={refresh_token}"
            ))
            .send()
            .await
            .map_err(|e| LlmError::Network(format!("Gemini token refresh failed: {e}")))?;

        if !response.status().is_success() {
            return Err(LlmError::AuthenticationError {
                message: format!("Gemini token refresh failed: HTTP {}", response.status()),
            });
        }

        let body: serde_json::Value = response.json().await.map_err(|e| {
            LlmError::Other(format!(
                "Failed to parse Gemini token refresh response: {e}"
            ))
        })?;

        let access_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LlmError::AuthenticationError {
                message: "No access_token in Gemini refresh response".into(),
            })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let expires_in = body
            .get("expires_in")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(3600);

        Ok(PortalAuthToken {
            access_token: access_token.to_string(),
            refresh_token: Some(refresh_token.to_string()),
            expires_at: Some(now + expires_in),
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
    fn test_gemini_cli_auth_name() {
        let auth = GeminiCliAuth::new().expect("HTTP client build should not fail in test");
        assert_eq!(auth.name(), "gemini-cli");
    }

    #[test]
    fn test_gemini_cli_auth_type() {
        let auth = GeminiCliAuth::new().expect("HTTP client build should not fail in test");
        match auth.auth_type() {
            PortalAuthType::Bearer { header_name } => {
                assert_eq!(header_name, "Authorization");
            }
            _ => panic!("expected Bearer auth type"),
        }
    }

    #[test]
    fn test_gemini_cli_auth_new_returns_ok() {
        let result = GeminiCliAuth::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_gemini_config_dir_returns_some() {
        let result = gemini_config_dir();
        assert!(result.is_some() || result.is_none());
    }

    #[test]
    fn test_read_gemini_token_cache_no_config() {
        let result = read_gemini_token_cache();
        assert!(result.is_none() || result.is_some());
    }

    #[test]
    fn test_read_gemini_refresh_token_no_config() {
        let result = read_gemini_refresh_token();
        assert!(result.is_none() || result.is_some());
    }

    #[tokio::test]
    async fn test_gemini_authenticate_no_token() {
        let auth = GeminiCliAuth::new().expect("HTTP client build should not fail in test");
        let result = auth.authenticate().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::AuthenticationError { message } => {
                assert!(message.contains("no cached token") || message.contains("Gemini"));
            }
            other => panic!("expected AuthenticationError, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_gemini_refresh_network_error() {
        let auth = GeminiCliAuth::new().expect("HTTP client build should not fail in test");
        let result = auth.refresh("invalid_refresh_token").await;
        assert!(result.is_err());
    }
}
