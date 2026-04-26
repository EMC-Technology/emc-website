//! `ReAct` Agent 核心实现
//!
//! 基于 `ReAct` (Reasoning + Acting) 模式的 AI Agent，
//! 通过 `Thought` → `Action` → `Observation` 循环自主完成任务。

#![allow(clippy::significant_drop_tightening)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{Level, debug, error, info, instrument, span, warn};
use uuid::Uuid;

use super::memory::MemorySystem;
use super::tools::{AgentContext, AgentToolInvoker};
use super::types::{
    CompletionOptions, LLMBackend, LLMMessage, MessageRole, Task, TaskResult, ToolSchema,
};

// ============================================================================
// 配置与状态
// ============================================================================

/// Agent 配置
///
/// 控制 Agent 的行为参数和执行约束。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// 最大推理步数（默认: 20）
    pub max_iterations: usize,
    /// 每步最大工具调用数（默认: 5）
    pub max_tool_calls_per_step: usize,
    /// LLM 温度（默认: 0.7）
    pub temperature: f64,
    /// 是否输出详细思考过程
    pub verbose: bool,
    /// 停止序列（用于控制 LLM 输出格式）
    pub stop_sequences: Vec<String>,
    /// 单次执行超时（默认: 5 分钟）
    pub timeout: Duration,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            max_tool_calls_per_step: 5,
            temperature: 0.7,
            verbose: false,
            stop_sequences: vec!["\nObservation:".to_string()],
            timeout: Duration::from_secs(300),
        }
    }
}

impl AgentConfig {
    /// 创建自定义配置的 builder 模式入口
    #[must_use]
    pub fn builder() -> AgentConfigBuilder {
        AgentConfigBuilder::default()
    }
}

/// `AgentConfig` 的 Builder
#[derive(Debug, Default)]
pub struct AgentConfigBuilder {
    config: AgentConfig,
}

impl AgentConfigBuilder {
    /// 设置最大迭代次数
    #[must_use]
    pub const fn max_iterations(mut self, n: usize) -> Self {
        self.config.max_iterations = n;
        self
    }

    /// 设置温度参数
    #[must_use]
    pub const fn temperature(mut self, t: f64) -> Self {
        self.config.temperature = t;
        self
    }

    /// 启用详细输出
    #[must_use]
    pub const fn verbose(mut self, v: bool) -> Self {
        self.config.verbose = v;
        self
    }

    /// 设置超时时间
    #[must_use]
    pub const fn timeout(mut self, d: Duration) -> Self {
        self.config.timeout = d;
        self
    }

    /// 构建配置
    #[must_use]
    pub fn build(self) -> AgentConfig {
        self.config
    }
}

/// Agent 当前状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// 任务 ID
    pub task_id: Uuid,
    /// 执行状态
    pub status: ExecutionStatus,
    /// 当前迭代次数
    pub current_iteration: usize,
    /// 推理步骤历史
    pub history: Vec<ReActStep>,
    /// 最终结果（完成后填充）
    pub result: Option<TaskResult>,
    /// 错误信息（失败时填充）
    pub error: Option<String>,
    /// 开始时间
    pub started_at: chrono::DateTime<Utc>,
    /// 完成时间（可选）
    pub completed_at: Option<chrono::DateTime<Utc>>,
}

/// 执行状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// 空闲
    Idle,
    /// 运行中
    Running,
    /// 等待工具返回
    WaitingForTool,
    /// 正在思考（LLM 调用中）
    Thinking,
    /// 已完成
    Completed,
    /// 执行失败
    Failed,
    /// 超时
    Timeout,
    /// 达到最大迭代次数
    MaxIterationsReached,
}

/// `ReAct` 推理步骤
///
/// 记录单次 `Thought`-`Action`-`Observation` 循环的完整信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReActStep {
    /// 步骤编号（从 0 开始）
    pub step_number: usize,
    /// 思考过程（Thought）
    pub thought: String,
    /// 要执行的动作（Action，可选）
    pub action: Option<Action>,
    /// 执行结果观察（Observation，可选）
    pub observation: Option<String>,
    /// 本步耗时（毫秒）
    pub duration_ms: u64,
    /// 本步消耗的 token 数
    pub tokens_used: u32,
}

