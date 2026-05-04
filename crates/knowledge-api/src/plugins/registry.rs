use std::collections::HashMap;
use std::sync::Arc;

use super::agent_backend::AgentBackend;
use super::knowledge_source::KnowledgeSource;
use super::llm_provider::LLMProvider;
use super::quality_gate::QualityGatePlugin;
use super::storage_backend::StorageBackend;
use crate::Result;

/// 插件注册中心 — 运行时组装的核心
///
/// 开源 RAG 提供 Registry 和默认实现。
/// 闭源 runner 在启动时注册闭源插件。
///
/// # 核心设计原则
///
/// **开源定义接口，闭源实现接口，运行时组装。**
///
/// - 开源 RAG 定义"插座"（trait）
/// - 闭源模块制造"插头"（实现 trait）
/// - `PluginRegistry` 是"排插"
/// - `knowledge-runner` 是"电源"
/// - 插上就通电，拔了也能用默认功能
///
/// # 依赖方向（关键）
///
/// ```text
/// 开源 RAG ←── 闭源 knowledge-infra（依赖开源的 trait）
/// 开源 RAG ←── 闭源 upcm-agent（依赖开源的 trait）
/// 开源 RAG ←── 闭源 knowledge-runner（组装所有插件）
/// ```
///
/// **绝不能反向依赖**——开源 RAG 的 `Cargo.toml` 中永远不出现闭源 crate。
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::PluginRegistry;
/// use std::sync::Arc;
///
/// // 使用默认插件（开源版本可独立运行）
/// let registry = PluginRegistry::with_defaults();
///
/// // 注册闭源插件（闭源 runner 中）
/// let mut registry = PluginRegistry::with_defaults();
/// registry.register_source(Arc::new(GitKnowledgeSource::from_config(&config)?));
/// registry.register_gate(Arc::new(InfraQualityGate::from_config(&config)?));
/// registry.register_agent(Arc::new(UpcmAgentBackend::from_config(&config)?));
/// registry.register_llm_provider(Arc::new(EnhancedLLMProvider::from_config(&config)?));
/// ```
pub struct PluginRegistry {
    knowledge_sources: HashMap<String, Arc<dyn KnowledgeSource>>,
    quality_gates: HashMap<String, Arc<dyn QualityGatePlugin>>,
    agent_backends: HashMap<String, Arc<dyn AgentBackend>>,
    storage_backends: HashMap<String, Arc<dyn StorageBackend>>,
    llm_providers: HashMap<String, Arc<dyn LLMProvider>>,
    active_source: Option<String>,
    active_gate: Option<String>,
    active_agent: Option<String>,
    active_storage: Option<String>,
    active_llm: Option<String>,
}

impl PluginRegistry {
    /// 创建空的插件注册中心
    #[must_use]
    pub fn new() -> Self {
        Self {
            knowledge_sources: HashMap::new(),
            quality_gates: HashMap::new(),
            agent_backends: HashMap::new(),
            storage_backends: HashMap::new(),
            llm_providers: HashMap::new(),
            active_source: None,
            active_gate: None,
            active_agent: None,
            active_storage: None,
            active_llm: None,
        }
    }

