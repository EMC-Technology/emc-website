//! Agent 类型定义
//!
//! 定义任务、结果、LLM 接口、错误类型等核心数据结构。

use std::future::Future;
use std::pin::Pin;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::react_agent::ExecutionStatus;

// ============================================================================
// 任务定义
// ============================================================================

/// 任务定义
///
/// 描述 Agent 需要完成的目标、约束和上下文信息。
/// 每个 `Task` 由 `TaskOrchestrator` 分配给合适的 `Agent` 执行。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// 唯一标识符
    pub id: Uuid,
    /// 自然语言描述（给 LLM 的指令）
    pub description: String,
    /// 最终目标（用于判断任务是否完成）
    pub goal: String,
    /// 上下文信息
    pub context: TaskContext,
    /// 约束条件列表
    pub constraints: Vec<Constraint>,
    /// 期望输出格式（可选）
    pub expected_output: Option<String>,
    /// 任务优先级
    pub priority: TaskPriority,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl Task {
    /// 创建新任务
    pub fn new(description: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            description: description.into(),
            goal: goal.into(),
            context: TaskContext::default(),
            constraints: Vec::new(),
            expected_output: None,
            priority: TaskPriority::Normal,
            created_at: Utc::now(),
        }
    }

    /// 设置任务上下文
    #[must_use]
    pub fn with_context(mut self, context: TaskContext) -> Self {
        self.context = context;
        self
    }

    /// 添加约束条件
    #[must_use]
    pub fn with_constraint(mut self, constraint: Constraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// 设置优先级
    #[must_use]
    pub const fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

/// 任务上下文
///
/// 包含执行任务所需的环境信息和前置条件。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskContext {
    /// 用户 ID（可选）
    pub user_id: Option<String>,
    /// 会话 ID（可选）
    pub session_id: Option<Uuid>,
    /// 相关文档 ID 列表
    pub relevant_documents: Vec<String>,
    /// 前置任务的执行结果（可选）
    pub previous_results: Option<serde_json::Value>,
    /// 当前可用的工具名称列表
    pub available_tools: Vec<String>,
}

/// 约束条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// 约束类型
    pub constraint_type: ConstraintType,
    /// 约束值
    pub value: String,
}

/// 约束类型枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintType {
    /// 最大步数限制
    MaxSteps(usize),
    /// 最大执行时长
    MaxDuration(u64), // 秒数
    /// 允许使用的工具白名单
    AllowedTools(Vec<String>),
    /// 禁止使用的工具黑名单
    ForbiddenTools(Vec<String>),
    /// 必需的输出格式
    RequiredOutputFormat(String),
}

/// 任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum TaskPriority {
    /// 低优先级
    Low = 0,
    /// 普通优先级
    #[default]
    Normal = 1,
    /// 高优先级
    High = 2,
    /// 紧急优先级
    Critical = 3,
}

/// Agent 类型
///
/// 定义不同类型的 Agent 及其专长领域。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    /// 研究型 Agent - 擅长信息收集和分析
    Researcher,
    /// 分析型 Agent - 擅长数据处理和模式识别
    Analyst,
    /// 写作型 Agent - 擅长内容生成和文档编写
    Writer,
    /// 审查型 Agent - 擅长质量检查和代码审查
    Reviewer,
    /// 协调型 Agent - 擅长任务分配和流程编排
    Coordinator,
}

// ============================================================================
// 任务结果
// ============================================================================

/// 任务执行结果
///
/// 包含最终输出、统计信息和生成的制品。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// 关联的任务 ID
    pub task_id: Uuid,
    /// 是否成功完成
    pub success: bool,
    /// 输出内容（JSON 格式）
    pub output: serde_json::Value,
    /// 实际执行的步骤数
    pub steps_taken: usize,
    /// 总耗时（毫秒）
    pub total_duration_ms: u64,
    /// 消耗的总 token 数
    pub total_tokens_used: u32,
    /// 结果摘要（自然语言）
    pub summary: String,
    /// 生成的制品列表（知识图谱、报告等）
    pub artifacts: Vec<Artifact>,
}