/// 动作定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// 动作类型
    pub action_type: ActionType,
    /// 工具名称（UseTool 时使用）
    pub tool_name: String,
    /// 工具参数（JSON 格式）
    pub arguments: serde_json::Value,
    /// 动作推理说明
    pub reasoning: String,
}

/// 动作类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    /// 调用工具
    UseTool,
    /// 任务完成
    Finish,
    /// 需要用户澄清
    AskClarification,
}

/// 解析后的动作
enum ActionParsed {
    /// 任务完成
    Finish(TaskResult),
    /// 调用工具
    UseTool(Action),
    /// 需要澄清
    AskClarification(String),
}

// ============================================================================
// ReAct Agent 核心
// ============================================================================

/// 基于 `ReAct` (Reasoning + Acting) 的 AI Agent
///
/// # 架构概述
///
/// Agent 遵循 `ReAct` 模式进行自主推理：
/// 1. **Thought**: LLM 分析当前状态并决定下一步行动
/// 2. **Action**: 执行工具调用或给出最终答案
/// 3. **Observation**: 获取工具执行的反馈
/// 4. **循环**: 将 `Observation` 加入上下文，重复上述过程
///
/// # 线程安全
///
/// `ReactAgent` 内部使用 `Arc<RwLock<>>` 保护可变状态，
/// 可以安全地在多个异步任务间共享。
///
/// # Examples
///
/// ```ignore
/// let llm = OpenAIBackend::new(api_key);
/// let tools = ToolRegistry::new();
/// let config = AgentConfig::default();
///
/// let agent = ReactAgent::new(llm, tools, config);
/// let task = Task::new("分析文档", "提取所有实体");
/// let result = agent.execute(&task).await?;
/// ```
pub struct ReactAgent<L: LLMBackend> {
    /// LLM 后端
    llm: Arc<L>,
    /// 工具注册表
    tools: Arc<AgentToolInvoker>,
    /// 记忆系统
    memory: Arc<RwLock<MemorySystem>>,
    /// Agent 配置
    config: AgentConfig,
    /// 运行时状态
    state: Arc<RwLock<AgentState>>,
    /// 当前用户身份（零信任：工具调用需携带用户上下文）
    user_id: Option<String>,
}

