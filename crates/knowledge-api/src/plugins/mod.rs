//! 插件化集成架构：闭源安全 + 即插即用
//!
//! # 核心原则
//!
//! **开源定义接口，闭源实现接口，运行时组装。**
//!
//! - 开源 RAG 定义"插座"（trait）
//! - 闭源模块制造"插头"（实现 trait）
//! - [`PluginRegistry`] 是"排插"
//! - `knowledge-runner` 是"电源"
//! - 插上就通电，拔了也能用默认功能
//!
//! # 依赖方向（关键）
//!
//! ```text
//! 开源 RAG ←── 闭源 knowledge-infra（依赖开源的 trait）
//! 开源 RAG ←── 闭源 upcm-agent（依赖开源的 trait）
//! 开源 RAG ←── 闭源 knowledge-runner（组装所有插件）
//! ```
//!
//! **绝不能反向依赖**——开源 RAG 的 `Cargo.toml` 中永远不出现闭源 crate。
//!
//! # 闭源安全三层防线
//!
//! 1. **代码隔离**：开源 RAG 只定义 trait，不包含任何闭源逻辑
//! 2. **编译隔离**：开源 RAG 编译时不需要闭源 crate；闭源 crate 依赖开源 RAG（单向依赖）
//! 3. **分发隔离**：开源 RAG 源码发布；闭源模块只以编译后的二进制 / Docker 镜像分发
//!
//! # 桥接 Trait 清单
//!
//! | Trait | 用途 | 开源默认实现 | 闭源增强实现 |
//! |-------|------|-------------|-------------|
//! | [`KnowledgeSource`] | 知识源插件 | `LocalFileSource` | `GitKnowledgeSource` |
//! | [`QualityGatePlugin`] | 质量门禁插件 | `AlwaysPassGate` | `InfraQualityGate` |
//! | [`AgentBackend`] | Agent 后端插件 | `ReActAgentBackend` | `UpcmAgentBackend` |
//! | [`StorageBackend`] | 存储后端插件 | `SurrealDbBackend` | `LightFieldBackend` |
//! | [`LLMProvider`] | LLM 统一接口 | `UllmProviderAdapter` | 闭源增强 LLM |
//! | [`TextSummarizer`] | 文本摘要器 | `LlmTextSummarizer` | 闭源增强摘要 |
//!
//! # 现有 Trait 整合（第12章）
//!
//! | 碎片化现状 | 统一方案 |
//! |-----------|---------|
//! | 4 个 LLM 抽象 | 统一为 [`LLMProvider`]（含适配器） |
//! | 2 个 `VectorStore` | Agent Memory 复用 Core 的 `VectorStore`（[`CoreVectorStoreAdapter`]） |
//! | 2 个 Summarizer | 统一为 [`TextSummarizer`]（含适配器） |

/// Agent 后端桥接 trait 及相关类型
pub mod agent_backend;
/// 开源默认插件实现
pub mod defaults;
/// 知识源桥接 trait 及相关类型
pub mod knowledge_source;
/// 统一 LLM Provider trait（整合 4 个碎片化 LLM 抽象）
pub mod llm_provider;
/// 质量门禁桥接 trait 及相关类型
pub mod quality_gate;
/// 插件注册中心
pub mod registry;
/// gRPC 远程 Agent 适配器
pub mod remote_agent;
/// 存储后端桥接 trait 及相关类型
pub mod storage_backend;
/// 统一文本摘要器 trait（整合 Agent Memory / Parser 两套）
pub mod summarizer_adapter;
/// Core `VectorStore` → Agent Memory `VectorStore` 适配器
pub mod vector_store_adapter;

pub use agent_backend::{
    AgentArtifact, AgentBackend, AgentCapabilities, AgentChunk, AgentChunkType, AgentResult,
    AgentTask,
};
pub use knowledge_source::{ChangeType, KnowledgeSource, SourceChange, SourceDocument};
pub use llm_provider::{
    LLMProvider, ProviderMessage, ProviderOptions, ProviderResponse, ProviderRole,
};
pub use quality_gate::{
    CodeChange, QualityGatePlugin, QualityVerdict, RuleInfo, Severity, Violation,
};
pub use registry::PluginRegistry;
pub use remote_agent::RemoteAgentBackend;
pub use storage_backend::{DynDatabaseClient, DynDatabaseClientWrapper, StorageBackend};
pub use summarizer_adapter::{LlmTextSummarizer, MemorySummarizerAdapter, TextSummarizer};
pub use vector_store_adapter::CoreVectorStoreAdapter;
