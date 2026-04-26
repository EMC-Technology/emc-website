# 文本全结构化知识系统

> 纯 Rust 全结构化知识库引擎 — 0 随机性，0 黑盒推断

[![CI](https://github.com/EMC-Technology/emc-website/actions/workflows/ci.yml/badge.svg)](https://github.com/EMC-Technology/emc-website/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../LICENSE)
[![Rust: 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

## 项目宪章

| 要素 | 定义 |
|------|------|
| **使命** | 让全球生态感知纯 Rust 知识库解决方案的可行性，公开验证错误驱动开发和流程驱动开发两大技术哲学 |
| **愿景** | 以 Rust 统一 GIT + CI/CD + 质量门禁 + 代码库管理 + 知识库管理 + 模型精确调用的完整基础设施闭环 |
| **价值观** | 确定性 · 统一性 · 可追溯性 · 编译时保证 |
| **设计哲学** | **0 随机性，0 黑盒推断** — 相同输入在任何环境下产生相同输出，系统的每个决策都是可解释的 |

## 核心特性

- 🔍 **三路混合检索** — BM25 + Vector + Graph，加权 RRF 融合
- 🧠 **GEMMA4-E4B 纯 Rust 推理** — 42 层 Transformer，Candle 框架，2560 维语义向量
- 🏗️ **四层节点模型** — Document → Block → Token → Community，Token 级全局偏移量
- 🔐 **企业级安全** — Cedar 风格 ABAC + PII 扫描(11 种模式) + AES-256-GCM + 审计链
- 📊 **CQRS + Event Sourcing** — 天然审计追踪，事件流即合规证据
- 🔄 **error-core 统一错误处理** — 四维分类指纹 + 错误码全局注册表 + 恢复状态机 + 断路器
- 📋 **UPCM 流程驱动先行实践** — DAG 编排引擎 + YAML 工作流定义 + 统一状态机
- 🤖 **MCP Server + ReAct Agent** — 7 Tool + 2 Prompt + Resources，推理-行动循环

## 产品矩阵

本项目是公司纯 Rust 产品矩阵的核心知识引擎层：

```
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│  ullm (开源)     │  │  知识库系统 (开源) │  │ knowledge-infra  │
│  LLM API 网关    │→ │  全结构化知识引擎  │→ │  (闭源)          │
│  14提供商/61+模型 │  │  GEMMA4+CQRS+ABAC│  │  DevOps基础设施  │
└─────────────────┘  └─────────────────┘  └─────────────────┘
         ┌─────────────────────────────────────────┐
         │  error-core (开源) │ UPCM (闭源，规划中)  │
         │  统一错误处理       │ 通用流程控制         │
         └─────────────────────────────────────────┘
```

## 快速开始

### 前置条件

- Rust >= 1.85 (2024 Edition)
- SurrealDB >= 1.3 (用于持久层)

### 构建与运行

```bash
git clone https://github.com/EMC-Technology/emc-website.git
cd emc-website/knowledge-system
cargo build --workspace
cargo test --workspace
```

### 启动开发服务器

```bash
# 启动 SurrealDB
surreal start --bind 0.0.0.0:8000 --user root --pass root memory &

# 启动 API 服务器
cargo run -p knowledge-api
```

详细指南请参阅 [Getting Started](docs/getting-started.md)。

## 架构

```
┌─────────────────────────────────────────────────────┐
│  表现层    Dioxus (WASM) + 图形化绑定配置器          │
├─────────────────────────────────────────────────────┤
│  通信层    Axum + REST/WebSocket + MCP Server        │
├─────────────────────────────────────────────────────┤
│  解析推理层 DAG流水线 + 图遍历 + 向量检索 + ReAct    │
├─────────────────────────────────────────────────────┤
│  持久层    SurrealDB (图+文档+向量) → LightField     │
├─────────────────────────────────────────────────────┤
│  基础设施  error-core (统一错误) │ UPCM (流程控制)    │
└─────────────────────────────────────────────────────┘
```

## Crate 结构

| Crate | 说明 | 状态 |
|-------|------|------|
| [`error-core`](crates/error-core) | 统一错误处理核心库 — 四维分类指纹、错误码注册表、恢复状态机 | REAL |
| [`knowledge-core`](crates/knowledge-core) | 核心领域逻辑 — 检索、重排序、安全、缓存、CQRS | REAL |
| [`knowledge-parser`](crates/knowledge-parser) | 确定性解析流水线 — DAG 编排、Markdown/Code 双分支 | REAL |
| [`knowledge-api`](crates/knowledge-api) | HTTP/WebSocket/MCP API — GEMMA4 推理、Agent、ABAC | REAL |
| [`knowledge-frontend`](crates/knowledge-frontend) | Dioxus WASM 前端 — CRUD + 图谱可视化 | PARTIAL |

## 两大技术哲学

### 错误驱动开发

世界软件生态的致命问题之一是错误模块的不统一。本项目通过 `error-core` 展示统一错误处理的完整范式：

- **四维分类指纹**：ErrorSource(15种) × Severity(4级) × ImpactScope(4级) × Recoverability(4级)
- **错误码全局注册表**：`ERR-{SRC}-{MOD}-{SEQ}_{SEV}_{IMP}`，helpers 模块强制使用
- **类型状态 Builder**：9 个 PhantomData 编译期保证错误对象完整性
- **因果链+上下文帧**：双链传播，兼顾可追溯性和脱敏需求
- **恢复状态机+断路器+确定性退避**：FNV-1a 哈希生成抖动，字节级可重现

### 流程驱动开发

世界软件生态的另一致命问题是传统软件聚焦功能实现，缺乏统一的流程引擎驱动。本项目中的 DAG 编排引擎、TaskOrchestrator 工作流引擎等是 UPCM（Universal Process Control Model）的先行实践：

- **DAG 编排**：Kahn 拓扑排序 + 循环检测
- **YAML 工作流定义**：声明式流程定义语言
- **统一状态机**：WorkflowStatus 7 种显式状态
- **跨域复用**：同一引擎驱动解析/CI/Agent 流程

## 模块实现完备性

| # | 模块 | 状态 |
|---|------|------|
| 1 | Embedding Model (GEMMA4-E4B) | ✅ REAL |
| 2 | Vector Store (HNSW) | ⚠️ PARTIAL |
| 3 | BM25 Search | ✅ REAL |
| 4 | Hybrid Search (RRF) | ✅ REAL |
| 5 | Cross-Encoder Reranker | ⚠️ PARTIAL |
| 6 | Community Detection (Leiden) | ✅ REAL |
| 7 | Knowledge Graph Construction | ✅ REAL |
| 8 | CQRS Event Store | ✅ REAL |
| 9 | SurrealDB Repository | ✅ REAL |
| 10 | PII Scanner | ✅ REAL |
| 11 | Crypto Module | ✅ REAL |
| 12 | MCP Server | ✅ REAL |
| 13 | ABAC Engine | ✅ REAL |
| 14 | Frontend | ⚠️ PARTIAL |

## 许可证

- **软件代码**：[MIT OR Apache-2.0](../LICENSE) — 与 Rust 生态一致，企业友好
- **知识内容**：[CC-BY-SA 4.0](../spec/LICENSE) — 防止知识封闭化

## 贡献

我们欢迎各种形式的贡献！请参阅 [CONTRIBUTING.md](../CONTRIBUTING.md)。

每个 commit 需包含 `Signed-off-by:` 标记（[DCO](https://developercertificate.org/)），与 Linux 内核和 Rust 项目一致。

## 安全漏洞报告

请参阅 [SECURITY.md](../SECURITY.md)。**请勿在公开 Issue 中报告安全漏洞。**

## 文档

- [架构总览](docs/architecture/system-overview.md)
- [数据模型](docs/architecture/data-model.md)
- [软件工程详细设计](docs/软件工程详细设计：文本全结构化知识系统.md)
- [差距分析](docs/差距分析：本项目vs世界顶级知识库项目.md)
- [开源目的说明](docs/开源目的说明.md)
- [ADR 架构决策记录](docs/architecture/decisions/)

---

*武汉质能科技有限公司 — 全栈 Rust 自研，错误驱动 + 流程驱动，定义 AI 时代的全新操作系统底座*