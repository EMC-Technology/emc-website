//! 工具调用基础设施
//!
//! 提供 Agent 专用的统一工具调用接口，

#![allow(clippy::significant_drop_tightening)]
//! 支持验证、重试、超时、日志记录等功能。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{Span, debug, error, info, instrument, warn};
use uuid::Uuid;

use super::types::ToolSchema;

// ============================================================================
// 工具定义与注册
// ============================================================================

/// 工具 trait
///
/// 所有可被 Agent 调用的工具都必须实现此 trait。
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称（唯一标识符）
    fn name(&self) -> &str;

    /// 工具描述（用于 LLM 理解工具用途）
    fn description(&self) -> &str;

    /// 参数 JSON Schema（用于参数验证）
    fn parameters_schema(&self) -> &serde_json::Value;

    /// 执行工具调用
    ///
    /// # Arguments
    ///
    /// * `arguments` - 经过验证的工具参数
    /// * `ctx` - Agent 上下文（包含追踪信息等）
    async fn execute(
        &self,
        arguments: serde_json::Value,
        ctx: &AgentContext,
    ) -> crate::Result<ToolOutput>;
}

/// 工具输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// 输出内容
    pub result: String,
    /// 是否成功
    pub success: bool,
    /// 元数据（可选）
    pub metadata: Option<serde_json::Value>,
}

impl ToolOutput {
    /// 创建成功的输出
    pub fn success(result: impl Into<String>) -> Self {
        Self {
            result: result.into(),
            success: true,
            metadata: None,
        }
    }

    /// 创建失败的输出
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            result: error.into(),
            success: false,
            metadata: None,
        }
    }

    /// 带元数据的成功输出
    pub fn success_with_metadata(result: impl Into<String>, metadata: serde_json::Value) -> Self {
        Self {
            result: result.into(),
            success: true,
            metadata: Some(metadata),
        }
    }
}

/// 工具注册表
///
/// 管理所有可用工具的注册、查找和调用。
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// 创建新的空注册表
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// 注册工具
    ///
    /// # Errors
    ///
    /// 如果同名工具已存在则返回错误。
    pub async fn register<T: Tool + 'static>(&self, tool: T) -> crate::Result<()> {
        let name = tool.name().to_string();
        let mut registry = self.tools.write().await;

        if registry.contains_key(&name) {
            return Err(error_core::helpers::internal_error(&format!(
                "工具 '{name}' 已存在"
            )));
        }

        let tool_name = name.clone();
        registry.insert(name, Arc::new(tool));
        info!(tool_name = %tool_name, "工具已注册");
        Ok(())
    }

    /// 获取工具
    pub async fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.read().await.get(name).cloned()
    }

    /// 检查工具是否存在
    pub async fn contains(&self, name: &str) -> bool {
        self.tools.read().await.contains_key(name)
    }

    /// 获取所有已注册工具的名称列表
    pub async fn list_names(&self) -> Vec<String> {
        self.tools.read().await.keys().cloned().collect()
    }

    /// 获取所有工具的 Schema 列表（用于 LLM）
    pub async fn get_schemas(&self) -> Vec<ToolSchema> {
        let registry = self.tools.read().await;
        registry
            .values()
            .map(|tool| ToolSchema {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters_schema().clone(),
            })
            .collect()
    }

    /// 注销工具
    ///
    /// # Errors
    ///
    /// 如果工具不存在则返回错误。
    pub async fn unregister(&self, name: &str) -> crate::Result<()> {
        let mut registry = self.tools.write().await;

        if registry.remove(name).is_some() {
            info!(tool_name = %name, "工具已注销");
            Ok(())
        } else {
            Err(error_core::helpers::internal_error(&format!(
                "工具 '{name}' 不存在"
            )))
        }
    }

    /// 获取已注册工具数量
    pub async fn count(&self) -> usize {
        self.tools.read().await.len()
    }
}

// ============================================================================
// Agent 工具调用器
// ============================================================================

/// 工具调用请求
#[derive(Debug, Clone)]
pub struct ToolCall {
    /// 工具名称
    pub tool_name: String,
    /// 工具参数
    pub arguments: serde_json::Value,
    /// 调用 ID（用于关联请求和响应）
    pub call_id: Uuid,
}

