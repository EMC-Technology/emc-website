#![allow(clippy::result_large_err, clippy::manual_async_fn)]
//! Knowledge Extractor Crate
//!
//! 语义抽取引擎：基于 LLM 的实体/关系自动抽取、消歧、去重、持久化管道。
//!
//! 详见文档: §9 | 用例: UC-026

pub mod config;
pub mod deduplication;
pub mod disambiguation;
pub mod error;
pub mod extractors;
pub mod pipeline;

pub use config::{DisambiguationStrategy, ExtractorConfig};
pub use deduplication::Deduplicator;
pub use disambiguation::Disambiguator;
pub use error::Result;
pub use extractors::{ExtractedRelation, LanguageModel, LlmExtractor};
pub use pipeline::RecoveryAction;

#[cfg(feature = "db")]
pub use pipeline::ExtractionPipeline;
