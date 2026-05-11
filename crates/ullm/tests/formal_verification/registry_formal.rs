#![cfg(test)]
//! Registry 模块形式化验证测试
//!
//! ## 不变量 (Invariants)
//!
//! 1. `find_model` 正确解析别名
//! 2. `unregister` 不影响其他 provider
//! 3. `role_model` 和 `default_model` 正确工作

use crate::registry::{ModelRegistry, ModelRole};
use crate::provider::{LanguageModel, LanguageModelProvider};
use crate::provider::types::{
    LanguageModelRequest, ModelId, ModelName, ProviderId, ProviderName,
};
use crate::stream::StreamFuture;
use crate::token_count::TokenCountFuture;
use std::sync::Arc;

struct MockModel {
    id: ModelId,
    name: ModelName,
    provider_id: ProviderId,
    provider_name: ProviderName,
}

#[async_trait::async_trait]
impl LanguageModel for MockModel {
    fn id(&self) -> &ModelId { &self.id }
    fn name(&self) -> &ModelName { &self.name }
    fn provider_id(&self) -> &ProviderId { &self.provider_id }
    fn provider_name(&self) -> &ProviderName { &self.provider_name }
    fn supports_tools(&self) -> bool { false }
    fn supports_streaming_tools(&self) -> bool { false }
    fn supports_images(&self) -> bool { false }
    fn supports_thinking(&self) -> bool { false }
    fn max_token_count(&self) -> u64 { 4096 }
    fn max_output_tokens(&self) -> Option<u64> { None }
    fn count_tokens(&self, _request: &LanguageModelRequest) -> TokenCountFuture<'_> {
        Box::pin(async { Ok(0) })
    }
    fn stream_completion(&self, _request: LanguageModelRequest) -> StreamFuture<'_> {
        Box::pin(async { Err(crate::error::LlmError::Other("not implemented".into())) })
    }
}

struct MockProvider {
    id: ProviderId,
    name: ProviderName,
    models: Vec<Arc<dyn LanguageModel>>,
    authenticated: std::sync::Mutex<bool>,
}

#[async_trait::async_trait]
impl LanguageModelProvider for MockProvider {
    fn id(&self) -> &ProviderId { &self.id }
    fn name(&self) -> &ProviderName { &self.name }
    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> { self.models.clone() }
    fn is_authenticated(&self) -> bool { *self.authenticated.lock().unwrap() }
    async fn authenticate(&self) -> Result<(), crate::error::LlmError> {
        *self.authenticated.lock().unwrap() = true;
        Ok(())
    }
    async fn reset_credentials(&self) -> Result<(), crate::error::LlmError> { Ok(()) }
    fn configuration(&self) -> &crate::provider::types::ProviderConfig {
        unimplemented!()
    }
}

fn make_provider(provider_id: &str, model_ids: &[&str]) -> Arc<dyn LanguageModelProvider> {
    let pid = ProviderId::new(provider_id);
    let pname = ProviderName::new(provider_id);
    let models: Vec<Arc<dyn LanguageModel>> = model_ids
        .iter()
        .map(|&mid| {
            Arc::new(MockModel {
                id: ModelId::new(mid),
                name: ModelName::new(mid),
                provider_id: pid.clone(),
                provider_name: pname.clone(),
            }) as Arc<dyn LanguageModel>
        })
        .collect();
    Arc::new(MockProvider {
        id: pid,
        name: pname,
        models,
        authenticated: std::sync::Mutex::new(true),
    })
}

#[test]
fn test_miri_registry_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ModelRegistry>();
    assert_send_sync::<ModelRole>();
}

#[test]
fn test_kani_registry_find_model_exhaustive() {
    let mut registry = ModelRegistry::new();
    registry.register(make_provider("openai", &["gpt-4o", "gpt-4o-mini"]));
    registry.register(make_provider("anthropic", &["claude-3", "claude-3-5-sonnet"]));

    assert!(
        registry.find_model("gpt-4o").is_some(),
        "THEOREM: find_model MUST find registered model"
    );
    assert!(
        registry.find_model("claude-3").is_some(),
        "THEOREM: find_model MUST find model from any provider"
    );
    assert!(
        registry.find_model("nonexistent").is_none(),
        "THEOREM: find_model MUST return None for unregistered model"
    );
}

#[test]
fn test_kani_registry_alias_resolution() {
    let mut registry = ModelRegistry::new();
    registry.register(make_provider("openai", &["gpt-4o"]));
    registry.add_alias("smart", "gpt-4o");
    registry.add_alias("latest", "gpt-4o");

    assert_eq!(
        registry.resolve_alias("smart"),
        Some("gpt-4o"),
        "THEOREM: resolve_alias MUST return correct model_id"
    );
    assert_eq!(
        registry.resolve_alias("latest"),
        Some("gpt-4o"),
        "THEOREM: resolve_alias MUST resolve multiple aliases to same model"
    );
    assert_eq!(
        registry.resolve_alias("nonexistent"),
        None,
        "THEOREM: resolve_alias MUST return None for unknown alias"
    );
}