    /// 创建包含默认插件的注册中心
    ///
    /// 默认插件保证开源 RAG 可独立编译运行：
    /// - [`LocalFileSource`](crate::plugins::defaults::LocalFileSource)：本地文件知识源
    /// - [`AlwaysPassGate`](crate::plugins::defaults::AlwaysPassGate)：全通过质量门禁
    /// - [`ReActAgentBackend`](crate::plugins::defaults::ReActAgentBackend)：基础 `ReAct` Agent
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register_source(Arc::new(
            crate::plugins::defaults::LocalFileSource::new(),
        ));
        registry.register_gate(Arc::new(
            crate::plugins::defaults::AlwaysPassGate::new(),
        ));
        registry.register_agent(Arc::new(
            crate::plugins::defaults::ReActAgentBackend::new(),
        ));
        registry
    }

    // ── 注册方法 ──────────────────────────────────────────────

    /// 注册知识源插件
    ///
    /// 首个注册的插件自动成为活跃插件。
    pub fn register_source(&mut self, source: Arc<dyn KnowledgeSource>) {
        let id = source.source_id().to_string();
        if self.active_source.is_none() {
            self.active_source = Some(id.clone());
        }
        self.knowledge_sources.insert(id, source);
    }

    /// 注册质量门禁插件
    ///
    /// 首个注册的插件自动成为活跃插件。
    pub fn register_gate(&mut self, gate: Arc<dyn QualityGatePlugin>) {
        let id = gate.gate_id().to_string();
        if self.active_gate.is_none() {
            self.active_gate = Some(id.clone());
        }
        self.quality_gates.insert(id, gate);
    }

    /// 注册 Agent 后端插件
    ///
    /// 首个注册的插件自动成为活跃插件。
    pub fn register_agent(&mut self, agent: Arc<dyn AgentBackend>) {
        let id = agent.backend_id().to_string();
        if self.active_agent.is_none() {
            self.active_agent = Some(id.clone());
        }
        self.agent_backends.insert(id, agent);
    }

    /// 注册存储后端插件
    ///
    /// 首个注册的插件自动成为活跃插件。
    pub fn register_storage(&mut self, storage: Arc<dyn StorageBackend>) {
        let id = storage.backend_id().to_string();
        if self.active_storage.is_none() {
            self.active_storage = Some(id.clone());
        }
        self.storage_backends.insert(id, storage);
    }

    /// 注册 LLM Provider 插件
    ///
    /// 首个注册的插件自动成为活跃插件。
    pub fn register_llm_provider(&mut self, provider: Arc<dyn LLMProvider>) {
        let id = format!("llm-{}", self.llm_providers.len());
        if self.active_llm.is_none() {
            self.active_llm = Some(id.clone());
        }
        self.llm_providers.insert(id, provider);
    }

    // ── 激活方法（切换活跃插件） ──────────────────────────────

    /// 切换活跃的知识源插件
    ///
    /// # Errors
    ///
    /// 当指定的 ID 不存在时返回 `not_found` 错误。
    pub fn set_active_source(&mut self, id: &str) -> Result<()> {
        if self.knowledge_sources.contains_key(id) {
            self.active_source = Some(id.to_string());
            Ok(())
        } else {
            Err(error_core::helpers::not_found("knowledge_source", id))
        }
    }

    /// 切换活跃的质量门禁插件
    ///
    /// # Errors
    ///
    /// 当指定的 ID 不存在时返回 `not_found` 错误。
    pub fn set_active_gate(&mut self, id: &str) -> Result<()> {
        if self.quality_gates.contains_key(id) {
            self.active_gate = Some(id.to_string());
            Ok(())
        } else {
            Err(error_core::helpers::not_found("quality_gate", id))
        }
    }

    /// 切换活跃的 Agent 后端插件
    ///
    /// # Errors
    ///
    /// 当指定的 ID 不存在时返回 `not_found` 错误。
    pub fn set_active_agent(&mut self, id: &str) -> Result<()> {
        if self.agent_backends.contains_key(id) {
            self.active_agent = Some(id.to_string());
            Ok(())
        } else {
            Err(error_core::helpers::not_found("agent_backend", id))
        }
    }

    /// 切换活跃的存储后端插件
    ///
    /// # Errors
    ///
    /// 当指定的 ID 不存在时返回 `not_found` 错误。
    pub fn set_active_storage(&mut self, id: &str) -> Result<()> {
        if self.storage_backends.contains_key(id) {
            self.active_storage = Some(id.to_string());
            Ok(())
        } else {
            Err(error_core::helpers::not_found("storage_backend", id))
        }
    }

    /// 切换活跃的 LLM Provider 插件
    ///
    /// # Errors
    ///
    /// 当指定的 ID 不存在时返回 `not_found` 错误。
    pub fn set_active_llm_provider(&mut self, id: &str) -> Result<()> {
        if self.llm_providers.contains_key(id) {
            self.active_llm = Some(id.to_string());
            Ok(())
        } else {
            Err(error_core::helpers::not_found("llm_provider", id))
        }
    }

    // ── 查询方法 ──────────────────────────────────────────────

    /// 获取当前活跃的知识源插件
    #[must_use]
    pub fn active_source(&self) -> Option<&Arc<dyn KnowledgeSource>> {
        self.active_source
            .as_ref()
            .and_then(|id| self.knowledge_sources.get(id))
    }

    /// 获取当前活跃的质量门禁插件
    #[must_use]
    pub fn active_gate(&self) -> Option<&Arc<dyn QualityGatePlugin>> {
        self.active_gate
            .as_ref()
            .and_then(|id| self.quality_gates.get(id))
    }

    /// 获取当前活跃的 Agent 后端插件
    #[must_use]
    pub fn active_agent(&self) -> Option<&Arc<dyn AgentBackend>> {
        self.active_agent
            .as_ref()
            .and_then(|id| self.agent_backends.get(id))
    }

    /// 获取当前活跃的存储后端插件
    #[must_use]
    pub fn active_storage(&self) -> Option<&Arc<dyn StorageBackend>> {
        self.active_storage
            .as_ref()
            .and_then(|id| self.storage_backends.get(id))
    }

    /// 获取当前活跃的 LLM Provider 插件
    #[must_use]
    pub fn active_llm_provider(&self) -> Option<&Arc<dyn LLMProvider>> {
        self.active_llm
            .as_ref()
            .and_then(|id| self.llm_providers.get(id))
    }

    // ── 列表方法 ──────────────────────────────────────────────

    /// 列出所有已注册的知识源 ID
    #[must_use]
    pub fn list_sources(&self) -> Vec<&str> {
        self.knowledge_sources.keys().map(String::as_str).collect()
    }

    /// 列出所有已注册的质量门禁 ID
    #[must_use]
    pub fn list_gates(&self) -> Vec<&str> {
        self.quality_gates.keys().map(String::as_str).collect()
    }

    /// 列出所有已注册的 Agent 后端 ID
    #[must_use]
    pub fn list_agents(&self) -> Vec<&str> {
        self.agent_backends.keys().map(String::as_str).collect()
    }

    /// 列出所有已注册的存储后端 ID
    #[must_use]
    pub fn list_storages(&self) -> Vec<&str> {
        self.storage_backends.keys().map(String::as_str).collect()
    }

    /// 列出所有已注册的 LLM Provider ID
    #[must_use]
    pub fn list_llm_providers(&self) -> Vec<&str> {
        self.llm_providers.keys().map(String::as_str).collect()
    }

    /// 获取活跃知识源的 ID
    #[must_use]
    pub fn active_source_id(&self) -> Option<&str> {
        self.active_source.as_deref()
    }

    /// 获取活跃质量门禁的 ID
    #[must_use]
    pub fn active_gate_id(&self) -> Option<&str> {
        self.active_gate.as_deref()
    }

    /// 获取活跃 Agent 后端的 ID
    #[must_use]
    pub fn active_agent_id(&self) -> Option<&str> {
        self.active_agent.as_deref()
    }

    /// 获取活跃存储后端的 ID
    #[must_use]
    pub fn active_storage_id(&self) -> Option<&str> {
        self.active_storage.as_deref()
    }

    /// 获取活跃 LLM Provider 的 ID
    #[must_use]
    pub fn active_llm_provider_id(&self) -> Option<&str> {
        self.active_llm.as_deref()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new_is_empty() {
        let registry = PluginRegistry::new();
        assert!(registry.list_sources().is_empty());
        assert!(registry.list_gates().is_empty());
        assert!(registry.list_agents().is_empty());
        assert!(registry.list_storages().is_empty());
        assert!(registry.list_llm_providers().is_empty());
        assert!(registry.active_source().is_none());
        assert!(registry.active_gate().is_none());
        assert!(registry.active_agent().is_none());
        assert!(registry.active_storage().is_none());
        assert!(registry.active_llm_provider().is_none());
    }

    #[test]
    fn test_registry_default_is_empty() {
        let registry = PluginRegistry::default();
        assert!(registry.list_sources().is_empty());
    }

    #[test]
    fn test_with_defaults_registers_three_plugins() {
        let registry = PluginRegistry::with_defaults();
        assert_eq!(registry.list_sources().len(), 1);
        assert_eq!(registry.list_gates().len(), 1);
        assert_eq!(registry.list_agents().len(), 1);
        assert!(registry.active_source().is_some());
        assert!(registry.active_gate().is_some());
        assert!(registry.active_agent().is_some());
    }

    #[test]
    fn test_with_defaults_active_ids() {
        let registry = PluginRegistry::with_defaults();
        assert_eq!(registry.active_source_id(), Some("local-file"));
        assert_eq!(registry.active_gate_id(), Some("always-pass"));
        assert_eq!(registry.active_agent_id(), Some("react"));
    }

    #[test]
    fn test_set_active_source_nonexistent_returns_error() {
        let mut registry = PluginRegistry::new();
        let result = registry.set_active_source("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_set_active_agent_nonexistent_returns_error() {
        let mut registry = PluginRegistry::new();
        let result = registry.set_active_agent("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_set_active_llm_provider_nonexistent_returns_error() {
        let mut registry = PluginRegistry::new();
        let result = registry.set_active_llm_provider("nonexistent");
        assert!(result.is_err());
    }
}