/// 工具调用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocationResult {
    /// 关联的调用 ID
    pub call_id: Uuid,
    /// 工具名称
    pub tool_name: String,
    /// 结果字符串
    pub result: String,
    /// 是否成功
    pub success: bool,
    /// 耗时（毫秒）
    pub duration_ms: u64,
    /// 重试次数
    pub retry_count: u32,
    /// 错误信息（失败时）
    pub error: Option<String>,
}

/// 重试策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 基础延迟（毫秒）
    pub base_delay_ms: u64,
    /// 最大延迟（毫秒）
    pub max_delay_ms: u64,
    /// 指数退避基数
    pub exponential_base: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            exponential_base: 2.0,
        }
    }
}

impl RetryPolicy {
    /// 创建无重试的策略
    #[must_use]
    pub const fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            base_delay_ms: 0,
            max_delay_ms: 0,
            exponential_base: 1.0,
        }
    }

    /// 计算第 n 次重试的等待时间
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_possible_wrap)]
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }

        let delay_ms = (self.base_delay_ms as f64 * self.exponential_base.powi(attempt as i32 - 1))
            .min(self.max_delay_ms as f64) as u64;

        Duration::from_millis(delay_ms)
    }
}

/// Agent 上下文
///
/// 包含执行工具调用所需的上下文信息。
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// 任务 ID
    pub task_id: Uuid,
    /// 会话 ID
    pub session_id: Uuid,
    /// 用户 ID（可选）
    pub user_id: Option<String>,
    /// 追踪 ID（用于分布式追踪）
    pub trace_id: String,
    /// 父级 Span（可选，用于构建追踪树）
    pub parent_span: Option<Span>,
}

impl AgentContext {
    /// 创建新的上下文
    #[must_use]
    pub fn new(task_id: Uuid) -> Self {
        Self {
            task_id,
            session_id: Uuid::new_v4(),
            user_id: None,
            trace_id: format!("trace-{}", Uuid::new_v4()),
            parent_span: None,
        }
    }
}

/// 统一的工具调用接口（Agent 专用）
///
/// 封装了工具调用的完整生命周期：
/// 参数验证 → 权限检查 → 执行 → 重试 → 日志记录 → 返回结果
pub struct AgentToolInvoker {
    /// 工具注册表
    registry: Arc<ToolRegistry>,
    /// 单次调用超时
    timeout: Duration,
    /// 重试策略
    retry_policy: RetryPolicy,
}

