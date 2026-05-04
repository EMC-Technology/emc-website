#![allow(clippy::result_large_err)] // ErrorObject 含因果链+上下文帧，体积较大但语义完整
//! Knowledge Evaluator Crate
//!
//! RAG 评估框架：RAGAS 指标、LLM-as-Judge、Golden Dataset 管理、报告生成。

pub mod config;
pub mod dataset;
pub mod engine;
pub mod error;
pub mod judge;
pub mod metrics;
pub mod report;

pub use config::EvalConfig;
pub use dataset::{GoldenDataset, GoldenSample, SampleDifficulty};
pub use engine::EvaluationEngine;
pub use error::Result;
pub use judge::{JudgeResult, LLMJudge, MockJudge, UllmJudge};
pub use metrics::{
    AnswerRelevancyMetric, AnswerSimilarityMetric, ContextPrecisionMetric,
    ContextRecallMetric, FaithfulnessMetric,
};
pub use report::{MarkdownExporter, MetricAggregator};