impl TaskResult {
    /// 创建成功的任务结果
    pub fn success(task_id: Uuid, output: serde_json::Value, summary: impl Into<String>) -> Self {
        Self {
            task_id,
            success: true,
            output,
            steps_taken: 0,
            total_duration_ms: 0,
            total_tokens_used: 0,
            summary: summary.into(),
            artifacts: Vec::new(),
        }
    }

    /// 创建失败的任务结果
    pub fn failure(task_id: Uuid, error: impl Into<String>) -> Self {
        Self {
            task_id,
            success: false,
            output: serde_json::json!({ "error": error.into() }),
            steps_taken: 0,
            total_duration_ms: 0,
            total_tokens_used: 0,
            summary: String::new(),
            artifacts: Vec::new(),
        }
    }
}

/// 制品（Artifact）
///
/// Agent 执行过程中产生的有价值的输出物。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// 制品类型
    pub artifact_type: ArtifactType,
    /// 制品内容（JSON 格式）
    pub content: serde_json::Value,
    /// 制品名称
    pub name: String,
}

/// 制品类型枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactType {
    /// 知识图谱
    KnowledgeGraph,
    /// 文档
    Document,
    /// 分析报告
    AnalysisReport,
    /// 代码
    Code,
    /// 数据可视化
    DataVisualization,
    /// 其他类型
    Other(String),
}

// ============================================================================
// LLM 后端接口
// ============================================================================

/// LLM 后端 trait
///
/// 定义与大型语言模型交互的统一接口。
/// 实现此 trait 可以支持不同的 LLM 提供商（OpenAI、Claude、本地模型等）。
pub trait LLMBackend: Send + Sync {
    /// 向 LLM 发送同步完成请求
    ///
    /// 将消息列表和补全选项发送给 LLM 后端，等待完整响应返回。
    ///
    /// # Parameters
    /// - `messages`: 对话消息列表，包含系统提示、用户输入和历史上下文
    /// - `options`: 补全参数（温度、最大 token 数等）
    ///
    /// # Errors
    /// 当 LLM 后端通信失败或响应解析异常时返回错误
    fn complete(
        &self,
        messages: &[LLMMessage],
        options: &CompletionOptions,
    ) -> impl Future<Output = crate::Result<LLMResponse>> + Send;

    /// 向 LLM 发送流式完成请求
    ///
    /// 与 [`complete`](LLMBackend::complete) 不同，此方法返回一个异步流，
    /// 逐 token 产出响应内容，适用于需要实时展示生成过程的场景。
    ///
    /// # Parameters
    /// - `messages`: 对话消息列表
    /// - `options`: 补全参数
    ///
    /// # Errors
    /// 流中每个 Item 均可能携带 LLM 后端通信错误
    fn complete_stream(
        &self,
        messages: &[LLMMessage],
        options: &CompletionOptions,
    ) -> Pin<Box<dyn futures::Stream<Item = crate::Result<String>> + Send>>;
}

/// LLM 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMMessage {
    /// 消息角色
    pub role: MessageRole,
    /// 消息内容
    pub content: String,
    /// 工具调用（仅 assistant 角色使用）
    pub tool_calls: Option<Vec<ToolCallSchema>>,
}

/// 消息角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    /// 系统消息
    System,
    /// 用户消息
    User,
    /// 助手消息
    Assistant,
    /// 工具消息（工具调用结果）
    Tool,
}

/// 补全选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionOptions {
    /// 温度参数（0.0 - 2.0）
    pub temperature: f64,
    /// 最大生成 token 数
    pub max_tokens: u32,
    /// 停止序列（可选）
    pub stop: Option<Vec<String>>,
    /// 可用工具定义（可选）
    pub tools: Option<Vec<ToolSchema>>,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 4096,
            stop: None,
            tools: None,
        }
    }
}

/// LLM 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    /// 生成的文本内容
    pub content: String,
    /// 工具调用（如果 LLM 决定调用工具）
    pub tool_calls: Option<Vec<ToolCallSchema>>,
    /// 使用的 prompt tokens 数
    pub prompt_tokens: u32,
    /// 生成的 completion tokens 数
    pub completion_tokens: u32,
    /// 总 token 数
    pub total_tokens: u32,
    /// 停止原因
    pub finish_reason: FinishReason,
}