impl Default for AgentToolInvoker {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentToolInvoker {
    /// 创建新的工具调用器（使用默认配置）
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: Arc::new(ToolRegistry::new()),
            timeout: Duration::from_secs(30),
            retry_policy: RetryPolicy::default(),
        }
    }

    /// 使用自定义配置创建
    pub const fn with_config(
        registry: Arc<ToolRegistry>,
        timeout: Duration,
        retry_policy: RetryPolicy,
    ) -> Self {
        Self {
            registry,
            timeout,
            retry_policy,
        }
    }

    /// 注册工具到内部注册表
    ///
    /// # Errors
    ///
    /// 如果同名工具已存在则返回错误。
    pub async fn register_tool<T: Tool + 'static>(&self, tool: T) -> crate::Result<()> {
        self.registry.register(tool).await
    }

    /// 调用工具（带验证、重试、日志）
    ///
    /// 完整的调用流程：
    /// 1. 查找工具是否存在
    /// 2. 验证参数格式
    /// 3. 执行工具（带重试机制）
    /// 4. 记录调用日志和指标
    /// 5. 返回标准化结果
    ///
    /// # Arguments
    ///
    /// * `tool_name` - 要调用的工具名称
    /// * `arguments` - 工具参数（JSON 格式）
    /// * `ctx` - Agent 上下文
    ///
    /// # Returns
    ///
    /// 返回工具调用结果，包含输出内容和元数据。
    ///
    /// # Errors
    ///
    /// - 工具不存在
    /// - 参数验证失败
    /// - 执行超时
    /// - 所有重试均失败
    #[instrument(skip(self, ctx), fields(tool = %tool_name))]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_possible_wrap)]
    pub async fn invoke(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
        ctx: &AgentContext,
    ) -> crate::Result<ToolInvocationResult> {
        let call_id = Uuid::new_v4();
        let start_time = std::time::Instant::now();

        debug!(
            tool = %tool_name,
            call_id = %call_id,
            task_id = %ctx.task_id,
            "开始工具调用"
        );

        // Step 1: 查找工具
        let tool = self.registry.get(tool_name).await.ok_or_else(|| {
            error_core::helpers::internal_error(&format!("工具 '{tool_name}' 不存在"))
        })?;

        // Step 2: 验证参数（基本结构验证）
        if let Err(e) = self.validate_arguments(&arguments, tool.parameters_schema()) {
            warn!(
                tool = %tool_name,
                error = %e,
                "参数验证失败"
            );

            return Ok(ToolInvocationResult {
                call_id,
                tool_name: tool_name.to_string(),
                result: format!("参数错误: {e}"),
                success: false,
                duration_ms: start_time.elapsed().as_millis() as u64,
                retry_count: 0,
                error: Some(e),
            });
        }

        // Step 3: 执行工具（带重试）
        let mut last_error = None;

        for attempt in 0..self.retry_policy.max_attempts {
            if attempt > 0 {
                let delay = self.retry_policy.delay_for_attempt(attempt);
                debug!(attempt, delay_ms = delay.as_millis() as u64, "等待重试");
                tokio::time::sleep(delay).await;
            }

            // 使用超时包装执行
            //
            // # 安全约束
            //
            // 当前工具执行在当前进程中运行，仅受超时保护。
            // 对于不受信任的工具实现，建议：
            // 1. 使用 WASM 沙箱隔离执行环境
            // 2. 通过 `spawn_blocking` 将 CPU 密集型工具移至专用线程池
            // 3. 对高风险工具实施资源配额（CPU 时间、内存上限）
            let exec_result =
                tokio::time::timeout(self.timeout, tool.execute(arguments.clone(), ctx)).await;

            match exec_result {
                Ok(Ok(output)) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;

                    info!(
                        tool = %tool_name,
                        call_id = %call_id,
                        success = output.success,
                        duration_ms,
                        attempts = attempt + 1,
                        "工具调用成功"
                    );

                    return Ok(ToolInvocationResult {
                        call_id,
                        tool_name: tool_name.to_string(),
                        result: output.result.clone(),
                        success: output.success,
                        duration_ms,
                        retry_count: attempt,
                        error: if output.success {
                            None
                        } else {
                            Some(output.result)
                        },
                    });
                }
                Ok(Err(e)) => {
                    warn!(
                        tool = %tool_name,
                        attempt,
                        error = %e,
                        "工具执行失败"
                    );
                    last_error = Some(e.to_string());
                }
                Err(_) => {
                    warn!(
                        tool = %tool_name,
                        attempt,
                        timeout_ms = self.timeout.as_millis() as u64,
                        "工具执行超时"
                    );
                    last_error = Some(format!("执行超时 ({:?})", self.timeout));
                }
            }
        }

        // 所有重试都失败
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let error_msg = last_error.unwrap_or_else(|| "未知错误".to_string());

        error!(
            tool = %tool_name,
            call_id = %call_id,
            attempts = self.retry_policy.max_attempts,
            error = %error_msg,
            "工具调用最终失败"
        );

        Ok(ToolInvocationResult {
            call_id,
            tool_name: tool_name.to_string(),
            result: format!(
                "错误（已重试 {} 次）: {error_msg}",
                self.retry_policy.max_attempts
            ),
            success: false,
            duration_ms,
            retry_count: self.retry_policy.max_attempts - 1,
            error: Some(error_msg),
        })
    }

    /// 批量并行调用多个工具
    ///
    /// 同时发起多个工具调用，等待全部完成后返回结果列表。
    /// 适用于无依赖关系的独立工具调用场景。
    ///
    /// # Arguments
    ///
    /// * `calls` - 工具调用请求列表
    /// * `ctx` - Agent 上下文
    ///
    /// # Returns
    ///
    /// 返回与输入顺序对应的结果列表。
    pub async fn invoke_batch(
        &self,
        calls: Vec<ToolCall>,
        ctx: &AgentContext,
    ) -> Vec<crate::Result<ToolInvocationResult>> {
        info!(count = calls.len(), "开始批量工具调用");

        let futures = calls.into_iter().map(|call| {
            let invoker = self;
            let ctx = ctx.clone();
            async move { invoker.invoke(&call.tool_name, call.arguments, &ctx).await }
        });

        join_all(futures).await
    }

    /// 验证工具参数
    ///
    /// 检查参数是否符合工具的 JSON Schema 定义。
    #[allow(clippy::unused_self)]
    fn validate_arguments(
        &self,
        arguments: &serde_json::Value,
        schema: &serde_json::Value,
    ) -> Result<(), String> {
        // 基本类型检查：确保 arguments 是对象
        if !arguments.is_object() {
            return Err("参数必须是 JSON 对象".to_string());
        }

        // 如果 schema 中定义了 required 字段，检查必填参数
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            for req_field in required {
                if let Some(field_name) = req_field.as_str() {
                    if arguments.get(field_name).is_none() {
                        return Err(format!("缺少必填参数: '{field_name}'"));
                    }
                }
            }
        }

        // 可以在这里添加更详细的 JSON Schema 验证
        // 目前只做基本的必填字段检查

        Ok(())
    }

    /// 获取工具注册表引用
    #[must_use]
    pub const fn registry(&self) -> &Arc<ToolRegistry> {
        &self.registry
    }

    /// 设置超时时间
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置重试策略
    #[must_use]
    pub const fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }
}

