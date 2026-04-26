//! 确定性解析流水线 — DAG 编排、Markdown/Code 双分支

pub mod ast_cache;
pub mod chunker;
pub mod code_pipeline;
pub mod code_token_mapper;
pub mod community_detector;
pub mod dag;
pub mod event_emitter;
pub mod execution_flow;
pub mod file_ingester;
pub mod grammar_cache;
pub mod graph_builder;
pub mod icu_tokenizer;
pub mod idempotency;
pub mod markdown_parser;
pub mod markdown_pipeline;
pub mod pipeline;
pub mod scope_stack;
pub mod semantic_chunker;
pub mod source_type_detector;
pub mod symbol_resolver;
pub mod text_splitter;
pub mod tree_sitter_parser;

/// 解析流水线库版本
pub const VERSION: &str = "0.1.0";

/// 解析流水线库名称
pub const NAME: &str = "knowledge-parser";
