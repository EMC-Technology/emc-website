//! 重排序管道模块
//!
//! 提供多阶段重排序管道，将候选文档通过交叉编码器、
//! LLM 判定等阶段进行精细化排序，提升检索精度。

/// 交叉编码器模型模块
///
/// 提供交叉编码器的核心 trait 定义和基础实现
pub mod cross_encoder;

/// 多阶段重排序管道模块
///
/// 提供完整的重排序管道实现，支持三阶段重排序流程
pub mod pipeline;

/// Cross-Encoder 推理配置
///
/// 提供 Candle Cross-Encoder 的配置管理
#[cfg(feature = "reranker-local")]
pub mod config;

/// Candle Cross-Encoder 模型加载与前向推理
///
/// 基于 Candle 框架的本地 Cross-Encoder 推理实现
#[cfg(feature = "reranker-local")]
pub mod candle_reranker_model;

/// Candle Cross-Encoder 重排序器
///
/// 实现 `CrossEncoderModel` trait 的 Candle 本地推理适配器
#[cfg(feature = "reranker-local")]
pub mod candle_reranker;

/// LLM 判决器实现
///
/// 基于大语言模型的重排序结果判决
pub mod llm_judger;

pub use cross_encoder::{CrossEncoderModel, Document, ScoredDocument};
pub use llm_judger::{LlmJudgerImpl, LlmLanguageModel};
pub use pipeline::LLMJudger;
pub use pipeline::{HybridRetriever, PipelineStats, RerankedResults, RerankingPipeline};

#[cfg(feature = "reranker-local")]
pub use config::CrossEncoderConfig;

#[cfg(feature = "reranker-local")]
pub use candle_reranker_model::CandleRerankerModel;

#[cfg(feature = "reranker-local")]
pub use candle_reranker::CandleReranker;
