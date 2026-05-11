#![allow(clippy::result_large_err, clippy::manual_async_fn)]
#![warn(missing_docs)] // TODO: 升级为 #![deny(missing_docs)] — 待所有公开 API 补全文档注释
//! HTTP / WebSocket 通信层（`Axum`）
//! 本 crate 提供 `RESTful API` 与实时双向通信能力：
//! - [`Axum`] 路由与中间件（含 `WebSocket` 支持）
//! - [`SurrealDB`] 数据持久化集成
//! - 请求/响应的统一序列化层
//! - 结构化日志输出（tracing-subscriber）
//! - [`KnowledgeVM`] 核心查询引擎（MVVM 架构中的 VM 层）
//! - 向量嵌入计算管道（Embedding Service）
//! - **可插拔嵌入模型**：`HashEmbedding` / `GEMMA 4.0 4B`
//! - **通用模型加载器**：支持 `candle`/`mistral.rs`/`ONNX`/`llama.cpp` 等多后端
//! - **零信任安全架构**：`Cedar` 策略引擎 + `RBAC`/`ABAC` + `mTLS` 认证

/// AI Agent 工作流引擎（ReAct 模式）
pub mod agent;
/// Axum 应用启动与生命周期管理
pub mod application;
/// 用户角色与权限控制（基础 JWT 认证）
pub mod auth;
/// 零信任授权：Cedar 策略引擎 + ABAC 条件求值
pub mod authz;
/// 应用配置加载与管理
pub mod config;
/// AI 上下文文件生成器
pub mod context_generator;
/// 数据传输对象（DTO）
pub mod dto;
/// 向量嵌入计算服务
pub mod embedding_service;
/// 嵌入计算后台工作线程
pub mod embedding_worker;
/// 统一错误响应处理器
pub mod error_handler;
/// HTTP 请求处理器（Handler 层）
pub mod handler;
/// 知识图谱 ViewModel（MVVM 架构中的 VM 层）
pub mod knowledge_vm;
/// 请求日志中间件
pub mod logging_middleware;
/// MCP (Model Context Protocol) 服务端
pub mod mcp;
/// 中间件集合（PII 脱敏、mTLS 认证等）
pub mod middleware;
/// 可观测性基础设施（OTel Traces / Prometheus Metrics / 结构化日志）
pub mod observability;
/// 可观测性端点（/metrics, /debug/tracing）
pub mod observability_endpoints;
/// 插件化集成架构：闭源安全 + 即插即用
pub mod plugins;
/// 查询类型定义
pub mod query_types;
/// 细粒度 RBAC 角色权限管理（含继承、范围控制）
pub mod rbac;
/// 路由注册与中间件链配置
pub mod router;
/// WebSocket 实时双向通信
pub mod ws;

/// 嵌入模型工厂
pub mod embedding_factory;
/// 嵌入模型抽象层
///
/// 提供统一的嵌入模型接口，支持多种后端：
/// - [`EmbeddingModel`] trait: 统一接口定义
/// - [`HashEmbedding`]: 基于哈希的伪嵌入（测试/原型）
/// - [`GemmaEmbedding`]: GEMMA 4.0 4B 真实语义嵌入（生产推荐）
/// - [`EmbeddingModelFactory`]: 模型工厂，支持配置化创建
pub mod embedding_model;
/// GEMMA 4.0 4B 嵌入模型实现（基于通用ModelLoader）
pub mod gemma_embedding;
/// 基于哈希的伪嵌入实现
pub mod hash_embedding;

/// Candle 框架模型加载器实现
pub mod candle_loader;
/// Gemma 4 文本模型自定义实现
pub mod gemma4_model;
/// `HuggingFace` Hub 模型下载器
pub mod hf_downloader;
/// 模型加载器工厂和注册中心
pub mod model_factory;
/// 通用模型加载器架构
///
/// 提供与具体ML框架无关的统一模型加载接口：
/// - [`ModelLoader`] trait: 框架无关的加载抽象
/// - [`CandleModelLoader`]: HuggingFace candle 实现（默认推荐）
/// - [`ModelLoaderFactory`]: 工厂模式 + 策略模式
/// - [`ModelRegistry`]: 模型注册中心（缓存+复用）
///
/// # 支持的后端
///
/// | 后端 | 协议 | 纯Rust | GPU | 支持模型 |
/// |------|------|--------|-----|---------|
/// | **Candle** | Apache-2.0 | ✅ 100% | CUDA/Metal | Gemma/Llama/Qwen/Mistral (20+) |
/// | **Mistral.rs** | Apache-2.0 | ✅ 100% | CUDA/Metal | Mistral/Llama/Gemma |
/// | **ONNX Runtime** | MIT | ❌ C++ Core | CUDA/DirectML/CoreML | Universal |
/// | **llama.cpp** | MIT | ❌ C++ Core | CUDA/Metal/Vulkan | Llama/Mistral/Qwen/Gemma |
pub mod model_loader;

/// RAG (Retrieval-Augmented Generation) 引擎
///
/// 端到端的知识问答系统，整合：
/// - 语义分块（SemanticChunker）
/// - LLM 生成（支持多种后端）
/// - 流式输出（SSE Server-Sent Events）
///
/// # 路线图
///
/// 以下功能已规划但尚未完整集成到 RAG 引擎：
/// - 混合搜索（BM25 + Vector）→ 当前通过 `knowledge-core` 的独立模块提供
/// - 多阶段重排序（Cross-Encoder + LLM Judge）→ 当前通过 `knowledge-core` 的 reranker 模块提供
pub mod rag;

/// 事件订阅与处理器注册（event-driven feature 启用时可用）
#[cfg(feature = "event-driven")]
pub mod event_subscriber;

pub use context_generator::ContextGenerator;
pub use embedding_factory::EmbeddingFactory as EmbeddingModelFactory;
pub use embedding_model::{
    EmbeddingModel as EmbeddingModelTrait, EmbeddingModelInfo, EmbeddingModelType,
    EmbeddingResult as ModelEmbeddingResult,
};
pub use embedding_service::{EmbeddingModel, EmbeddingResult, EmbeddingService};
pub use knowledge_vm::{
    ImpactAnalysisResult, ImpactLayer, KnowledgeVm as KnowledgeVM, KnowledgeVmConfig, RiskLevel,
};
pub use query_types::{
    TraceReferencesRequest, TraceReferencesResponse, VectorSearchRequest, VectorSearchResponse,
    VectorSearchResultItem,
};

pub use model_factory::{ModelLoaderFactory, ModelRegistry, load_model, load_model_with_config};
pub use model_loader::{LoadedModel, ModelConfig as LoaderConfig, ModelLoader, ModelLoaderError};

pub use dto::{ApiResponse, HealthStatus, ListQueryParams, PaginationMeta, UploadDocumentRequest};
pub use handler::AppState;
pub use mcp::run_mcp_server;
pub use router::build_router;

/// 统一结果类型别名
///
/// 基于 `error_core::Result` 的项目级错误处理类型，
/// 所有公共函数的返回值应使用此类型以保持一致性。
pub type Result<T> = error_core::Result<T>;

/// API 服务版本标识
pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");
