# ADR-007: 实现 MCP 协议集成

## 状态
✅ **Accepted** (2026-04-13)

## Context (背景)

文本全结构化知识系统的核心使用场景之一是作为 AI 编程助手（如 Cursor、Windsurf、Claude Code 等 IDE 插件）的知识后端。这些工具正在快速采用 **MCP (Model Context Protocol)** 作为标准化的 AI 工具协议。

MCP 由 Anthropic 提出，已成为 AI Agent 与外部工具/数据源交互的事实标准：
- 定义了 Tool（工具）、Resource（资源）、Prompt（提示词模板）三种交互原语
- 支持流式传输（stdio/SSE）、认证授权、速率限制等企业级特性
- 主流 AI 编程助手均已支持或计划支持 MCP Client

如果不实现 MCP 协议集成，系统将面临：
- 每种 AI 工具都需要自定义集成适配器
- 无法享受 MCP 生态的工具发现和自动配置能力
- 在 AI 编程助手生态中被边缘化

## Decision (决定)

基于 [rmcp](https://crates.io/crates/rmcp) crate 实现 **完整的 MCP Server**，暴露系统的核心能力为标准化的 MCP 工具。

### MCP 架构设计

```
┌──────────────────────────────────────────────────────┐
│                  AI 编程助手 (MCP Client)              │
│  ┌─────────┐  ┌─────────┐  ┌─────────────────────┐  │
│  │ Cursor  │  │ Claude  │  │ Custom IDE Plugin   │  │
│  │ Code    │  │ Code    │  │ (MCP Client SDK)    │  │
│  └────┬────┘  └────┬────┘  └──────────┬──────────┘  │
└───────┼────────────┼─────────────────┼───────────────┘
        │            │                 │
        └────────────┼─────────────────┘
                     │ MCP Protocol (JSON-RPC over stdio/SSE)
                     ▼
┌──────────────────────────────────────────────────────┐
│              knowledge-api (MCP Server)               │
│                                                       │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────┐  │
│  │ MCP Tools   │  │ MCP Resources│  │ MCP Prompts│  │
│  │ (操作能力)   │  │ (数据访问)   │  │ (模板管理) │  │
│  ├─────────────┤  ├──────────────┤  ├────────────┤  │
│  │ search_      │  │ document://  │  │ code_review│  │
│  │ knowledge   │  │ block://     │  │ explain_   │  │
│  │ get_document│  │ reference:// │  │ change     │  │
│  │ ingest_file │  │ graph://     │  │ impact_    │  │
│  │ list_refs   │  │ community:// │  │ analysis   │  │
│  │ get_graph_  │  │ process://   │  │            │  │
│  │ stats       │  │             │  │            │  │
│  └─────────────┘  └──────────────┘  └────────────┘  │
│                                                       │
│  ┌─────────────┐  ┌──────────────┐                    │
│  │ Registry    │  │ Rate Limiter │                    │
│  │ (工具注册)   │  │ (流量控制)   │                    │
│  └─────────────┘  └──────────────┘                    │
└──────────────────────────────────────────────────────┘
```

### 核心 MCP 工具定义

#### Tools（操作类）

| 工具名 | 功能 | 输入 Schema | 返回值 |
|--------|------|-------------|--------|
| `search_knowledge` | 语义搜索知识库 | `{query, limit, filters}` | `SearchResult[]` |
| `get_document` | 获取文档详情 | `{document_id}` | `Document` |
| `ingest_file` | 摄入新文件到知识库 | `{path, source_type}` | `IngestResult` |
| `list_references` | 列出实体的引用关系 | `{entity_id, direction}` | `Reference[]` |
| `get_graph_stats` | 获取图谱统计信息 | `{}` | `GraphStats` |
| `analyze_impact` | 分析代码变更影响 | `{changed_files}` | `ImpactAnalysis` |

#### Resources（数据访问类）

| URI Scheme | 说明 | 示例 |
|-----------|------|------|
| `document://{id}` | 访问文档实体 | `document://doc_abc123` |
| `block://{id}` | 访问块内容 | `block://blk_def456` |
| `reference://{id}` | 访用引用边 | `reference://ref_789` |
| `graph://{query}` | 图查询结果 | `graph://?type=Usage&from=token_x` |

#### Prompts（提示词模板）

| Prompt 名 | 用途 | 参数 |
|-----------|------|------|
| `code_review_prompt` | 代码审查辅助 | `{file_path, diff_content}` |
| `explain_change_prompt` | 变更解释生成 | `{changed_symbols}` |
| `impact_analysis_prompt` | 影响分析报告 | `{impact_result}` |

### 技术实现要点

```rust
// 基于 rmcp crate 的 MCP Server 实现
use rmcp::{Server, service::ServiceExt};

#[derive(Debug, Clone, Server)]
#[server(name = "knowledge-system")]
struct KnowledgeMcpServer {
    state: Arc<AppState>,
}

impl KnowledgeMcpServer {
    /// 注册所有 MCP 工具
    async fn register_tools(&self, registry: &mut ToolRegistry) {
        registry.register(search_knowledge_tool());
        registry.register(get_document_tool());
        registry.register(ingest_file_tool());
        // ...
    }
}
```

## Consequences (影响)

### 正面影响

- 🟢 **标准化接入**：任何支持 MCP 的 AI 工具都可以零配置连接
- 🟢 **生态红利**：自动获得 MCP Client 的工具发现、权限管理等能力
- 🟢 **未来-proof**：随着 MCP 生态扩展，系统价值自然增长
- 🟢 **关注点分离**：MCP 层与内部 API 层解耦

### 负面影响

- 🔴 **协议依赖**：需要跟踪 MCP 协议规范的演进
- 🔴 **额外抽象层**：MCP 工具包装增加了间接调用开销
- 🔴 **安全考量**：MCP Server 暴露了更多攻击面（需严格的认证和限流）

### 安全保障

- JWT 认证集成 MCP Session 管理
- 每个工具独立的速率限制器 (`RateLimiter`)
- PII 数据在 MCP 响应中自动脱敏
- 操作审计日志记录所有 MCP 调用

## Alternatives (替代方案)

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| **纯 REST API** | 无额外依赖 | 每种客户端需自定义适配 | ❌ 集成成本高 |
| **OpenAI Function Calling** | OpenAI 生态原生 | 绑定特定厂商 | ❎ 厂商锁定 |
| **LangChain Tools** | Python 生态丰富 | 非 Rust 原生 | ❌ 技术栈不一致 |
| **自研协议** | 完全控制 | 无生态支持 | ❌ 重复造轮子 |
| **MCP Protocol** ✅ | 行业标准、广泛支持 | 需要跟踪规范演进 | ✅ **选定方案** |

## References

- [MCP 规范文档](https://modelcontextprotocol.io/)
- [rmcp - Rust MCP SDK](https://docs.rs/rmcp/latest/rmcp/)
- [Anthropic MCP Blog Post](https://www.anthropic.com/engineering/model-context-protocol)
