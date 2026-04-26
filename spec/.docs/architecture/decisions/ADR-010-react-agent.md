# ADR-010: ReAct Agent 框架

## 状态
✅ **Accepted** (2026-04-13)

## Context (背景)

文本全结构化知识系统不仅是一个被动的知识检索引擎，还需要具备主动的**智能推理和任务执行能力**。典型的高级使用场景包括：

1. **自动化代码审查**：分析 PR 变更的影响范围，找出潜在的风险引用
2. **复杂问答**：回答需要多步推理的问题（"这个函数的变更会影响哪些下游服务？"）
3. **知识图谱维护**：自动检测过时的引用关系并提出修复建议
4. **工作流编排**：将文档解析→索引构建→嵌入计算→质量检查串联成自动化流程

单纯的 RAG 检索 + LLM 生成无法胜任这类需要多步推理和工具调用的复杂任务。需要一个 **Agent 框架** 来编排推理循环。

## Decision (决定)

采用 **ReAct (Reasoning + Acting)** 模式实现 AI Agent 引擎，结合 MCP 工具集进行多步推理和任务执行。

### ReAct Agent 架构

```
┌──────────────────────────────────────────────────────────────┐
│                    ReAct Agent 循环                            │
│                                                               │
│   ┌─────────┐                                                │
│   │  用户    │                                                │
│   │  任务    │                                                │
│   └────┬────┘                                                │
│        ▼                                                      │
│   ┌─────────────────────────────────────────────┐            │
│   │              Thought (推理)                   │            │
│   │  "我需要先找到函数 X 的定义，然后查找它的      │            │
│   │   所有调用者..."                              │            │
│   └──────────────────┬──────────────────────────┘            │
│                      ▼                                        │
│   ┌─────────────────────────────────────────────┐            │
│   │              Action (行动)                   │            │
│   │  tool_call: search_knowledge(query="fn X")  │            │
│   └──────────────────┬──────────────────────────┘            │
│                      ▼                                        │
│   ┌─────────────────────────────────────────────┐            │
│   │              Observation (观察)               │            │
│   │  { results: [...], confidence: 0.95 }        │            │
│   └──────────────────┬──────────────────────────┘            │
│                      │                                        │
│          ┌───────────┴───────────┐                           │
│          ▼                       ▼                           │
│   ┌─────────────┐         ┌─────────────┐                    │
│   │  需要继续?   │         │  任务完成?   │                    │
│   │  → 继续 Loop │         │ → 返回结果   │                    │
│   └─────────────┘         └─────────────┘                    │
│                                                               │
│   安全限制: max_steps=15, timeout=30s                         │
└──────────────────────────────────────────────────────────────┘
```

### 核心组件设计

位于 [`agent/mod.rs`](../../crates/knowledge-api/src/agent/mod.rs)：

#### 1. ReAct Agent 引擎

```rust
/// ReAct Agent — 推理-行动循环引擎
pub struct ReActAgent {
    llm_client: LlmClient,          // LLM 推理引擎
    tools: Arc<dyn ToolBelt>,       // 可用工具集
    memory: AgentMemory,            // 工作记忆
    executor: TaskExecutor,         // 任务执行器
    config: AgentConfig,            // 配置参数
}

pub struct AgentConfig {
    pub max_reasoning_steps: u32,   // 最大推理步数 (默认: 15)
    pub max_execution_time: Duration,// 单次执行超时 (默认: 30s)
    pub temperature: f64,           // LLM 温度参数 (默认: 0.1)
    pub verbose: bool,              // 是否输出详细推理过程
}

impl ReActAgent {
    /// 执行 ReAct 推理循环
    pub async fn run(&self, task: AgentTask) -> Result<AgentResult> {
        let mut memory = self.memory.new_session();
        let mut step_count = 0;

        loop {
            step_count += 1;
            if step_count > self.config.max_reasoning_steps {
                return Err(AgentError::MaxStepsExceeded);
            }

            // 1. Thought: LLM 推理下一步行动
            let thought = self.think(&task, &memory).await?;

            // 2. Action: 执行工具调用
            let observation = match thought.action {
                Action::Finish(answer) => return Ok(AgentResult::done(answer)),
                Action::ToolCall(call) => {
                    self.execute_tool(call, &memory).await?
                }
            };

            // 3. Observation: 记录观察结果
            memory.observe(thought, observation);
        }
    }
}
```

#### 2. Tool Belt（工具带）

