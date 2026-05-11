//! 忠实度指标
//!
//! 详见文档: §3.1 | 用例: UC-035

use std::sync::Arc;

use async_trait::async_trait;

use crate::dataset::sample::{MetricScore, RAGMetric};
use crate::error::Result;
use crate::judge::LLMJudge;

/// 忠实度指标
///
/// 详见文档: §3.1 | 用例: UC-035
pub struct FaithfulnessMetric {
    judge: Arc<dyn LLMJudge>,
}

impl FaithfulnessMetric {
    /// 创建忠实度指标
    #[must_use]
    pub fn new(judge: Arc<dyn LLMJudge>) -> Self {
        Self { judge }
    }
}

#[async_trait]
impl RAGMetric for FaithfulnessMetric {
    fn name(&self) -> &'static str {
        "faithfulness"
    }

    /// 详见文档: §3.1 | 用例: UC-035 | 方法: M-050
    async fn evaluate(
        &self,
        _query: &str,
        context: &[String],
        answer: &str,
        _ground_truth: &str,
    ) -> Result<MetricScore> {
        if answer.is_empty() || context.is_empty() {
            return Ok(MetricScore {
                metric_name: self.name().to_string(),
                score: 0.0,
                explanation: "答案或上下文为空".to_string(),
                llm_judgment: None,
            });
        }

        let claims_prompt = format!("请从以下答案中提取所有事实声明（每个声明一行）：\n\n{answer}");
        let claims_response = self.judge.judge(&claims_prompt, "提取事实声明").await?;
        let claims: Vec<String> = claims_response
            .content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        if claims.is_empty() {
            return Ok(MetricScore {
                metric_name: self.name().to_string(),
                score: 1.0,
                explanation: "无事实声明".to_string(),
                llm_judgment: None,
            });
        }

        let context_text = context.join("\n");
        let mut supported_count = 0;

        for claim in &claims {
            let prompt = format!(
                "请判断以下声明是否被上下文支持：\n\n上下文：{context_text}\n\n声明：{claim}\n\n如果声明被上下文明确支持或暗示，请输出 'yes'，否则输出 'no'。"
            );
            let verification = self.judge.judge(&prompt, "验证声明").await?;
            if verification
                .content
                .trim()
                .to_lowercase()
                .starts_with("yes")
            {
                supported_count += 1;
            }
        }

        #[allow(clippy::cast_precision_loss)]
        let score = f64::from(supported_count) / claims.len() as f64;

        Ok(MetricScore {
            metric_name: self.name().to_string(),
            score,
            explanation: format!(
                "答案包含 {} 个事实声明，其中 {} 个被上下文支持",
                claims.len(),
                supported_count
            ),
            llm_judgment: Some(claims_response.content),
        })
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::judge::MockJudge;

    #[tokio::test]
    async fn test_faithfulness_empty_input() {
        let judge = Arc::new(MockJudge::new());
        let metric = FaithfulnessMetric::new(judge);
        let result = metric.evaluate("q", &[], "", "gt").await.unwrap();
        assert_eq!(result.score, 0.0);
    }

    #[tokio::test]
    async fn test_faithfulness_with_mock() {
        let judge = Arc::new(MockJudge::new());
        let metric = FaithfulnessMetric::new(judge);
        let result = metric
            .evaluate("q", &["ctx".to_string()], "answer", "gt")
            .await
            .unwrap();
        assert_eq!(result.metric_name, "faithfulness");
    }
}
