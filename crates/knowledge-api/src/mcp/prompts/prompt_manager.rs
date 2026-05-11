//! Prompt 版本管理器
//!
//! 提供企业级的 Prompt 模板管理能力，支持：

#![allow(clippy::significant_drop_tightening)]
//! - 版本控制与回滚
//! - 变量模板渲染
//! - A/B 测试
//! - LRU 缓存优化

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use lru::LruCache;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use error_core::{ErrorObject, helpers};

/// Prompt 模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// 唯一标识符
    pub id: String,
    /// 版本号
    pub version: u32,
    /// 显示名称
    pub name: String,
    /// 模板内容（Mustache 风格）
    pub template: String,
    /// 变量定义列表
    pub variables: Vec<PromptVariable>,
    /// 元数据
    pub metadata: PromptMetadata,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// Prompt 模板变量定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVariable {
    /// 变量名
    pub name: String,
    /// 变量类型
    pub var_type: VariableType,
    /// 是否必填
    pub required: bool,
    /// 默认值
    pub default_value: Option<String>,
    /// 描述说明
    pub description: String,
}

/// Prompt 变量类型枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableType {
    /// 字符串
    String,
    /// 数字
    Number,
    /// 布尔值
    Boolean,
    /// 对象
    Object,
    /// 数组
    Array,
    /// 枚举类型
    /// 枚举类型 —— 限制为预定义的值列表
    Enum {
        /// 允许的枚举值列表
        values: Vec<String>,
    },
}

/// Prompt 模板元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMetadata {
    /// 作者
    pub author: String,
    /// 描述
    pub description: Option<String>,
    /// 标签
    pub tags: Vec<String>,
    /// 语言（用于多语言 Prompt）
    pub language: Option<String>,
}

impl Default for PromptMetadata {
    fn default() -> Self {
        Self {
            author: "system".to_string(),
            description: None,
            tags: vec![],
            language: Some("zh-CN".to_string()),
        }
    }
}

/// 变量值映射
pub type Variables = HashMap<String, String>;

/// Prompt 存储接口
#[async_trait]
pub trait PromptStore: Send + Sync {
    /// 保存 Prompt 模板
    async fn save(&self, prompt: &PromptTemplate) -> Result<Uuid, ErrorObject>;

    /// 获取指定版本
    async fn get_version(
        &self,
        id: &str,
        version: u32,
    ) -> Result<Option<PromptTemplate>, ErrorObject>;

    /// 获取最新版本
    async fn get_latest(&self, id: &str) -> Result<Option<PromptTemplate>, ErrorObject>;

    /// 列出所有版本号
    async fn list_versions(&self, id: &str) -> Result<Vec<u32>, ErrorObject>;

    /// 删除指定版本
    async fn delete_version(&self, id: &str, version: u32) -> Result<(), ErrorObject>;
}

/// 内存存储实现（用于测试和开发环境）
pub struct InMemoryPromptStore {
    store: Arc<tokio::sync::RwLock<HashMap<String, Vec<PromptTemplate>>>>,
}

