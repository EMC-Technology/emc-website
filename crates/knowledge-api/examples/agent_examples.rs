//! Agent 工作流引擎使用示例
//!
//! 本文件展示如何使用 [`ReAct`] Agent 框架构建智能应用。
//! 包含以下示例：
//! 1. 基本的单 Agent 使用
//! 2. 工具注册与调用
//! 3. 记忆系统集成
//! 4. DAG 工作流编排
//! 5. 完整的知识图谱构建流程

use std::time::Duration;

use knowledge_api::agent::tools::{AgentContext, SearchTool};
use knowledge_api::agent::types::TaskPriority;
use knowledge_api::agent::{AgentConfig, AgentToolInvoker, MemorySystem, Task, TaskOrchestrator};

// ============================================================================
// 示例 1: 基本的 ReAct Agent 使用
// ============================================================================

/// # 示例：创建并运行一个简单的 Agent
///
/// 展示了创建 Agent、定义任务和执行的基本流程。
async fn example_basic_agent() {
    // 1. 创建 LLM 后端（这里使用 Mock，实际应替换为真实实现）
    // let llm = OpenAIBackend::new("your-api-key");

    // 2. 创建工具注册表并注册工具
    let tools = AgentToolInvoker::new();
    tools.register_tool(SearchTool).await.unwrap();

    // 3. 配置 Agent
    let _config = AgentConfig::builder()
        .max_iterations(15)
        .temperature(0.7)
        .verbose(true)
        .timeout(Duration::from_secs(120))
        .build();

    // 4. 创建 Agent（需要真实的 LLM 后端）
    // let agent = ReactAgent::new(llm, tools, config);

    // 5. 定义任务
    let _task = Task::new(
        "分析这份技术文档，提取所有 API 端点及其功能描述",
        "生成完整的 API 文档摘要",
    )
    .with_priority(TaskPriority::High);

    // 6. 执行任务
    // let result = agent.execute(&task).await.unwrap();

    println!("✅ 任务执行完成");
}

// ============================================================================
// 示例 2: 自定义工具开发与注册
// ============================================================================

/// # 示例：实现自定义工具
///
/// 展示如何创建自定义工具并注册到 Agent。
async fn example_custom_tools() {
    use knowledge_api::agent::tools::{Tool, ToolOutput};

    /// 自定义工具：数据库查询
    struct DatabaseQueryTool;

    #[async_trait::async_trait]
    impl Tool for DatabaseQueryTool {
        fn name(&self) -> &'static str {
            "db_query"
        }

        fn description(&self) -> &'static str {
            "在知识库中执行结构化查询"
        }

        fn parameters_schema(&self) -> &serde_json::Value {
            static SCHEMA: std::sync::LazyLock<serde_json::Value> =
                std::sync::LazyLock::new(|| {
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "table": {
                                "type": "string",
                                "description": "要查询的表名"
                            },
                            "filter": {
                                "type": "string",
                                "description": "过滤条件 (WHERE 子句)"
                            },
                            "limit": {
                                "type": "integer",
                                "description": "返回结果数量上限"
                            }
                        },
                        "required": ["table"]
                    })
                });
            &SCHEMA
        }

        async fn execute(
            &self,
            arguments: serde_json::Value,
            _ctx: &AgentContext,
        ) -> knowledge_api::Result<ToolOutput> {
            let table = arguments
                .get("table")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            Ok(ToolOutput::success_with_metadata(
                format!("{{\"table\": \"{table}\", \"rows\": []}}"),
                serde_json::json!({"query_executed": true}),
            ))
        }
    }

    // 注册自定义工具
    let invoker = AgentToolInvoker::new();
    invoker.register_tool(DatabaseQueryTool).await.unwrap();
    invoker.register_tool(SearchTool).await.unwrap();

    println!("✅ 已注册 {} 个工具", invoker.registry().count().await);
}

// ============================================================================
// 示例 3: 记忆系统配置
// ============================================================================

/// # 示例：配置和使用记忆系统
///
/// 展示如何配置短期、长期和事件记忆。
fn example_memory_system() {
    // 1. 创建短期工作记忆（滑动窗口）
    let _memory = MemorySystem::short_term_only();

    // 2. 添加观察和思考
    let _task_id = uuid::Uuid::new_v4();

    // 注意：memory 需要是可变的才能调用 add_observation
    // 在实际应用中，应该通过 Arc<Mutex<>> 或类似机制共享

    println!("✅ 记忆系统已初始化");
}

// ============================================================================
// 示例 4: DAG 工作流编排
// ============================================================================

