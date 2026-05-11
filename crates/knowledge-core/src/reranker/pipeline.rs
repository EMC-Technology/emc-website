//! 多阶段重排序管道模块
//!
//! 提供完整的重排序管道实现，支持三阶段重排序流程：
//! 1. 检索阶段：从混合检索器获取候选文档
//! 2. 精排阶段：使用交叉编码器对候选文档精确评分
//! 3. 判决阶段（可选）：使用 LLM 对结果进行最终判决

use crate::reranker::cross_encoder::{CrossEncoderModel, Document, ScoredDocument};
use error_core::Result;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::Arc;
use tracing::{debug, info};

/// 混合检索器 trait
///
/// 抽象检索阶段，支持 BM25、向量检索或混合检索等不同实现。
pub trait HybridRetriever: Send + Sync {
    /// 检索候选文档
    fn retrieve(
        &self,
        query: &str,
        top_k: usize,
    ) -> impl Future<Output = Result<Vec<Document>>> + Send;
}

/// LLM 判决器 trait
///
/// 使用大语言模型对重排序结果进行最终判决，
/// 过滤低相关性文档并调整排序。
#[async_trait::async_trait]
pub trait LLMJudger: Send + Sync {
    /// 对重排序结果进行 LLM 判决
    ///
    /// # Errors
    /// LLM 推理失败时返回错误
    async fn judge_relevance(
        &self,
        query: &str,
        documents: &[ScoredDocument],
    ) -> Result<Vec<ScoredDocument>>;
}

/// 管道执行统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStats {
    /// Stage 1: 候选文档数量
    pub stage1_candidates: usize,
    /// Stage 2: 重排序后文档数量
    pub stage2_scored: usize,
    /// Stage 3: 最终输出文档数量
    pub stage3_final: usize,
    /// 总耗时（毫秒）
    pub total_duration_ms: u128,
}

/// 重排序结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankedResults {
    /// 排序后的文档列表
    pub results: Vec<ScoredDocument>,
    /// 管道执行统计
    pub pipeline_stats: PipelineStats,
}

impl RerankedResults {
    /// 获取前 k 个结果
    #[must_use]
    pub fn top_k(&self, k: usize) -> &[ScoredDocument] {
        &self.results[..k.min(self.results.len())]
    }

    /// 获取最佳结果
    #[must_use]
    pub fn best(&self) -> Option<&ScoredDocument> {
        self.results.first()
    }

    /// 按相关性阈值过滤结果
    #[must_use]
    pub fn filter_by_threshold(mut self, threshold: f64) -> Self {
        self.results.retain(|d| d.relevance_score >= threshold);
        self.pipeline_stats.stage3_final = self.results.len();
        self
    }
}

/// 重排序管道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankerConfig {
    /// Stage 1 初始候选数量
    pub top_k_initial: usize,
    /// Stage 2 重排序后保留数量
    pub top_k_reranked: usize,
    /// Stage 3 最终输出数量
    pub top_k_final: usize,
    /// 是否启用 LLM 判决阶段
    pub enable_llm_judge: bool,
}

impl Default for RerankerConfig {
    fn default() -> Self {
        Self {
            top_k_initial: 50,
            top_k_reranked: 20,
            top_k_final: 10,
            enable_llm_judge: false,
        }
    }
}

/// 多阶段重排序管道
///
/// 执行三阶段重排序流程：
/// 1. **检索阶段**：从混合检索器获取候选文档
/// 2. **精排阶段**：使用交叉编码器对候选文档精确评分
/// 3. **判决阶段**（可选）：使用 LLM 对结果进行最终判决
pub struct RerankingPipeline<R: HybridRetriever, C: CrossEncoderModel> {
    retriever: Arc<R>,
    cross_encoder: Arc<C>,
    llm_judge: Option<Arc<dyn LLMJudger>>,
    config: RerankerConfig,
}

impl<R: HybridRetriever, C: CrossEncoderModel> RerankingPipeline<R, C> {
    /// 创建重排序管道
    pub fn new(retriever: R, cross_encoder: C) -> Self {
        Self {
            retriever: Arc::new(retriever),
            cross_encoder: Arc::new(cross_encoder),
            llm_judge: None,
            config: RerankerConfig::default(),
        }
    }

