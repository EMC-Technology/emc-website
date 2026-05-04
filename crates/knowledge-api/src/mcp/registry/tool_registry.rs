//! `` `MCP` `` 动态工具注册中心
//!
//! 提供符合 `` `MCP` `` 规范的工具注册、发现和管理能力，支持：

#![allow(clippy::significant_drop_tightening)]
//! - 运行时动态注册/注销工具
//! - 基于标签和类别的工具发现
//! - 工具调用统计与监控
//! - 速率限制集成
//! - 认证需求声明

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use error_core::ErrorObject;
use error_core::helpers;

/// 工具定义（符合 `MCP` 规范）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolDefinition {
    #[serde(rename = "name")]
    /// 工具唯一标识名称，在注册中心内全局唯一
    pub name: String,

    #[serde(rename = "description")]
    /// 工具功能描述，供调用方理解工具用途
    pub description: Option<String>,

    #[serde(rename = "inputSchema")]
    /// 输入参数的 JSON Schema 定义，用于参数验证
    pub input_schema: serde_json::Value,

    #[serde(default)]
    /// 工具能力声明
    pub capabilities: ToolCapabilities,

    #[serde(default)]
    /// 速率限制配置
    pub rate_limits: RateLimitConfig,

    #[serde(default)]
    /// 认证需求声明
    pub auth_requirements: AuthRequirements,

    /// 工具元数据
    pub metadata: ToolMetadata,
}

/// JSON Schema 根节点类型别名
pub type RootSchema = serde_json::Value;

/// 工具能力声明，描述工具支持的运行时特性
#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
#[allow(clippy::struct_excessive_bools)]
pub struct ToolCapabilities {
    /// 是否支持流式输出
    pub streaming: bool,
    /// 是否支持阻塞式调用
    pub blocking: bool,
    /// 是否具有幂等性（相同输入始终产生相同结果）
    pub idempotent: bool,
    /// 是否已弃用
    pub deprecated: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests_per_minute: 60,
            max_tokens_per_request: 4096,
            burst_limit: 10,
        }
    }
}

/// MCP 工具速率限制配置
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RateLimitConfig {
    /// 每分钟最大请求数
    pub max_requests_per_minute: u32,
    /// 每次请求最大 token 数
    pub max_tokens_per_request: u32,
    /// 突发限制
    pub burst_limit: u32,
}

/// MCP 工具认证要求
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AuthRequirements {
    /// 所需权限范围
    pub required_scopes: Vec<String>,
    /// 是否允许匿名访问
    pub allow_anonymous: bool,
}

impl Default for AuthRequirements {
    fn default() -> Self {
        Self {
            required_scopes: vec![],
            allow_anonymous: true,
        }
    }
}

/// MCP 工具元数据（版本、作者、分类等）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolMetadata {
    /// 版本号
    pub version: String,
    /// 作者
    pub author: String,
    /// 创建时间
    #[schemars(with = "String")]
    pub created_at: DateTime<Utc>,
    /// 更新时间
    #[schemars(with = "String")]
    pub updated_at: DateTime<Utc>,
    /// 标签列表
    pub tags: Vec<String>,
    /// 工具类别
    pub category: ToolCategory,
}

/// MCP 工具分类枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
pub enum ToolCategory {
    /// 知识图谱操作
    Knowledge,
    /// 搜索与检索
    Search,
    /// 文档处理
    Document,
    /// 向量嵌入
    Embedding,
    /// 数据分析
    Analysis,
    /// 系统管理
    System,
    /// 自定义工具
    #[default]
    Custom,
}

/// 已注册的内部工具状态
struct RegisteredTool {
    definition: ToolDefinition,
    registered_at: DateTime<Utc>,
    call_count: u64,
    last_called_at: Option<DateTime<Utc>>,
    status: ToolStatus,
}

/// 工具状态，表示工具在注册中心中的生命周期阶段
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ToolStatus {
    /// 活跃状态，工具可正常调用
    #[default]
    Active,
    /// 已弃用状态，工具仍可调用但建议迁移
    Deprecated,
    /// 已禁用状态，工具不可调用
    Disabled,
}

