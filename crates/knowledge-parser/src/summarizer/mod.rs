//! 社区摘要生成模块
//!
//! 详见文档: §6 | 用例: UC-054

pub mod config;
pub mod engine;
pub mod prompt;
pub mod store;
#[cfg(feature = "llm")]
pub mod ollama_adapter;

pub use config::SummarizerConfig;
pub use engine::{CommunityInput, CommunitySummarizer, LanguageModel};
pub use store::{CommunitySummaryStore, InMemoryStore};

#[cfg(feature = "db")]
pub use store::SurrealSummaryStore;

#[cfg(feature = "llm")]
pub use ollama_adapter::{OllamaLlm, OllamaLlmConfig};
