//! MCP Server 核心结构体与生命周期管理

use std::sync::Arc;

use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{PromptMessage, PromptMessageRole, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, prompt, prompt_router, tool, tool_handler, tool_router};

use super::prompts::{DetectImpactParams, GenerateMapParams};
#[allow(clippy::wildcard_imports)]
use super::tools::*;
use crate::KnowledgeVM;
use crate::config::ConfigLoader;

/// `MCP` 协议服务端
///
/// 持有 `KnowledgeVM` 的共享引用，通过 `MCP` 协议对外暴露
/// 知识图谱的查询、文档管理等能力。
///
/// # Examples
///
/// ```ignore
/// let vm = KnowledgeVM::with_client(db);
/// let server = McpServer::new(vm);
/// server.run_stdio().await?;
/// ```
pub struct McpServer {
    vm: Arc<KnowledgeVM>,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

impl McpServer {
    /// 使用 `KnowledgeVM` 实例构造 `MCP` 服务端
    #[must_use]
    pub fn new(vm: KnowledgeVM) -> Self {
        Self {
            vm: Arc::new(vm),
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }

    /// 获取内部 `KnowledgeVM` 的共享引用
    #[must_use]
    pub const fn vm(&self) -> &Arc<KnowledgeVM> {
        &self.vm
    }

    /// 获取所有已注册工具的名称列表
    #[must_use]
    pub fn registered_tool_names(&self) -> Vec<String> {
        self.tool_router
            .list_all()
            .iter()
            .map(|t| t.name.to_string())
            .collect()
    }

    /// 通过标准输入/输出运行 MCP 服务端
    ///
    /// # Errors
    ///
    /// 当 MCP 协议握手失败或 I/O 通信异常时返回错误。
    pub async fn run_stdio(&self) -> crate::Result<()> {
        let service = Self {
            vm: self.vm.clone(),
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        };
        let (stdin, stdout) = rmcp::transport::io::stdio();
        rmcp::serve_server(service, (stdin, stdout))
            .await
            .map_err(|e| error_core::helpers::internal_error(&format!("MCP 服务启动失败: {e}")))?;
        Ok(())
    }
}

/// 启动 `MCP` Server 的 `CLI` 入口函数
///
/// 执行以下步骤：
/// 1. 初始化日志订阅器
/// 2. 从环境加载配置并校验
/// 3. 连接 `SurrealDB`
/// 4. 构造 `KnowledgeVM` 实例
/// 5. 创建 `McpServer` 并通过 stdio 运行
///
/// # Errors
///
/// 配置加载失败、数据库连接失败或 `MCP` 协议通信异常时返回错误。
pub async fn run_mcp_server() -> crate::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("启动 MCP Server {}", crate::API_VERSION);

    let config = ConfigLoader::load_default()?;
    ConfigLoader::validate(&config)?;

    tracing::info!("配置加载完成: 数据库={}", config.database.addr);

    let db_client = knowledge_core::SurrealDbClient::new(
        &config.database.addr,
        &config.database.namespace,
        &config.database.database,
    )
    .await?;

    tracing::info!("数据库连接成功: {}", config.database.addr);

    let knowledge_vm = KnowledgeVM::with_embedding_dim(db_client, config.parser.embedding_dim)
        .map_err(|e| error_core::helpers::internal_error(&e.to_string()))?;
    let mcp_server = McpServer::new(knowledge_vm);

    tracing::info!("MCP Server 已就绪，等待 stdio 连接");