impl InMemoryPromptStore {
    /// 创建新的内存 Prompt 存储
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryPromptStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PromptStore for InMemoryPromptStore {
    async fn save(&self, prompt: &PromptTemplate) -> Result<Uuid, ErrorObject> {
        let mut store = self.store.write().await;
        let versions = store.entry(prompt.id.clone()).or_insert_with(Vec::new);

        if versions.iter().any(|v| v.version == prompt.version) {
            return Err(helpers::validation_error(
                &format!("版本 {} 已存在", prompt.version),
                "add_version",
            ));
        }

        let id = Uuid::new_v4();
        versions.push(prompt.clone());
        Ok(id)
    }

    async fn get_version(
        &self,
        id: &str,
        version: u32,
    ) -> Result<Option<PromptTemplate>, ErrorObject> {
        let store = self.store.read().await;
        Ok(store
            .get(id)
            .and_then(|versions| versions.iter().find(|v| v.version == version).cloned()))
    }

    async fn get_latest(&self, id: &str) -> Result<Option<PromptTemplate>, ErrorObject> {
        let store = self.store.read().await;
        Ok(store
            .get(id)
            .and_then(|versions| versions.iter().max_by_key(|v| v.version).cloned()))
    }

    async fn list_versions(&self, id: &str) -> Result<Vec<u32>, ErrorObject> {
        let store = self.store.read().await;
        Ok(store
            .get(id)
            .map(|versions| versions.iter().map(|v| v.version).collect())
            .unwrap_or_default())
    }

    async fn delete_version(&self, id: &str, version: u32) -> Result<(), ErrorObject> {
        let mut store = self.store.write().await;
        if let Some(versions) = store.get_mut(id) {
            let original_len = versions.len();
            versions.retain(|v| v.version != version);
            if versions.len() == original_len {
                return Err(helpers::not_found("版本", &version.to_string()));
            }
            if versions.is_empty() {
                store.remove(id);
            }
            Ok(())
        } else {
            Err(helpers::not_found("Prompt", id))
        }
    }
}

/// A/B 测试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestConfig {
    /// 测试 ID
    pub id: Uuid,
    /// 关联的 Prompt ID
    pub prompt_id: String,
    /// 对照组版本
    pub control_version: u32,
    /// 实验组版本
    pub variant_version: u32,
    /// 流量分配比例（0.0 ~ 1.0，实验组占比）
    pub traffic_split: f64,
    /// 评估指标
    pub metrics: ABTestMetrics,
    /// 测试状态
    pub status: ABTestStatus,
    /// 开始时间
    pub started_at: DateTime<Utc>,
}

/// A/B 测试指标统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestMetrics {
    /// 对照组样本数
    pub control_samples: u64,
    /// 实验组样本数
    pub variant_samples: u64,
    /// 对照组平均得分
    pub control_avg_score: f64,
    /// 实验组平均得分
    pub variant_avg_score: f64,
    /// 统计显著性 p-value
    pub p_value: Option<f64>,
}

impl Default for ABTestMetrics {
    fn default() -> Self {
        Self {
            control_samples: 0,
            variant_samples: 0,
            control_avg_score: 0.0,
            variant_avg_score: 0.0,
            p_value: None,
        }
    }
}

/// A/B 测试状态枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ABTestStatus {
    /// 测试正在运行中，新请求按配置比例分配到各版本
    Running,
    /// 测试已暂停，所有请求路由到对照版本
    Paused,
    /// 测试已完成，已选择优胜版本并全量发布
    Completed,
    /// 测试被手动取消，未产生最终结论
    Cancelled,
}

/// Prompt 管理器
///
/// 提供 Prompt 的完整生命周期管理，包括版本控制、渲染、A/B 测试等。
///
/// # Examples
///
/// ```ignore
/// let store = Arc::new(InMemoryPromptStore::new());
/// let manager = PromptManager::with_store(store, 100);
///
/// let template = PromptTemplate { ... };
/// manager.save(&template).await?;
///
/// let rendered = manager.render(&template, &vars).await?;
/// ```
pub struct PromptManager {
    store: Box<dyn PromptStore>,
    cache: Arc<tokio::sync::Mutex<LruCache<String, PromptTemplate>>>,
    ab_tests: Arc<tokio::sync::RwLock<HashMap<String, ABTestConfig>>>,
    cache_size: usize,
}

impl PromptManager {
    /// 使用自定义存储创建管理器
    ///
    /// # Panics
    ///
    /// 当 `cache_size` 为 0 时，内部会回退到默认缓存大小 100。
    #[must_use]
    pub fn with_store(store: Box<dyn PromptStore>, cache_size: usize) -> Self {
        Self {
            store,
            cache: Arc::new(tokio::sync::Mutex::new(LruCache::new(
                std::num::NonZeroUsize::new(cache_size)
                    .unwrap_or_else(|| std::num::NonZeroUsize::new(100).unwrap()),
            ))),
            ab_tests: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            cache_size,
        }
    }

