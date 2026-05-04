use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio_stream::Stream;

use crate::Result;

/// Agent 后端插件 — UPMC 外脑的"插座"
///
/// 开源默认实现：[`ReActAgentBackend`](crate::plugins::defaults::ReActAgentBackend)（基础推理-行动循环）
/// 闭源增强实现：UpcmAgentBackend（100x 更强的 UPMC 引擎）
///
/// # 架构角色
///
/// `AgentBackend` 是 Agent 能力的统一抽象。闭源 UPMC 引擎通过实现此 trait
/// 接入系统，实现"即插即用"——插上 UPMC 就获得 100x 能力，拔了也能用默认 `ReAct`。
///
/// # 接入模式
///
/// - **模式 A（进程内）**：`UpcmAgentBackend` 直接注册，零延迟
/// - **模式 B（进程外）**：[`RemoteAgentBackend`](crate::plugins::remote_agent::RemoteAgentBackend) 通过 gRPC 连接
/// - **模式 C（不接入）**：默认使用 `ReActAgentBackend`
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::AgentBackend;
/// use std::sync::Arc;
///
/// let agent: Arc<dyn AgentBackend> = Arc::new(UpcmAgentBackend::from_config(&config)?);
/// registry.register_agent(agent);
/// ```
#[async_trait]
pub trait AgentBackend: Send + Sync {
    /// 后端唯一标识
    fn backend_id(&self) -> &str;

    /// 执行任务
    async fn execute(&self, task: &AgentTask) -> Result<AgentResult>;

    /// 流式执行（实时输出）
    async fn execute_stream(
        &self,
        task: &AgentTask,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentChunk> + Send>>>;

    /// 取消正在执行的任务
    async fn cancel(&self, task_id: &str) -> Result<()>;

    /// 获取后端能力描述
    fn capabilities(&self) -> AgentCapabilities;

    /// 健康检查
    async fn health_check(&self) -> Result<bool>;
}

/// Agent 任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    /// 任务 ID
    pub task_id: String,
    /// 自然语言指令
    pub instruction: String,
    /// 任务上下文
    pub context: serde_json::Value,
    /// 最大执行步数
    pub max_steps: Option<u32>,
    /// 可用工具列表
    pub tools: Vec<String>,
}

/// Agent 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// 关联的任务 ID
    pub task_id: String,
    /// 是否成功
    pub success: bool,
    /// 输出内容
    pub output: String,
    /// 实际执行步数
    pub steps_taken: u32,
    /// 执行产物列表
    pub artifacts: Vec<AgentArtifact>,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
}

/// Agent 流式输出块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentChunk {
    /// 关联的任务 ID
    pub task_id: String,
    /// 输出块类型
    pub chunk_type: AgentChunkType,
    /// 块内容
    pub content: String,
}

/// Agent 输出块类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentChunkType {
    /// 思考过程
    Thinking,
    /// 动作执行
    Action,
    /// 观察结果
    Observation,
    /// 最终答案
    FinalAnswer,
    /// 错误信息
    Error,
}

/// Agent 产物
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentArtifact {
    /// 产物类型
    pub artifact_type: String,
    /// 产物名称
    pub name: String,
    /// 产物内容
    pub content: String,
}

/// Agent 能力描述
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct AgentCapabilities {
    /// 是否支持流式输出
    pub supports_streaming: bool,
    /// 是否支持取消任务
    pub supports_cancellation: bool,
    /// 最大上下文 token 数
    pub max_context_tokens: usize,
    /// 可用工具列表
    pub available_tools: Vec<String>,
    /// 是否支持多 Agent 协作
    pub supports_multi_agent: bool,
    /// 是否支持人机协作（Human-in-the-loop）
    pub supports_human_in_loop: bool,
}
