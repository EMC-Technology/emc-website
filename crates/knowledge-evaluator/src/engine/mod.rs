//! 评估引擎模块
//!
//! 详见文档: §7 | 用例: UC-043

use std::sync::Arc;
use std::time::Instant;

use crate::config::EvalConfig;
use crate::dataset::GoldenDataset;
use crate::dataset::sample::{EvaluationReport, RAGMetric, SampleEvalResult};
use crate::error::Result;
use crate::report::MetricAggregator;

/// RAG 评估引擎
///
/// 详见文档: §7.1 | 用例: UC-043
pub struct EvaluationEngine {
    metrics: Vec<Arc<dyn RAGMetric>>,
    config: EvalConfig,
    aggregator: MetricAggregator,
}

impl EvaluationEngine {
    /// 创建评估引擎
    #[must_use]
    pub fn new(metrics: Vec<Arc<dyn RAGMetric>>, config: EvalConfig) -> Self {
        Self {
            metrics,
            config,
            aggregator: MetricAggregator::new(),
        }
    }

    /// 对 Golden Dataset 执行完整评估
    ///
    /// 详见文档: §7.1 | 用例: UC-043 | 方法: M-061
    ///
    /// # Errors
    ///
    /// 当评估过程中发生错误时返回
    pub async fn evaluate(
        &self,
        dataset: &GoldenDataset,
        rag_fn: &(dyn Fn(&str) -> (Vec<String>, String) + Send + Sync),
    ) -> Result<EvaluationReport> {
        let start = Instant::now();

        let mut results = Vec::with_capacity(dataset.samples.len());
        let mut failed_samples = 0usize;

        for chunk in dataset.samples.chunks(self.config.max_concurrent_evals) {
            let futures = chunk.iter().map(|sample| {
                let metrics = self.metrics.clone();
                let (context, answer) = rag_fn(&sample.query);
                let query = sample.query.clone();
                let expected = sample.expected_answer.clone();
                let sample_id = sample.id.clone();

                async move {
                    let sample_start = Instant::now();
                    let mut scores = Vec::with_capacity(metrics.len());
                    for metric in &metrics {
                        let score = metric.evaluate(&query, &context, &answer, &expected).await?;
                        scores.push(score);
                    }
                    #[allow(clippy::cast_possible_truncation)] // as_millis() 返回 u128，duration_ms 为 u128，实际不存在截断
                    let result = SampleEvalResult {
                        sample_id,
                        scores,
                        duration_ms: sample_start.elapsed().as_millis(),
                    };
                    Ok::<_, error_core::ErrorObject>(result)
                }
            });

            let chunk_results: Vec<_> = futures::future::join_all(futures).await;
            for r in chunk_results {
                match r {
                    Ok(sample_result) => results.push(sample_result),
                    Err(e) => {
                        tracing::warn!(error = %e, "样本评估失败");
                        failed_samples += 1;
                    }
                }
            }
        }

        let total_duration = start.elapsed().as_millis();

        let aggregated = self.aggregator.aggregate(&results);
        let by_difficulty = self.aggregator.group_by_difficulty(dataset, &results);
        let by_category = self.aggregator.group_by_category(dataset, &results);

        Ok(EvaluationReport {
            report_id: format!(
                "{:016x}",
                blake3::hash(
                    format!("{}:{}:{}", dataset.dataset_id, dataset.version, start.elapsed().as_millis()).as_bytes()
                )
                .as_bytes()[..8]
                .try_into()
                .map_or(0, |b: [u8; 8]| u64::from_le_bytes(b))
            ),
            dataset_id: dataset.dataset_id.clone(),
            dataset_version: dataset.version.clone(),
            evaluated_at: chrono::Utc::now(),
            total_duration_ms: total_duration,
            total_samples: dataset.samples.len(),
            successful_samples: results.len(),
            failed_samples,
            aggregated_scores: aggregated,
            sample_results: results,
            scores_by_difficulty: by_difficulty,
            scores_by_category: by_category,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::sample::{GoldenSample, SampleDifficulty};
    use crate::judge::MockJudge;
    use crate::metrics::FaithfulnessMetric;

    fn make_dataset() -> GoldenDataset {
        GoldenDataset {
            dataset_id: "test".to_string(),
            description: String::new(),
            version: "1.0".to_string(),
            samples: vec![GoldenSample {
                id: "s1".to_string(),
                query: "What is Rust?".to_string(),
                expected_answer: "A systems programming language".to_string(),
                context_ids: vec!["c1".to_string()],
                difficulty: SampleDifficulty::Easy,
                category: "programming".to_string(),
                metadata: serde_json::Value::Null,
            }],
            created_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_evaluate() {
        let judge = Arc::new(MockJudge::new());
        let metrics: Vec<Arc<dyn RAGMetric>> = vec![Arc::new(FaithfulnessMetric::new(judge))];
        let engine = EvaluationEngine::new(metrics, EvalConfig::default());

        let dataset = make_dataset();
        let rag_fn = |query: &str| (vec![format!("Context for: {query}")], format!("Answer for: {query}"));

        let report = engine.evaluate(&dataset, &rag_fn).await.unwrap();
        assert_eq!(report.total_samples, 1);
        assert_eq!(report.successful_samples, 1);
    }
}