/// # 示例：创建和执行 DAG 工作流
///
/// 展示如何定义复杂的多步骤工作流。
fn example_workflow_orchestration() {
    use knowledge_api::agent::executor::{
        AgentType, Edge, GlobalConfig, TaskNode, WorkflowDefinition,
    };

    // 1. 定义工作流节点
    let nodes = vec![
        // 节点 1: 数据收集
        TaskNode::new(
            "collect",
            "数据收集",
            Task::new("收集相关文档", "获取所有输入材料"),
            AgentType::Researcher,
        ),
        // 节点 2: 分析处理（依赖节点 1）
        TaskNode::new(
            "analyze",
            "数据分析",
            Task::new("分析文档内容", "提取关键信息"),
            AgentType::Analyst,
        )
        .with_dependency("collect"),
        // 节点 3: 报告生成（依赖节点 2）
        TaskNode::new(
            "report",
            "报告生成",
            Task::new("撰写分析报告", "输出最终报告"),
            AgentType::Writer,
        )
        .with_dependency("analyze"),
        // 节点 4: 质量审核（依赖节点 3）
        TaskNode::new(
            "review",
            "质量审核",
            Task::new("审查报告质量", "确保准确性"),
            AgentType::Reviewer,
        )
        .with_dependency("report"),
    ];

    // 2. 定义边（依赖关系）
    let edges = vec![
        Edge {
            from: "collect".to_string(),
            to: "analyze".to_string(),
            condition: None,
        },
        Edge {
            from: "analyze".to_string(),
            to: "report".to_string(),
            condition: None,
        },
        Edge {
            from: "report".to_string(),
            to: "review".to_string(),
            condition: None,
        },
    ];

    // 3. 构建工作流定义
    let workflow = WorkflowDefinition {
        id: "analysis_pipeline".to_string(),
        name: "数据分析流水线".to_string(),
        description: "自动化的数据分析工作流".to_string(),
        nodes,
        edges,
        global_config: GlobalConfig {
            max_duration: Some(Duration::from_secs(1800)),
            fail_fast: true,
            max_concurrency: 2,
        },
    };

    // 4. 验证工作流定义
    match workflow.validate() {
        Ok(()) => println!("✅ 工作流定义验证通过"),
        Err(e) => println!("❌ 工作流验证失败: {e}"),
    }

    // 5. 创建编排器并加载工作流
    let _orchestrator = TaskOrchestrator::new();

    println!("✅ 工作流编排器已准备就绪");
}

// ============================================================================
// 示例 5: 并行工作流（Fan-out/Fan-in）
// ============================================================================

/// # 示例：并行执行的 Fan-out/Fan-in 模式
///
/// 展示如何利用 DAG 实现任务的并行化。
fn example_parallel_workflow() {
    use knowledge_api::agent::executor::{
        AgentType, Edge, GlobalConfig, TaskNode, WorkflowDefinition,
    };

    let workflow = WorkflowDefinition {
        id: "parallel_demo".to_string(),
        name: "并行执行演示".to_string(),
        description: "展示 Fan-out/Fan-in 模式".to_string(),
        nodes: vec![
            TaskNode::new(
                "start",
                "开始",
                Task::new("初始化", ""),
                AgentType::Coordinator,
            ),
            TaskNode::new(
                "task_a",
                "并行任务 A",
                Task::new("执行 A", ""),
                AgentType::Analyst,
            )
            .with_dependency("start"),
            TaskNode::new(
                "task_b",
                "并行任务 B",
                Task::new("执行 B", ""),
                AgentType::Writer,
            )
            .with_dependency("start"),
            TaskNode::new(
                "end",
                "汇总",
                Task::new("合并结果", ""),
                AgentType::Coordinator,
            )
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

    println!(
        "✅ 并行工作流定义完成（包含 {} 个节点）",
        workflow.nodes.len()
    );
}

// ============================================================================
// 主函数：运行所有示例
// ============================================================================

#[tokio::main]
async fn main() {
    println!("🚀 Agent 工作流引擎 - 使用示例\n");

    println!("--- 示例 1: 基本 Agent 使用 ---");
    example_basic_agent().await;

    println!("\n--- 示例 2: 自定义工具 ---");
    example_custom_tools().await;

    println!("\n--- 示例 3: 记忆系统 ---");
    example_memory_system();

    println!("\n--- 示例 4: DAG 工作流编排 ---");
    example_workflow_orchestration();

    println!("\n--- 示例 5: 并行工作流 ---");
    example_parallel_workflow();

    println!("\n✨ 所有示例运行完毕！");
}