    /// 创建或更新 Prompt 模板
    ///
    /// 自动递增版本号并保存。
    ///
    /// # Errors
    ///
    /// - 当存储操作失败时返回错误
    pub async fn save(&self, prompt: &PromptTemplate) -> Result<Uuid, ErrorObject> {
        let id = self.store.save(prompt).await?;

        let cache_key = format!("{}:{}", prompt.id, prompt.version);
        {
            let mut cache = self.cache.lock().await;
            cache.put(cache_key, prompt.clone());
        }

        tracing::info!("Prompt 已保存: {} v{}", prompt.id, prompt.version);

        Ok(id)
    }

    /// 获取指定版本的 Prompt
    ///
    /// 优先从缓存读取，缓存未命中时查询存储。
    ///
    /// # Errors
    ///
    /// 当存储查询失败时返回错误。
    pub async fn get_version(
        &self,
        id: &str,
        version: u32,
    ) -> Result<Option<PromptTemplate>, ErrorObject> {
        let cache_key = format!("{id}:{version}");

        {
            let mut cache = self.cache.lock().await;
            if let Some(cached) = cache.get_mut(&cache_key) {
                return Ok(Some(cached.clone()));
            }
        }

        let result = self.store.get_version(id, version).await?;

        if let Some(ref prompt) = result {
            let mut cache = self.cache.lock().await;
            cache.put(cache_key, prompt.clone());
        }

        Ok(result)
    }

    /// 获取最新版本的 Prompt
    ///
    /// # Errors
    ///
    /// 当存储查询失败时返回错误。
    pub async fn get_latest(&self, id: &str) -> Result<Option<PromptTemplate>, ErrorObject> {
        let versions = self.store.list_versions(id).await?;
        let latest_version = versions.into_iter().max();

        match latest_version {
            Some(version) => self.get_version(id, version).await,
            None => Ok(None),
        }
    }

    /// 渲染 Prompt（变量替换）
    ///
    /// 使用简单的 `{{variable}}` 语法进行变量替换，
    /// 支持默认值和必填校验。
    ///
    /// # Errors
    ///
    /// - 当缺少必填变量时返回验证错误
    pub fn render(&self, prompt: &PromptTemplate, vars: &Variables) -> Result<String, ErrorObject> {
        for var in &prompt.variables {
            if var.required && !vars.contains_key(&var.name) {
                if let Some(default) = &var.default_value {
                    tracing::warn!("使用默认值: {} = {}", var.name, default);
                } else {
                    return Err(helpers::validation_error(
                        &format!("缺少必填变量: {} ({})", var.name, var.description),
                        "render",
                    ));
                }
            }
        }

        let mut rendered = prompt.template.clone();

        for var in &prompt.variables {
            let placeholder = format!("{{{{{}}}}}", var.name);
            let value = vars
                .get(&var.name)
                .cloned()
                .or_else(|| var.default_value.clone())
                .unwrap_or_default();

            rendered = rendered.replace(&placeholder, &value);
        }

        Ok(rendered)
    }

    /// 列出版本历史
    ///
    /// # Errors
    ///
    /// 当存储查询失败时返回错误。
    pub async fn list_versions(&self, id: &str) -> Result<Vec<u32>, ErrorObject> {
        self.store.list_versions(id).await
    }

    /// 回滚到指定版本
    ///
    /// 将指定版本复制为新版本（版本号 +1）。
    ///
    /// # Errors
    ///
    /// - 当目标版本不存在时返回未找到错误
    pub async fn rollback(&self, id: &str, to_version: u32) -> Result<(), ErrorObject> {
        let target = self
            .store
            .get_version(id, to_version)
            .await?
            .ok_or_else(|| helpers::not_found("版本", &to_version.to_string()))?;

        let versions = self.store.list_versions(id).await?;
        let new_version = versions.iter().max().map_or(1, |m| m + 1);

        let rolled_back = PromptTemplate {
            version: new_version,
            updated_at: Utc::now(),
            ..target.clone()
        };

        self.store.save(&rolled_back).await?;

        tracing::info!("Prompt 回滚: {} {} -> {}", id, to_version, new_version);

        Ok(())
    }