    mcp_server.run_stdio().await
}

#[tool_router(router = tool_router)]
impl McpServer {
    /// 混合搜索工具：对知识图谱执行全文搜索，返回匹配的 Block 及其文档信息
    #[tool(
        name = "query",
        description = "混合搜索工具：对知识图谱执行全文搜索，返回匹配的 Block 及其文档信息"
    )]
    pub async fn query(
        &self,
        Parameters(params): Parameters<QueryParams>,
    ) -> Result<String, String> {
        let vm = self.vm.clone();
        let limit = params.limit.unwrap_or(10);
        let blocks = vm
            .full_text_search(&params.query, limit)
            .map_err(|e| e.to_string())?;
        let result = QueryResult {
            blocks: blocks
                .iter()
                .filter_map(|b| serde_json::to_value(b).ok())
                .collect(),
            total: blocks.len(),
        };
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    /// 360° 符号上下文视图：获取指定符号的所有引用关系，包括调用方和被调用方
    #[tool(
        name = "context",
        description = "360° 符号上下文视图：获取指定符号的所有引用关系，包括调用方和被调用方"
    )]
    pub async fn context(
        &self,
        Parameters(params): Parameters<ContextParams>,
    ) -> Result<String, String> {
        let vm = self.vm.clone();
        let refs = vm
            .trace_references(&params.symbol, None)
            .map_err(|e| e.to_string())?;
        let mut callers = Vec::new();
        let mut callees = Vec::new();
        let mut all_refs = Vec::new();
        for r in &refs {
            let val = serde_json::to_value(r).map_err(|e| e.to_string())?;
            all_refs.push(val.clone());
            match r.ref_type {
                knowledge_core::model::RefType::Definition => callees.push(val),
                knowledge_core::model::RefType::Usage => callers.push(val),
                _ => {}
            }
        }
        let result = ContextResult {
            symbol: params.symbol,
            callers,
            callees,
            references: all_refs,
        };
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    /// 多跳影响分析：从指定符号出发，BFS 遍历引用关系，按跳数分层展示受影响的符号
    #[tool(
        name = "impact",
        description = "多跳影响分析：从指定符号出发，BFS 遍历引用关系，按跳数分层展示受影响的符号"
    )]
    pub async fn impact(
        &self,
        Parameters(params): Parameters<ImpactParams>,
    ) -> Result<String, String> {
        let vm = self.vm.clone();
        let max_depth = params.depth.unwrap_or(3);
        let mut visited = std::collections::HashSet::new();
        visited.insert(params.symbol.clone());
        let mut current_level = vec![params.symbol.clone()];
        let mut layers = Vec::new();

        for hop in 1..=max_depth {
            let mut next_level = Vec::new();
            let mut affected_this_hop = Vec::new();

            for token_id in &current_level {
                let refs = vm
                    .trace_references(token_id, None)
                    .map_err(|e| e.to_string())?;
                for r in &refs {
                    let target = r.to_id.to_string();
                    if !visited.contains(&target) {
                        visited.insert(target.clone());
                        next_level.push(target.clone());
                        let val = serde_json::to_value(r).map_err(|e| e.to_string())?;
                        affected_this_hop.push(val);
                    }
                }
            }

            let count = affected_this_hop.len();
            if count == 0 {
                break;
            }
            layers.push(ImpactLayer {
                hop,
                affected: affected_this_hop,
                count,
            });
            current_level = next_level;
        }

        let total_affected: usize = layers.iter().map(|l| l.count).sum();
        let risk_level = match total_affected {
            n if n <= 3 => "low",
            n if n <= 10 => "medium",
            _ => "high",
        }
        .to_string();

        let result = ImpactResult {
            symbol: params.symbol,
            layers,
            total_affected,
            risk_level,
        };
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    /// 执行流追踪：从入口点出发，沿 Usage 类型引用追踪执行路径，返回有序步骤及置信度
    #[tool(
        name = "trace",
        description = "执行流追踪：从入口点出发，沿 Usage 类型引用追踪执行路径，返回有序步骤及置信度"
    )]
    pub async fn trace(
        &self,
        Parameters(params): Parameters<TraceParams>,
    ) -> Result<String, String> {
        let vm = self.vm.clone();
        let max_depth = params.max_depth.unwrap_or(10);
        let mut steps = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut current = params.entry_point.clone();
        visited.insert(current.clone());

        for step_num in 1..=max_depth {
            let refs = vm
                .trace_references(&current, Some(knowledge_core::model::RefType::Usage))
                .map_err(|e| e.to_string())?;

            let next = refs.into_iter().find(|r| {
                let target = r.to_id.to_string();
                !visited.contains(&target)
            });

            match next {
                Some(r) => {
                    let target = r.to_id.to_string();
                    visited.insert(target.clone());
                    let confidence = match r.ref_type {
                        knowledge_core::model::RefType::Usage => 0.9,
                        knowledge_core::model::RefType::Definition => 0.8,
                        _ => 0.5,
                    };
                    steps.push(TraceStep {
                        step: step_num,
                        symbol: target.clone(),
                        ref_type: format!("{:?}", r.ref_type),
                        confidence,
                    });
                    current = target;
                }
                None => break,
            }
        }

        let total_steps = steps.len();
        let result = TraceResult {
            entry_point: params.entry_point,
            steps,
            total_steps,
        };
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    /// 受限 `SurrealQL` 查询：仅允许 SELECT 语句，执行后返回 JSON 结果
    #[tool(
        name = "search",
        description = "受限 SurrealQL 查询：仅允许 SELECT 语句，执行后返回 JSON 结果"
    )]
    pub async fn search(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<String, String> {
        params.validate_query()?;

        let vm = self.vm.clone();
        let bindings = params
            .params
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
        let response = vm
            .execute_parameterized_query(&params.query, &bindings)
            .map_err(|e| e.to_string())?;
        let rows: Vec<serde_json::Value> = response;
        let row_count = rows.len();
        let result = SearchResult { rows, row_count };
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    /// 文档图谱可视化数据：获取指定文档的 Block 节点和 Reference 边，用于图谱渲染
    #[tool(
        name = "graph",
        description = "文档图谱可视化数据：获取指定文档的 Block 节点和 Reference 边，用于图谱渲染"
    )]
    pub async fn graph(
        &self,
        Parameters(params): Parameters<GraphParams>,
    ) -> Result<String, String> {
        let vm = self.vm.clone();
        let (blocks, references) = vm
            .get_graph_for_document(&params.doc_id)
            .map_err(|e| e.to_string())?;
        let nodes: Vec<serde_json::Value> = blocks
            .iter()
            .filter_map(|b| serde_json::to_value(b).ok())
            .collect();
        let edges: Vec<serde_json::Value> = references
            .iter()
            .filter_map(|r| serde_json::to_value(r).ok())
            .collect();
        let node_count = nodes.len();
        let edge_count = edges.len();
        let result = GraphResult {
            nodes,
            edges,
            node_count,
            edge_count,
        };
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }

    /// 变更影响检测：检查文档哈希值并分析受影响的引用关系
    #[tool(
        name = "detect_changes",
        description = "变更影响检测：检查文档哈希值并分析受影响的引用关系"
    )]
    pub async fn detect_changes(
        &self,
        Parameters(params): Parameters<DetectChangesParams>,
    ) -> Result<String, String> {
        let vm = self.vm.clone();
        let doc = vm.get_document(&params.doc_id).map_err(|e| e.to_string())?;

        let blocks = vm
            .list_blocks_by_document(&params.doc_id)
            .map_err(|e| e.to_string())?;

        let block_ids: Vec<String> = blocks
            .iter()
            .filter_map(|b| b.id.as_ref().map(std::string::ToString::to_string))
            .collect();

        let mut affected_refs_detail = Vec::new();
        for bid in &block_ids {
            let refs = vm.trace_references(bid, None).map_err(|e| e.to_string())?;
            for r in &refs {
                let val = serde_json::to_value(r).map_err(|e| e.to_string())?;
                affected_refs_detail.push(val);
            }
        }

        let affected_references = affected_refs_detail.len();
        let status = if doc.is_valid_hash() {
            "unchanged"
        } else {
            "modified"
        }
        .to_string();

        let result = DetectChangesResult {
            doc_id: params.doc_id,
            status,
            current_hash: Some(doc.hash),
            affected_references,
            affected_refs_detail,
        };
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
    }
}

