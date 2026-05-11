use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn groq_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        (
            "llama-3.3-70b-versatile".into(),
            "Llama 3.3 70B (Groq)".into(),
            128_000,
            Some(32768),
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
            "llama-3.1-8b-instant".into(),
            "Llama 3.1 8B (Groq)".into(),
            128_000,
            Some(8192),
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
            "mixtral-8x7b-32768".into(),
            "Mixtral 8x7B (Groq)".into(),
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
    ]
}

/// 创建 Groq 供应商实例。
#[must_use]
pub fn create_groq_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "groq",
        "Groq",
        "https://api.groq.com/openai/v1",
        Some("GROQ_API_KEY".to_string()),
        groq_models(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LanguageModelProvider;

    #[test]
    fn test_groq_provider_models() {
        let provider = create_groq_provider();
        assert_eq!(provider.provided_models().len(), 3);
    }

    #[test]
    fn test_groq_api_url() {
        let provider = create_groq_provider();
        assert_eq!(
            provider.configuration().api_url,
            "https://api.groq.com/openai/v1"
        );
    }

    #[test]
    fn test_groq_provider_id() {
        let provider = create_groq_provider();
        assert_eq!(provider.id().as_ref(), "groq");
    }
}