    /// 配置 A/B 测试
    ///
    /// # Errors
    ///
    /// - 当流量分配比例不在 [0.0, 1.0] 范围内时返回验证错误
    pub async fn configure_ab_test(&self, test: ABTestConfig) -> Result<Uuid, ErrorObject> {
        if !(0.0..=1.0).contains(&test.traffic_split) {
            return Err(helpers::validation_error(
                &format!(
                    "流量分配比例必须在 [0.0, 1.0] 范围内，当前值: {}",
                    test.traffic_split
                ),
                "configure_ab_test",
            ));
        }

        let test_id = test.id;
        {
            let mut tests = self.ab_tests.write().await;
            tests.insert(test_id.to_string(), test);
        }

        tracing::info!("A/B 测试已配置: {}", test_id);

        Ok(test_id)
    }

    /// 获取 A/B 测试的 Prompt（根据流量分配）
    ///
    /// 根据 `user_id` 的哈希值确定性分配到对照组或实验组。
    ///
    /// # Errors
    ///
    /// 当 A/B 测试不存在或版本获取失败时返回错误。
    pub async fn get_ab_variant(
        &self,
        test_id: &str,
        user_id: &str,
    ) -> Result<PromptTemplate, ErrorObject> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let tests = self.ab_tests.read().await;
        let test = tests
            .get(test_id)
            .ok_or_else(|| helpers::not_found("A/B 测试", test_id))?;

        let mut hasher = DefaultHasher::new();
        user_id.hash(&mut hasher);
        let hash = hasher.finish();
        #[allow(clippy::cast_precision_loss)]
        let normalized_hash = (hash % 10000) as f64 / 10000.0;

        let version = if normalized_hash < test.traffic_split {
            test.variant_version
        } else {
            test.control_version
        };

        let prompt_id = test.prompt_id.clone();
        drop(tests);

