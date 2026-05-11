use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::provider::types::ModelId;
use crate::provider::{LanguageModel, LanguageModelProvider};

/// 模型角色分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelRole {
    /// 默认角色
    Default,
    /// 快速响应
    Fast,
    /// 高质量推理
    Smart,
    /// 摘要生成
    Summary,
    /// 文本嵌入
    Embedding,
}

/// 模型注册表，管理供应商、模型、别名和角色映射
pub struct ModelRegistry {
    providers: Vec<Arc<dyn LanguageModelProvider>>,
    aliases: HashMap<String, ModelId>,
    default_model_id: Option<ModelId>,
    role_models: HashMap<ModelRole, ModelId>,
}

impl ModelRegistry {
    /// 创建空注册表
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            aliases: HashMap::new(),
            default_model_id: None,
            role_models: HashMap::new(),
        }
    }

    /// 注册供应商
    pub fn register(&mut self, provider: Arc<dyn LanguageModelProvider>) {
        self.providers.push(provider);
    }

    /// 注销指定供应商
    pub fn unregister(&mut self, provider_id: &str) {
        self.providers.retain(|p| p.id().as_ref() != provider_id);
    }

    /// 按模型 ID 查找模型（自动解析别名）
    #[must_use]
    pub fn find_model(&self, model_id: &str) -> Option<Arc<dyn LanguageModel>> {
        let resolved = self.resolve_alias(model_id).unwrap_or(model_id);
        self.providers.iter().find_map(|p| {
            p.provided_models()
                .into_iter()
                .find(|m| m.id().as_ref() == resolved)
        })
    }

    /// 按供应商 ID 查找供应商
    #[must_use]
    pub fn provider_by_id(&self, provider_id: &str) -> Option<Arc<dyn LanguageModelProvider>> {
        self.providers
            .iter()
            .find(|p| p.id().as_ref() == provider_id)
            .cloned()
    }

    /// 获取所有已注册模型
    #[must_use]
    pub fn all_models(&self) -> Vec<Arc<dyn LanguageModel>> {
        self.providers
            .iter()
            .flat_map(|p| p.provided_models())
            .collect()
    }

    /// 获取指定供应商下的所有模型
    #[must_use]
    pub fn models_by_provider(&self, provider_id: &str) -> Vec<Arc<dyn LanguageModel>> {
        self.providers
            .iter()
            .find(|p| p.id().as_ref() == provider_id)
            .map(|p| p.provided_models())
            .unwrap_or_default()
    }

    /// 获取所有已注册供应商
    #[must_use]
    pub fn providers(&self) -> &[Arc<dyn LanguageModelProvider>] {
        &self.providers
    }

    /// 添加模型别名
    pub fn add_alias(&mut self, alias: impl Into<String>, model_id: impl Into<ModelId>) {
        self.aliases.insert(alias.into(), model_id.into());
    }

    /// 解析别名到模型 ID
    pub fn resolve_alias(&self, name: &str) -> Option<&str> {
        self.aliases.get(name).map(std::convert::AsRef::as_ref)
    }

    /// 设置默认模型
    pub fn set_default_model(&mut self, model_id: impl Into<ModelId>) {
        self.default_model_id = Some(model_id.into());
    }

    /// 获取默认模型
    #[must_use]
    pub fn default_model(&self) -> Option<Arc<dyn LanguageModel>> {
        self.default_model_id
            .as_ref()
            .and_then(|id| self.find_model(id.as_ref()))
    }

    /// 设置角色对应的模型
    pub fn set_role_model(&mut self, role: ModelRole, model_id: impl Into<ModelId>) {
        self.role_models.insert(role, model_id.into());
    }

    /// 获取指定角色的模型
    #[must_use]
    pub fn role_model(&self, role: ModelRole) -> Option<Arc<dyn LanguageModel>> {
        self.role_models
            .get(&role)
            .and_then(|id| self.find_model(id.as_ref()))
    }

    /// 注册嵌入模型
    pub fn register_embedding(&mut self, _provider_id: &str, model_id: impl Into<ModelId>) {
        self.role_models
            .insert(ModelRole::Embedding, model_id.into());
    }

    /// 获取嵌入模型
    #[must_use]
    pub fn embedding_model(&self) -> Option<Arc<dyn LanguageModel>> {
        self.role_model(ModelRole::Embedding)
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::types::{
        LanguageModelRequest, ModelId, ModelName, ProviderId, ProviderName,
    };
    use crate::stream::StreamFuture;
    use crate::token_count::TokenCountFuture;

    struct MockProvider {
        id: ProviderId,
        name: ProviderName,
        models: Vec<Arc<dyn LanguageModel>>,
    }

    struct MockModel {
        id: ModelId,
        name: ModelName,
        provider_id: ProviderId,
        provider_name: ProviderName,
    }

    #[async_trait::async_trait]
    impl LanguageModel for MockModel {
        fn id(&self) -> &ModelId {
            &self.id
        }
        fn name(&self) -> &ModelName {
            &self.name
        }
        fn provider_id(&self) -> &ProviderId {
            &self.provider_id
        }
        fn provider_name(&self) -> &ProviderName {
            &self.provider_name
        }
        fn supports_tools(&self) -> bool {
            false
        }
        fn supports_streaming_tools(&self) -> bool {
            false
        }
        fn supports_images(&self) -> bool {
            false
        }
        fn supports_thinking(&self) -> bool {
            false
        }
        fn max_token_count(&self) -> u64 {
            4096
        }
        fn max_output_tokens(&self) -> Option<u64> {
            None
        }
        fn count_tokens(&self, _request: &LanguageModelRequest) -> TokenCountFuture<'_> {
            Box::pin(async { Ok(0) })
        }
        fn stream_completion(&self, _request: LanguageModelRequest) -> StreamFuture<'_> {
            Box::pin(async { Err(crate::error::LlmError::Other("not implemented".into())) })
        }
    }

    #[async_trait::async_trait]
    impl LanguageModelProvider for MockProvider {
        fn id(&self) -> &ProviderId {
            &self.id
        }
        fn name(&self) -> &ProviderName {
            &self.name
        }
        fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
            self.models.clone()
        }
        fn is_authenticated(&self) -> bool {
            true
        }
        async fn authenticate(&self) -> Result<(), crate::error::LlmError> {
            Ok(())
        }
        async fn reset_credentials(&self) -> Result<(), crate::error::LlmError> {
            Ok(())
        }
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
        })
    }

    #[test]
    fn test_registry_register_and_find() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o", "gpt-4o-mini"]));
        assert!(registry.find_model("gpt-4o").is_some());
        assert!(registry.find_model("gpt-4o-mini").is_some());
        assert!(registry.find_model("unknown").is_none());
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o"]));
        registry.register(make_provider("anthropic", &["claude-3"]));
        assert!(registry.find_model("gpt-4o").is_some());
        registry.unregister("openai");
        assert!(registry.find_model("gpt-4o").is_none());
        assert!(registry.find_model("claude-3").is_some());
    }

    #[test]
    fn test_registry_models_by_provider() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o", "gpt-4o-mini"]));
        let models = registry.models_by_provider("openai");
        assert_eq!(models.len(), 2);
        let models = registry.models_by_provider("unknown");
        assert!(models.is_empty());
    }

    #[test]
    fn test_registry_alias() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o"]));
        registry.add_alias("smart", "gpt-4o");
        assert_eq!(registry.resolve_alias("smart"), Some("gpt-4o"));
        assert!(registry.resolve_alias("unknown").is_none());
        let model = registry.find_model("smart");
        assert!(model.is_some());
    }

    #[test]
    fn test_registry_default_model() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o"]));
        assert!(registry.default_model().is_none());
        registry.set_default_model("gpt-4o");
        assert!(registry.default_model().is_some());
    }

    #[test]
    fn test_registry_role_model() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o", "gpt-4o-mini"]));
        registry.set_role_model(ModelRole::Smart, "gpt-4o");
        registry.set_role_model(ModelRole::Fast, "gpt-4o-mini");
        let smart = registry.role_model(ModelRole::Smart);
        assert!(smart.is_some());
        assert_eq!(smart.unwrap().id().as_str(), "gpt-4o");
        let fast = registry.role_model(ModelRole::Fast);
        assert!(fast.is_some());
        assert_eq!(fast.unwrap().id().as_str(), "gpt-4o-mini");
    }

    #[test]
    fn test_model_role_serde() {
        let role = ModelRole::Smart;
        let json = serde_json::to_string(&role).unwrap();
        let deserialized: ModelRole = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ModelRole::Smart);
    }

    #[test]
    fn test_registry_default() {
        let registry = ModelRegistry::default();
        assert!(registry.all_models().is_empty());
    }

    #[test]
    fn test_registry_provider_by_id() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o"]));
        let provider = registry.provider_by_id("openai");
        assert!(provider.is_some());
        let missing = registry.provider_by_id("unknown");
        assert!(missing.is_none());
    }

    #[test]
    fn test_registry_all_models() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o", "gpt-4o-mini"]));
        registry.register(make_provider("anthropic", &["claude-3"]));
        let all = registry.all_models();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_registry_providers() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o"]));
        registry.register(make_provider("anthropic", &["claude-3"]));
        assert_eq!(registry.providers().len(), 2);
    }

    #[test]
    fn test_registry_default_model_none_when_no_models() {
        let registry = ModelRegistry::new();
        assert!(registry.default_model().is_none());
    }

    #[test]
    fn test_registry_default_model_none_when_set_but_not_found() {
        let mut registry = ModelRegistry::new();
        registry.set_default_model("nonexistent");
        assert!(registry.default_model().is_none());
    }

    #[test]
    fn test_registry_role_model_none_when_not_set() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o"]));
        assert!(registry.role_model(ModelRole::Smart).is_none());
    }

    #[test]
    fn test_registry_role_model_none_when_set_but_not_found() {
        let mut registry = ModelRegistry::new();
        registry.set_role_model(ModelRole::Smart, "nonexistent");
        assert!(registry.role_model(ModelRole::Smart).is_none());
    }

    #[test]
    fn test_registry_find_model_via_alias() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o"]));
        registry.add_alias("best", "gpt-4o");
        let model = registry.find_model("best");
        assert!(model.is_some());
        assert_eq!(model.unwrap().id().as_str(), "gpt-4o");
    }

    #[test]
    fn test_registry_find_model_no_alias_match() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o"]));
        let model = registry.find_model("unknown");
        assert!(model.is_none());
    }

    #[test]
    fn test_model_role_all_variants_serde() {
        for role in [
            ModelRole::Default,
            ModelRole::Fast,
            ModelRole::Smart,
            ModelRole::Summary,
            ModelRole::Embedding,
        ] {
            let json = serde_json::to_string(&role).unwrap();
            let back: ModelRole = serde_json::from_str(&json).unwrap();
            assert_eq!(role, back);
        }
    }

    #[test]
    fn test_registry_unregister_nonexistent() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["gpt-4o"]));
        registry.unregister("nonexistent");
        assert_eq!(registry.providers().len(), 1);
    }

    #[test]
    fn test_registry_models_by_provider_empty() {
        let registry = ModelRegistry::new();
        assert!(registry.models_by_provider("anything").is_empty());
    }

    #[test]
    fn test_registry_register_embedding_and_find() {
        let mut registry = ModelRegistry::new();
        registry.register(make_provider("openai", &["text-embedding-3-small"]));
        registry.register_embedding("openai", "text-embedding-3-small");
        let model = registry.embedding_model();
        assert!(model.is_some());
        assert_eq!(model.unwrap().id().as_str(), "text-embedding-3-small");
    }

    #[test]
    fn test_registry_embedding_model_none_when_not_set() {
        let registry = ModelRegistry::new();
        assert!(registry.embedding_model().is_none());
    }
}
