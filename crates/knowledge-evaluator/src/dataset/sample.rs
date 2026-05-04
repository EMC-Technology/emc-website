//! 通用数据类型
//!
//! 详见文档: §5.1 | 用例: UC-042

use serde::{Deserialize, Serialize};

/// 样本难度分级
///
/// 详见文档: §5.1 | 用例: UC-042
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleDifficulty {
    /// 简单
    Easy,
    /// 中等
    Medium,
    /// 困难
    Hard,
    /// 专家级
    Expert,
}

/// Golden Dataset 评估样本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenSample {
    /// 样本唯一标识
    pub id: String,
    /// 查询文本
    pub query: String,
    /// 期望答案
    pub expected_answer: String,
    /// 上下文 ID 列表
    pub context_ids: Vec<String>,
    /// 难度分级
    pub difficulty: SampleDifficulty,
    /// 类别标签
    pub category: String,
    /// 附加元数据
    pub metadata: serde_json::Value,
}

/// 单个指标评分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricScore {
    /// 指标名称
    pub metric_name: String,
    /// 分数 [0.0, 1.0]
    pub score: f64,
    /// 评分解释
    pub explanation: String,
    /// LLM 判决原文
    pub llm_judgment: Option<String>,
}

/// 单个样本的评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleEvalResult {
    /// 样本 ID
    pub sample_id: String,
    /// 各指标评分
    pub scores: Vec<MetricScore>,
    /// 评估耗时（毫秒）
    pub duration_ms: u128,
}

/// 聚合指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetric {
    /// 指标名称
    pub metric_name: String,
    /// 均值
    pub mean: f64,
    /// 中位数
    pub median: f64,
    /// 标准差
    pub std_dev: f64,
    /// 最小值
    pub min: f64,
    /// 最大值
    pub max: f64,
    /// 样本数
    pub sample_count: usize,
}

/// 完整评估报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationReport {
    /// 报告唯一标识
    pub report_id: String,
    /// 数据集 ID
    pub dataset_id: String,
    /// 数据集版本
    pub dataset_version: String,
    /// 评估时间
    pub evaluated_at: chrono::DateTime<chrono::Utc>,
    /// 总耗时（毫秒）
    pub total_duration_ms: u128,
    /// 总样本数
    pub total_samples: usize,
    /// 成功样本数
    pub successful_samples: usize,
    /// 失败样本数
    pub failed_samples: usize,
    /// 聚合评分
    pub aggregated_scores: Vec<AggregatedMetric>,
    /// 各样本评估结果
    pub sample_results: Vec<SampleEvalResult>,
    /// 按难度分组的评分
    pub scores_by_difficulty: std::collections::HashMap<String, Vec<AggregatedMetric>>,
    /// 按类别分组的评分
    pub scores_by_category: std::collections::HashMap<String, Vec<AggregatedMetric>>,
}

/// RAG 评估指标 trait
///
/// 详见文档: §3 | 用例: UC-035~UC-039
#[async_trait::async_trait]
pub trait RAGMetric: Send + Sync {
    /// 获取指标名称
    fn name(&self) -> &str;
    /// 评估单个样本
    ///
    /// # Errors
    ///
    /// 当指标计算失败时返回错误
    async fn evaluate(
        &self,
        query: &str,
        context: &[String],
        answer: &str,
        ground_truth: &str,
    ) -> crate::error::Result<MetricScore>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_difficulty_serde() {
        let d = SampleDifficulty::Hard;
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, "\"hard\"");
        let parsed: SampleDifficulty = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, SampleDifficulty::Hard);
    }

    #[test]
    fn test_metric_score_serialization() {
        let score = MetricScore {
            metric_name: "faithfulness".to_string(),
            score: 0.85,
            explanation: "test".to_string(),
            llm_judgment: None,
        };
        let json = serde_json::to_string(&score).unwrap();
        assert!(json.contains("faithfulness"));
    }
}
