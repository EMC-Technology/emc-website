//! MCP 工具注册和使用示例
//!
//! 展示如何注册自定义 MCP (Model Context Protocol) 工具，
//! 以及如何通过 MCP Server 将系统的能力暴露给 AI 编程助手。
//!
//! 运行方式:
//!   cargo run --example mcp_tools --package knowledge-api

use std::sync::Arc;

use knowledge_api::mcp::{
    server::KnowledgeMcpServer,
    tools::{McpTool, ToolRegistry, ToolResult},
};
use knowledge_api::AppState;
use schemars::{schema_for!, JsonSchema};

/// 自定义 MCP 工具：代码影响分析
#[derive(Debug, Clone)]
struct ImpactAnalysisTool {
    state: Arc<AppState>,
}

impl McpTool for ImpactAnalysisTool {
    fn name(&self) -> &str {
        "analyze_code_impact"
    }

    fn description(&self) -> &str {
        "分析代码变更的影响范围，找出受影响的下游服务和引用关系"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::to_value(schema_for!(ImpactAnalysisInput)).unwrap_or_default()
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let params: ImpactAnalysisInput = serde_json::from_value(input)?;

        println!("🔍 分析代码影响: {:?}", params.changed_files);

        let analysis = self.state.knowledge_vm.analyze_impact(&params.changed_files).await?;

        Ok(ToolResult::success(serde_json::json!({
            "impact_analysis": analysis,
            "affected_symbols": analysis.affected_symbols.len(),
            "risk_level": analysis.max_risk_level(),
            "suggestions": analysis.suggestions,
        })))
    }
}

#[derive(Debug, JsonSchema)]
struct ImpactAnalysisInput {
    changed_files: Vec<String>,
    include_transitive: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,knowledge_api::mcp=debug")
        .init();

    println!("🔌 文本全结构化知识系统 - MCP 工具示例\n");
    println!("═".repeat(55));

    // =========================================================================
    // 1. 初始化 MCP Server
    // =========================================================================
    println!("\n📌 步骤 1: 初始化 MCP Server");

    let state = AppState::new().await?;
    let mut mcp_server = KnowledgeMcpServer::new(state.clone());

    println!("   MCP Server 已初始化");
    println!("   协议版本: 2024-11-05");

    // =========================================================================
    // 2. 注册内置工具
    // =========================================================================
    println!("\n📌 步骤 2: 注册内置 MCP 工具");

    mcp_server.register_default_tools().await?;

    println!("   内置工具列表:");
    println!("   ├─ search_knowledge     — 语义搜索知识库");
    println!("   ├─ get_document          — 获取文档详情");
    println!("   ├─ ingest_file           — 摄入文件到知识库");
    println!("   ├─ list_references       — 列出引用关系");
    println!("   ├─ get_graph_stats       — 图谱统计信息");
    println!("   └─ analyze_impact        — 代码影响分析");

    // =========================================================================
    // 3. 注册自定义工具
    // =========================================================================
    println!("\n📌 步骤 3: 注册自定义工具");

    let impact_tool = ImpactAnalysisTool { state: state.clone() };
    mcp_server.register_tool(Box::new(impact_tool)).await?;

    println!("   ✅ 自定义工具已注册: analyze_code_impact");

    // =========================================================================
    // 4. 列出所有可用工具
    // =========================================================================
    println!("\n📌 步骤 4: 列出所有工具");

    let tools = mcp_server.list_tools().await;
    println!("\n   📋 当前已注册工具 (共 {} 个):", tools.len());

    for (i, tool) in tools.iter().enumerate() {
        println!(
            "   {:2}. {:20} | {}",
            i + 1,
            tool.name,
            &tool.description[..tool.description.len().min(45)]
        );
    }

    // =========================================================================
    // 5. 模拟工具调用
    // =========================================================================
    println!("\n📌 步骤 5: 模拟 MCP 工具调用");

    let test_calls = [
        ("search_knowledge", serde_json::json!({
            "query": "CQRS Event Sourcing",
            "limit": 5
        })),
        ("get_graph_stats", serde_json::json!({})),
        ("analyze_code_impact", serde_json::json!({
            "changed_files": ["src/cqrs/command.rs"],
            "include_transitive": true
        })),
    ];

    for (tool_name, args) in &test_calls {
        println!("\n   🔧 调用工具: {}", tool_name);
        println!("   📥 参数: {}", args);

        match mcp_server.call_tool(tool_name, args.clone()).await {
            Ok(result) => {
                println!("   📤 结果: {}", result.status);
                if let Some(content) = result.content.first() {
                    let preview = format!("{:?}", content);
                    println!("   📄 内容预览: {}...", &preview[..preview.len().min(80)]);
                }
            }
            Err(e) => {
                println!("   ❌ 错误: {}", e);
            }
        }
    }

    // =========================================================================
    // 6. 启动 MCP Server (stdio 模式)
    // =========================================================================
    println!("\n{}", "═".repeat(55));
    println!("🚀 MCP Server 可用模式:");

    println!("\n   1. stdio 模式 (AI 编程助手集成):");
    println!("      cargo run --bin knowledge-api -- --mcp-transport stdio");

    println!("\n   2. SSE 模式 (HTTP 长连接):");
    println!("      cargo run --bin knowledge-api -- --mcp-transport sse --mcp-port 3000");

    println!("\n   3. 在 Cursor/Windsurf/Claude Code 中配置:");
    println!("      ```json");
    println!("      {{");
    println!("        \"mcpServers\": {{");
    println!("          \"knowledge-system\": {{");
    println!("            \"command\": \"cargo\",");
    println!("            \"args\": [\"run\", \"--bin\", \"knowledge-api\", \"--\", \"--mcp\"]");
    println!("          }}");
    println!("        }}");
    println!("      }}");
    println!("      ```");

    println!("\n✅ MCP 示例完成！");

    Ok(())
}
