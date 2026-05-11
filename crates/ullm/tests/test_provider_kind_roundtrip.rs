use ullm::provider::ProviderKind;

#[test]
fn test_provider_kind_all_named_variants_roundtrip() {
    let variants = vec![
        (ProviderKind::OpenAi, "openai"),
        (ProviderKind::Anthropic, "anthropic"),
        (ProviderKind::Ollama, "ollama"),
        (ProviderKind::XAI, "xai"),
        (ProviderKind::CodeGeeX, "codegeex"),
        (ProviderKind::WebScraper, "web-scraper"),
        (ProviderKind::Glm, "glm"),
        (ProviderKind::DeepSeek, "deepseek"),
        (ProviderKind::Kimi, "kimi"),
        (ProviderKind::Qwen, "qwen"),
        (ProviderKind::OpenRouter, "openrouter"),
        (ProviderKind::Gemini, "gemini"),
        (ProviderKind::Mistral, "mistral"),
        (ProviderKind::Groq, "groq"),
    ];
    for (variant, expected_name) in variants {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_name}\""));
        let deserialized: ProviderKind = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, variant);
    }
}

#[test]
fn test_provider_kind_openai_compatible_roundtrip() {
    let kind = ProviderKind::OpenAiCompatible("my-custom-provider".to_string());
    let json = serde_json::to_string(&kind).unwrap();
    assert_eq!(json, "\"my-custom-provider\"");
    let deserialized: ProviderKind = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, kind);
}

#[test]
fn test_provider_kind_unknown_string_becomes_openai_compatible() {
    let json = "\"some-future-provider\"";
    let kind: ProviderKind = serde_json::from_str(json).unwrap();
    assert_eq!(
        kind,
        ProviderKind::OpenAiCompatible("some-future-provider".to_string())
    );
}

#[test]
fn test_provider_kind_display_matches_serde() {
    let kinds = vec![
        ProviderKind::OpenAi,
        ProviderKind::Glm,
        ProviderKind::DeepSeek,
        ProviderKind::Qwen,
        ProviderKind::Groq,
    ];
    for kind in kinds {
        let display = format!("{kind}");
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, format!("\"{display}\""));
    }
}
