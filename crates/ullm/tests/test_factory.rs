use ullm::provider::ProviderKind;
use ullm::provider::factory::create_provider_by_kind;

#[test]
fn test_factory_creates_correct_provider_types() {
    let test_cases = vec![
        (ProviderKind::Glm, "glm", 16),
        (ProviderKind::DeepSeek, "deepseek", 2),
        (ProviderKind::Kimi, "kimi", 5),
        (ProviderKind::Qwen, "qwen", 9),
        (ProviderKind::OpenRouter, "openrouter", 5),
        (ProviderKind::Gemini, "gemini", 3),
        (ProviderKind::Mistral, "mistral", 4),
        (ProviderKind::Groq, "groq", 3),
    ];
    for (kind, expected_id, expected_model_count) in test_cases {
        let provider = create_provider_by_kind(&kind)
            .unwrap_or_else(|| panic!("factory returned None for {kind:?}"));
        assert_eq!(provider.id().as_ref(), expected_id, "wrong id for {kind:?}");
        assert_eq!(
            provider.provided_models().len(),
            expected_model_count,
            "wrong model count for {kind:?}"
        );
    }
}

#[test]
fn test_factory_all_providers_have_valid_config() {
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
        let provider = create_provider_by_kind(&kind).unwrap();
        let config = provider.configuration();
        assert!(!config.api_url.is_empty(), "empty api_url for {kind:?}");
        assert!(
            config.api_key_env.is_some(),
            "missing api_key_env for {kind:?}"
        );
    }
}