/// 注册中心指标
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_field_names)]
pub struct RegistryMetrics {
    total_registrations: u64,
    total_unregistrations: u64,
    total_calls: u64,
    total_errors: u64,
}

/// 工具过滤器
#[derive(Debug, Clone, Default)]
pub struct ToolFilter {
    /// 按类别过滤
    pub category: Option<ToolCategory>,
    /// 按标签过滤（匹配任一标签）
    pub tags: Option<Vec<String>>,
    /// 仅返回活跃工具
    pub active_only: bool,
    /// 支持流式的工具
    pub streaming_only: bool,
    /// 关键词搜索（匹配名称或描述）
    pub keyword: Option<String>,
}

/// 工具使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct ToolStats {
    /// 工具名称
    pub tool_name: String,
    /// 总调用次数
    pub call_count: u64,
    /// 最后调用时间
    pub last_called_at: Option<DateTime<Utc>>,
    /// 注册时间
    pub registered_at: DateTime<Utc>,
    /// 当前状态
    pub status: String,
}

/// 调用上下文
#[derive(Debug, Clone)]
pub struct CallContext {
    /// 会话 ID
    pub session_id: Uuid,
    /// 用户 ID（可选）
    pub user_id: Option<String>,
    /// 请求 ID
    pub request_id: Uuid,
    /// 追踪 ID
    pub trace_id: String,
    /// 超时时间
    pub timeout: Duration,
}

/// 工具调用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// 内容块列表
    pub content: Vec<ContentBlock>,
    /// 是否为错误结果
    pub is_error: bool,
    /// 调用元数据
    pub metadata: CallMetadata,
}

/// 内容块，表示工具调用结果中的结构化内容单元
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentBlock {
    /// 文本内容
    Text {
        /// 文本内容
        text: String,
    },
    /// 图片内容
    Image {
        /// 图片的 Base64 编码数据
        data: String,
        /// 图片的 MIME 类型
        mime_type: String,
    },
    /// 资源引用
    Resource {
        /// 资源的 URI 标识
        uri: String,
        /// 资源的 MIME 类型
        mime_type: String,
    },
    /// 内嵌资源
    #[serde(rename = "embeddedResource")]
    EmbeddedResource {
        /// 内嵌的资源对象
        resource: EmbeddedResource,
    },
}

/// 内嵌资源，表示工具返回结果中引用的子资源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedResource {
    /// 资源的唯一标识 URI
    pub uri: String,
    /// 资源的可读名称（可选）
    pub name: Option<String>,
    /// 资源的 MIME 类型
    pub mime_type: String,
    /// 资源的描述信息（可选）
    pub description: Option<String>,
}

/// 调用元数据，记录工具调用的性能与缓存信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallMetadata {
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 使用的 token 数
    pub tokens_used: u32,
    /// 是否来自缓存
    pub cached: bool,
}

/// 工具处理器 trait
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// 处理工具调用
    async fn handle(
        &self,
        args: serde_json::Value,
        ctx: CallContext,
    ) -> std::result::Result<ToolCallResult, ErrorObject>;

    /// 验证参数
    ///
    /// # Errors
    ///
    /// 当参数不符合 schema 时返回错误。
    fn validate_args(
        &self,
        _schema: &serde_json::Value,
        _args: &serde_json::Value,
    ) -> std::result::Result<(), ErrorObject> {
        Ok(())
    }
}

/// 动态工具注册中心
///
/// 提供线程安全的工具注册表，支持运行时动态添加/移除工具，
/// 并提供基于 `DashMap` 的高并发读取性能。
///
/// # Examples
///
/// ```ignore
/// let registry = ToolRegistry::new();
///
/// let definition = ToolDefinition { ... };
/// let handler = Arc::new(MyHandler);
///
/// let tool_id = registry.register_tool(definition, handler).await?;
/// let result = registry.call_tool("my_tool", args, ctx).await?;
/// ```
pub struct ToolRegistry {
    tools: DashMap<String, RegisteredTool>,
    handlers: DashMap<String, Arc<dyn ToolHandler>>,
    metrics: Arc<RwLock<RegistryMetrics>>,
}

