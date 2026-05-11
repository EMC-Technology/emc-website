use std::sync::Arc;

use crate::provider::glm::GlmProvider;
use crate::provider::{LanguageModelProvider, ProviderKind};

/// 创建智谱 GLM 供应商实例。
#[must_use]
pub fn create_glm_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(GlmProvider::new())
}

/// 创建 `DeepSeek` 供应商实例。
#[must_use]
pub fn create_deepseek_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::deepseek::create_deepseek_provider())
}

/// 创建 Kimi（月之暗面）供应商实例。
#[must_use]
pub fn create_kimi_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::kimi::create_kimi_provider())
}

/// 创建通义千问（Qwen）供应商实例。
#[must_use]
pub fn create_qwen_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::qwen::create_qwen_provider())
}

/// 创建 `OpenRouter` 供应商实例。
#[must_use]
pub fn create_openrouter_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::openrouter::create_openrouter_provider())
}

/// 创建 Google Gemini 供应商实例。
#[must_use]
pub fn create_gemini_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::gemini::create_gemini_provider())
}

/// 创建 Mistral 供应商实例。
#[must_use]
pub fn create_mistral_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::mistral::create_mistral_provider())
}

/// 创建 Groq 供应商实例。
#[must_use]
pub fn create_groq_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::groq::create_groq_provider())
}

/// 根据供应商类型创建对应的供应商实例。
///
/// 对于 `OpenAi`、`Anthropic`、`Ollama`、`XAI`、`CodeGeeX`、`WebScraper`
/// 及 `OpenAiCompatible` 类型返回 `None`，需通过各自的构造函数创建。
#[must_use]
pub fn create_provider_by_kind(kind: &ProviderKind) -> Option<Arc<dyn LanguageModelProvider>> {
    match kind {
        ProviderKind::Glm => Some(create_glm_provider()),
        ProviderKind::DeepSeek => Some(create_deepseek_provider()),
        ProviderKind::Kimi => Some(create_kimi_provider()),
        ProviderKind::Qwen => Some(create_qwen_provider()),
        ProviderKind::OpenRouter => Some(create_openrouter_provider()),
        ProviderKind::Gemini => Some(create_gemini_provider()),
        ProviderKind::Mistral => Some(create_mistral_provider()),
        ProviderKind::Groq => Some(create_groq_provider()),
        ProviderKind::OpenAi
        | ProviderKind::Anthropic
        | ProviderKind::Ollama
        | ProviderKind::XAI
        | ProviderKind::CodeGeeX
        | ProviderKind::WebScraper
        | ProviderKind::OpenAiCompatible(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_provider_by_kind_glm() {
        let provider = create_provider_by_kind(&ProviderKind::Glm).unwrap();
        assert_eq!(provider.id().as_ref(), "glm");
        assert_eq!(provider.provided_models().len(), 16);
    }

    #[test]
    fn test_create_provider_by_kind_deepseek() {
        let provider = create_provider_by_kind(&ProviderKind::DeepSeek).unwrap();
        assert_eq!(provider.id().as_ref(), "deepseek");
        assert_eq!(provider.provided_models().len(), 2);
    }

    #[test]
    fn test_create_provider_by_kind_kimi() {
        let provider = create_provider_by_kind(&ProviderKind::Kimi).unwrap();
        assert_eq!(provider.id().as_ref(), "kimi");
        assert_eq!(provider.provided_models().len(), 5);
    }

    #[test]
    fn test_create_provider_by_kind_qwen() {
        let provider = create_provider_by_kind(&ProviderKind::Qwen).unwrap();
        assert_eq!(provider.id().as_ref(), "qwen");
        assert_eq!(provider.provided_models().len(), 9);
    }

    #[test]
    fn test_create_provider_by_kind_openrouter() {
        let provider = create_provider_by_kind(&ProviderKind::OpenRouter).unwrap();
        assert_eq!(provider.id().as_ref(), "openrouter");
        assert_eq!(provider.provided_models().len(), 5);
    }

    #[test]
    fn test_create_provider_by_kind_gemini() {
        let provider = create_provider_by_kind(&ProviderKind::Gemini).unwrap();
        assert_eq!(provider.id().as_ref(), "gemini");
        assert_eq!(provider.provided_models().len(), 3);
    }

    #[test]
    fn test_create_provider_by_kind_mistral() {
        let provider = create_provider_by_kind(&ProviderKind::Mistral).unwrap();
        assert_eq!(provider.id().as_ref(), "mistral");
        assert_eq!(provider.provided_models().len(), 4);
    }

    #[test]
    fn test_create_provider_by_kind_groq() {
        let provider = create_provider_by_kind(&ProviderKind::Groq).unwrap();
        assert_eq!(provider.id().as_ref(), "groq");
        assert_eq!(provider.provided_models().len(), 3);
    }

    #[test]
    fn test_create_provider_by_kind_unsupported() {
        assert!(create_provider_by_kind(&ProviderKind::OpenAi).is_none());
        assert!(create_provider_by_kind(&ProviderKind::Anthropic).is_none());
        assert!(create_provider_by_kind(&ProviderKind::Ollama).is_none());
    }
}