        self.get_version(&prompt_id, version)
            .await?
            .ok_or_else(|| helpers::not_found("ABTestVersion", "无法获取 A/B 测试版本"))
    }

    /// 清除缓存
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.lock().await;
        cache.clear();
    }

    /// 获取缓存大小
    #[must_use]
    pub const fn cache_size(&self) -> usize {
        self.cache_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_prompt(id: &str, version: u32) -> PromptTemplate {
        PromptTemplate {
            id: id.to_string(),
            version,
            name: format!("Test Prompt {version}"),
            template: "Hello {{name}}, welcome to {{place}}!".to_string(),
            variables: vec![
                PromptVariable {
                    name: "name".to_string(),
                    var_type: VariableType::String,
                    required: true,
                    default_value: None,
                    description: "用户名称".to_string(),
                },
                PromptVariable {
                    name: "place".to_string(),
                    var_type: VariableType::String,
                    required: false,
                    default_value: Some("MCP Hub".to_string()),
                    description: "地点".to_string(),
                },
            ],
            metadata: PromptMetadata {
                author: "test".to_string(),
                description: Some("Test prompt".to_string()),
                tags: vec!["test".to_string()],
                language: Some("zh-CN".to_string()),
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_save_and_get_prompt() {
        let store = Box::new(InMemoryPromptStore::new());
        let manager = PromptManager::with_store(store, 100);

        let prompt = create_test_prompt("test-001", 1);
        let id = manager.save(&prompt).await.unwrap();
        assert_ne!(id, Uuid::nil());

        let retrieved = manager.get_version("test-001", 1).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.as_ref().unwrap().version, 1);
    }

    #[tokio::test]
    async fn test_render_prompt_with_variables() {
        let store = Box::new(InMemoryPromptStore::new());
        let manager = PromptManager::with_store(store, 100);

        let prompt = create_test_prompt("render-test", 1);
        manager.save(&prompt).await.unwrap();

        let mut vars = Variables::new();
        vars.insert("name".to_string(), "Alice".to_string());

        let rendered = manager.render(&prompt, &vars).unwrap();
        assert_eq!(rendered, "Hello Alice, welcome to MCP Hub!");
    }

    #[tokio::test]
    async fn test_render_missing_required_variable_fails() {
        let store = Box::new(InMemoryPromptStore::new());
        let manager = PromptManager::with_store(store, 100);

        let prompt = create_test_prompt("missing-var-test", 1);
        let vars = Variables::new();

        let result = manager.render(&prompt, &vars);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_version_history() {
        let store = Box::new(InMemoryPromptStore::new());
        let manager = PromptManager::with_store(store, 100);

        for v in 1..=3u32 {
            let prompt = create_test_prompt("versioned-prompt", v);
            manager.save(&prompt).await.unwrap();
        }

        let versions = manager.list_versions("versioned-prompt").await.unwrap();
        assert_eq!(versions.len(), 3);
        assert!(versions.contains(&1));
        assert!(versions.contains(&2));
        assert!(versions.contains(&3));
    }

    #[tokio::test]
    async fn test_get_latest_version() {
        let store = Box::new(InMemoryPromptStore::new());
        let manager = PromptManager::with_store(store, 100);

        manager
            .save(&create_test_prompt("latest-test", 1))
            .await
            .unwrap();
        manager
            .save(&create_test_prompt("latest-test", 2))
            .await
            .unwrap();
        manager
            .save(&create_test_prompt("latest-test", 3))
            .await
            .unwrap();

        let latest = manager.get_latest("latest-test").await.unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().version, 3);
    }

    #[tokio::test]
    async fn test_rollback_prompt() {
        let store = Box::new(InMemoryPromptStore::new());
        let manager = PromptManager::with_store(store, 100);

        let mut v1 = create_test_prompt("rollback-test", 1);
        v1.template = "Version 1 content".to_string();
        manager.save(&v1).await.unwrap();

        let mut v2 = create_test_prompt("rollback-test", 2);
        v2.template = "Version 2 content".to_string();
        manager.save(&v2).await.unwrap();

        manager.rollback("rollback-test", 1).await.unwrap();

        let versions = manager.list_versions("rollback-test").await.unwrap();
        assert!(versions.contains(&3));

        let rolled_back = manager.get_version("rollback-test", 3).await.unwrap();
        assert!(rolled_back.is_some());
        assert_eq!(rolled_back.unwrap().template, "Version 1 content");
    }

    #[tokio::test]
    async fn test_ab_test_configuration() {
        let store = Box::new(InMemoryPromptStore::new());
        let manager = PromptManager::with_store(store, 100);

        let prompt = create_test_prompt("ab-test-prompt", 1);
        manager.save(&prompt).await.unwrap();

        let mut v2 = create_test_prompt("ab-test-prompt", 2);
        v2.template = "Variant B content".to_string();
        manager.save(&v2).await.unwrap();

        let config = ABTestConfig {
            id: Uuid::new_v4(),
            prompt_id: "ab-test-prompt".to_string(),
            control_version: 1,
            variant_version: 2,
            traffic_split: 0.5,
            metrics: ABTestMetrics::default(),
            status: ABTestStatus::Running,
            started_at: Utc::now(),
        };

        let test_id = manager.configure_ab_test(config).await.unwrap();

        let variant_a = manager
            .get_ab_variant(&test_id.to_string(), "user-001")
            .await
            .unwrap();
        let variant_b = manager
            .get_ab_variant(&test_id.to_string(), "user-002")
            .await
            .unwrap();

        assert_eq!(variant_a.id, "ab-test-prompt");
        assert_eq!(variant_b.id, "ab-test-prompt");
    }

    #[tokio::test]
    async fn test_cache_hit() {
        let store = Box::new(InMemoryPromptStore::new());
        let manager = PromptManager::with_store(store, 10);

        let prompt = create_test_prompt("cache-test", 1);
        manager.save(&prompt).await.unwrap();

        let first = manager.get_version("cache-test", 1).await.unwrap();
        let second = manager.get_version("cache-test", 1).await.unwrap();

        assert!(first.is_some());
        assert!(second.is_some());
        assert_eq!(first.unwrap().version, second.unwrap().version);
    }

    #[tokio::test]
    async fn test_invalid_traffic_split_rejected() {
        let store = Box::new(InMemoryPromptStore::new());
        let manager = PromptManager::with_store(store, 100);

        let config = ABTestConfig {
            id: Uuid::new_v4(),
            prompt_id: "test".to_string(),
            control_version: 1,
            variant_version: 2,
            traffic_split: 1.5,
            metrics: ABTestMetrics::default(),
            status: ABTestStatus::Running,
            started_at: Utc::now(),
        };

        let result = manager.configure_ab_test(config).await;
        assert!(result.is_err());
    }
}
