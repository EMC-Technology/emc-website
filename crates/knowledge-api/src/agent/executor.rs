//! DAG 任务编排器
//!
//! 提供基于有向无环图（DAG）的工作流定义和执行能力。

#![allow(clippy::significant_drop_tightening)]
//! 支持并行执行、条件分支、重试、暂停/恢复等功能。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures::future::join_all;
use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex as TokioMutex, RwLock, Semaphore};
use tracing::{debug, error, info, instrument, warn};

use super::react_agent::ReactAgent;
use super::types::{Artifact, LLMBackend, Task, TaskResult};

// ============================================================================
// 工作流定义
// ============================================================================

/// `Agent` 类型枚举
///
/// 定义不同类型的 Agent 及其专长领域。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    /// 研究型 Agent（擅长信息收集）
    Researcher,
    /// 分析型 Agent（擅长数据处理）
    Analyst,
    /// 写作型 Agent（擅长内容生成）
    Writer,
    /// 审查型 Agent（擅长质量检查）
    Reviewer,
    /// 协调型 Agent（负责子任务分配）
    Coordinator,
    /// 自定义类型
    Custom(String),
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Researcher => write!(f, "Researcher"),
            Self::Analyst => write!(f, "Analyst"),
            Self::Writer => write!(f, "Writer"),
            Self::Reviewer => write!(f, "Reviewer"),
            Self::Coordinator => write!(f, "Coordinator"),
            Self::Custom(name) => write!(f, "{name}"),
        }
    }
}

/// 任务执行重试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔（毫秒）
    pub retry_delay_ms: u64,
    /// 指数退避
    pub exponential_backoff: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 1000,
            exponential_backoff: true,
        }
    }
}

/// 任务节点
///
/// DAG 中的一个可执行单元。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    /// 节点 ID
    pub id: String,
    /// 节点名称
    pub name: String,
    /// 节点要执行的任务
    pub task: Task,
    /// 依赖的其他节点 ID 列表
    pub dependencies: Vec<String>,
    /// Agent 类型
    pub agent_type: AgentType,
    /// 重试配置
    pub retry_config: RetryConfig,
    /// 超时时间
    pub timeout: Duration,
}

impl TaskNode {
    /// 创建新的任务节点
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        task: Task,
        agent_type: AgentType,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            task,
            dependencies: Vec::new(),
            agent_type,
            retry_config: RetryConfig::default(),
            timeout: Duration::from_secs(300),
        }
    }

    /// 添加依赖节点
    #[must_use]
    pub fn with_dependency(mut self, dep_id: impl Into<String>) -> Self {
        self.dependencies.push(dep_id.into());
        self
    }

    /// 设置超时时间
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置重试配置
    #[must_use]
    pub const fn with_retry(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }
}

/// 边（连接两个节点）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// 起始节点 ID
    pub from: String,
    /// 目标节点 ID
    pub to: String,
    /// 可选的条件表达式（用于条件分支）
    pub condition: Option<String>,
}

/// 全局工作流配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// 整个工作流的最大执行时间
    pub max_duration: Option<Duration>,
    /// 是否在任一节点失败时停止整个工作流
    pub fail_fast: bool,
    /// 最大并发节点数
    pub max_concurrency: usize,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            max_duration: None,
            fail_fast: true,
            max_concurrency: 5,
        }
    }
}

/// 工作流定义
///
/// 使用 YAML/RON 格式定义的完整工作流结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// 工作流 ID
    pub id: String,
    /// 工作流名称
    pub name: String,
    /// 工作流描述
    pub description: String,
    /// 任务节点列表
    pub nodes: Vec<TaskNode>,
    /// 边列表（定义节点间的依赖关系）
    pub edges: Vec<Edge>,
    /// 全局配置
    pub global_config: GlobalConfig,
}

impl WorkflowDefinition {
    /// 验证工作流定义的合法性
    ///
    /// # Errors
    ///
    /// - 存在循环依赖
    /// - 引用了不存在的节点
    /// - 存在没有入口的孤立节点
    pub fn validate(&self) -> crate::Result<()> {
        // 构建节点 ID 到索引的映射
        let node_ids: HashSet<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();

        // 检查边引用的节点是否存在
        for edge in &self.edges {
            if !node_ids.contains(edge.from.as_str()) {
                return Err(error_core::helpers::validation_error(
                    &format!("边引用了不存在的起始节点: {}", edge.from),
                    "validate",
                ));
            }
            if !node_ids.contains(edge.to.as_str()) {
                return Err(error_core::helpers::validation_error(
                    &format!("边引用了不存在的目标节点: {}", edge.to),
                    "validate",
                ));
            }
        }

        // 构建图并检查循环
        let mut graph = DiGraph::<&str, ()>::new();
        let mut node_indices: HashMap<&str, NodeIndex> = HashMap::new();

        for node in &self.nodes {
            let idx = graph.add_node(node.id.as_str());
            node_indices.insert(node.id.as_str(), idx);
        }

        for edge in &self.edges {
            let from_idx = node_indices[edge.from.as_str()];
            let to_idx = node_indices[edge.to.as_str()];
            graph.add_edge(from_idx, to_idx, ());
        }

        // 检测环
        if petgraph::algo::is_cyclic_directed(&graph) {
            Err(error_core::helpers::validation_error(
                "工作流包含循环依赖",
                "validate",
            ))
        } else {
            Ok(())
        }
    }
}

// ============================================================================
// 执行状态与结果
// ============================================================================

/// 节点执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// 等待中
    Pending,
    /// 运行中
    Running,
    /// 成功完成
    Succeeded,
    /// 执行失败
    Failed,
    /// 已跳过
    Skipped,
    /// 超时
    TimedOut,
}

/// 节点执行错误类型枚举（Axiom-3: 可枚举的封闭集合）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum NodeErrorKind {
    /// 节点依赖未满足
    DependencyFailed { dependency_id: String },
    /// 执行超时
    Timeout { elapsed_secs: u64 },
    /// 工具调用失败
    ToolError { tool_name: String },
    /// 内部逻辑错误
    InternalError { reason: String },
}