```rust
/// Agent 可用工具 trait
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;  // JSON Schema
    async fn execute(&self, input: Value) -> Result<Value>;
}

/// 工具带 — 管理所有可用工具
pub struct ToolBelt {
    tools: DashMap<String, Box<dyn Tool>>,
}

impl ToolBelt {
    /// 注册 MCP 工具为 Agent 工具
    pub fn register_mcp_tools(&self, mcp_server: &McpServer) {
        for tool in mcp_server.list_tools() {
            self.register(McpToolAdapter::new(tool));
        }
    }
}
```

**内置工具列表**：

| 工具名 | 功能 | 对应 MCP 工具 |
|--------|------|---------------|
| `search_knowledge` | 语义搜索知识库 | `search_knowledge` |
| `get_document` | 获取文档详情 | `get_document` |
| `get_references` | 获取引用关系 | `list_references` |
| `get_graph_stats` | 图谱统计 | `get_graph_stats` |
| `analyze_impact` | 影响分析 | `analyze_impact` |
| `rag_query` | RAG 问答 | (内部工具) |

#### 3. Agent Memory（工作记忆）

```rust
/// Agent 工作记忆 — 维护推理过程中的上下文
pub struct AgentMemory {
    thoughts: VecDeque<ThoughtRecord>,   // 推理历史
    observations: VecDeque<Observation>,  // 观察历史
    context_window: usize,                // 上下文窗口大小
}

pub struct ThoughtRecord {
    pub step: u32,
    pub reasoning: String,    // 推理过程文本
    pub action: Action,       // 决定的行动
}
```

#### 4. Task Executor（任务执行器）

```rust
/// 任务执行器 — 负责 Agent 行动的实际执行
pub struct TaskExecutor {
    http_client: reqwest::Client,  // HTTP 调用
    db_pool: DbPool,               // 数据库操作
    timeout: Duration,
}
```

### 安全约束

| 约束 | 默认值 | 说明 |
|------|--------|------|
| **最大步数** | 15 | 防止无限循环 |
| **超时时间** | 30s | 单次任务最大执行时间 |
| **工具白名单** | 仅注册工具 | 防止任意命令执行 |
| **输出截断** | 8KB | 防止 prompt 注入 |
| **速率限制** | 10 tasks/min | 防 Agent 滥用 |

## Consequences (影响)

### 正面影响

- 🟢 **复杂推理能力**：支持多步推理任务，超越简单 QA
- 🟢 **工具可扩展**：新增工具只需实现 `Tool` trait
- 🟢 **可解释性**：完整的 Thought→Action→Observation 链可追溯
- 🟢 **与 MCP 对接**：MCP 工具可直接注册为 Agent 工具

### 负面影响

- 🔴 **LLM 依赖**：每次推理步骤都需要 LLM 调用，成本较高
- 🔴 **不确定性**：LLM 推理结果不确定，可能导致循环或偏离
- 🔴 **延迟累积**：多步推理的延迟是单步的 N 倍
- 🔴 **调试困难**：Agent 行为由 LLM 决定，非确定性难以重现

### 缓解措施

- 严格的最大步数和超时限制
- 结构化的 Thought 格式强制 LLM 输出可解析的行动指令
- Verbose 模式记录完整推理链用于事后审计
- 幂等的工具调用允许安全重试

### UPCM 对齐说明（V4.0 补充）

本项目中的 `TaskOrchestrator`（DAG 任务编排器）和 `WorkflowDefinition`（YAML 工作流定义）是公司自研 UPCM（Universal Process Control Model）流程引擎的先行实践。ReAct Agent 的推理-行动循环可视为 UPCM SubProcess 的一种特化实现。未来 UPCM 统一引擎发布后，ReAct 循环将作为 UPCM 的一个节点类型（Decision Node + Tool Node 循环）被统一编排，实现从 Agent 工作流到 CI/CD 流水线到 OS 调度的跨域统一。

## Alternatives (替代方案)

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| **Plan-and-Execute** | 全局规划更优 | 过度规划问题、规划成本高 | ❌ 开放域场景不适用 |
| **Function Calling Only** | 简单直接 | 无推理循环、单步限制 | ❌ 能力不足 |
| **LangChain Agent** | Python 生态丰富 | 非 Rust 原生、性能较差 | ❌ 技术栈不一致 |
| **ReAct (自研)** ✅ | 完全控制、深度集成 | 需要自行实现 | ✅ **选定方案** |

## References

- [ReAct: Synergizing Reasoning and Acting in Language Models](https://arxiv.org/abs/2210.03629)
- [AI Agent Patterns](https://lilianweng.github.io/posts/2023-06-23-agent/)
- [MCP Tool Use Guidelines](https://modelcontextprotocol.io/docs/concepts/tools)
