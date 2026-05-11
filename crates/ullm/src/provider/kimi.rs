use std::sync::Arc;

use crate::credential::EnvCredentialProvider;
use crate::credential::alias::AliasCredentialProvider;
use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn kimi_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        (
            "moonshot-v1-8k".into(),
            "Moonshot V1 8K".into(),
            8192,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 8192,
            },
        ),
        (
            "moonshot-v1-32k".into(),
            "Moonshot V1 32K".into(),
            32768,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 32768,
            },
        ),
        (
            "moonshot-v1-128k".into(),
            "Moonshot V1 128K".into(),
            131_072,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 131_072,
            },
        ),
        (
            "kimi-k2-0711".into(),
            "Kimi K2".into(),
            256_000,
            Some(16384),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: true,
                parallel_tool_calls: true,
                max_tokens: 256_000,
            },
        ),
        (
            "kimi-latest".into(),
            "Kimi Latest".into(),
            131_072,
            Some(8192),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 131_072,
            },
        ),
    ]
}

/// 创建 Kimi（月之暗面）供应商实例。
#[must_use]
pub fn create_kimi_provider() -> OpenAiCompatibleProvider {
    let alias_provider = AliasCredentialProvider::new(
        "kimi",
        Arc::new(EnvCredentialProvider::new()),
        vec!["MOONSHOT_API_KEY".to_string()],
    );
    OpenAiCompatibleProvider::new(
        "kimi",
        "Kimi（月之暗面）",
        "https://api.moonshot.cn/v1",
        None,
        kimi_models(),
    )
    .with_credentials(Arc::new(alias_provider))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LanguageModelProvider;
    use crate::thinking::is_reasoning_model;
    use serial_test::serial;

    #[test]
    fn test_kimi_provider_models() {
        let provider = create_kimi_provider();
        assert_eq!(provider.provided_models().len(), 5);
    }

    #[test]
    fn test_kimi_api_url() {
        let provider = create_kimi_provider();
        assert_eq!(
            provider.configuration().api_url,
            "https://api.moonshot.cn/v1"
        );
    }

    #[test]
    fn test_kimi_k2_is_reasoning() {
        assert!(is_reasoning_model("kimi-k2-0711"));
    }

    #[test]
    fn test_kimi_provider_id() {
        let provider = create_kimi_provider();
        assert_eq!(provider.id().as_ref(), "kimi");
    }

    #[test]
    fn test_kimi_k2_model_capabilities() {
        let provider = create_kimi_provider();
        let models = provider.provided_models();
        let k2 = models
            .iter()
            .find(|m| m.id().as_ref() == "kimi-k2-0711")
            .unwrap();
        assert!(k2.supports_thinking());
        assert_eq!(k2.max_token_count(), 256_000);
    }

    #[tokio::test]
    #[serial]
    #[allow(clippy::disallowed_methods)]
    async fn test_kimi_alias_moonshot_key() {
        use crate::credential::CredentialsProvider;
        let unique_var = format!("MOONSHOT_API_KEY_TEST_{}", std::process::id());
        // SAFETY: 测试使用进程唯一的环境变量名，不会与其他线程冲突。
        unsafe {
            std::env::set_var(&unique_var, "test_moonshot_key");
        }
        let alias_provider = AliasCredentialProvider::new(
            "kimi_alias_test_unique_xyz",
            Arc::new(EnvCredentialProvider::new()),
            vec![unique_var.clone()],
        );
        let key = alias_provider
            .get_api_key("kimi_alias_test_unique_xyz")
            .await
            .unwrap();
        assert_eq!(key, "test_moonshot_key");
        // SAFETY: 同上，清理测试环境变量。
        unsafe {
            std::env::remove_var(&unique_var);
        }
    }

    #[tokio::test]
    #[serial]
    #[allow(clippy::disallowed_methods)]
    async fn test_kimi_alias_kimi_key() {
        use crate::credential::CredentialsProvider;
        let unique_id = format!("kimi_test_{}", std::process::id());
        let env_key = format!("KIMI_TEST_{}_API_KEY", std::process::id());
        // SAFETY: 测试使用进程唯一的环境变量名，不会与其他线程冲突。
        unsafe {
            std::env::set_var(&env_key, "test_kimi_key");
        }
        let inner = Arc::new(EnvCredentialProvider::new());
        let key = inner.get_api_key(&unique_id).await.unwrap();
        assert_eq!(key, "test_kimi_key");
        // SAFETY: 同上，清理测试环境变量。
        unsafe {
            std::env::remove_var(&env_key);
        }
    }

    #[tokio::test]
    #[serial]
    #[allow(clippy::disallowed_methods)]
    async fn test_kimi_alias_priority() {
        use crate::credential::CredentialsProvider;
        let unique_id = format!("kimi_priority_test_{}", std::process::id());
        let primary_env = format!("KIMI_PRIORITY_TEST_{}_API_KEY", std::process::id());
        let alias_env = format!("MOONSHOT_PRIORITY_TEST_{}", std::process::id());
        // SAFETY: 测试使用进程唯一的环境变量名，不会与其他线程冲突。
        unsafe {
            std::env::set_var(&primary_env, "kimi_primary_key");
            std::env::set_var(&alias_env, "moonshot_alias_key");
        }
        let alias_provider = AliasCredentialProvider::new(
            &unique_id,
            Arc::new(EnvCredentialProvider::new()),
            vec![alias_env.clone()],
        );
        let key = alias_provider.get_api_key(&unique_id).await.unwrap();
        assert_eq!(key, "kimi_primary_key");
        // SAFETY: 同上，清理测试环境变量。
        unsafe {
            std::env::remove_var(&primary_env);
            std::env::remove_var(&alias_env);
        }
    }
}