#[test]
fn test_kani_registry_unregister_independence() {
    let mut registry = ModelRegistry::new();
    registry.register(make_provider("openai", &["gpt-4o"]));
    registry.register(make_provider("anthropic", &["claude-3"]));

    registry.unregister("openai");

    assert!(
        registry.find_model("gpt-4o").is_none(),
        "THEOREM: unregister MUST remove provider's models"
    );
    assert!(
        registry.find_model("claude-3").is_some(),
        "THEOREM: unregister of one provider MUST NOT affect other providers"
    );
}

#[test]
fn test_deductive_registry_default_model_none_when_empty() {
    let registry = ModelRegistry::new();

    assert!(
        registry.default_model().is_none(),
        "THEOREM: default_model() MUST return None when registry is empty"
    );
}

#[test]
fn test_deductive_registry_default_model_resolution() {
    let mut registry = ModelRegistry::new();
    registry.register(make_provider("openai", &["gpt-4o"]));
    registry.set_default_model("gpt-4o");

    let model = registry.default_model();
    assert!(
        model.is_some(),
        "THEOREM: default_model() MUST return Some when model exists"
    );
    assert_eq!(
        model.unwrap().id().as_str(),
        "gpt-4o",
        "THEOREM: default_model() MUST return correct model"
    );
}

#[test]
fn test_deductive_registry_role_model_independence() {
    let mut registry = ModelRegistry::new();
    registry.register(make_provider("openai", &["gpt-4o", "gpt-4o-mini"]));

    registry.set_role_model(ModelRole::Smart, "gpt-4o");
    registry.set_role_model(ModelRole::Fast, "gpt-4o-mini");

    let smart = registry.role_model(ModelRole::Smart);
    let fast = registry.role_model(ModelRole::Fast);

    assert!(
        smart.is_some() && smart.unwrap().id().as_str() == "gpt-4o",
        "THEOREM: role_model(Smart) MUST return gpt-4o"
    );
    assert!(
        fast.is_some() && fast.unwrap().id().as_str() == "gpt-4o-mini",
        "THEOREM: role_model(Fast) MUST return gpt-4o-mini"
    );
}

#[test]
fn test_deductive_registry_models_by_provider() {
    let mut registry = ModelRegistry::new();
    registry.register(make_provider("openai", &["gpt-4o", "gpt-4o-mini", "gpt-3.5-turbo"]));
    registry.register(make_provider("anthropic", &["claude-3"]));

    let openai_models = registry.models_by_provider("openai");
    assert_eq!(
        openai_models.len(),
        3,
        "THEOREM: models_by_provider MUST return all models from specified provider"
    );

    let anthropic_models = registry.models_by_provider("anthropic");
    assert_eq!(
        anthropic_models.len(),
        1,
        "THEOREM: models_by_provider MUST return only specified provider's models"
    );

    let unknown_models = registry.models_by_provider("unknown");
    assert!(
        unknown_models.is_empty(),
        "THEOREM: models_by_provider MUST return empty for unknown provider"
    );
}

#[test]
fn test_deductive_registry_all_models() {
    let mut registry = ModelRegistry::new();
    registry.register(make_provider("openai", &["gpt-4o", "gpt-4o-mini"]));
    registry.register(make_provider("anthropic", &["claude-3"]));
    registry.register(make_provider("deepseek", &["deepseek-chat"]));

    let all = registry.all_models();
    assert_eq!(
        all.len(),
        4,
        "THEOREM: all_models() MUST return models from ALL providers"
    );
}

#[test]
fn test_deductive_registry_find_model_via_alias() {
    let mut registry = ModelRegistry::new();
    registry.register(make_provider("openai", &["gpt-4o"]));
    registry.add_alias("best", "gpt-4o");

    let model = registry.find_model("best");
    assert!(
        model.is_some(),
        "THEOREM: find_model MUST resolve aliases before searching"
    );
    assert_eq!(
        model.unwrap().id().as_str(),
        "gpt-4o",
        "THEOREM: find_model via alias MUST return correct model"
    );
}

#[test]
fn test_deductive_registry_provider_by_id() {
    let mut registry = ModelRegistry::new();
    registry.register(make_provider("openai", &["gpt-4o"]));
    registry.register(make_provider("anthropic", &["claude-3"]));

    let provider = registry.provider_by_id("openai");
    assert!(
        provider.is_some(),
        "THEOREM: provider_by_id MUST return Some for registered provider"
    );

    let unknown = registry.provider_by_id("unknown");
    assert!(
        unknown.is_none(),
        "THEOREM: provider_by_id MUST return None for unknown provider"
    );
}