use ullm::provider::ProviderKind;
use ullm::provider::factory::create_provider_by_kind;

#[test]
fn test_all_factory_providers_return_non_empty_models() {
    let kinds = vec![
        ProviderKind::Glm,
        ProviderKind::DeepSeek,
        ProviderKind::Kimi,
        ProviderKind::Qwen,
        ProviderKind::OpenRouter,
        ProviderKind::Gemini,
        ProviderKind::Mistral,
        ProviderKind::Groq,
    ];
    for kind in kinds {
        let provider = create_provider_by_kind(&kind)
            .unwrap_or_else(|| panic!("factory returned None for {kind:?}"));
        let models = provider.provided_models();
        assert!(
            !models.is_empty(),
            "provider {kind:?} returned empty models"
        );
        for model in models {
            assert!(
                !model.id().as_ref().is_empty(),
                "model id is empty for {kind:?}"
            );
            assert!(
                !model.name().as_ref().is_empty(),
                "model name is empty for {kind:?}"
            );
            assert!(
                model.max_token_count() > 0,
                "max_token_count is 0 for model in {kind:?}"
            );
        }
    }
}

#[test]
fn test_unsupported_kinds_return_none() {
    let unsupported = vec![
        ProviderKind::OpenAi,
        ProviderKind::Anthropic,
        ProviderKind::Ollama,
        ProviderKind::XAI,
        ProviderKind::CodeGeeX,
        ProviderKind::WebScraper,
    ];
    for kind in unsupported {
        assert!(
            create_provider_by_kind(&kind).is_none(),
            "expected None for {kind:?}"
        );
    }
}
