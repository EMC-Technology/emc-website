//! 指标聚合器
//!
//! 详见文档: §6.1 | 用例: UC-044

use std::collections::HashMap;

use crate::dataset::GoldenDataset;
use crate::dataset::sample::{AggregatedMetric, EvaluationReport, GoldenSample, SampleEvalResult};

/// 指标聚合器
///
/// 详见文档: §6.1 | 用例: UC-044
pub struct MetricAggregator;

impl MetricAggregator {
    /// 创建聚合器
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// 聚合所有指标
    ///
    /// 详见文档: §6.1 | 用例: UC-044 | 方法: M-062
    #[must_use]
    pub fn aggregate(&self, sample_results: &[SampleEvalResult]) -> Vec<AggregatedMetric> {
        let mut metric_scores: HashMap<String, Vec<f64>> = HashMap::new();
        for result in sample_results {
            for score in &result.scores {
                metric_scores
                    .entry(score.metric_name.clone())
                    .or_default()
                    .push(score.score);
            }
        }

        metric_scores
            .into_iter()
            .map(|(name, scores)| Self::compute_aggregated(&name, &scores))
            .collect()
    }

    /// 按难度分组聚合
    ///
    /// 详见文档: §6.1 | 用例: UC-044 | 方法: M-063
    #[must_use]
    /// 按难度分组评估结果
    pub fn group_by_difficulty(
        &self,
        dataset: &GoldenDataset,
        sample_results: &[SampleEvalResult],
    ) -> HashMap<String, Vec<AggregatedMetric>> {
        let mut groups: HashMap<String, Vec<SampleEvalResult>> = HashMap::new();
        for result in sample_results {
            if let Some(sample) = dataset.samples.iter().find(|s| s.id == result.sample_id) {
                groups
                    .entry(format!("{:?}", sample.difficulty))
                    .or_default()
                    .push(result.clone());
            }
        }
        groups
            .into_iter()
            .map(|(k, v)| (k, self.aggregate(&v)))
            .collect()
    }

    /// 按类别分组聚合
    ///
    /// 详见文档: §6.1 | 用例: UC-044 | 方法: M-064
    #[must_use]
    pub fn group_by_category(
        &self,
        dataset: &GoldenDataset,
        sample_results: &[SampleEvalResult],
    ) -> HashMap<String, Vec<AggregatedMetric>> {
        let mut groups: HashMap<String, Vec<SampleEvalResult>> = HashMap::new();
        for result in sample_results {
            if let Some(sample) = dataset.samples.iter().find(|s| s.id == result.sample_id) {
                groups
                    .entry(sample.category.clone())
                    .or_default()
                    .push(result.clone());
            }
        }
        groups
            .into_iter()
            .map(|(k, v)| (k, self.aggregate(&v)))
            .collect()
    }

    /// 构建完整评估报告
    #[must_use]
    pub fn build_report(
        dataset_id: &str,
        dataset_version: &str,
        samples: &[GoldenSample],
        results: &[SampleEvalResult],
    ) -> EvaluationReport {
        let successful = results.iter().filter(|r| !r.scores.is_empty()).count();
        let failed = results.len() - successful;

        let aggregator = Self::new();
        let aggregated_scores = aggregator.aggregate(results);
        let scores_by_difficulty = aggregator.group_by_difficulty(
            &GoldenDataset {
                dataset_id: dataset_id.to_string(),
                description: String::new(),
                version: dataset_version.to_string(),
                samples: samples.to_vec(),
                created_at: chrono::Utc::now(),
            },
            results,
        );
        let scores_by_category = aggregator.group_by_category(
            &GoldenDataset {
                dataset_id: dataset_id.to_string(),
                description: String::new(),
                version: dataset_version.to_string(),
                samples: samples.to_vec(),
                created_at: chrono::Utc::now(),
            },
            results,
        );

        let total_duration: u128 = results.iter().map(|r| r.duration_ms).sum();

        EvaluationReport {
            report_id: format!(
                "{:016x}",
                blake3::hash(
                    format!("{dataset_id}:{dataset_version}").as_bytes()
                )
                .as_bytes()[..8]
                .try_into()
                .map_or(0, |b: [u8; 8]| u64::from_le_bytes(b))
            ),
            dataset_id: dataset_id.to_string(),
            dataset_version: dataset_version.to_string(),
            evaluated_at: chrono::Utc::now(),
            total_duration_ms: total_duration,
            total_samples: results.len(),
            successful_samples: successful,
            failed_samples: failed,
            aggregated_scores,
            sample_results: results.to_vec(),
            scores_by_difficulty,
            scores_by_category,
        }
    }

