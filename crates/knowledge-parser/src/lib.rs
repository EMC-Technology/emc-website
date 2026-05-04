#![allow(clippy::result_large_err, clippy::manual_async_fn)] // ErrorObject 含因果链+上下文帧，体积较大但语义完整
//! 文本解析流水线（DAG 驱动）
//!
//! 本 crate 实现从原始文本到结构化知识的完整解析链路：
//! - 文件输入与哈希（`blake3` 增量哈希 + 流式读取）
//! - 源类型检测（扩展名 → `SourceType` MECE 映射）
//! - Markdown 解析（comrak）
//! - 智能分块（text-splitter + `icu_segmenter` 中文分词）
//! - 语法树构建（tree-sitter）
//! - **Code 解析路径**（DAG 分支 B：`tree-sitter` AST → Token 映射）
//! - **Markdown 解析路径**（DAG 分支 A：`comrak` AST → `TextSplitter` → `IcuTokenizer`）
//! - **符号解析**（Token 流 → Definition/Usage/Link 引用关系检测）
//! - **作用域管理**（ScopeStack：AST 级别的作用域追踪）
//! - **图构建**（GraphBuilder：组装完整实体图 + 幂等键生成）
//! - DAG 编排引擎，支持有向无环图的阶段依赖管理

/// 文件输入与 blake3 增量哈希
pub mod file_ingester;
/// 源类型检测（扩展名 → `SourceType` MECE 映射）
pub mod source_type_detector;
/// DAG 编排引擎核心
pub mod pipeline;
/// 智能文本分块器（text-splitter + icu_segmenter）
pub mod chunker;
/// DAG 有向无环图阶段依赖管理
pub mod dag;
/// Markdown 解析器（comrak）
pub mod markdown_parser;
/// Markdown 解析流水线
pub mod markdown_pipeline;
/// ICU 中文分词器
pub mod icu_tokenizer;
/// 文本分割器
pub mod text_splitter;

/// tree-sitter 语法树解析器
pub mod tree_sitter_parser;
/// tree-sitter Grammar 缓存
pub mod grammar_cache;
/// AST LRU 缓存（缓存 tree-sitter 语法树，避免重复解析）
pub mod ast_cache;
/// 代码 Token 映射器
pub mod code_token_mapper;
/// 代码解析流水线
pub mod code_pipeline;

/// 跨文件符号解析与图构建模块
///
/// Task 3.4 实现：
/// - `symbol_resolver`：Token 流 → 符号表 + 引用边
/// - `scope_stack`：AST 作用域管理栈
/// - `graph_builder`：完整实体图组装器
/// - `idempotency`：BLAKE3 幂等键生成器
/// 符号解析器：Token 流 → 符号表 + 引用边
pub mod symbol_resolver;
/// 作用域栈：AST 级别的作用域追踪
pub mod scope_stack;
/// 图构建器：组装完整实体图 + 幂等键生成
pub mod graph_builder;
/// BLAKE3 幂等键生成器
pub mod idempotency;
/// Leiden 社区检测算法
pub mod community_detector;
/// 执行流追踪引擎
pub mod execution_flow;

/// 语义感知分块器
///
/// 基于句子嵌入相似度的智能文本分块，相比固定大小分割：
/// - **语义完整性**：在主题边界处切分
/// - **上下文保留**：块间重叠保持连贯性
/// - **自适应大小**：根据内容密度动态调整
pub mod semantic_chunker;

/// 社区摘要生成模块
///
/// 为 Leiden 社区检测产出的每个社区生成自然语言摘要，支持 GraphRAG Global Search
pub mod summarizer;

/// 事件发射辅助（event-driven feature 启用时通过 EventBus 发射事件）
#[cfg(feature = "event-driven")]
pub mod event_emitter;

pub use file_ingester::{FileIngester, IngestedFile};
pub use source_type_detector::SourceTypeDetector;
pub use code_pipeline::CodePipeline;
pub use code_pipeline::ParseOutput;
pub use graph_builder::{GraphBuilder, BuiltGraph};
pub use markdown_pipeline::MarkdownPipeline;
pub use icu_tokenizer::IcuTokenizer;
pub use text_splitter::TextSplitterBlocker;
pub use community_detector::{CommunityDetector, SymbolGraph, ProcessCommunitiesStage};
pub use execution_flow::{ExecutionFlowTracer, ProcessExecutionFlowsStage};
pub use ast_cache::{LruAstCache, AstCache};
pub use summarizer::{
    CommunitySummarizer, CommunityInput, SummarizerConfig, LanguageModel,
    CommunitySummaryStore, InMemoryStore,
};

#[cfg(feature = "db")]
pub use summarizer::SurrealSummaryStore;

/// 当前 crate 统一结果类型别名
pub type Result<T> = error_core::Result<T>;

/// `str::floor_char_boundary` 的 MSRV 兼容实现
///
/// 标准库 `floor_char_boundary` 自 Rust 1.91.0 起稳定，
/// 本项目 MSRV 为 1.85.0，因此提供此 polyfill。
pub(crate) fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        s.len()
    } else {
        while !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    }
}