    /// 使用自定义配置
    #[must_use]
    pub const fn with_config(mut self, config: RerankerConfig) -> Self {
        self.config = config;
        self
    }

    /// 启用 LLM 判决阶段
    #[must_use]
    pub fn with_llm_judge(mut self, judge: Arc<dyn LLMJudger>) -> Self {
        self.llm_judge = Some(judge);
        self.config.enable_llm_judge = true;
        self
    }

    /// 执行完整重排序流程
    ///
    /// # Errors
    ///
    /// 检索、重排序或判决任一阶段失败时返回错误
    pub async fn rerank(&self, query: &str) -> Result<RerankedResults> {
        let start = std::time::Instant::now();
        info!(
            query_preview = &query[..query.len().min(50)],
            "pipeline start"
        );

        let candidates = self
            .retriever
            .retrieve(query, self.config.top_k_initial)
            .await?;
        debug!(s1 = candidates.len());

        let mut scored = self.cross_encoder.rerank(query, &candidates).await?;
        scored.truncate(self.config.top_k_reranked);
        debug!(s2 = scored.len());

        let final_results = if self.config.enable_llm_judge {
            if let Some(ref j) = self.llm_judge {
                let judged = j.judge_relevance(query, &scored).await?;
                let mut v = judged;
                v.truncate(self.config.top_k_final);
                v
            } else {
                scored[..self.config.top_k_final.min(scored.len())].to_vec()
            }
        } else {
            scored[..self.config.top_k_final.min(scored.len())].to_vec()
        };

        let final_count = final_results.len();
        Ok(RerankedResults {
            results: final_results,
            pipeline_stats: PipelineStats {
                stage1_candidates: candidates.len(),
                stage2_scored: scored.len(),
                stage3_final: final_count,
                total_duration_ms: start.elapsed().as_millis(),
            },
        })
    }
}

/// Mock 检索器，用于测试
pub struct MockRetriever {
    docs: Vec<Document>,
}
impl MockRetriever {
    /// 创建新的 Mock 检索器
    pub const fn new(docs: Vec<Document>) -> Self {
        Self { docs }
    }
}

#[allow(clippy::manual_async_fn)]
impl HybridRetriever for MockRetriever {
    #[allow(clippy::manual_async_fn)]
    fn retrieve(
        &self,
        _query: &str,
        top_k: usize,
    ) -> impl Future<Output = Result<Vec<Document>>> + Send {
        async move { Ok(self.docs.iter().take(top_k).cloned().collect()) }
    }
}

/// Mock LLM 判定器，用于测试
#[derive(Default)]
pub struct MockLLMJudger {}

#[async_trait::async_trait]
impl LLMJudger for MockLLMJudger {
    async fn judge_relevance(
        &self,
        _query: &str,
        docs: &[ScoredDocument],
    ) -> Result<Vec<ScoredDocument>> {
        let mut j = docs.to_vec();
        for d in &mut j {
            d.relevance_score = (d.relevance_score * 0.95).min(1.0);
        }
        j.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.document.id.cmp(&b.document.id))
        });
        for (i, item) in j.iter_mut().enumerate() {
            item.rank = i;
        }
        Ok(j)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reranker::cross_encoder::MockCrossEncoder;
    use serde_json::json;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_pipeline_basic() {
        let retriever = MockRetriever::new(vec![
            Document::new(Uuid::new_v4(), "Rust is systems language", json!({})),
            Document::new(Uuid::new_v4(), "Python is interpreted", json!({})),
        ]);
        let cross_encoder: MockCrossEncoder = MockCrossEncoder::new();
        let pipe = RerankingPipeline::new(retriever, cross_encoder).with_config(RerankerConfig {
            top_k_initial: 5,
            top_k_reranked: 2,
            ..Default::default()
        });
        let res = pipe.rerank("Rust").await.unwrap();
        assert!(!res.results.is_empty());
    }
}