impl std::fmt::Display for NodeErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DependencyFailed { dependency_id } => {
                write!(f, "依赖节点失败: {dependency_id}")
            }
            Self::Timeout { elapsed_secs } => {
                write!(f, "节点执行超时: {elapsed_secs}s")
            }
            Self::ToolError { tool_name } => {
                write!(f, "工具调用失败: {tool_name}")
            }
            Self::InternalError { reason } => write!(f, "{reason}"),
        }
    }
}

/// 单个节点的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    /// 节点 ID
    pub node_id: String,
    /// 执行状态
    pub status: NodeStatus,
    /// 任务结果（成功或部分成功时有值）
    pub result: Option<TaskResult>,
    /// 结构化错误信息（Axiom-3: 可枚举的封闭集合）
    pub error: Option<NodeErrorKind>,
    /// 开始时间
    pub started_at: chrono::DateTime<Utc>,
    /// 结束时间
    pub completed_at: Option<chrono::DateTime<Utc>>,
    /// 重试次数
    pub retry_count: u32,
}

/// 工作流整体状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    /// 未开始
    Idle,
    /// 运行中
    Running,
    /// 已暂停
    Paused,
    /// 成功完成
    Succeeded,
    /// 失败
    Failed,
    /// 部分失败
    PartialFailure,
    /// 超时
    TimedOut,
}

impl WorkflowStatus {
    /// 验证状态转换是否合法
    ///
    /// 合法转换路径：
    /// - `Idle` → `Running`
    /// - `Running` → `Paused` | `Succeeded` | `Failed` | `PartialFailure` | `TimedOut`
    /// - `Paused` → `Running`
    ///
    /// 终态（`Succeeded`/`Failed`/`PartialFailure`/`TimedOut`）无出边。
    #[must_use]
    pub fn can_transition_to(&self, target: &Self) -> bool {
        matches!(
            (self, target),
            (Self::Idle | Self::Paused, Self::Running)
                | (
                    Self::Running,
                    Self::Paused
                        | Self::Succeeded
                        | Self::Failed
                        | Self::PartialFailure
                        | Self::TimedOut
                )
        )
    }

    /// 是否为终态
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::PartialFailure | Self::TimedOut
        )
    }
}

/// 工作流执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    /// 工作流 ID
    pub workflow_id: String,
    /// 整体状态
    pub status: WorkflowStatus,
    /// 所有节点的执行结果
    pub node_results: HashMap<String, NodeResult>,
    /// 开始时间
    pub started_at: chrono::DateTime<Utc>,
    /// 结束时间
    pub completed_at: Option<chrono::DateTime<Utc>>,
    /// 总耗时（毫秒）
    pub total_duration_ms: u64,
    /// 生成的制品汇总
    pub artifacts: Vec<Artifact>,
}

// ============================================================================
// 共享状态
// ============================================================================

/// 工作流运行时的共享状态
struct SharedState {
    /// 当前工作流定义
    workflow: Option<WorkflowDefinition>,
    /// 各节点的执行状态
    node_statuses: HashMap<String, NodeStatus>,
    /// 各节点的执行结果
    node_results: HashMap<String, NodeResult>,
    /// 工作流整体状态
    status: WorkflowStatus,
    /// 是否已暂停
    paused: bool,
}

impl Default for SharedState {
    fn default() -> Self {
        Self {
            workflow: None,
            node_statuses: HashMap::new(),
            node_results: HashMap::new(),
            status: WorkflowStatus::Idle,
            paused: false,
        }
    }
}

// ============================================================================
// DAG 任务编排器
// ============================================================================

type AgentMap = HashMap<String, Arc<Box<dyn AgentExecutor>>>;

/// DAG 任务编排器
///
/// 核心组件，负责：
/// - 解析和验证工作流定义
/// - 管理任务间的依赖关系
/// - 并行调度可执行的节点
/// - 处理错误和重试
/// - 支持暂停/恢复
///
/// # 架构说明
///
/// 编排器使用 `petgraph` 库管理 DAG 结构，
/// 使用 `tokio` 实现异步并发执行，
/// 通过共享状态实现线程安全的协调。
///
/// # Examples
///
/// ```ignore
/// let orchestrator = TaskOrchestrator::new();
///
/// // 从 YAML 加载工作流
/// let definition = WorkflowDefinition::from_yaml_file("workflow.yaml")?;
/// orchestrator.load_workflow(&definition).await?;
///
/// // 执行工作流
/// let result = orchestrator.execute("workflow_001").await?;
/// ```
pub struct TaskOrchestrator {
    /// DAG 图结构
    graph: RwLock<DiGraph<String, Edge>>,
    /// 节点 ID 到索引的映射
    node_map: RwLock<HashMap<String, NodeIndex>>,
    /// 注册的 Agent 实例
    agents: Arc<TokioMutex<AgentMap>>,
    /// 共享运行状态
    shared_state: Arc<RwLock<SharedState>>,
}

/// Agent 执行器 trait（用于动态分发）
#[async_trait::async_trait]
pub trait AgentExecutor: Send + Sync {
    /// 执行指定的 Agent 任务并返回结果
    ///
    /// # Arguments
    ///
    /// * `task` - 待执行的任务定义，包含目标、约束和上下文
    ///
    /// # Errors
    ///
    /// 当任务执行过程中发生错误时返回（如 LLM 调用失败、工具执行超时等）。
    async fn execute_task(&self, task: &Task) -> crate::Result<TaskResult>;
}

#[async_trait::async_trait]
impl<L: LLMBackend + 'static> AgentExecutor for ReactAgent<L> {
    async fn execute_task(&self, task: &Task) -> crate::Result<TaskResult> {
        self.execute(task).await
    }
}