#[allow(missing_docs)]
#[prompt_router(router = "prompt_router")]
impl McpServer {
    #[prompt(
        name = "detect_impact",
        description = "Pre-commit 变更影响分析：分析指定符号的变更对上游依赖、执行流和风险等级的影响"
    )]
    /// 执行变更影响分析，返回预填充的用户/助手消息对。
    ///
    /// 分析指定符号的变更对上游依赖、执行流和风险等级的影响，
    /// 生成引导 LLM 进行多跳依赖追踪的 prompt 消息。
    ///
    /// # Arguments
    ///
    /// * `params` - 影响分析参数，包含待分析的符号列表
    pub async fn detect_impact(
        &self,
        Parameters(params): Parameters<DetectImpactParams>,
    ) -> Vec<PromptMessage> {
        let symbols = params.symbols;
        vec![
            PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    "Analyze the impact of changes to the following symbols: {symbols}. \
                     Consider upstream dependencies, affected execution flows, and risk level."
                ),
            ),
            PromptMessage::new_text(
                PromptMessageRole::Assistant,
                format!(
                    "I will analyze the impact of changes to: {symbols}\n\n\
                     Please provide me with:\n\
                     1. The upstream dependencies that reference these symbols\n\
                     2. The execution flows that pass through these symbols\n\
                     3. A risk level assessment (low / medium / high)\n\n\
                     Use the `impact` tool with each symbol to trace multi-hop reference chains, \
                     and the `context` tool to get the 360° symbol context view."
                ),
            ),
        ]
    }

    #[prompt(
        name = "generate_map",
        description = "架构文档生成：从知识图谱生成架构文档，包含模块结构、关键执行流和依赖关系"
    )]
    /// 生成架构文档映射，返回预填充的用户/助手消息对。
    ///
    /// 从知识图谱生成架构文档，包含模块结构、关键执行流和依赖关系，
    /// 生成引导 LLM 构建完整架构图的 prompt 消息。
    ///
    /// # Arguments
    ///
    /// * `_params` - 架构文档生成参数（预留扩展）
    pub async fn generate_map(
        &self,
        Parameters(_params): Parameters<GenerateMapParams>,
    ) -> Vec<PromptMessage> {
        vec![
            PromptMessage::new_text(
                PromptMessageRole::User,
                "Generate architecture documentation from the knowledge graph. \
                 Include: module structure (communities), key execution flows, \
                 and dependency relationships."
                    .to_string(),
            ),
            PromptMessage::new_text(
                PromptMessageRole::Assistant,
                "I will generate architecture documentation from the knowledge graph.\n\n\
                 Steps:\n\
                 1. Use the `query` tool to search for high-level module definitions and entry points\n\
                 2. Use the `graph` tool to retrieve the graph structure for key documents\n\
                 3. Use the `trace` tool on entry points to map execution flows\n\
                 4. Use the `impact` tool to identify critical dependency hubs\n\n\
                 The output will include:\n\
                 - Module structure with community groupings\n\
                 - Key execution flows with step-by-step traces\n\
                 - Dependency relationships and risk hotspots"
                    .to_string(),
            ),
        ]
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: rmcp::model::ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_resources()
                .build(),
            server_info: rmcp::model::Implementation {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("知识图谱 MCP 服务".to_string()),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "知识图谱查询服务，支持全文搜索、符号上下文、影响分析、执行流追踪、SurrealQL 查询、图谱可视化、变更检测、资源浏览和提示模板"
                    .to_string(),
            ),
        }
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListResourcesResult, rmcp::ErrorData> {
        let resources = super::resources::build_static_resources();
        Ok(rmcp::model::ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListResourceTemplatesResult, rmcp::ErrorData> {
        let resource_templates = super::resources::build_resource_templates();
        Ok(rmcp::model::ListResourceTemplatesResult {
            resource_templates,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        request: rmcp::model::ReadResourceRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ReadResourceResult, rmcp::ErrorData> {
        let contents: Vec<rmcp::model::ResourceContents> =
            super::resources::read_resource(&request.uri, &self.vm).map_err(
                |e: error_core::ErrorObject| rmcp::ErrorData::internal_error(e.to_string(), None),
            )?;
        Ok(rmcp::model::ReadResourceResult { contents })
    }

    async fn get_prompt(
        &self,
        request: rmcp::model::GetPromptRequestParam,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::GetPromptResult, rmcp::ErrorData> {
        let prompt_context = rmcp::handler::server::prompt::PromptContext::new(
            self,
            request.name,
            request.arguments,
            context,
        );
        self.prompt_router.get_prompt(prompt_context).await
    }

    async fn list_prompts(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListPromptsResult, rmcp::ErrorData> {
        let prompts = self.prompt_router.list_all();
        Ok(rmcp::model::ListPromptsResult {
            prompts,
            next_cursor: None,
        })
    }
}