/// 停止原因
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinishReason {
    /// 正常停止
    Stop,
    /// 达到最大 token 数
    Length,
    /// 调用了工具
    ToolCalls,
    /// 内容过滤
    ContentFilter,
    /// 其他原因
    Other(String),
}

/// 工具调用 Schema（符合 [`OpenAI`] Function Calling 格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallSchema {
    /// 调用的工具 ID
    pub id: String,
    /// 工具名称
    pub name: String,
    /// 参数（JSON 字符串）
    pub arguments: String,
}

/// 工具 Schema 定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 参数 JSON Schema
    pub parameters: serde_json::Value,
}

// ============================================================================
// Agent 错误类型
// ============================================================================

/// Agent 错误枚举
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// LLM 调用失败
    #[error("LLM 调用失败: {0}")]
    LlmError(String),

    /// 工具调用失败
    #[error("工具 '{tool}' 调用失败: {error}")]
    ToolError {
        /// 失败的工具名称
        tool: String,
        /// 错误描述信息
        error: String,
    },

    /// 工具不存在
    #[error("工具 '{0}' 不存在")]
    ToolNotFound(String),

    /// 达到最大迭代次数
    #[error("达到最大迭代次数: {0}")]
    MaxIterationsReached(usize),

    /// 执行超时
    #[error("执行超时")]
    Timeout,

    /// 需要用户澄清
    #[error("需要澄清: {0}")]
    NeedsClarification(String),

    /// 解析动作失败
    #[error("无法解析 LLM 输出的动作: {0}")]
    ParseActionError(String),

    /// 记忆系统错误
    #[error("记忆系统错误: {0}")]
    MemoryError(String),

    /// 无效的任务状态转换
    #[error("无效的状态转换: 从 {from:?} 到 {to:?}")]
    InvalidStateTransition {
        /// 转换前的原始状态
        from: ExecutionStatus,
        /// 目标状态（非法目标）
        to: ExecutionStatus,
    },

    /// 工作流执行错误
    #[error("工作流错误: {0}")]
    WorkflowError(String),

    /// 序列化/反序列化错误
    #[error("序列化错误: {0}")]
    SerializationError(String),

    /// 权限不足
    #[error("权限不足: {0}")]
    PermissionDenied(String),

    /// 安全检查未通过
    #[error("安全检查未通过: {0}")]
    SafetyCheckFailed(String),
}

impl From<AgentError> for crate::Result<()> {
    fn from(err: AgentError) -> Self {
        Err(error_core::helpers::agent_workflow_error(&err.to_string()))
    }
}

