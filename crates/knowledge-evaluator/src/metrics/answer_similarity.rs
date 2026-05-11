//! 答案相似度指标
//!
//! 详见文档: §3.5 | 用例: UC-039

use async_trait::async_trait;
use knowledge_core::math::cosine_similarity;

use crate::dataset::sample::{MetricScore, RAGMetric};
use crate::error::Result;

/// 嵌入模型抽象
///
/// 用于计算答案与标准答案的向量相似度。
#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    /// 编码文本为嵌入向量
    ///
    /// # Errors
    ///
    /// 当编码失败时返回空向量
    /// 编码文本为嵌入向量
    ///
    /// # Errors
    ///
    /// 当编码失败时返回错误
    async fn encode(&self, text: &str) -> std::result::Result<Vec<f32>, String>;
}

/// 答案相似度指标
///
/// 详见文档: §3.5 | 用例: UC-039
pub struct AnswerSimilarityMetric {
    embedding_model: Option<Box<dyn EmbeddingModel>>,
    bleu_weight: f64,
    embedding_weight: f64,
}

impl AnswerSimilarityMetric {
    /// 创建答案相似度指标
    ///
    /// 详见文档: §3.5 | 用例: UC-039 | 方法: M-054
    #[must_use]
    pub fn new(embedding_model: Option<Box<dyn EmbeddingModel>>) -> Self {
        Self {
            embedding_model,
            bleu_weight: 0.4,
            embedding_weight: 0.6,
        }
    }

    /// 计算 BLEU 简化分数（基于词袋重叠率）
    fn compute_word_overlap_precision(answer: &str, ground_truth: &str) -> f64 {
        let answer_tokens: std::collections::HashSet<&str> = answer.split_whitespace().collect();
        let gt_tokens: std::collections::HashSet<&str> = ground_truth.split_whitespace().collect();
        if gt_tokens.is_empty() {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let intersection_count = answer_tokens.intersection(&gt_tokens).count() as f64;
        #[allow(clippy::cast_precision_loss)]
        let gt_len = gt_tokens.len() as f64;
        intersection_count / gt_len
    }
}

#[async_trait]
impl RAGMetric for AnswerSimilarityMetric {
    fn name(&self) -> &'static str {
        "answer_similarity"
    }

    /// 详见文档: §3.5 | 用例: UC-039 | 方法: M-055
    async fn evaluate(
        &self,
        _query: &str,
        _context: &[String],
        answer: &str,
        ground_truth: &str,
    ) -> Result<MetricScore> {
        if answer.is_empty() || ground_truth.is_empty() {
            return Ok(MetricScore {
                metric_name: self.name().to_string(),
                score: 0.0,
                explanation: "答案或标准答案为空".to_string(),
                llm_judgment: None,
            });
        }

        let bleu_score = Self::compute_word_overlap_precision(answer, ground_truth);

        let embedding_score = match &self.embedding_model {
            Some(model) => {
                let answer_emb = model.encode(answer).await;
                let gt_emb = model.encode(ground_truth).await;
                match (answer_emb, gt_emb) {
                    (Ok(a), Ok(g)) if !a.is_empty() && !g.is_empty() => cosine_similarity(&a, &g),
                    _ => 0.0,
                }
            }
            None => 0.0,
        };

        let score = if self.embedding_model.is_some() {
            self.bleu_weight * bleu_score + self.embedding_weight * embedding_score
        } else {
            bleu_score
        };

        Ok(MetricScore {
            metric_name: self.name().to_string(),
            score: score.clamp(0.0, 1.0),
            explanation: format!("BLEU: {bleu_score:.3}, 嵌入相似度: {embedding_score:.3}"),
            llm_judgment: None,
        })
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_bleu_identical() {
        let score =
            AnswerSimilarityMetric::compute_word_overlap_precision("hello world", "hello world");
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_compute_bleu_no_overlap() {
        let score = AnswerSimilarityMetric::compute_word_overlap_precision("aaa bbb", "ccc ddd");
        assert!((score - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_compute_bleu_partial() {
        let score =
            AnswerSimilarityMetric::compute_word_overlap_precision("hello world", "hello there");
        assert!(score > 0.0 && score < 1.0);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_answer_similarity_empty() {
        let metric = AnswerSimilarityMetric::new(None);
        let result = metric.evaluate("q", &[], "", "gt").await.unwrap();
        assert_eq!(result.score, 0.0);
    }

    #[tokio::test]
    async fn test_answer_similarity_with_bleu() {
        let metric = AnswerSimilarityMetric::new(None);
        let result = metric
            .evaluate("q", &[], "hello world", "hello world")
            .await
            .unwrap();
        assert!((result.score - 1.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_answer_similarity_whitespace_only_ground_truth() {
        let metric = AnswerSimilarityMetric::new(None);
        let result = metric.evaluate("q", &[], "hello", " ").await.unwrap();
        assert_eq!(result.score, 0.0);
    }
}
