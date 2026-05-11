//! RAG (Retrieval-Augmented Generation) 引擎模块
//!
//! 提供端到端的 RAG 查询能力，整合：
//! - **语义分块器**: 文档智能分块
//! - **混合搜索引擎**: BM25 + Vector 多路召回
//! - **重排序管道**: Cross-Encoder + LLM Judge 精排
//! - **LLM 集成**: 基于检索上下文生成回答
//! - **流式输出**: SSE (Server-Sent Events) 实时响应
//!
//! # 架构流程
//!
//! ```text
//! User Query
//!     │
//!     ▼
//! ┌─────────────┐
//! │ Query Engine │  解析、路由、缓存检查
//! └──────┬───────┘
//!        │
//!        ▼
//! ┌─────────────┐     ┌──────────────┐
//! │ HybridSearch │────▶│ Reranker     │  检索 + 重排序
//! └─────────────┘     └──────┬───────┘
//!                             │
//!                             ▼
//!                     ┌──────────────┐
//! │◀── Stream ──────│   LLM Client  │  上下文增强生成
//!                     └──────────────┘
//! ```

pub mod engine;
pub mod ollama_client;
pub mod stream;

pub use engine::{
    LLMClient, MockRetriever, RAGConfig, RAGEngine, RAGRequest, RAGResponse, RetrievalResult,
    Retriever,
};
pub use ollama_client::{OllamaConfig, OllamaLLMClient};
pub use stream::{ChunkType, RAGStreamChunk, SSEFormatter};