impl From<AgentError> for error_core::ErrorObject {
    fn from(err: AgentError) -> Self {
        use error_core::helpers;
        match err {
            AgentError::LlmError(msg) => helpers::agent_llm_error(&msg),
            AgentError::ToolError { tool, error } => helpers::agent_tool_error(&tool, &error),
            AgentError::ToolNotFound(tool) => helpers::agent_tool_not_found(&tool),
            AgentError::MaxIterationsReached(max) => helpers::agent_max_iterations(max),
            AgentError::Timeout => helpers::agent_timeout(),
            AgentError::NeedsClarification(msg) => helpers::agent_clarification(&msg),
            AgentError::ParseActionError(msg) => helpers::agent_parse_action_error(&msg),
            AgentError::MemoryError(msg) => helpers::agent_memory_error(&msg),
            AgentError::InvalidStateTransition { from, to } => {
                helpers::agent_invalid_transition(&format!("{from:?}"), &format!("{to:?}"))
            }
            AgentError::WorkflowError(msg) => helpers::agent_workflow_error(&msg),
            AgentError::SerializationError(msg) => helpers::agent_serialization_error(&msg),
            AgentError::PermissionDenied(msg) => helpers::agent_permission_denied(&msg),
            AgentError::SafetyCheckFailed(msg) => helpers::agent_safety_check_failed(&msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new("Analyze code", "Find bugs");
        assert_eq!(task.description, "Analyze code");
        assert_eq!(task.goal, "Find bugs");
        assert_eq!(task.priority, TaskPriority::Normal);
        assert!(task.constraints.is_empty());
    }

    #[test]
    fn test_task_with_context_and_constraint() {
        let ctx = TaskContext {
            user_id: Some("u1".to_string()),
            session_id: None,
            relevant_documents: vec!["doc1".to_string()],
            previous_results: None,
            available_tools: vec!["search".to_string()],
        };
        let task = Task::new("test", "goal")
            .with_context(ctx.clone())
            .with_constraint(Constraint {
                constraint_type: ConstraintType::MaxSteps(10),
                value: "10".to_string(),
            })
            .with_priority(TaskPriority::High);
        assert_eq!(task.context.user_id, Some("u1".to_string()));
        assert_eq!(task.constraints.len(), 1);
        assert_eq!(task.priority, TaskPriority::High);
    }

    #[test]
    fn test_task_result_success() {
        let id = Uuid::new_v4();
        let result = TaskResult::success(id, serde_json::json!({"ok": true}), "Done");
        assert!(result.success);
        assert_eq!(result.task_id, id);
    }

    #[test]
    fn test_task_result_failure() {
        let id = Uuid::new_v4();
        let result = TaskResult::failure(id, "Something went wrong");
        assert!(!result.success);
        assert!(result.output["error"].is_string());
    }

    #[test]
    fn test_task_priority_ordering() {
        assert!(TaskPriority::Low < TaskPriority::Normal);
        assert!(TaskPriority::Normal < TaskPriority::High);
        assert!(TaskPriority::High < TaskPriority::Critical);
    }

    #[test]
    fn test_completion_options_default() {
        let opts = CompletionOptions::default();
        assert!((opts.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(opts.max_tokens, 4096);
        assert!(opts.stop.is_none());
        assert!(opts.tools.is_none());
    }

    #[test]
    fn test_llm_message_serialization() {
        let msg = LLMMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
            tool_calls: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let de: LLMMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(de.role, MessageRole::User);
        assert_eq!(de.content, "Hello");
    }

    #[test]
    fn test_message_role_serialization_roundtrip() {
        let roles = [
            MessageRole::System,
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::Tool,
        ];
        for r in &roles {
            let json = serde_json::to_string(r).unwrap();
            let de: MessageRole = serde_json::from_str(&json).unwrap();
            assert_eq!(*r, de);
        }
    }

    #[test]
    fn test_finish_reason_serialization() {
        let reasons = [
            FinishReason::Stop,
            FinishReason::Length,
            FinishReason::ToolCalls,
            FinishReason::ContentFilter,
            FinishReason::Other("custom".to_string()),
        ];
        for r in &reasons {
            let json = serde_json::to_string(r).unwrap();
            let de: FinishReason = serde_json::from_str(&json).unwrap();
            assert_eq!(*r, de);
        }
    }

    #[test]
    fn test_agent_error_display() {
        assert!(format!("{}", AgentError::LlmError("timeout".to_string())).contains("timeout"));
        assert!(
            format!(
                "{}",
                AgentError::ToolError {
                    tool: "search".to_string(),
                    error: "fail".to_string()
                }
            )
            .contains("search")
        );
        assert!(format!("{}", AgentError::ToolNotFound("x".to_string())).contains('x'));
        assert!(format!("{}", AgentError::MaxIterationsReached(10)).contains("10"));
        assert!(format!("{}", AgentError::Timeout).contains("超时"));
        assert!(
            format!("{}", AgentError::NeedsClarification("what?".to_string())).contains("what?")
        );
        assert!(format!("{}", AgentError::ParseActionError("bad".to_string())).contains("bad"));
        assert!(format!("{}", AgentError::MemoryError("oom".to_string())).contains("oom"));
        assert!(format!("{}", AgentError::WorkflowError("fail".to_string())).contains("fail"));
        assert!(format!("{}", AgentError::SerializationError("json".to_string())).contains("json"));
        assert!(
            format!("{}", AgentError::PermissionDenied("denied".to_string())).contains("denied")
        );
        assert!(
            format!("{}", AgentError::SafetyCheckFailed("unsafe".to_string())).contains("unsafe")
        );
    }

    #[test]
    fn test_agent_error_into_error_object() {
        let errors = vec![
            AgentError::LlmError("err".to_string()),
            AgentError::ToolError {
                tool: "t".to_string(),
                error: "e".to_string(),
            },
            AgentError::ToolNotFound("t".to_string()),
            AgentError::MaxIterationsReached(5),
            AgentError::Timeout,
            AgentError::NeedsClarification("w".to_string()),
            AgentError::ParseActionError("p".to_string()),
            AgentError::MemoryError("m".to_string()),
            AgentError::InvalidStateTransition {
                from: ExecutionStatus::Idle,
                to: ExecutionStatus::Completed,
            },
            AgentError::WorkflowError("w".to_string()),
            AgentError::SerializationError("s".to_string()),
            AgentError::PermissionDenied("p".to_string()),
            AgentError::SafetyCheckFailed("s".to_string()),
        ];
        for err in errors {
            let _obj: error_core::ErrorObject = err.into();
        }
    }

    #[test]
    fn test_agent_error_into_result() {
        let err = AgentError::Timeout;
        let result: crate::Result<()> = err.into();
        assert!(result.is_err());
    }

    #[test]
    fn test_artifact_type_serialization() {
        let json = serde_json::to_string(&ArtifactType::KnowledgeGraph).unwrap();
        let de: ArtifactType = serde_json::from_str(&json).unwrap();
        match de {
            ArtifactType::KnowledgeGraph => {}
            _ => panic!("Expected KnowledgeGraph"),
        }
        let json = serde_json::to_string(&ArtifactType::Other("custom".to_string())).unwrap();
        let de: ArtifactType = serde_json::from_str(&json).unwrap();
        match de {
            ArtifactType::Other(name) => assert_eq!(name, "custom"),
            _ => panic!("Expected Other"),
        }
    }

    #[test]
    fn test_tool_call_schema_serialization() {
        let schema = ToolCallSchema {
            id: "call_1".to_string(),
            name: "search".to_string(),
            arguments: r#"{"query": "test"}"#.to_string(),
        };
        let json = serde_json::to_string(&schema).unwrap();
        let de: ToolCallSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(de.id, "call_1");
    }

    #[test]
    fn test_tool_schema_serialization() {
        let schema = ToolSchema {
            name: "search".to_string(),
            description: "Search tool".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let json = serde_json::to_string(&schema).unwrap();
        let de: ToolSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(de.name, "search");
    }

    #[test]
    fn test_llm_response_serialization() {
        let response = LLMResponse {
            content: "Hello".to_string(),
            tool_calls: None,
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            finish_reason: FinishReason::Stop,
        };
        let json = serde_json::to_string(&response).unwrap();
        let de: LLMResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(de.content, "Hello");
        assert_eq!(de.total_tokens, 15);
    }

    #[test]
    fn test_constraint_type_serialization() {
        let json = serde_json::to_string(&ConstraintType::MaxSteps(10)).unwrap();
        let de: ConstraintType = serde_json::from_str(&json).unwrap();
        match de {
            ConstraintType::MaxSteps(n) => assert_eq!(n, 10),
            _ => panic!("Expected MaxSteps"),
        }
        let json = serde_json::to_string(&ConstraintType::MaxDuration(60)).unwrap();
        let de: ConstraintType = serde_json::from_str(&json).unwrap();
        match de {
            ConstraintType::MaxDuration(n) => assert_eq!(n, 60),
            _ => panic!("Expected MaxDuration"),
        }
        let json = serde_json::to_string(&ConstraintType::RequiredOutputFormat("json".to_string()))
            .unwrap();
        let de: ConstraintType = serde_json::from_str(&json).unwrap();
        match de {
            ConstraintType::RequiredOutputFormat(fmt) => assert_eq!(fmt, "json"),
            _ => panic!("Expected RequiredOutputFormat"),
        }
    }
}