impl Default for TaskOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskOrchestrator {
    /// 创建新的任务编排器
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: RwLock::new(DiGraph::new()),
            node_map: RwLock::new(HashMap::new()),
            agents: Arc::new(TokioMutex::new(HashMap::new())),
            shared_state: Arc::new(RwLock::new(SharedState::default())),
        }
    }

    /// 注册 Agent 实例
    ///
    /// 将特定类型的 Agent 绑定到名称，供节点引用。
    ///
    /// # Arguments
    ///
    /// * `name` - Agent 名称（对应 `AgentType`）
    /// * `agent` - Agent 实例
    pub async fn register_agent(
        &self,
        name: impl Into<String>,
        agent: Arc<Box<dyn AgentExecutor>>,
    ) {
        let mut agents = self.agents.lock().await;
        agents.insert(name.into(), agent);
        info!("Agent 已注册");
    }

    /// 从定义加载工作流
    ///
    /// # Arguments
    ///
    /// * `definition` - 工作流定义
    ///
    /// # Errors
    ///
    /// - 定义验证失败
    /// - 图构建失败
    #[instrument(skip(self, definition), fields(workflow_id = %definition.id))]
    pub async fn load_workflow(&self, definition: &WorkflowDefinition) -> crate::Result<()> {
        // 验证定义
        definition.validate()?;

        // 构建 DAG 图
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();

        // 添加所有节点
        for node in &definition.nodes {
            let idx = graph.add_node(node.id.clone());
            node_map.insert(node.id.clone(), idx);
        }

        // 添加所有边
        for edge in &definition.edges {
            let from_idx = node_map.get(&edge.from).ok_or_else(|| {
                error_core::helpers::validation_error(
                    &format!("找不到节点: {}", edge.from),
                    "workflow_edge",
                )
            })?;
            let to_idx = node_map.get(&edge.to).ok_or_else(|| {
                error_core::helpers::validation_error(
                    &format!("找不到节点: {}", edge.to),
                    "workflow_edge",
                )
            })?;

            graph.add_edge(*from_idx, *to_idx, edge.clone());
        }

        // 更新内部状态
        {
            let mut g = self.graph.write().await;
            *g = graph;
        }
        {
            let mut nm = self.node_map.write().await;
            *nm = node_map;
        }
        {
            let mut state = self.shared_state.write().await;
            state.workflow = Some(definition.clone());
            state.status = WorkflowStatus::Idle;
            state.node_statuses.clear();
            state.node_results.clear();

            for node in &definition.nodes {
                state
                    .node_statuses
                    .insert(node.id.clone(), NodeStatus::Pending);
            }

            tracing::info!(
                workflow_id = %definition.id,
                new_status = ?WorkflowStatus::Idle,
                "Axiom-2: 工作流状态变更已记录"
            );
        }

        info!(
            workflow_id = %definition.id,
            nodes = definition.nodes.len(),
            edges = definition.edges.len(),
            "工作流加载完成"
        );

        Ok(())
    }

    /// 执行工作流
    ///
    /// 主调度方法，按照拓扑顺序执行各节点。
    /// 支持并行执行无依赖关系的节点。
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - 要执行的工作流 ID
    ///
    /// # Returns
    ///
    /// 返回工作流执行结果
    ///
    /// # Errors
    ///
    /// - 工作流未加载
    /// - 执行过程中发生错误
    #[instrument(skip(self), fields(workflow_id = %workflow_id))]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_possible_truncation)]
    pub async fn execute(&self, workflow_id: &str) -> crate::Result<WorkflowResult> {
        let start_time = std::time::Instant::now();

        // 设置状态为 Running
        {
            let mut state = self.shared_state.write().await;

            match &state.workflow {
                Some(wf) if wf.id == workflow_id => {}
                Some(_) => {
                    return Err(error_core::helpers::validation_error(
                        &format!("当前加载的工作流 ID 不匹配: {workflow_id}"),
                        "workflow_id_mismatch",
                    ));
                }
                None => {
                    return Err(error_core::helpers::validation_error(
                        "未加载任何工作流",
                        "workflow_not_loaded",
                    ));
                }
            }

            state.status = WorkflowStatus::Running;
            state.paused = false;
            tracing::info!(
                workflow_id = %workflow_id,
                new_status = ?WorkflowStatus::Running,
                "Axiom-2: 工作流状态变更已记录"
            );
        }

        info!(workflow_id = %workflow_id, "开始执行工作流");

        // 主执行循环
        loop {
            // 检查是否暂停
            {
                let state = self.shared_state.read().await;
                if state.paused {
                    drop(state);

                    // 等待恢复信号（简单实现：轮询检查）
                    loop {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        let state = self.shared_state.read().await;
                        if !state.paused {
                            break;
                        }
                    }
                }
            }

            // 找出所有可以执行的节点（依赖已完成且自身为 Pending）
            let ready_nodes = self.find_ready_nodes().await;

            if ready_nodes.is_empty() {
                // 没有可执行的节点，检查是否全部完成
                break;
            }

            debug!(count = ready_nodes.len(), "发现可执行的节点");

            let max_concurrency = {
                let state = self.shared_state.read().await;
                state
                    .workflow
                    .as_ref()
                    .map_or(5, |wf| wf.global_config.max_concurrency)
            };
            let semaphore = Arc::new(Semaphore::new(max_concurrency));

            let futures: Vec<_> = ready_nodes
                .into_iter()
                .map(|node_id| {
                    let orchestrator = self;
                    let sem = semaphore.clone();
                    async move {
                        let _permit = sem.acquire().await.map_err(|_| {
                            error_core::helpers::agent_workflow_error("Semaphore 已关闭")
                        })?;
                        orchestrator.execute_node(&node_id).await
                    }
                })
                .collect();

            let results = join_all(futures).await;

            // 处理结果并更新状态
            let mut should_stop = false;
            for result in results {
                match result {
                    Ok((node_id, node_result)) => {
                        let is_failure = matches!(
                            node_result.status,
                            NodeStatus::Failed | NodeStatus::TimedOut
                        );

                        {
                            let mut state = self.shared_state.write().await;
                            state
                                .node_results
                                .insert(node_id.clone(), node_result.clone());
                            state
                                .node_statuses
                                .insert(node_id.clone(), node_result.status);
                        }

                        // 如果配置了 fail_fast 且有节点失败
                        if is_failure {
                            let state = self.shared_state.read().await;
                            if let Some(wf) = &state.workflow
                                && wf.global_config.fail_fast
                            {
                                warn!(
                                    node = %node_id,
                                    "节点失败，根据 fail_fast 配置停止工作流"
                                );
                                should_stop = true;
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "节点执行出错");

                        let mut state = self.shared_state.write().await;
                        let node_id = "unknown".to_string();
                        state
                            .node_statuses
                            .insert(node_id.clone(), NodeStatus::Failed);
                        state.node_results.insert(
                            node_id,
                            NodeResult {
                                node_id: "unknown".to_string(),
                                status: NodeStatus::Failed,
                                result: None,
                                error: Some(NodeErrorKind::InternalError {
                                    reason: e.to_string(),
                                }),
                                started_at: Utc::now(),
                                completed_at: Some(Utc::now()),
                                retry_count: 0,
                            },
                        );

                        should_stop = true;
                    }
                }
            }

            if should_stop {
                break;
            }
        }

        // 收集最终结果
        let final_status = self.determine_final_status().await;
        let total_duration_ms = start_time.elapsed().as_millis() as u64;

        {
            let mut state = self.shared_state.write().await;
            let old_status = state.status;
            state.status = final_status;
            tracing::info!(
                old_status = ?old_status,
                new_status = ?final_status,
                "Axiom-2: 工作流状态变更已记录"
            );
        }

        let workflow_result = {
            let state = self.shared_state.read().await;

            let mut nodes_with_artifacts: Vec<_> = state
                .node_results
                .values()
                .filter(|nr| nr.status == NodeStatus::Succeeded)
                .filter_map(|nr| nr.result.as_ref().map(|tr| (nr.started_at, &tr.artifacts)))
                .collect();

            nodes_with_artifacts.sort_by_key(|(started_at, _)| *started_at);

            let artifacts: Vec<Artifact> = nodes_with_artifacts
                .into_iter()
                .flat_map(|(_, artifacts)| artifacts.clone())
                .collect();

            WorkflowResult {
                workflow_id: workflow_id.to_string(),
                status: final_status,
                node_results: state.node_results.clone(),
                started_at: state
                    .workflow
                    .as_ref()
                    .map_or_else(Utc::now, |_| Utc::now()),
                completed_at: Some(Utc::now()),
                total_duration_ms,
                artifacts,
            }
        };

        info!(
            workflow_id = %workflow_id,
            status = ?final_status,
            duration_ms = total_duration_ms,
            "工作流执行结束"
        );

        Ok(workflow_result)
    }

    /// 查找所有就绪的节点（依赖已完成且自身为 Pending）
    async fn find_ready_nodes(&self) -> Vec<String> {
        let mut ready = Vec::new();

        let graph = self.graph.read().await;
        let node_map = self.node_map.read().await;
        let state = self.shared_state.read().await;

        for (node_id, &idx) in node_map.iter() {
            // 只处理 Pending 状态的节点
            if state.node_statuses.get(node_id) != Some(&NodeStatus::Pending) {
                continue;
            }

            // 检查所有前置依赖是否都已完成（Succeeded 或 Skipped）
            let mut dependencies = graph.neighbors_directed(idx, Direction::Incoming);
            let all_deps_done = dependencies.all(|dep_idx| {
                let dep_node_id = &graph[dep_idx];
                matches!(
                    state.node_statuses.get(dep_node_id),
                    Some(NodeStatus::Succeeded | NodeStatus::Skipped)
                )
            });

            if all_deps_done {
                ready.push((*node_id).clone());
            }
        }

        ready
    }

    /// 解析节点配置和对应的 Agent
    async fn resolve_node_and_agent(
        &self,
        node_id: &str,
    ) -> crate::Result<(TaskNode, Arc<Box<dyn AgentExecutor>>)> {
        let state = self.shared_state.read().await;
        let workflow = state.workflow.as_ref().ok_or_else(|| {
            error_core::helpers::validation_error("工作流未加载", "workflow_not_loaded")
        })?;

        let node = workflow
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .ok_or_else(|| error_core::helpers::not_found("node", node_id))?
            .clone();

        let agent_name = format!("{}", node.agent_type);
        let agents = self.agents.lock().await;
        let agent = agents
            .get(&agent_name)
            .cloned()
            .ok_or_else(|| error_core::helpers::not_found("agent", &agent_name))?;

        Ok((node, agent))
    }

    /// 计算重试延迟（支持指数退避）
    fn calculate_retry_delay(
        base_delay: Duration,
        retry_count: u32,
        exponential: bool,
    ) -> Duration {
        if exponential {
            base_delay.saturating_mul(2u32.saturating_pow(retry_count.saturating_sub(1)))
        } else {
            base_delay
        }
    }

    /// 执行单个节点（带重试逻辑）
    async fn execute_node(&self, node_id: &str) -> crate::Result<(String, NodeResult)> {
        let start_time = std::time::Instant::now();

        info!(node = %node_id, "开始执行节点");

        {
            let mut state = self.shared_state.write().await;
            state
                .node_statuses
                .insert(node_id.to_string(), NodeStatus::Running);
        }

        let (node, agent) = self.resolve_node_and_agent(node_id).await?;

        let mut retry_count: u32 = 0;
        let max_retries = node.retry_config.max_retries;
        let base_delay = Duration::from_millis(node.retry_config.retry_delay_ms);

        let result = loop {
            let execution_result =
                tokio::time::timeout(node.timeout, agent.execute_task(&node.task)).await;

            match execution_result {
                Ok(Ok(task_result)) => {
                    info!(node = %node_id, "节点执行成功");
                    break NodeResult {
                        node_id: node_id.to_string(),
                        status: NodeStatus::Succeeded,
                        result: Some(task_result),
                        error: None,
                        started_at: Utc::now(),
                        completed_at: Some(Utc::now()),
                        retry_count,
                    };
                }
                Ok(Err(e)) => {
                    retry_count += 1;
                    if retry_count <= max_retries {
                        let delay = Self::calculate_retry_delay(
                            base_delay,
                            retry_count,
                            node.retry_config.exponential_backoff,
                        );
                        warn!(
                            node = %node_id,
                            attempt = retry_count,
                            max_retries,
                            delay_ms = delay.as_millis(),
                            error = %e,
                            "节点执行失败，准备重试"
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    error!(node = %node_id, error = %e, "节点执行失败（重试耗尽）");
                    break NodeResult {
                        node_id: node_id.to_string(),
                        status: NodeStatus::Failed,
                        result: None,
                        error: Some(NodeErrorKind::InternalError {
                            reason: e.to_string(),
                        }),
                        started_at: Utc::now(),
                        completed_at: Some(Utc::now()),
                        retry_count,
                    };
                }
                Err(_) => {
                    retry_count += 1;
                    if retry_count <= max_retries {
                        let delay = Self::calculate_retry_delay(
                            base_delay,
                            retry_count,
                            node.retry_config.exponential_backoff,
                        );
                        warn!(
                            node = %node_id,
                            attempt = retry_count,
                            max_retries,
                            "节点执行超时，准备重试"
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    let timeout_ms = node.timeout.as_millis() as u64;
                    warn!(node = %node_id, timeout_ms, "节点执行超时（重试耗尽）");
                    break NodeResult {
                        node_id: node_id.to_string(),
                        status: NodeStatus::TimedOut,
                        result: None,
                        error: Some(NodeErrorKind::Timeout {
                            elapsed_secs: timeout_ms / 1000,
                        }),
                        started_at: Utc::now(),
                        completed_at: Some(Utc::now()),
                        retry_count,
                    };
                }
            }
        };

        #[allow(clippy::cast_possible_truncation)]
        let duration_ms = start_time.elapsed().as_millis() as u64;
        debug!(node = %node_id, duration_ms, status = ?result.status, retry_count, "节点执行完成");

        Ok((node_id.to_string(), result))
    }

    /// 确定工作流的最终状态
    async fn determine_final_status(&self) -> WorkflowStatus {
        let state = self.shared_state.read().await;

        let mut has_failed = false;
        let mut has_succeeded = false;
        let mut has_pending = false;

        for status in state.node_statuses.values() {
            match status {
                NodeStatus::Succeeded => has_succeeded = true,
                NodeStatus::Failed | NodeStatus::TimedOut => has_failed = true,
                NodeStatus::Pending | NodeStatus::Running => has_pending = true,
                NodeStatus::Skipped => {}
            }
        }

        if has_pending {
            WorkflowStatus::Running
        } else if has_failed && has_succeeded {
            WorkflowStatus::PartialFailure
        } else if has_failed {
            WorkflowStatus::Failed
        } else {
            WorkflowStatus::Succeeded
        }
    }

    /// 暂停工作流执行
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - 工作流 ID
    ///
    /// # Errors
    ///
    /// 当工作流不在运行状态时返回错误。
    pub async fn pause(&self, _workflow_id: &str) -> crate::Result<()> {
        let mut state = self.shared_state.write().await;

        if state.status != WorkflowStatus::Running {
            return Err(error_core::helpers::validation_error(
                "工作流不在运行状态，无法暂停",
                "workflow_pause",
            ));
        }

        state.paused = true;
        state.status = WorkflowStatus::Paused;

        info!("工作流已暂停");
        Ok(())
    }

    /// 恢复被暂停的工作流
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - 工作流 ID
    ///
    /// # Errors
    ///
    /// 当工作流不在暂停状态时返回错误。
    pub async fn resume(&self, _workflow_id: &str) -> crate::Result<()> {
        let mut state = self.shared_state.write().await;

        if state.status != WorkflowStatus::Paused {
            return Err(error_core::helpers::validation_error(
                "工作流不在暂停状态，无法恢复",
                "workflow_resume",
            ));
        }

        state.paused = false;
        state.status = WorkflowStatus::Running;

        info!("工作流已恢复");
        Ok(())
    }

    /// 获取工作流当前状态
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - 工作流 ID
    pub async fn status(&self, _workflow_id: &str) -> WorkflowStatus {
        let state = self.shared_state.read().await;
        state.status
    }

    /// 获取详细的状态快照（用于调试和监控）
    pub async fn get_detailed_status(&self) -> HashMap<String, NodeStatus> {
        let state = self.shared_state.read().await;
        state.node_statuses.clone()
    }
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::types::ArtifactType;
    use super::*;

    // Mock Agent Executor 用于测试
    struct MockAgentExecutor;

    #[async_trait::async_trait]
    impl AgentExecutor for MockAgentExecutor {
        async fn execute_task(&self, task: &Task) -> crate::Result<TaskResult> {
            Ok(TaskResult::success(
                task.id,
                serde_json::json!({"mock": true}),
                "Mock 完成",
            ))
        }
    }

    #[tokio::test]
    async fn test_workflow_definition_validation_success() {
        let definition = WorkflowDefinition {
            id: "test_wf".to_string(),
            name: "测试工作流".to_string(),
            description: "测试用".to_string(),
            nodes: vec![
                TaskNode::new(
                    "node1",
                    "节点1",
                    Task::new("任务1", "目标1"),
                    AgentType::Coordinator,
                ),
                TaskNode::new(
                    "node2",
                    "节点2",
                    Task::new("任务2", "目标2"),
                    AgentType::Researcher,
                )
                .with_dependency("node1"),
            ],
            edges: vec![Edge {
                from: "node1".to_string(),
                to: "node2".to_string(),
                condition: None,
            }],
            global_config: GlobalConfig::default(),
        };

        assert!(definition.validate().is_ok());
    }

    #[tokio::test]
    async fn test_workflow_definition_cycle_detection() {
        let definition = WorkflowDefinition {
            id: "cycle_wf".to_string(),
            name: "循环工作流".to_string(),
            description: "应该失败的".to_string(),
            nodes: vec![
                TaskNode::new("a", "A", Task::new("", ""), AgentType::Coordinator),
                TaskNode::new("b", "B", Task::new("", ""), AgentType::Researcher),
            ],
            edges: vec![
                Edge {
                    from: "a".to_string(),
                    to: "b".to_string(),
                    condition: None,
                },
                Edge {
                    from: "b".to_string(),
                    to: "a".to_string(),
                    condition: None,
                },
            ],
            global_config: GlobalConfig::default(),
        };

        assert!(definition.validate().is_err());
    }

    #[tokio::test]
    async fn test_task_node_creation() {
        let task = Task::new("测试", "目标");
        let node = TaskNode::new("id", "名称", task, AgentType::Analyst)
            .with_dependency("prev")
            .with_timeout(Duration::from_secs(60));

        assert_eq!(node.id, "id");
        assert_eq!(node.agent_type, AgentType::Analyst);
        assert_eq!(node.timeout, Duration::from_secs(60));
        assert!(node.dependencies.contains(&"prev".to_string()));
    }

    #[tokio::test]
    async fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert!(config.exponential_backoff);
    }

    #[tokio::test]
    async fn test_global_config_default() {
        let config = GlobalConfig::default();
        assert!(config.fail_fast);
        assert_eq!(config.max_concurrency, 5);
    }

    #[tokio::test]
    async fn test_agent_type_display() {
        assert_eq!(format!("{}", AgentType::Researcher), "Researcher");
        assert_eq!(
            format!("{}", AgentType::Custom("MyAgent".to_string())),
            "MyAgent"
        );
    }

    #[tokio::test]
    async fn test_orchestrator_creation_and_registration() {
        let orchestrator = TaskOrchestrator::new();

        let mock_agent = Arc::new(Box::new(MockAgentExecutor) as Box<dyn AgentExecutor>);
        orchestrator.register_agent("Coordinator", mock_agent).await;

        // 应该不会 panic
    }

    #[tokio::test]
    async fn test_simple_linear_workflow_execution() {
        let orchestrator = TaskOrchestrator::new();

        // 注册 Agent
        let mock_agent = Arc::new(Box::new(MockAgentExecutor) as Box<dyn AgentExecutor>);
        orchestrator
            .register_agent("Coordinator", mock_agent.clone())
            .await;
        orchestrator.register_agent("Researcher", mock_agent).await;

        // 创建简单的线性工作流
        let definition = WorkflowDefinition {
            id: "linear_test".to_string(),
            name: "线性测试".to_string(),
            description: String::new(),
            nodes: vec![
                TaskNode::new(
                    "step1",
                    "步骤1",
                    Task::new("第一步", "完成步骤1"),
                    AgentType::Coordinator,
                ),
                TaskNode::new(
                    "step2",
                    "步骤2",
                    Task::new("第二步", "完成步骤2"),
                    AgentType::Researcher,
                )
                .with_dependency("step1"),
            ],
            edges: vec![Edge {
                from: "step1".to_string(),
                to: "step2".to_string(),
                condition: None,
            }],
            global_config: GlobalConfig {
                fail_fast: true,
                ..Default::default()
            },
        };

        orchestrator.load_workflow(&definition).await.unwrap();
        let result = orchestrator.execute("linear_test").await.unwrap();

        assert_eq!(result.status, WorkflowStatus::Succeeded);
        assert_eq!(result.node_results.len(), 2);
    }

    #[tokio::test]
    async fn test_node_status_serialization() {
        let status = NodeStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: NodeStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, NodeStatus::Running);
    }

    #[tokio::test]
    async fn test_workflow_status_transitions() {
        let orchestrator = TaskOrchestrator::new();

        // 初始状态应该是 Idle
        assert_eq!(orchestrator.status("test").await, WorkflowStatus::Idle);
    }

    #[tokio::test]
    async fn test_parallel_workflow_definition() {
        // 测试并行节点的工作流定义
        let definition = WorkflowDefinition {
            id: "parallel_test".to_string(),
            name: "并行测试".to_string(),
            description: String::new(),
            nodes: vec![
                TaskNode::new(
                    "start",
                    "开始",
                    Task::new("初始化", ""),
                    AgentType::Coordinator,
                ),
                TaskNode::new(
                    "task_a",
                    "任务 A",
                    Task::new("并行任务 A", ""),
                    AgentType::Analyst,
                )
                .with_dependency("start"),
                TaskNode::new(
                    "task_b",
                    "任务 B",
                    Task::new("并行任务 B", ""),
                    AgentType::Writer,
                )
                .with_dependency("start"),
                TaskNode::new("end", "结束", Task::new("汇总", ""), AgentType::Reviewer)
                    .with_dependency("task_a")
                    .with_dependency("task_b"),
            ],
            edges: vec![
                Edge {
                    from: "start".to_string(),
                    to: "task_a".to_string(),
                    condition: None,
                },
                Edge {
                    from: "start".to_string(),
                    to: "task_b".to_string(),
                    condition: None,
                },
                Edge {
                    from: "task_a".to_string(),
                    to: "end".to_string(),
                    condition: None,
                },
                Edge {
                    from: "task_b".to_string(),
                    to: "end".to_string(),
                    condition: None,
                },
            ],
            global_config: GlobalConfig::default(),
        };

        assert!(definition.validate().is_ok());
    }

    #[test]
    fn test_artifact_collection_all_nodes_succeeded() {
        let now = Utc::now();

        let mut node_results = HashMap::new();
        node_results.insert(
            "node1".to_string(),
            NodeResult {
                node_id: "node1".to_string(),
                status: NodeStatus::Succeeded,
                result: Some(TaskResult {
                    task_id: uuid::Uuid::new_v4(),
                    success: true,
                    output: serde_json::json!({}),
                    steps_taken: 1,
                    total_duration_ms: 100,
                    total_tokens_used: 10,
                    summary: String::new(),
                    artifacts: vec![Artifact {
                        artifact_type: ArtifactType::Document,
                        content: serde_json::json!({"data": "doc1"}),
                        name: "doc1".to_string(),
                    }],
                }),
                error: None,
                started_at: now,
                completed_at: Some(now),
                retry_count: 0,
            },
        );
        node_results.insert(
            "node2".to_string(),
            NodeResult {
                node_id: "node2".to_string(),
                status: NodeStatus::Succeeded,
                result: Some(TaskResult {
                    task_id: uuid::Uuid::new_v4(),
                    success: true,
                    output: serde_json::json!({}),
                    steps_taken: 1,
                    total_duration_ms: 200,
                    total_tokens_used: 20,
                    summary: String::new(),
                    artifacts: vec![
                        Artifact {
                            artifact_type: ArtifactType::AnalysisReport,
                            content: serde_json::json!({"data": "report1"}),
                            name: "report1".to_string(),
                        },
                        Artifact {
                            artifact_type: ArtifactType::KnowledgeGraph,
                            content: serde_json::json!({"data": "kg1"}),
                            name: "kg1".to_string(),
                        },
                    ],
                }),
                error: None,
                started_at: now + chrono::Duration::seconds(1),
                completed_at: Some(now),
                retry_count: 0,
            },
        );

        let mut nodes_with_artifacts: Vec<_> = node_results
            .values()
            .filter(|nr| nr.status == NodeStatus::Succeeded)
            .filter_map(|nr| nr.result.as_ref().map(|tr| (nr.started_at, &tr.artifacts)))
            .collect();

        nodes_with_artifacts.sort_by_key(|(started_at, _)| *started_at);

        let artifacts: Vec<Artifact> = nodes_with_artifacts
            .into_iter()
            .flat_map(|(_, artifacts)| artifacts.clone())
            .collect();

        assert_eq!(artifacts.len(), 3);
        assert_eq!(artifacts[0].name, "doc1");
        assert_eq!(artifacts[1].name, "report1");
        assert_eq!(artifacts[2].name, "kg1");
    }

    #[test]
    fn test_artifact_collection_partial_failure() {
        let now = Utc::now();

        let mut node_results = HashMap::new();
        node_results.insert(
            "node1".to_string(),
            NodeResult {
                node_id: "node1".to_string(),
                status: NodeStatus::Succeeded,
                result: Some(TaskResult {
                    task_id: uuid::Uuid::new_v4(),
                    success: true,
                    output: serde_json::json!({}),
                    steps_taken: 1,
                    total_duration_ms: 100,
                    total_tokens_used: 10,
                    summary: String::new(),
                    artifacts: vec![Artifact {
                        artifact_type: ArtifactType::Document,
                        content: serde_json::json!({"data": "doc1"}),
                        name: "doc1".to_string(),
                    }],
                }),
                error: None,
                started_at: now,
                completed_at: Some(now),
                retry_count: 0,
            },
        );
        node_results.insert(
            "node2".to_string(),
            NodeResult {
                node_id: "node2".to_string(),
                status: NodeStatus::Failed,
                result: None,
                error: Some(NodeErrorKind::InternalError {
                    reason: "执行失败".to_string(),
                }),
                started_at: now + chrono::Duration::seconds(1),
                completed_at: Some(now),
                retry_count: 0,
            },
        );

        let mut nodes_with_artifacts: Vec<_> = node_results
            .values()
            .filter(|nr| nr.status == NodeStatus::Succeeded)
            .filter_map(|nr| nr.result.as_ref().map(|tr| (nr.started_at, &tr.artifacts)))
            .collect();

        nodes_with_artifacts.sort_by_key(|(started_at, _)| *started_at);

        let artifacts: Vec<Artifact> = nodes_with_artifacts
            .into_iter()
            .flat_map(|(_, artifacts)| artifacts.clone())
            .collect();

        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "doc1");
    }

    #[test]
    fn test_artifact_collection_no_artifacts() {
        let now = Utc::now();

        let mut node_results = HashMap::new();
        node_results.insert(
            "node1".to_string(),
            NodeResult {
                node_id: "node1".to_string(),
                status: NodeStatus::Succeeded,
                result: Some(TaskResult {
                    task_id: uuid::Uuid::new_v4(),
                    success: true,
                    output: serde_json::json!({}),
                    steps_taken: 1,
                    total_duration_ms: 100,
                    total_tokens_used: 10,
                    summary: String::new(),
                    artifacts: Vec::new(),
                }),
                error: None,
                started_at: now,
                completed_at: Some(now),
                retry_count: 0,
            },
        );

        let mut nodes_with_artifacts: Vec<_> = node_results
            .values()
            .filter(|nr| nr.status == NodeStatus::Succeeded)
            .filter_map(|nr| nr.result.as_ref().map(|tr| (nr.started_at, &tr.artifacts)))
            .collect();

        nodes_with_artifacts.sort_by_key(|(started_at, _)| *started_at);

        let artifacts: Vec<Artifact> = nodes_with_artifacts
            .into_iter()
            .flat_map(|(_, artifacts)| artifacts.clone())
            .collect();

        assert!(artifacts.is_empty());
    }

    #[test]
    fn test_workflow_status_can_transition_valid() {
        assert!(WorkflowStatus::Idle.can_transition_to(&WorkflowStatus::Running));
        assert!(WorkflowStatus::Running.can_transition_to(&WorkflowStatus::Paused));
        assert!(WorkflowStatus::Running.can_transition_to(&WorkflowStatus::Succeeded));
        assert!(WorkflowStatus::Running.can_transition_to(&WorkflowStatus::Failed));
        assert!(WorkflowStatus::Running.can_transition_to(&WorkflowStatus::PartialFailure));
        assert!(WorkflowStatus::Running.can_transition_to(&WorkflowStatus::TimedOut));
        assert!(WorkflowStatus::Paused.can_transition_to(&WorkflowStatus::Running));
    }

    #[test]
    fn test_workflow_status_can_transition_invalid() {
        assert!(!WorkflowStatus::Idle.can_transition_to(&WorkflowStatus::Idle));
        assert!(!WorkflowStatus::Idle.can_transition_to(&WorkflowStatus::Succeeded));
        assert!(!WorkflowStatus::Idle.can_transition_to(&WorkflowStatus::Paused));
        assert!(!WorkflowStatus::Succeeded.can_transition_to(&WorkflowStatus::Running));
        assert!(!WorkflowStatus::Failed.can_transition_to(&WorkflowStatus::Running));
        assert!(!WorkflowStatus::TimedOut.can_transition_to(&WorkflowStatus::Running));
        assert!(!WorkflowStatus::PartialFailure.can_transition_to(&WorkflowStatus::Idle));
        assert!(!WorkflowStatus::Paused.can_transition_to(&WorkflowStatus::Succeeded));
    }

    #[test]
    fn test_workflow_status_is_terminal() {
        assert!(WorkflowStatus::Succeeded.is_terminal());
        assert!(WorkflowStatus::Failed.is_terminal());
        assert!(WorkflowStatus::PartialFailure.is_terminal());
        assert!(WorkflowStatus::TimedOut.is_terminal());
        assert!(!WorkflowStatus::Idle.is_terminal());
        assert!(!WorkflowStatus::Running.is_terminal());
        assert!(!WorkflowStatus::Paused.is_terminal());
    }

    #[test]
    fn test_node_error_kind_display() {
        let dep_err = NodeErrorKind::DependencyFailed {
            dependency_id: "node_a".to_string(),
        };
        assert_eq!(format!("{dep_err}"), "依赖节点失败: node_a");

        let timeout_err = NodeErrorKind::Timeout { elapsed_secs: 30 };
        assert_eq!(format!("{timeout_err}"), "节点执行超时: 30s");

        let tool_err = NodeErrorKind::ToolError {
            tool_name: "search".to_string(),
        };
        assert_eq!(format!("{tool_err}"), "工具调用失败: search");

        let internal_err = NodeErrorKind::InternalError {
            reason: "未知错误".to_string(),
        };
        assert_eq!(format!("{internal_err}"), "未知错误");
    }

    #[test]
    fn test_agent_type_display_all_variants() {
        assert_eq!(format!("{}", AgentType::Researcher), "Researcher");
        assert_eq!(format!("{}", AgentType::Analyst), "Analyst");
        assert_eq!(format!("{}", AgentType::Writer), "Writer");
        assert_eq!(format!("{}", AgentType::Reviewer), "Reviewer");
        assert_eq!(format!("{}", AgentType::Coordinator), "Coordinator");
        assert_eq!(
            format!("{}", AgentType::Custom("Special".to_string())),
            "Special"
        );
    }

    #[tokio::test]
    async fn test_workflow_definition_validate_missing_from_node() {
        let definition = WorkflowDefinition {
            id: "test".to_string(),
            name: "test".to_string(),
            description: String::new(),
            nodes: vec![TaskNode::new(
                "a",
                "A",
                Task::new("", ""),
                AgentType::Coordinator,
            )],
            edges: vec![Edge {
                from: "nonexistent".to_string(),
                to: "a".to_string(),
                condition: None,
            }],
            global_config: GlobalConfig::default(),
        };
        assert!(definition.validate().is_err());
    }

    #[tokio::test]
    async fn test_workflow_definition_validate_missing_to_node() {
        let definition = WorkflowDefinition {
            id: "test".to_string(),
            name: "test".to_string(),
            description: String::new(),
            nodes: vec![TaskNode::new(
                "a",
                "A",
                Task::new("", ""),
                AgentType::Coordinator,
            )],
            edges: vec![Edge {
                from: "a".to_string(),
                to: "nonexistent".to_string(),
                condition: None,
            }],
            global_config: GlobalConfig::default(),
        };
        assert!(definition.validate().is_err());
    }

    #[test]
    fn test_task_node_with_retry() {
        let retry = RetryConfig {
            max_retries: 5,
            retry_delay_ms: 200,
            exponential_backoff: false,
        };
        let node = TaskNode::new("id", "name", Task::new("task", "goal"), AgentType::Writer)
            .with_retry(retry.clone());
        assert_eq!(node.retry_config.max_retries, 5);
        assert_eq!(node.retry_config.retry_delay_ms, 200);
        assert!(!node.retry_config.exponential_backoff);
    }

    #[test]
    fn test_node_status_all_variants_serialization() {
        let statuses = [
            NodeStatus::Pending,
            NodeStatus::Running,
            NodeStatus::Succeeded,
            NodeStatus::Failed,
            NodeStatus::Skipped,
            NodeStatus::TimedOut,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let de: NodeStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, de);
        }
    }

    #[test]
    fn test_workflow_status_all_variants_serialization() {
        let statuses = [
            WorkflowStatus::Idle,
            WorkflowStatus::Running,
            WorkflowStatus::Paused,
            WorkflowStatus::Succeeded,
            WorkflowStatus::Failed,
            WorkflowStatus::PartialFailure,
            WorkflowStatus::TimedOut,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let de: WorkflowStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, de);
        }
    }

    #[test]
    fn test_node_error_kind_serialization() {
        let errors = [
            NodeErrorKind::DependencyFailed {
                dependency_id: "n1".to_string(),
            },
            NodeErrorKind::Timeout { elapsed_secs: 10 },
            NodeErrorKind::ToolError {
                tool_name: "t1".to_string(),
            },
            NodeErrorKind::InternalError {
                reason: "err".to_string(),
            },
        ];
        for e in &errors {
            let json = serde_json::to_string(e).unwrap();
            let de: NodeErrorKind = serde_json::from_str(&json).unwrap();
            assert_eq!(*e, de);
        }
    }

    #[test]
    fn test_global_config_with_max_duration() {
        let config = GlobalConfig {
            max_duration: Some(Duration::from_secs(3600)),
            fail_fast: false,
            max_concurrency: 10,
        };
        assert_eq!(config.max_duration, Some(Duration::from_secs(3600)));
        assert!(!config.fail_fast);
        assert_eq!(config.max_concurrency, 10);
    }

    #[test]
    fn test_edge_with_condition() {
        let edge = Edge {
            from: "a".to_string(),
            to: "b".to_string(),
            condition: Some("x > 0".to_string()),
        };
        assert_eq!(edge.condition, Some("x > 0".to_string()));
    }
}
