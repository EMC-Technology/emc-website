use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;
use std::collections::HashMap;

fn openrouter_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        (
            "openai/gpt-4o".into(),
            "GPT-4o (via OpenRouter)".into(),
            128_000,
            Some(16384),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: true,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 128_000,
            },
        ),
        (
            "anthropic/claude-sonnet-4".into(),
            "Claude Sonnet 4 (via OpenRouter)".into(),
            200_000,
            Some(64000),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: true,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 200_000,
            },
        ),
        (
            "google/gemini-2.5-pro".into(),
            "Gemini 2.5 Pro (via OpenRouter)".into(),
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
            "deepseek/deepseek-chat".into(),
            "DeepSeek Chat (via OpenRouter)".into(),
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
            "meta-llama/llama-3.3-70b-instruct".into(),
            "Llama 3.3 70B (via OpenRouter)".into(),
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
    ]
}

/// 创建 `OpenRouter` 供应商实例。
#[must_use]
pub fn create_openrouter_provider() -> OpenAiCompatibleProvider {
    let mut extra_headers = HashMap::new();
    extra_headers.insert(
        "HTTP-Referer".to_string(),
        "https://github.com/ai-ide".to_string(),
    );
    extra_headers.insert("X-Title".to_string(), "AI-IDE".to_string());
    OpenAiCompatibleProvider::new(
        "openrouter",
        "OpenRouter",
        "https://openrouter.ai/api/v1",
        Some("OPENROUTER_API_KEY".to_string()),
        openrouter_models(),
    )
    .with_extra_headers(extra_headers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LanguageModelProvider;

    #[test]
    fn test_openrouter_provider_models() {
        let provider = create_openrouter_provider();
        assert_eq!(provider.provided_models().len(), 5);
    }

    #[test]
    fn test_openrouter_extra_headers() {
        let provider = create_openrouter_provider();
        assert!(
            provider
                .configuration()
                .extra_headers
                .contains_key("HTTP-Referer")
        );
        assert!(
            provider
                .configuration()
                .extra_headers
                .contains_key("X-Title")
        );
    }
}
