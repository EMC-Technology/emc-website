use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn gemini_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        (
            "gemini-2.5-pro".into(),
            "Gemini 2.5 Pro".into(),
            1_000_000,
            Some(65536),
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
            "gemini-2.5-flash".into(),
            "Gemini 2.5 Flash".into(),
            1_000_000,
            Some(65536),
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
            "gemini-2.0-flash".into(),
            "Gemini 2.0 Flash".into(),
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
    ]
}

/// 创建 Google Gemini 供应商实例。
#[must_use]
pub fn create_gemini_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "gemini",
        "Google Gemini",
        "https://generativelanguage.googleapis.com/v1beta/openai",
        Some("GEMINI_API_KEY".to_string()),
        gemini_models(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LanguageModelProvider;

    #[test]
    fn test_gemini_provider_models() {
        let provider = create_gemini_provider();
        assert_eq!(provider.provided_models().len(), 3);
    }

    #[test]
    fn test_gemini_api_url() {
        let provider = create_gemini_provider();
        assert_eq!(
            provider.configuration().api_url,
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
    }

    #[test]
    fn test_gemini_provider_id() {
        let provider = create_gemini_provider();
        assert_eq!(provider.id().as_ref(), "gemini");
    }
}
