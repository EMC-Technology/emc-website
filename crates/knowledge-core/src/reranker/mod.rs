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

pub use cross_encoder::{CrossEncoderModel, ScoredDocument, Document};
pub use pipeline::{RerankingPipeline, RerankedResults, PipelineStats, HybridRetriever};
