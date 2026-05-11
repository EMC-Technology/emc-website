use std::sync::Arc;

use crate::credential::EnvCredentialProvider;
use crate::credential::alias::AliasCredentialProvider;
use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

type ModelSpec = (String, String, u64, Option<u64>, ModelCapabilities);

fn qwen_base_models() -> Vec<ModelSpec> {
    vec![
        (
            "qwen-max".into(),
            "Qwen Max".into(),
            32768,
            Some(8192),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: true,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 32768,
            },
        ),
        (
            "qwen-plus".into(),
            "Qwen Plus".into(),
            131_072,
            Some(8192),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: true,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 131_072,
            },
        ),
        (
            "qwen-turbo".into(),
            "Qwen Turbo".into(),
            1_000_000,
            Some(8192),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: true,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 1_000_000,
            },
        ),
        (
            "qwen-long".into(),
            "Qwen Long".into(),
            10_000_000,
            Some(6000),
            ModelCapabilities {
                tools: false,
                streaming_tools: false,
                images: false,
                thinking: false,
                parallel_tool_calls: false,
                max_tokens: 10_000_000,
            },
        ),
    ]
}

fn qwen_advanced_models() -> Vec<ModelSpec> {
    vec![
        (
            "qwq-32b".into(),
            "QwQ 32B".into(),
            131_072,
            Some(16384),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: true,
                parallel_tool_calls: true,
                max_tokens: 131_072,
            },
        ),
        (
            "qwen-qwq-32b".into(),
            "Qwen QwQ 32B".into(),
            131_072,
            Some(8192),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: true,
                parallel_tool_calls: true,
                max_tokens: 131_072,
            },
        ),
        (
            "qwen3-235b-a22b".into(),
            "Qwen3 235B".into(),
            131_072,
            Some(8192),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: true,
                parallel_tool_calls: true,
                max_tokens: 131_072,
            },
        ),
        (
            "qwen-vl-max".into(),
            "Qwen VL Max".into(),
            32768,
            Some(2048),
            ModelCapabilities {
                tools: false,
                streaming_tools: false,
                images: true,
                thinking: false,
                parallel_tool_calls: false,
                max_tokens: 32768,
            },
        ),
        (
            "qwen-coder-plus".into(),
            "Qwen Coder Plus".into(),
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

fn qwen_models() -> Vec<ModelSpec> {
    let mut models = Vec::with_capacity(9);
    models.extend(qwen_base_models());
    models.extend(qwen_advanced_models());
    models
}

/// 创建通义千问（Qwen）供应商实例。
#[must_use]
pub fn create_qwen_provider() -> OpenAiCompatibleProvider {
    let alias_provider = AliasCredentialProvider::new(
        "qwen",
        Arc::new(EnvCredentialProvider::new()),
        vec!["DASHSCOPE_API_KEY".to_string()],
    );
    OpenAiCompatibleProvider::new(
        "qwen",
        "Qwen（千问）",
        "https://dashscope.aliyuncs.com/compatible-mode/v1",
        None,
        qwen_models(),
    )
    .with_credentials(Arc::new(alias_provider))
    .with_max_request_body_bytes(6 * 1024 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LanguageModelProvider;
    use crate::thinking::is_reasoning_model;
    use serial_test::serial;

    #[test]
    fn test_qwen_provider_models() {
        let provider = create_qwen_provider();
        assert_eq!(provider.provided_models().len(), 9);
    }

    #[test]
    fn test_qwen_api_url() {
        let provider = create_qwen_provider();
        assert_eq!(
            provider.configuration().api_url,
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        );
    }

    #[test]
    fn test_qwq_is_reasoning() {
        assert!(is_reasoning_model("qwq-32b"));
    }

    #[test]
    fn test_qwen_max_request_body_bytes() {
        let provider = create_qwen_provider();
        assert_eq!(
            provider.configuration().max_request_body_bytes,
            Some(6 * 1024 * 1024)
        );
    }

    #[test]
    fn test_qwen_provider_id() {
        let provider = create_qwen_provider();
        assert_eq!(provider.id().as_ref(), "qwen");
    }

    #[test]
    fn test_qwen_long_model_no_tools() {
        let provider = create_qwen_provider();
        let models = provider.provided_models();
        let long_model = models
            .iter()
            .find(|m| m.id().as_ref() == "qwen-long")
            .unwrap();
        assert!(!long_model.supports_tools());
        assert_eq!(long_model.max_token_count(), 10_000_000);
    }

    #[tokio::test]
    #[serial]
    #[allow(clippy::disallowed_methods)]
    async fn test_qwen_alias_dashscope_key() {
        use crate::credential::CredentialsProvider;
        let unique_var = format!("DASHSCOPE_API_KEY_TEST_{}", std::process::id());
        // SAFETY: 测试使用进程唯一的环境变量名，不会与其他线程冲突。
        unsafe {
            std::env::set_var(&unique_var, "test_dashscope_key");
        }
        let alias_provider = AliasCredentialProvider::new(
            "qwen_alias_test_unique_xyz",
            Arc::new(EnvCredentialProvider::new()),
            vec![unique_var.clone()],
        );
        let key = alias_provider
            .get_api_key("qwen_alias_test_unique_xyz")
            .await
            .unwrap();
        assert_eq!(key, "test_dashscope_key");
        // SAFETY: 同上，清理测试环境变量。
        unsafe {
            std::env::remove_var(&unique_var);
        }
    }
}
