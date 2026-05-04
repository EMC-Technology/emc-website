//! 答案相关性指标
//!
//! 详见文档: §3.2 | 用例: UC-036

use std::sync::Arc;

use async_trait::async_trait;

use crate::dataset::sample::{MetricScore, RAGMetric};
use crate::error::Result;
use crate::judge::LLMJudge;

/// 答案相关性指标
///
/// 详见文档: §3.2 | 用例: UC-036
pub struct AnswerRelevancyMetric {
    judge: Arc<dyn LLMJudge>,
}

impl AnswerRelevancyMetric {
    /// 创建答案相关性指标
    #[must_use]
    pub fn new(judge: Arc<dyn LLMJudge>) -> Self {
        Self { judge }
    }
}

#[async_trait]
impl RAGMetric for AnswerRelevancyMetric {
    fn name(&self) -> &'static str {
        "answer_relevancy"
    }

    /// 详见文档: §3.2 | 用例: UC-036 | 方法: M-051
    async fn evaluate(
        &self,
        query: &str,
        _context: &[String],
        answer: &str,
        _ground_truth: &str,
    ) -> Result<MetricScore> {
        if answer.is_empty() || query.is_empty() {
            return Ok(MetricScore {
                metric_name: self.name().to_string(),
                score: 0.0,
                explanation: "查询或答案为空".to_string(),
                llm_judgment: None,
            });
        }

        let prompt = format!(
            "请评估以下答案与查询的相关性：\n\n查询：{query}\n\n答案：{answer}\n\n\
             请从 1 到 5 分评分：\n1 = 完全不相关\n2 = 仅提及关键词\n3 = 部分回答\n4 = 基本准确\n5 = 完美回答\n\n\
             仅输出一个数字（1-5）。"
        );

        let judgment = self.judge.judge(&prompt, "评估答案相关性").await?;
        let raw_score: f64 = judgment
            .content
            .trim()
            .parse()
            .unwrap_or(1.0_f64)
            .clamp(1.0, 5.0);
        let score = (raw_score - 1.0) / 4.0;

        Ok(MetricScore {
            metric_name: self.name().to_string(),
            score,
            explanation: format!("LLM 相关性评分：{raw_score}/5"),
            llm_judgment: Some(judgment.content),
        })
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::judge::MockJudge;

    #[tokio::test]
    async fn test_answer_relevancy_empty() {
        let judge = Arc::new(MockJudge::new());
        let metric = AnswerRelevancyMetric::new(judge);
        let result = metric.evaluate("", &[], "", "gt").await.unwrap();
        assert_eq!(result.score, 0.0);
    }

    #[tokio::test]
    async fn test_answer_relevancy_with_mock() {
        let judge = Arc::new(MockJudge::new());
        let metric = AnswerRelevancyMetric::new(judge);
        let result = metric
            .evaluate("query", &[], "answer", "gt")
            .await
            .unwrap();
        assert_eq!(result.metric_name, "answer_relevancy");
    }
}