impl ToolRegistry {
    /// 创建新的工具注册中心
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: DashMap::new(),
            handlers: DashMap::new(),
            metrics: Arc::new(RwLock::new(RegistryMetrics::default())),
        }
    }

    /// 注册新工具
    ///
    /// # Errors
    ///
    /// - 当工具名称已存在时返回冲突错误
    /// - 当处理器验证失败时返回验证错误
    pub async fn register_tool(
        &self,
        definition: ToolDefinition,
        handler: Arc<dyn ToolHandler>,
    ) -> Result<Uuid, ErrorObject> {
        let tool_name = definition.name.clone();
        let tool_id = Uuid::new_v4();

        if self.tools.contains_key(&tool_name) {
            return Err(helpers::validation_error(
                &format!("工具已注册，不允许重复注册: {tool_name}"),
                "register_tool",
            ));
        }

        let registered_tool = RegisteredTool {
            registered_at: Utc::now(),
            call_count: 0,
            last_called_at: None,
            status: ToolStatus::Active,
            definition,
        };

        self.tools.insert(tool_name.clone(), registered_tool);
        self.handlers.insert(tool_name.clone(), handler);

        {
            let mut metrics = self.metrics.write().await;
            metrics.total_registrations += 1;
        }

        tracing::info!("工具已注册: {} (id={})", tool_name, tool_id);

        Ok(tool_id)
    }

    /// 注销工具
    ///
    /// # Errors
    ///
    /// - 当工具不存在时返回未找到错误
    pub async fn unregister_tool(&self, name: &str) -> Result<(), ErrorObject> {
        let removed_tool = self.tools.remove(name);
        let removed_handler = self.handlers.remove(name);

        match (removed_tool, removed_handler) {
            (Some(_), Some(_)) => {
                let mut metrics = self.metrics.write().await;
                metrics.total_unregistrations += 1;

                tracing::info!("工具已注销: {}", name);
                Ok(())
            }
            _ => Err(helpers::not_found("工具", name)),
        }
    }

    /// 调用工具
    ///
    /// 执行完整的调用流程：
    /// 1. 检查工具是否存在且活跃
    /// 2. 验证参数
    /// 3. 调用处理器
    /// 4. 更新统计信息
    ///
    /// # Errors
    ///
    /// - 当工具不存在或已禁用时返回错误
    /// - 当参数验证失败时返回验证错误
    /// - 当处理器执行失败时返回处理器错误
    ///
    /// # Panics
    ///
    /// 当工具在注册表中存在但无法获取可变引用时 panic（逻辑上不应发生）。
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
        context: CallContext,
    ) -> Result<ToolCallResult, ErrorObject> {
        let tool_entry = self
            .tools
            .get(name)
            .ok_or_else(|| helpers::not_found("工具", name))?;

        match &tool_entry.status {
            ToolStatus::Disabled => {
                return Err(helpers::auth_error(
                    &format!("工具 '{name}' 已被禁用"),
                    "call_tool",
                ));
            }
            ToolStatus::Deprecated => {
                tracing::warn!("调用已弃用的工具: {name}");
            }
            ToolStatus::Active => {}
        }

        let definition = &tool_entry.definition;

        let handler = self
            .handlers
            .get(name)
            .ok_or_else(|| helpers::not_found("ToolHandler", &format!("工具 '{name}' 缺少处理器")))?;

        handler.validate_args(&definition.input_schema, &arguments)?;

        let start = std::time::Instant::now();
        let result = handler.handle(arguments, context).await;
        #[allow(clippy::cast_possible_truncation)]
        let _duration_ms = start.elapsed().as_millis() as u64;

        {
            if let Some(mut tool_mut) = self.tools.get_mut(name) {
                tool_mut.call_count += 1;
                tool_mut.last_called_at = Some(Utc::now());
            } else {
                tracing::warn!("工具 '{name}' 在执行期间被注销，跳过指标更新");
            }
        }

        {
            let mut metrics = self.metrics.write().await;
            metrics.total_calls += 1;
            if result.is_err() {
                metrics.total_errors += 1;
            }
        }

        result
    }

    /// 列出所有可用工具
    #[must_use]
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .filter(|entry| matches!(entry.status, ToolStatus::Active))
            .map(|entry| entry.definition.clone())
            .collect()
    }

    /// 获取工具详情
    #[must_use]
    pub fn get_tool(&self, name: &str) -> Option<ToolDefinition> {
        self.tools.get(name).map(|entry| entry.definition.clone())
    }

    /// 发现工具（按过滤器条件）
    #[must_use]
    pub fn discover_tools(&self, filter: &ToolFilter) -> Vec<ToolDefinition> {
        let mut results: Vec<ToolDefinition> = self
            .tools
            .iter()
            .filter(|entry| {
                if filter.active_only && !matches!(entry.status, ToolStatus::Active) {
                    return false;
                }

                if let Some(ref cat) = filter.category {
                    if entry.definition.metadata.category != *cat {
                        return false;
                    }
                }

                if filter.streaming_only && !entry.definition.capabilities.streaming {
                    return false;
                }

                if let Some(ref tags) = filter.tags {
                    let has_matching_tag = tags
                        .iter()
                        .any(|tag| entry.definition.metadata.tags.iter().any(|t| t == tag));
                    if !has_matching_tag {
                        return false;
                    }
                }

                if let Some(ref keyword) = filter.keyword {
                    let name_match = entry
                        .definition
                        .name
                        .to_lowercase()
                        .contains(&keyword.to_lowercase());
                    let desc_match = entry
                        .definition
                        .description
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&keyword.to_lowercase());
                    if !name_match && !desc_match {
                        return false;
                    }
                }

                true
            })
            .map(|entry| entry.definition.clone())
            .collect();

        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    }

    /// 获取工具使用统计
    #[must_use]
    pub fn get_tool_stats(&self, name: &str) -> Option<ToolStats> {
        self.tools.get(name).map(|entry| ToolStats {
            tool_name: name.to_string(),
            call_count: entry.call_count,
            last_called_at: entry.last_called_at,
            registered_at: entry.registered_at,
            status: format!("{:?}", entry.status),
        })
    }

    /// 获取注册中心整体指标
    #[must_use]
    pub fn get_registry_metrics(&self) -> RegistryMetrics {
        self.metrics.blocking_read().clone()
    }

    /// 获取已注册工具数量
    #[must_use]
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// 更新工具状态
    ///
    /// # Errors
    ///
    /// - 当工具不存在时返回未找到错误
    pub fn set_tool_status(&self, name: &str, status: &ToolStatus) -> Result<(), ErrorObject> {
        let mut tool = self
            .tools
            .get_mut(name)
            .ok_or_else(|| helpers::not_found("工具", name))?;

        tool.status = status.clone();
        tracing::info!("工具状态更新: {} -> {:?}", name, status);

        Ok(())
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::QueryParams;

    struct MockHandler;

    #[async_trait]
    impl ToolHandler for MockHandler {
        async fn handle(
            &self,
            _args: serde_json::Value,
            _ctx: CallContext,
        ) -> std::result::Result<ToolCallResult, ErrorObject> {
            Ok(ToolCallResult {
                content: vec![ContentBlock::Text {
                    text: "mock result".to_string(),
                }],
                is_error: false,
                metadata: CallMetadata {
                    duration_ms: 10,
                    tokens_used: 5,
                    cached: false,
                },
            })
        }
    }

    fn create_test_definition(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: Some(format!("Test tool: {name}")),
            input_schema: serde_json::to_value(schemars::schema_for!(QueryParams))
                .unwrap_or_default(),
            capabilities: ToolCapabilities::default(),
            rate_limits: RateLimitConfig::default(),
            auth_requirements: AuthRequirements::default(),
            metadata: ToolMetadata {
                version: "1.0.0".to_string(),
                author: "test".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                tags: vec!["test".to_string()],
                category: ToolCategory::Custom,
            },
        }
    }

    #[tokio::test]
    async fn test_register_and_call_tool() {
        let registry = ToolRegistry::new();
        let def = create_test_definition("test_tool");
        let handler = Arc::new(MockHandler);

        let tool_id = registry.register_tool(def, handler).await.unwrap();
        assert_ne!(tool_id, Uuid::nil());

        let ctx = CallContext {
            session_id: Uuid::new_v4(),
            user_id: None,
            request_id: Uuid::new_v4(),
            trace_id: "test-trace".to_string(),
            timeout: Duration::from_secs(30),
        };

        let result = registry
            .call_tool("test_tool", serde_json::json!({}), ctx)
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
    }

    #[tokio::test]
    async fn test_duplicate_registration_fails() {
        let registry = ToolRegistry::new();
        let def = create_test_definition("dup_tool");
        let handler = Arc::new(MockHandler);

        registry
            .register_tool(def.clone(), handler.clone())
            .await
            .unwrap();

        let result = registry.register_tool(def, handler).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unregister_tool() {
        let registry = ToolRegistry::new();
        let def = create_test_definition("temp_tool");
        let handler = Arc::new(MockHandler);

        registry.register_tool(def, handler).await.unwrap();
        registry.unregister_tool("temp_tool").await.unwrap();

        assert!(registry.get_tool("temp_tool").is_none());
    }

    #[tokio::test]
    async fn test_list_and_discover_tools() {
        let registry = ToolRegistry::new();

        let mut def1 = create_test_definition("search_tool");
        def1.metadata.category = ToolCategory::Search;
        def1.metadata.tags = vec!["search".to_string(), "fast".to_string()];
        registry
            .register_tool(def1, Arc::new(MockHandler))
            .await
            .unwrap();

        let mut def2 = create_test_definition("analysis_tool");
        def2.metadata.category = ToolCategory::Analysis;
        def2.description = Some("Deep analysis tool".to_string());
        registry
            .register_tool(def2, Arc::new(MockHandler))
            .await
            .unwrap();

        let all_tools = registry.list_tools();
        assert_eq!(all_tools.len(), 2);

        let search_filter = ToolFilter {
            category: Some(ToolCategory::Search),
            ..Default::default()
        };
        let search_tools = registry.discover_tools(&search_filter);
        assert_eq!(search_tools.len(), 1);
        assert_eq!(search_tools[0].name, "search_tool");

        let keyword_filter = ToolFilter {
            keyword: Some("analysis".to_string()),
            ..Default::default()
        };
        let found = registry.discover_tools(&keyword_filter);
        assert_eq!(found.len(), 1);
    }

    #[tokio::test]
    async fn test_tool_statistics() {
        let registry = ToolRegistry::new();
        let def = create_test_definition("stats_tool");
        let handler = Arc::new(MockHandler);

        registry.register_tool(def, handler).await.unwrap();

        let stats = registry.get_tool_stats("stats_tool").unwrap();
        assert_eq!(stats.call_count, 0);

        let ctx = CallContext {
            session_id: Uuid::new_v4(),
            user_id: None,
            request_id: Uuid::new_v4(),
            trace_id: "test".to_string(),
            timeout: Duration::from_secs(30),
        };

        registry
            .call_tool("stats_tool", serde_json::json!({}), ctx)
            .await
            .unwrap();

        let stats_after = registry.get_tool_stats("stats_tool").unwrap();
        assert_eq!(stats_after.call_count, 1);
        assert!(stats_after.last_called_at.is_some());

        let metrics = registry.get_registry_metrics();
        assert_eq!(metrics.total_calls, 1);
        assert_eq!(metrics.total_registrations, 1);
    }

    #[tokio::test]
    async fn test_disabled_tool_rejected() {
        let registry = ToolRegistry::new();
        let def = create_test_definition("disabled_tool");
        let handler = Arc::new(MockHandler);

        registry.register_tool(def, handler).await.unwrap();
        registry
            .set_tool_status("disabled_tool", &ToolStatus::Disabled)
            .unwrap();

        let ctx = CallContext {
            session_id: Uuid::new_v4(),
            user_id: None,
            request_id: Uuid::new_v4(),
            trace_id: "test".to_string(),
            timeout: Duration::from_secs(30),
        };

        let result = registry
            .call_tool("disabled_tool", serde_json::json!({}), ctx)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let registry = Arc::new(ToolRegistry::new());

        let mut handles = vec![];
        for i in 0..100 {
            let shared = registry.clone();
            let def = create_test_definition(&format!("concurrent_{i}"));
            let tool_handler = Arc::new(MockHandler);

            handles.push(tokio::spawn(async move {
                shared.register_tool(def, tool_handler).await
            }));
        }

        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }

        assert_eq!(registry.tool_count(), 100);
    }
}
