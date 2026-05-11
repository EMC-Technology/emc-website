use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn mistral_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        (
            "mistral-large-latest".into(),
            "Mistral Large".into(),
            128_000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 128_000,
            },
        ),
        (
            "mistral-medium-latest".into(),
            "Mistral Medium".into(),
            32000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 32000,
            },
        ),
        (
            "mistral-small-latest".into(),
            "Mistral Small".into(),
            32000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 32000,
            },
        ),
        (
            "codestral-latest".into(),
            "Codestral".into(),
            256_000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 256_000,
            },
        ),
    ]
}

/// 创建 Mistral 供应商实例。
#[must_use]
pub fn create_mistral_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "mistral",
        "Mistral",
        "https://api.mistral.ai/v1",
        Some("MISTRAL_API_KEY".to_string()),
        mistral_models(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LanguageModelProvider;

    #[test]
    fn test_mistral_provider_models() {
        let provider = create_mistral_provider();
        assert_eq!(provider.provided_models().len(), 4);
    }

    #[test]
    fn test_mistral_api_url() {
        let provider = create_mistral_provider();
        assert_eq!(
            provider.configuration().api_url,
            "https://api.mistral.ai/v1"
        );
    }

    #[test]
    fn test_mistral_provider_id() {
        let provider = create_mistral_provider();
        assert_eq!(provider.id().as_ref(), "mistral");
    }
}