// ============================================================================
// 内置工具示例
// ============================================================================

/// 示例：搜索工具
pub struct SearchTool;

#[async_trait::async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &'static str {
        "search"
    }

    fn description(&self) -> &'static str {
        "在知识库中搜索相关文档和信息"
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "搜索查询文本"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "返回结果数量上限",
                        "default": 10
                    }
                },
                "required": ["query"]
            })
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        _ctx: &AgentContext,
    ) -> crate::Result<ToolOutput> {
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let limit = arguments
            .get("limit")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(10);

        debug!(query, limit, "执行搜索");

        // 这里应该调用实际的搜索逻辑
        // 简化实现，返回模拟结果
        Ok(ToolOutput::success_with_metadata(
            format!(
                r#"{{"query": "{query}", "results": [], "total": 0, "limit": {limit}}}"#
            ),
            serde_json::json!({"mock": true}),
        ))
    }
}

/// 示例：获取上下文工具
pub struct GetContextTool;

#[async_trait::async_trait]
impl Tool for GetContextTool {
    fn name(&self) -> &'static str {
        "get_context"
    }

    fn description(&self) -> &'static str {
        "获取指定符号的完整上下文信息（调用者、被调用者、引用关系）"
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "目标符号标识（Token ID）"
                    }
                },
                "required": ["symbol"]
            })
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        _ctx: &AgentContext,
    ) -> crate::Result<ToolOutput> {
        let symbol = arguments
            .get("symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        debug!(symbol = %symbol, "获取上下文");

        Ok(ToolOutput::success_with_metadata(
            format!(
                r#"{{"symbol": "{symbol}", "callers": [], "callees": [], "references": []}}"#
            ),
            serde_json::json!({"mock": true}),
        ))
    }
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Mock Tool 用于测试
    struct EchoTool;

    #[async_trait::async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &'static str {
            "echo"
        }

        fn description(&self) -> &'static str {
            "返回输入的内容"
        }

        fn parameters_schema(&self) -> &serde_json::Value {
            static SCHEMA: std::sync::LazyLock<serde_json::Value> =
                std::sync::LazyLock::new(|| {
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "message": {"type": "string"}
                        },
                        "required": ["message"]
                    })
                });
            &SCHEMA
        }

        async fn execute(
            &self,
            arguments: serde_json::Value,
            _ctx: &AgentContext,
        ) -> crate::Result<ToolOutput> {
            let msg = arguments
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(ToolOutput::success(msg))
        }
    }

    // Failing Tool 用于测试重试
    #[allow(dead_code)]
    struct FailingTool {
        fail_count: std::sync::atomic::AtomicU32,
    }

    #[async_trait::async_trait]
    impl Tool for FailingTool {
        fn name(&self) -> &'static str {
            "failing_tool"
        }

        fn description(&self) -> &'static str {
            "总是失败的工具（测试用）"
        }

        fn parameters_schema(&self) -> &serde_json::Value {
            static SCHEMA: std::sync::LazyLock<serde_json::Value> =
                std::sync::LazyLock::new(|| serde_json::json!({"type": "object", "properties": {}}));
            &SCHEMA
        }

        async fn execute(
            &self,
            _arguments: serde_json::Value,
            _ctx: &AgentContext,
        ) -> crate::Result<ToolOutput> {
            self.fail_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(error_core::helpers::internal_error("模拟失败"))
        }
    }

    #[tokio::test]
    async fn test_tool_registry_register_and_get() {
        let registry = ToolRegistry::new();
        registry.register(EchoTool).await.unwrap();

        assert!(registry.contains("echo").await);
        assert!(!registry.contains("nonexistent").await);

        let tool = registry.get("echo").await;
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name(), "echo");
    }

    #[tokio::test]
    async fn test_tool_registry_duplicate_registration() {
        let registry = ToolRegistry::new();
        registry.register(EchoTool).await.unwrap();

        let result = registry.register(EchoTool).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_invoker_invoke_success() {
        let invoker = AgentToolInvoker::new();
        invoker.register_tool(EchoTool).await.unwrap();

        let ctx = AgentContext::new(Uuid::new_v4());
        let result = invoker
            .invoke("echo", serde_json::json!({"message": "hello"}), &ctx)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.result, "hello");
    }

    #[tokio::test]
    async fn test_tool_invoker_tool_not_found() {
        let invoker = AgentToolInvoker::new();

        let ctx = AgentContext::new(Uuid::new_v4());
        let result = invoker
            .invoke("nonexistent", serde_json::json!({}), &ctx)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_invoker_missing_required_param() {
        let invoker = AgentToolInvoker::new();
        invoker.register_tool(EchoTool).await.unwrap();

        let ctx = AgentContext::new(Uuid::new_v4());
        let result = invoker
            .invoke("echo", serde_json::json!({}), &ctx)
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_retry_policy_calculation() {
        let policy = RetryPolicy::default();

        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(0));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(400));
    }

    #[tokio::test]
    async fn test_retry_policy_max_delay_cap() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay_ms: 1000,
            max_delay_ms: 2000,
            exponential_base: 3.0,
        };

        // 3^2 = 2700 > 2000, 应该被限制为 2000
        let delay = policy.delay_for_attempt(3);
        assert_eq!(delay, Duration::from_millis(2000));
    }

    #[tokio::test]
    async fn test_batch_invocation() {
        let invoker = AgentToolInvoker::new();
        invoker.register_tool(EchoTool).await.unwrap();

        let ctx = AgentContext::new(Uuid::new_v4());

        let calls = vec![
            ToolCall {
                tool_name: "echo".to_string(),
                arguments: serde_json::json!({"message": "first"}),
                call_id: Uuid::new_v4(),
            },
            ToolCall {
                tool_name: "echo".to_string(),
                arguments: serde_json::json!({"message": "second"}),
                call_id: Uuid::new_v4(),
            },
        ];

        let results = invoker.invoke_batch(calls, &ctx).await;

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(Result::is_ok));
    }

    #[tokio::test]
    async fn test_agent_context_creation() {
        let task_id = Uuid::new_v4();
        let ctx = AgentContext::new(task_id);

        assert_eq!(ctx.task_id, task_id);
        assert!(ctx.user_id.is_none());
        assert!(!ctx.trace_id.is_empty());
    }

    #[tokio::test]
    async fn test_tool_output_helpers() {
        let success = ToolOutput::success("好的结果");
        assert!(success.success);
        assert_eq!(success.result, "好的结果");

        let failure = ToolOutput::failure("出错了");
        assert!(!failure.success);
        assert_eq!(failure.result, "出错了");
    }
}
