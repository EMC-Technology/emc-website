use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use crate::credential::{ApiKeyState, CredentialsProvider};
use crate::error::LlmError;

/// 凭证别名提供者，支持通过多个别名环境变量查找 API Key。
pub struct AliasCredentialProvider {
    provider_id: String,
    inner: Arc<dyn CredentialsProvider>,
    aliases: Vec<String>,
    alias_cache: RwLock<HashMap<String, String>>,
}

impl AliasCredentialProvider {
    /// 创建新的凭证别名提供者。
    ///
    /// `aliases` 为备选环境变量名列表，按优先级依次尝试。
    pub fn new(
        provider_id: impl Into<String>,
        inner: Arc<dyn CredentialsProvider>,
        aliases: Vec<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            inner,
            aliases,
            alias_cache: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl CredentialsProvider for AliasCredentialProvider {
    async fn get_api_key(&self, provider_id: &str) -> Result<String, LlmError> {
        if let Some(key) = self.alias_cache.read().get(provider_id).cloned() {
            return Ok(key);
        }
        if let Ok(key) = self.inner.get_api_key(&self.provider_id).await {
            self.alias_cache
                .write()
                .insert(provider_id.to_string(), key.clone());
            return Ok(key);
        }
        for alias in &self.aliases {
            if let Ok(val) = std::env::var(alias)
                && !val.is_empty()
            {
                self.alias_cache
                    .write()
                    .insert(provider_id.to_string(), val.clone());
                return Ok(val);
            }
        }
        Err(LlmError::MissingCredentials {
            provider: provider_id.to_string(),
            env_vars: self.aliases.clone(),
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
        self.alias_cache.write().remove(provider_id);
        self.inner.invalidate(provider_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_alias_credential_provider_new() {
        let inner = Arc::new(crate::credential::EnvCredentialProvider::new());
        let provider =
            AliasCredentialProvider::new("qwen", inner, vec!["DASHSCOPE_API_KEY".to_string()]);
        assert_eq!(provider.provider_id, "qwen");
        assert_eq!(provider.aliases.len(), 1);
    }

    #[tokio::test]
    #[serial]
    #[allow(clippy::disallowed_methods)]
    async fn test_alias_provider_primary_key() {
        let unique_id = format!("alias_primary_test_{}", std::process::id());
        let env_key = format!("ALIAS_PRIMARY_TEST_{}_API_KEY", std::process::id());
        // SAFETY: 测试使用进程唯一的环境变量名，不会与其他线程冲突。
        // set_var/remove_var 在 Rust 2024 edition 中标记为 unsafe，
        // 因为多线程并发修改环境变量不安全，但测试是串行执行的。
        unsafe {
            std::env::set_var(&env_key, "primary_key_value");
        }
        let inner = Arc::new(crate::credential::EnvCredentialProvider::new());
        let provider = AliasCredentialProvider::new(
            &unique_id,
            inner,
            vec!["NONEXISTENT_ALIAS_FALLBACK_XYZ".to_string()],
        );
        let key = provider.get_api_key(&unique_id).await.unwrap();
        assert_eq!(key, "primary_key_value");
        // SAFETY: 同上，清理测试环境变量。
        unsafe {
            std::env::remove_var(&env_key);
        }
    }

    #[tokio::test]
    #[serial]
    #[allow(clippy::disallowed_methods)]
    async fn test_alias_provider_fallback() {
        let unique_id = format!("alias_fallback_test_{}", std::process::id());
        let fallback_var = format!("ALIAS_FALLBACK_TEST_{}", std::process::id());
        // SAFETY: 测试使用进程唯一的环境变量名，不会与其他线程冲突。
        unsafe {
            std::env::set_var(&fallback_var, "fallback_key_value");
        }
        let inner = Arc::new(crate::credential::EnvCredentialProvider::new());
        let provider = AliasCredentialProvider::new(
            format!("nonexistent_primary_{}", std::process::id()),
            inner,
            vec![fallback_var.clone()],
        );
        let key = provider.get_api_key(&unique_id).await.unwrap();
        assert_eq!(key, "fallback_key_value");
        // SAFETY: 同上，清理测试环境变量。
        unsafe {
            std::env::remove_var(&fallback_var);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_alias_provider_both_missing() {
        let inner = Arc::new(crate::credential::EnvCredentialProvider::new());
        let provider = AliasCredentialProvider::new(
            "nonexistent_alias_test_xyz",
            inner,
            vec!["NONEXISTENT_ALIAS_VAR_XYZ".to_string()],
        );
        let result = provider.get_api_key("nonexistent_alias_test_xyz").await;
        assert!(matches!(result, Err(LlmError::MissingCredentials { .. })));
    }

    #[tokio::test]
    #[serial]
    async fn test_alias_provider_invalidate() {
        let inner = Arc::new(crate::credential::EnvCredentialProvider::new());
        let provider = AliasCredentialProvider::new(
            "test_invalidate_alias",
            inner,
            vec!["TEST_INVALIDATE_ALIAS_VAR".to_string()],
        );
        provider.alias_cache.write().insert(
            "test_invalidate_alias".to_string(),
            "cached_key".to_string(),
        );
        assert_eq!(
            provider.state("test_invalidate_alias").await,
            ApiKeyState::Valid
        );
        provider.invalidate("test_invalidate_alias").await;
        assert!(
            provider
                .alias_cache
                .read()
                .get("test_invalidate_alias")
                .is_none()
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_alias_provider_state_missing() {
        let inner = Arc::new(crate::credential::EnvCredentialProvider::new());
        let provider = AliasCredentialProvider::new(
            "missing_state_test_xyz",
            inner,
            vec!["MISSING_STATE_VAR_XYZ".to_string()],
        );
        assert_eq!(
            provider.state("missing_state_test_xyz").await,
            ApiKeyState::Missing
        );
    }

    #[tokio::test]
    #[serial]
    #[allow(clippy::disallowed_methods)]
    async fn test_alias_provider_fallback_alias_env() {
        let unique_alias = format!("ULLM_TEST_ALIAS_{}", std::process::id());
        let unique_primary = format!("ULLM_TEST_PRIMARY_{}", std::process::id());
        // SAFETY: 测试使用进程唯一的环境变量名，不会与其他线程冲突。
        unsafe {
            std::env::set_var(&unique_alias, "alias_key_value");
        }
        let inner = Arc::new(crate::credential::EnvCredentialProvider::new());
        let provider =
            AliasCredentialProvider::new(&unique_primary, inner, vec![unique_alias.clone()]);
        let key = provider.get_api_key(&unique_primary).await.unwrap();
        assert_eq!(key, "alias_key_value");
        // SAFETY: 同上，清理测试环境变量。
        unsafe {
            std::env::remove_var(&unique_alias);
        }
    }

    #[tokio::test]
    #[serial]
    #[allow(clippy::disallowed_methods)]
    async fn test_alias_provider_primary_env_priority() {
        let unique_alias = format!("ULLM_TEST_ALIAS_PRI_{}", std::process::id());
        let unique_primary = format!("ULLM_TEST_PRI_{}", std::process::id());
        let env_key = format!(
            "{}_API_KEY",
            unique_primary.to_uppercase().replace('-', "_")
        );
        // SAFETY: 测试使用进程唯一的环境变量名，不会与其他线程冲突。
        unsafe {
            std::env::set_var(&unique_alias, "alias_key");
            std::env::set_var(&env_key, "primary_key");
        }
        let inner = Arc::new(crate::credential::EnvCredentialProvider::new());
        let provider =
            AliasCredentialProvider::new(&unique_primary, inner, vec![unique_alias.clone()]);
        let key = provider.get_api_key(&unique_primary).await.unwrap();
        assert_eq!(key, "primary_key");
        // SAFETY: 同上，清理测试环境变量。
        unsafe {
            std::env::remove_var(&unique_alias);
            std::env::remove_var(&env_key);
        }
    }
}