impl<L: LLMBackend + 'static> ReactAgent<L> {
    /// 创建新的 Agent 实例
    ///
    /// # Arguments
    ///
    /// * `llm` - LLM 后端实现
    /// * `tools` - 工具调用器
    /// * `config` - Agent 配置
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let agent = ReactAgent::new(llm_backend, tool_invoker, AgentConfig::default());
    /// ```
    pub fn new(llm: L, tools: AgentToolInvoker, config: AgentConfig) -> Self {
        Self {
            llm: Arc::new(llm),
            tools: Arc::new(tools),
            memory: Arc::new(RwLock::new(MemorySystem::default())),
            config,
            state: Arc::new(RwLock::new(AgentState {
                task_id: Uuid::nil(),
                status: ExecutionStatus::Idle,
                current_iteration: 0,
                history: Vec::new(),
                result: None,
                error: None,
                started_at: Utc::now(),
                completed_at: None,
            })),
            user_id: None,
        }
    }

    /// 设置当前用户身份
    ///
    /// # 零信任安全
    ///
    /// 用户身份将传递到所有工具调用的 `AgentContext` 中，
    /// 确保工具执行时能进行用户级权限检查。
    pub fn set_user_id(&mut self, user_id: impl Into<String>) {
        self.user_id = Some(user_id.into());
    }

    /// 使用自定义记忆系统创建 Agent
    pub fn with_memory(
        llm: L,
        tools: AgentToolInvoker,
        config: AgentConfig,
        memory: MemorySystem,
    ) -> Self {
        Self {
            llm: Arc::new(llm),
            tools: Arc::new(tools),
            memory: Arc::new(RwLock::new(memory)),
            config,
            user_id: None,
            state: Arc::new(RwLock::new(AgentState {
                task_id: Uuid::nil(),
                status: ExecutionStatus::Idle,
                current_iteration: 0,
                history: Vec::new(),
                result: None,
                error: None,
                started_at: Utc::now(),
                completed_at: None,
            })),
        }
    }

    /// 执行任务（主循环）
    ///
    /// 这是 Agent 的核心方法，实现完整的 `ReAct` 循环：
    /// `Thought` → `Action` → `Observation` → (repeat)
    ///
    /// # Arguments
    ///
    /// * `task` - 要执行的任务定义
    ///
    /// # Returns
    ///
    /// 返回任务执行结果，包含最终输出、统计信息和制品。
    ///
    /// # Errors
    ///
    /// - `AgentError::Timeout`: 执行超时
    /// - `AgentError::MaxIterationsReached`: 达到最大迭代次数
    /// - `AgentError::LlmError`: LLM 调用失败
    /// - `AgentError::ToolError`: 工具调用失败
    #[instrument(skip(self, task), fields(task_id = %task.id))]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_possible_truncation)]
    pub async fn execute(&self, task: &Task) -> crate::Result<TaskResult> {
        let task_span = span!(Level::INFO, "agent_execute", task_id = %task.id);
        let _enter = task_span.enter();

        // 初始化状态
        self.initialize_state(task).await?;

        let start_time = Instant::now();

        // 主循环: Thought → Action → Observation
        for iteration in 0..self.config.max_iterations {
            debug!(iteration, "开始第 {} 次迭代", iteration + 1);

            // 检查超时
            if start_time.elapsed() > self.config.timeout {
                self.set_status(ExecutionStatus::Timeout).await;
                return Err(error_core::helpers::internal_error(&format!(
                    "执行超时: {:?}",
                    self.config.timeout
                )));
            }

            // Step 1: Thought - LLM 思考下一步行动
            let step_start = Instant::now();
            self.set_status(ExecutionStatus::Thinking).await;

            let thought_result = self.think(task, iteration).await;
            let thought_duration = step_start.elapsed().as_millis() as u64;

            let thought = match thought_result {
                Ok(t) => t,
                Err(e) => {
                    error!(error = %e, "LLM 思考过程出错");
                    self.set_status(ExecutionStatus::Failed).await;
                    return Err(e);
                }
            };

            if self.config.verbose {
                info!(thought = %thought, "=== Thought ===");
            }

            // 记录思考过程
            self.record_thought(thought.clone(), iteration, thought_duration, 0)
                .await?;

            // Step 2: Action - 解析并执行动作
            let task_id = {
                let state = self.state.read().await;
                state.task_id
            };
            match self.parse_action(&thought, task_id)? {
                ActionParsed::Finish(result) => {
                    info!("任务完成");
                    self.finalize(result.clone()).await?;
                    return Ok(result);
                }
                ActionParsed::UseTool(action) => {
                    if self.config.verbose {
                        info!(
                            tool = %action.tool_name,
                            args = %action.arguments,
                            "=== Action ==="
                        );
                    }

                    // Step 3: Observation - 执行工具并获取结果
                    self.set_status(ExecutionStatus::WaitingForTool).await;
                    let obs_start = Instant::now();

                    let observation = self.execute_action(action.clone()).await;
                    let obs_duration = obs_start.elapsed().as_millis() as u64;

                    match observation {
                        Ok(obs) => {
                            if self.config.verbose {
                                info!(observation = %obs, "=== Observation ===");
                            }

                            // 更新当前步骤的 action 和 observation
                            self.update_step_with_action_and_observation(
                                iteration,
                                Some(action),
                                Some(obs.clone()),
                                obs_duration,
                            )
                            .await?;

                            // 更新记忆系统
                            {
                                let mut memory = self.memory.write().await;
                                if let Err(e) = memory.add_observation(task.id, obs.clone()).await {
                                    tracing::warn!(error = %e, "添加观察到记忆系统失败");
                                }
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "工具执行失败");
                            let error_obs = format!("工具执行错误: {e}");

                            self.update_step_with_action_and_observation(
                                iteration,
                                Some(action),
                                Some(error_obs.clone()),
                                obs_duration,
                            )
                            .await?;

                            // 将错误作为 observation 继续循环，让 LLM 决定如何处理
                            {
                                let mut memory = self.memory.write().await;
                                let _ = memory.add_observation(task.id, error_obs).await;
                            }
                        }
                    }
                }
                ActionParsed::AskClarification(question) => {
                    info!(question = %question, "需要用户澄清");
                    self.set_status(ExecutionStatus::Failed).await;
                    return Err(error_core::helpers::internal_error(&format!(
                        "需要澄清: {question}"
                    )));
                }
            }

            // 更新迭代计数
            {
                let mut state = self.state.write().await;
                state.current_iteration = iteration + 1;
            }
        }

        // 达到最大迭代次数
        warn!(max = self.config.max_iterations, "达到最大迭代次数");
        self.set_status(ExecutionStatus::MaxIterationsReached).await;

        // 尝试生成部分结果
        let _partial_result = self.generate_partial_result(task).await;
        Err(error_core::helpers::internal_error(&format!(
            "达到最大迭代次数: {}, 已完成 {} 步",
            self.config.max_iterations,
            self.state.read().await.current_iteration
        )))
    }

    /// 初始化 Agent 状态
    async fn initialize_state(&self, task: &Task) -> crate::Result<()> {
        let mut state = self.state.write().await;
        state.task_id = task.id;
        state.status = ExecutionStatus::Running;
        state.current_iteration = 0;
        state.history = Vec::new();
        state.result = None;
        state.error = None;
        state.started_at = Utc::now();
        state.completed_at = None;
        Ok(())
    }

    /// LLM 思考过程
    ///
    /// 构建上下文消息并发送给 LLM，获取其思考和动作决策。
    async fn think(&self, task: &Task, _iteration: usize) -> crate::Result<String> {
        // 构建系统提示词
        let system_prompt = self.build_system_prompt(task);

        // 构建对话历史
        let history = {
            let state = self.state.read().await;
            state.history.clone()
        };
        let messages = Self::build_messages(task, &system_prompt, &history);

        // 构建补全选项
        let options = CompletionOptions {
            temperature: self.config.temperature,
            max_tokens: 2048,
            stop: Some(self.config.stop_sequences.clone()),
            tools: Some(self.get_tool_schemas()),
        };

        // 调用 LLM
        let response = self
            .llm
            .complete(&messages, &options)
            .await
            .map_err(|e| error_core::helpers::internal_error(&format!("LLM 调用失败: {e}")))?;

        // 记录 token 使用情况
        {
            let state = self.state.read().await;
            if let Some(_last_step) = state.history.last() {
                // tokens_used 在 record_thought 中设置为 0，这里无法直接修改
                // 实际应用中可以通过内部可变性或其他方式处理
            }
        }

        Ok(response.content)
    }

    /// 构建 `ReAct` 系统提示词
    fn build_system_prompt(&self, task: &Task) -> String {
        format!(
            r#"你是一个专业的知识管理 AI 助手。请按照以下 ReAct 格式进行推理：

## 任务目标
{goal}

## 任务描述
{description}

## 可用工具
{tools}

## 输出格式要求
请严格按照以下格式输出你的思考过程：

Thought: [你的分析和推理]
Action: {{"action_type": "UseTool"|"Finish", "tool_name": "...", "arguments": {{...}}, "reasoning": "..."}}
Observation: [等待工具返回的结果]

或者当任务完成时：
Thought: [总结]
Action: {{"action_type": "Finish", "output": {{...}}, "summary": "..."}}

## 注意事项
1. 每次只输出一个 Thought 和一个 Action
2. 如果需要调用多个工具，分多步进行
3. 当你认为任务已完成时，使用 Finish 动作
4. 充分利用工具来获取信息，不要猜测
5. 保持思考过程的逻辑性和连贯性"#,
            goal = task.goal,
            description = task.description,
            tools = self.format_available_tools()
        )
    }

    /// 格式化可用工具列表
    #[allow(clippy::unused_self)]
    fn format_available_tools(&self) -> String {
        // 这里应该从 tools 获取实际的工具列表
        // 简化实现，实际应该查询 AgentToolInvoker
        "（工具列表由系统动态提供）".to_string()
    }

    /// 获取工具 Schema 列表
    #[allow(clippy::unused_self)]
    const fn get_tool_schemas(&self) -> Vec<ToolSchema> {
        // 实际实现应从 AgentToolInvoker 获取
        Vec::new()
    }

    /// 构建发送给 LLM 的消息列表
    fn build_messages(task: &Task, system_prompt: &str, history: &[ReActStep]) -> Vec<LLMMessage> {
        let mut messages = vec![LLMMessage {
            role: MessageRole::System,
            content: system_prompt.to_string(),
            tool_calls: None,
        }];

        // 添加用户任务描述
        messages.push(LLMMessage {
            role: MessageRole::User,
            content: format!(
                "请开始执行任务：{}\n\n目标：{}",
                task.description, task.goal
            ),
            tool_calls: None,
        });

        // 添加历史对话（ReAct 步骤）
        for step in history {
            // 添加 Thought
            messages.push(LLMMessage {
                role: MessageRole::Assistant,
                content: format!("Thought: {}", step.thought),
                tool_calls: None,
            });

            // 添加 Action
            if let Some(ref action) = step.action {
                let action_str = serde_json::to_string(action).unwrap_or_default();
                messages.push(LLMMessage {
                    role: MessageRole::Assistant,
                    content: format!("Action: {action_str}"),
                    tool_calls: None,
                });
            }

            // 添加 Observation
            if let Some(ref obs) = step.observation {
                messages.push(LLMMessage {
                    role: MessageRole::Tool,
                    content: format!("Observation: {obs}"),
                    tool_calls: None,
                });
            }
        }

        messages
    }

    /// 解析 LLM 输出为结构化动作
    #[allow(clippy::unused_self)]
    fn parse_action(&self, thought: &str, task_id: Uuid) -> crate::Result<ActionParsed> {
        // 尝试从 thought 中提取 JSON 格式的 Action
        if let Some(action_start) = thought.find("Action:") {
            let action_str = &thought[action_start + 7..];

            // 提取 JSON 部分
            if let Some(json_start) = action_str.find('{') {
                if let Some(json_end) = action_str.rfind('}') {
                    let json_str = &action_str[json_start..=json_end];

                    let action_value: serde_json::Value =
                        serde_json::from_str(json_str).map_err(|e| {
                            error_core::helpers::internal_error(&format!(
                                "解析 Action JSON 失败: {e}, 内容: {json_str}"
                            ))
                        })?;

                    let action_type = action_value
                        .get("action_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    match action_type {
                        "Finish" => {
                            let output = action_value
                                .get("output")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            let summary = action_value
                                .get("summary")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            let result = TaskResult::success(task_id, output, summary);
                            Ok(ActionParsed::Finish(result))
                        }
                        "UseTool" => {
                            let action = Action {
                                action_type: ActionType::UseTool,
                                tool_name: action_value
                                    .get("tool_name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                arguments: action_value
                                    .get("arguments")
                                    .cloned()
                                    .unwrap_or(serde_json::json!({})),
                                reasoning: action_value
                                    .get("reasoning")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            };
                            Ok(ActionParsed::UseTool(action))
                        }
                        "AskClarification" => {
                            let question = action_value
                                .get("question")
                                .and_then(|v| v.as_str())
                                .unwrap_or("需要更多信息")
                                .to_string();
                            Ok(ActionParsed::AskClarification(question))
                        }
                        _ => Err(error_core::helpers::internal_error(&format!(
                            "未知的动作类型: {action_type}"
                        ))),
                    }
                } else {
                    Err(error_core::helpers::internal_error(
                        "未找到完整的 Action JSON",
                    ))
                }
            } else {
                Err(error_core::helpers::internal_error(
                    "Action 字段中没有 JSON 数据",
                ))
            }
        } else {
            // 没有 Action，可能是纯文本回复，视为 Finish
            let output = serde_json::json!({"text": thought});
            let result = TaskResult::success(task_id, output, "任务完成（无显式 Action）");
            Ok(ActionParsed::Finish(result))
        }
    }

    /// 执行工具调用
    #[instrument(skip(self, action), fields(tool = %action.tool_name))]
    async fn execute_action(&self, action: Action) -> crate::Result<String> {
        let ctx = AgentContext {
            task_id: self.state.read().await.task_id,
            session_id: Uuid::new_v4(),
            user_id: self.user_id.clone(),
            trace_id: format!("agent-{}", Uuid::new_v4()),
            parent_span: None,
        };

        self.tools
            .invoke(&action.tool_name, action.arguments, &ctx)
            .await
            .map_err(|e| error_core::helpers::internal_error(&format!("工具调用失败: {e}")))
            .map(|r| r.result)
    }

    /// 记录思考过程
    async fn record_thought(
        &self,
        thought: String,
        step_number: usize,
        duration_ms: u64,
        tokens_used: u32,
    ) -> crate::Result<()> {
        let step = ReActStep {
            step_number,
            thought,
            action: None,
            observation: None,
            duration_ms,
            tokens_used,
        };

        let mut state = self.state.write().await;
        state.history.push(step);
        Ok(())
    }

    /// 更新步骤的 action 和 observation
    async fn update_step_with_action_and_observation(
        &self,
        step_number: usize,
        action: Option<Action>,
        observation: Option<String>,
        obs_duration_ms: u64,
    ) -> crate::Result<()> {
        let mut state = self.state.write().await;

        if let Some(step) = state
            .history
            .iter_mut()
            .find(|s| s.step_number == step_number)
        {
            step.action = action;
            step.observation = observation;
            step.duration_ms += obs_duration_ms;
        }

        Ok(())
    }

    /// 完成任务并设置最终状态
    async fn finalize(&self, result: TaskResult) -> crate::Result<()> {
        let mut state = self.state.write().await;
        state.status = ExecutionStatus::Completed;
        state.result = Some(result.clone());
        state.completed_at = Some(Utc::now());

        // 计算总耗时
        if state.completed_at.is_some() {
            // duration 已经在 result 中计算
        }

        info!(
            steps = result.steps_taken,
            tokens = result.total_tokens_used,
            duration_ms = result.total_duration_ms,
            "任务执行完成"
        );

        Ok(())
    }

    /// 生成部分结果（当达到最大迭代次数时）
    async fn generate_partial_result(&self, task: &Task) -> Option<TaskResult> {
        let state = self.state.read().await;
        let summary = format!(
            "任务未完全完成。已执行 {} 步推理。",
            state.current_iteration
        );

        Some(TaskResult {
            task_id: task.id,
            success: false,
            output: serde_json::json!({
                "partial": true,
                "steps_completed": state.current_iteration,
                "last_thought": state.history.last().map(|s| s.thought.clone()),
            }),
            steps_taken: state.current_iteration,
            total_duration_ms: 0,
            total_tokens_used: 0,
            summary,
            artifacts: Vec::new(),
        })
    }

    /// 设置 Agent 状态
    async fn set_status(&self, status: ExecutionStatus) {
        let mut state = self.state.write().await;
        state.status = status;
    }

    /// 检查是否超时
    #[allow(dead_code)]
    fn check_timeout(&self, started_at: Instant) -> bool {
        started_at.elapsed() > self.config.timeout
    }

    /// 中断当前执行
    ///
    /// 可以从外部调用以停止正在运行的 Agent。
    ///
    /// # Errors
    ///
    /// 当 Agent 未处于运行状态时返回错误。
    pub async fn interrupt(&self) -> crate::Result<()> {
        let mut state = self.state.write().await;

        match state.status {
            ExecutionStatus::Running
            | ExecutionStatus::Thinking
            | ExecutionStatus::WaitingForTool => {
                state.status = ExecutionStatus::Failed;
                state.error = Some("用户中断".to_string());
                state.completed_at = Some(Utc::now());
                info!("Agent 已被中断");
                Ok(())
            }
            _ => Err(error_core::helpers::internal_error(
                "Agent 未处于运行状态，无法中断",
            )),
        }
    }

    /// 获取当前状态（只读快照）
    pub async fn get_state(&self) -> AgentState {
        self.state.read().await.clone()
    }

    /// 获取执行历史
    pub async fn get_history(&self) -> Vec<ReActStep> {
        self.state.read().await.history.clone()
    }

    /// 清除历史记录（用于新的会话）
    pub async fn clear_history(&self) {
        let mut state = self.state.write().await;
        state.history.clear();
        state.current_iteration = 0;
    }
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use std::pin::Pin;

    use super::*;
    use crate::agent::types::{FinishReason, LLMResponse, TaskPriority};

    // Mock LLM Backend 用于测试
    struct MockLlmBackend;

    #[async_trait::async_trait]
    impl LLMBackend for MockLlmBackend {
        async fn complete(
            &self,
            _messages: &[LLMMessage],
            _options: &CompletionOptions,
        ) -> crate::Result<LLMResponse> {
            Ok(LLMResponse {
                content: "Thought: 我需要查询一些信息\nAction: {\"action_type\": \"Finish\", \"output\": {\"result\": \"测试成功\"}, \"summary\": \"测试完成\"}".to_string(),
                tool_calls: None,
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
                finish_reason: FinishReason::Stop,
            })
        }

        async fn complete_stream(
            &self,
            _messages: &[LLMMessage],
            _options: &CompletionOptions,
        ) -> Pin<Box<dyn futures::Stream<Item = crate::Result<String>> + Send>> {
            Box::pin(futures::stream::empty())
        }
    }

    #[tokio::test]
    async fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_iterations, 20);
        assert_eq!(config.max_tool_calls_per_step, 5);
        assert!((config.temperature - 0.7).abs() < f64::EPSILON);
        assert!(!config.verbose);
    }

    #[tokio::test]
    async fn test_agent_config_builder() {
        let config = AgentConfig::builder()
            .max_iterations(10)
            .temperature(0.5)
            .verbose(true)
            .build();

        assert_eq!(config.max_iterations, 10);
        assert!((config.temperature - 0.5).abs() < f64::EPSILON);
        assert!(config.verbose);
    }

    #[tokio::test]
    async fn test_task_creation() {
        let task = Task::new("测试任务", "完成测试").with_priority(TaskPriority::High);

        assert_eq!(task.description, "测试任务");
        assert_eq!(task.goal, "完成测试");
        assert_eq!(task.priority, TaskPriority::High);
    }

    #[tokio::test]
    async fn test_parse_action_finish() {
        let config = AgentConfig::default();
        let mock_llm = MockLlmBackend;
        let tools = AgentToolInvoker::new();
        let agent = ReactAgent::new(mock_llm, tools, config);

        let thought = "Thought: 任务已完成\nAction: {\"action_type\": \"Finish\", \"output\": {\"result\": \"done\"}, \"summary\": \"完成\"}";
        let result = agent.parse_action(thought, Uuid::new_v4());

        assert!(result.is_ok());
        match result.unwrap() {
            ActionParsed::Finish(r) => {
                assert!(r.success);
            }
            _ => panic!("Expected Finish action"),
        }
    }

    #[tokio::test]
    async fn test_parse_action_use_tool() {
        let config = AgentConfig::default();
        let mock_llm = MockLlmBackend;
        let tools = AgentToolInvoker::new();
        let agent = ReactAgent::new(mock_llm, tools, config);

        let thought = "Thought: 我需要查询\nAction: {\"action_type\": \"UseTool\", \"tool_name\": \"search\", \"arguments\": {\"query\": \"test\"}, \"reasoning\": \"需要搜索\"}";
        let result = agent.parse_action(thought, Uuid::new_v4());

        assert!(result.is_ok());
        match result.unwrap() {
            ActionParsed::UseTool(action) => {
                assert_eq!(action.tool_name, "search");
                assert_eq!(action.action_type, ActionType::UseTool);
            }
            _ => panic!("Expected UseTool action"),
        }
    }

    #[tokio::test]
    async fn test_execution_status_transitions() {
        let config = AgentConfig::default();
        let mock_llm = MockLlmBackend;
        let tools = AgentToolInvoker::new();
        let agent = ReactAgent::new(mock_llm, tools, config);

        // 初始状态应该是 Idle
        let state = agent.get_state().await;
        assert_eq!(state.status, ExecutionStatus::Idle);
    }

    #[tokio::test]
    async fn test_react_step_serialization() {
        let step = ReActStep {
            step_number: 0,
            thought: "测试思考".to_string(),
            action: None,
            observation: None,
            duration_ms: 100,
            tokens_used: 50,
        };

        let json = serde_json::to_string(&step).expect("序列化失败");
        let deserialized: ReActStep = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(deserialized.step_number, 0);
        assert_eq!(deserialized.thought, "测试思考");
    }
}
