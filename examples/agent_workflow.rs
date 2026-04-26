//! Agent 工作流执行示例
//!
//! 展示 ReAct Agent 如何执行复杂的多步推理任务，
//! 包括 Thought→Action→Observation 循环和工具调用链。
//!
//! 运行方式:
//!   cargo run --example agent_workflow --package knowledge-api

use std::sync::Arc;

use knowledge_api::agent::{
    AgentConfig, AgentTask, AgentResult, ReActAgent, ToolBelt,
};
use knowledge_api::AppState;
use knowledge_core::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,knowledge_api::agent=trace")
        .init();

    println!("🤖 文本全结构化知识系统 - ReAct Agent 示例\n");
    println!("═".repeat(58));

    // =========================================================================
    // 1. 初始化 Agent 引擎
    // =========================================================================
    println!("\n📌 步骤 1: 初始化 ReAct Agent");

    let state = AppState::new().await?;

    let config = AgentConfig {
        max_reasoning_steps: 15,
        max_execution_time: std::time::Duration::from_secs(30),
        temperature: 0.1,
        verbose: true,
    };

    let tool_belt = ToolBelt::default_with_state(state.clone());
    let agent = ReActAgent::new(state.llm_client(), tool_belt, config);

    println!("   Agent 配置:");
    println!("   ├─ 最大推理步数: {}", config.max_reasoning_steps);
    println!("   ├─ 单次超时: {:?}", config.max_execution_time);
    println!("   ├─ 温度参数: {}", config.temperature);
    println!("   └─ 详细输出: {}", config.verbose);

    // =========================================================================
    // 2. 定义任务
    // =========================================================================
    println!("\n📌 步骤 2: 定义 Agent 任务");

    let tasks = vec![
        AgentTask {
            description: "查找 Rust 异步运行时 Tokio 的核心组件，并分析其与 async/await 的关系".to_string(),
            context: Some("用户正在学习 Rust 异步编程".to_string()),
        },
        AgentTask {
            description: "分析 SurrealDB 的关系表(RELATION)特性如何支持知识图谱建模".to_string(),
            context: Some("技术选型评估".to_string()),
        },
        AgentTask {
            description: "对比 CQRS+Event Sourcing 与传统 CRUD 架构在审计追踪方面的差异".to_string(),
            context: Some("架构设计决策".to_string()),
        },
    ];

    for (i, task) in tasks.iter().enumerate() {
        println!("\n   任务 #{}: {}", i + 1, task.description);
        if let Some(ctx) = &task.context {
            println!("   上下文: {}", ctx);
        }
    }

    // =========================================================================
    // 3. 执行 Agent 推理循环
    // =========================================================================
    println!("\n{}", "═".repeat(58));
    println!("🔄 开始 ReAct 推理循环\n");

    for (task_idx, task) in tasks.iter().enumerate() {
        println!("{}", "─".repeat(58));
        println!("🎯 执行任务 #{}: {}\n", task_idx + 1, task.description);

        match agent.run(task.clone()).await {
            Ok(result) => {
                match result {
                    AgentResult::Done(answer) => {
                        println!("✅ 任务完成!");
                        println!("\n📝 最终答案:");
                        println!("{}\n", "─".repeat(40));
                        println!("{}", answer.content);
                        println!("{}", "─".repeat(40));
                        println!("\n📊 统计信息:");
                        println!("   ├─ 推理步数: {}", answer.steps_taken);
                        println!("   ├─ 工具调用次数: {}", answer.tool_calls);
                        println!("   └─ 总耗时: {:?}", answer.elapsed);
                    }
                    AgentResult::MaxStepsExceeded(steps) => {
                        println!("⚠️ 达到最大步数限制 ({})", steps);
                        println!("   建议: 增加 max_reasoning_steps 或简化任务描述");
                    }
                    AgentResult::Timeout(duration) => {
                        println!("⏰ 执行超时 ({:?})", duration);
                        println!("   建议: 增加 max_execution_time 或拆分任务");
                    }
                    AgentResult::Error(e) => {
                        println!("❌ Agent 错误: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ 执行失败: {}", e);
            }
        }

        if task_idx < tasks.len() - 1 {
            println!("\n⏳ 准备下一个任务...\n");
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    // =========================================================================
    // ReAct 循环可视化
    // =========================================================================
    println!("\n{}", "═".repeat(58));
    println!("🔄 ReAct 循环工作流:");
    println!();
    println!("   ┌──────────┐");
    println!("   │  用户任务  │");
    println!("   └────┬─────┘");
    println!("        ▼");
    println!("   ┌─────────────────────────────────────┐");
    println!("   │  Thought (LLM 推理)                  │");
    println!("   │  '我需要先搜索 X 的定义...'          │");
    println!("   └──────────────────┬──────────────────┘");
    println!("                      ▼");
    println!("   ┌─────────────────────────────────────┐");
    println!("   │  Action (工具调用)                    │");
    println!("   │  search_knowledge(query='Tokio 核心')│");
    println!("   └──────────────────┬──────────────────┘");
    println!("                      ▼");
    println!("   ┌─────────────────────────────────────┐");
    println!("   │  Observation (观察结果)               │");
    println!("   │  { results: [...], confidence: 0.95 }│");
    println!("   └──────────────────┬──────────────────┘");
    println!("                      │");
    println!("           ┌──────────┴──────────┐");
    println!("           ▼                     ▼");
    println!("    ┌─────────────┐      ┌─────────────┐");
    println!("    │ 继续推理    │      │ 任务完成    │");
    println!("    │ → Loop     │      │ → Return    │");
    println!("    └─────────────┘      └─────────────┘");
    println!();
    println!("   安全约束: max_steps=15, timeout=30s");

    // =========================================================================
    // 完成
    // =========================================================================
    println!("\n✅ Agent 工作流示例完成！");
    println!("\n💡 Agent 能力概览:");
    println!("   🔍 search_knowledge — 语义搜索");
    println!("   📄 get_document — 文档详情");
    println!("   🔗 list_references — 引用关系");
    println!("   📊 get_graph_stats — 图谱统计");
    println!("   🎯 analyze_impact — 影响分析");
    println!("   💬 rag_query — RAG 问答");

    Ok(())
}