    fn compute_aggregated(metric_name: &str, scores: &[f64]) -> AggregatedMetric {
        if scores.is_empty() {
            return AggregatedMetric {
                metric_name: metric_name.to_string(),
                mean: 0.0,
                median: 0.0,
                std_dev: 0.0,
                min: 0.0,
                max: 0.0,
                sample_count: 0,
            };
        }

        let mut sorted = scores.to_vec();
        sorted.sort_by(f64::total_cmp);

        #[allow(clippy::cast_precision_loss)]
        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        let median = if sorted.len() % 2 == 0 {
            f64::midpoint(sorted[sorted.len() / 2 - 1], sorted[sorted.len() / 2])
        } else {
            sorted[sorted.len() / 2]
        };
        #[allow(clippy::cast_precision_loss)]
        let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64;
        let std_dev = variance.sqrt();

        AggregatedMetric {
            metric_name: metric_name.to_string(),
            mean,
            median,
            std_dev,
            min: sorted[0],
            max: *sorted.last().unwrap_or(&0.0),
            sample_count: scores.len(),
        }
    }
}

impl Default for MetricAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::sample::{MetricScore, SampleDifficulty};

    fn make_sample(id: &str, difficulty: SampleDifficulty, category: &str) -> GoldenSample {
        GoldenSample {
            id: id.to_string(),
            query: "q".to_string(),
            expected_answer: "a".to_string(),
            context_ids: vec![],
            difficulty,
            category: category.to_string(),
            metadata: serde_json::Value::Null,
        }
    }

    fn make_result(id: &str, scores: Vec<(&str, f64)>) -> SampleEvalResult {
        SampleEvalResult {
            sample_id: id.to_string(),
            scores: scores
                .into_iter()
                .map(|(n, s)| MetricScore {
                    metric_name: n.to_string(),
                    score: s,
                    explanation: String::new(),
                    llm_judgment: None,
                })
                .collect(),
            duration_ms: 100,
        }
    }

    fn make_dataset(samples: Vec<GoldenSample>) -> GoldenDataset {
        GoldenDataset {
            dataset_id: "ds1".to_string(),
            description: String::new(),
            version: "1.0".to_string(),
            samples,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_aggregate_basic() {
        let results = vec![
            make_result("s1", vec![("faithfulness", 0.8), ("answer_relevancy", 0.9)]),
            make_result("s2", vec![("faithfulness", 0.6), ("answer_relevancy", 0.7)]),
        ];

        let aggregator = MetricAggregator::new();
        let aggregated = aggregator.aggregate(&results);

        assert_eq!(aggregated.len(), 2);
        let faithfulness = aggregated
            .iter()
            .find(|m| m.metric_name == "faithfulness")
            .unwrap();
        assert!((faithfulness.mean - 0.7).abs() < 1e-6);
    }

    #[test]
    fn test_aggregate_empty() {
        let aggregator = MetricAggregator::new();
        let aggregated = aggregator.aggregate(&[]);
        assert!(aggregated.is_empty());
    }

    #[test]
    fn test_group_by_difficulty() {
        let samples = vec![
            make_sample("s1", SampleDifficulty::Easy, "cat1"),
            make_sample("s2", SampleDifficulty::Hard, "cat2"),
        ];
        let results = vec![
            make_result("s1", vec![("faithfulness", 0.8)]),
            make_result("s2", vec![("faithfulness", 0.5)]),
        ];

        let dataset = make_dataset(samples);
        let aggregator = MetricAggregator::new();
        let groups = aggregator.group_by_difficulty(&dataset, &results);

        assert!(groups.contains_key("Easy"));
        assert!(groups.contains_key("Hard"));
    }

    #[test]
    fn test_group_by_category() {
        let samples = vec![
            make_sample("s1", SampleDifficulty::Easy, "programming"),
            make_sample("s2", SampleDifficulty::Easy, "science"),
        ];
        let results = vec![
            make_result("s1", vec![("faithfulness", 0.9)]),
            make_result("s2", vec![("faithfulness", 0.6)]),
        ];

        let dataset = make_dataset(samples);
        let aggregator = MetricAggregator::new();
        let groups = aggregator.group_by_category(&dataset, &results);

        assert!(groups.contains_key("programming"));
        assert!(groups.contains_key("science"));
    }

    #[test]
    fn test_build_report() {
        let samples = vec![
            make_sample("s1", SampleDifficulty::Easy, "cat1"),
            make_sample("s2", SampleDifficulty::Easy, "cat1"),
        ];
        let results = vec![
            make_result("s1", vec![("faithfulness", 0.8), ("answer_relevancy", 0.9)]),
            make_result("s2", vec![("faithfulness", 0.6), ("answer_relevancy", 0.7)]),
        ];

        let report = MetricAggregator::build_report("ds1", "1.0", &samples, &results);

        assert_eq!(report.dataset_id, "ds1");
        assert_eq!(report.total_samples, 2);
        assert_eq!(report.successful_samples, 2);
        assert_eq!(report.failed_samples, 0);
        assert_eq!(report.aggregated_scores.len(), 2);

        let faithfulness = report
            .aggregated_scores
            .iter()
            .find(|m| m.metric_name == "faithfulness")
            .unwrap();
        assert!((faithfulness.mean - 0.7).abs() < 1e-6);
    }
}
