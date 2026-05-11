use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn deepseek_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        (
            "deepseek-chat".into(),
            "DeepSeek Chat".into(),
            64000,
            Some(8192),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 64000,
            },
        ),
        (
            "deepseek-reasoner".into(),
            "DeepSeek Reasoner".into(),
            64000,
            Some(8192),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: true,
                parallel_tool_calls: true,
                max_tokens: 64000,
            },
        ),
    ]
}

/// 创建 `DeepSeek` 供应商实例。
#[must_use]
pub fn create_deepseek_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "deepseek",
        "DeepSeek",
        "https://api.deepseek.com",
        Some("DEEPSEEK_API_KEY".to_string()),
        deepseek_models(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LanguageModelProvider;
    use crate::thinking::is_reasoning_model;

    #[test]
    fn test_deepseek_provider_models() {
        let provider = create_deepseek_provider();
        assert_eq!(provider.provided_models().len(), 2);
    }

    #[test]
    fn test_deepseek_api_url() {
        let provider = create_deepseek_provider();
        assert_eq!(provider.configuration().api_url, "https://api.deepseek.com");
    }

    #[test]
    fn test_deepseek_reasoner_is_reasoning() {
        assert!(is_reasoning_model("deepseek-reasoner"));
    }

    #[test]
    fn test_deepseek_chat_not_reasoning() {
        assert!(!is_reasoning_model("deepseek-chat"));
    }

    #[test]
    fn test_deepseek_provider_id() {
        let provider = create_deepseek_provider();
        assert_eq!(provider.id().as_ref(), "deepseek");
    }

    #[test]
    fn test_deepseek_reasoner_model_capabilities() {
        let provider = create_deepseek_provider();
        let models = provider.provided_models();
        let reasoner = models
            .iter()
            .find(|m| m.id().as_ref() == "deepseek-reasoner")
            .unwrap();
        assert!(reasoner.supports_thinking());
    }
}
