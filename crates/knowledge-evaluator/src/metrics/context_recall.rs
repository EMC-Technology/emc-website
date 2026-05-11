//! 上下文召回率指标
//!
//! 详见文档: §3.3 | 用例: UC-037

use std::sync::Arc;

use async_trait::async_trait;

use crate::dataset::sample::{MetricScore, RAGMetric};
use crate::error::Result;
use crate::judge::LLMJudge;

/// 上下文召回率指标
///
/// 详见文档: §3.3 | 用例: UC-037
pub struct ContextRecallMetric {
    judge: Arc<dyn LLMJudge>,
}

impl ContextRecallMetric {
    /// 创建上下文召回率指标
    #[must_use]
    pub fn new(judge: Arc<dyn LLMJudge>) -> Self {
        Self { judge }
    }
}

#[async_trait]
impl RAGMetric for ContextRecallMetric {
    fn name(&self) -> &'static str {
        "context_recall"
    }

    /// 详见文档: §3.3 | 用例: UC-037 | 方法: M-052
    async fn evaluate(
        &self,
        query: &str,
        context: &[String],
        _answer: &str,
        ground_truth: &str,
    ) -> Result<MetricScore> {
        if ground_truth.is_empty() {
            return Ok(MetricScore {
                metric_name: self.name().to_string(),
                score: 0.0,
                explanation: "标准答案为空".to_string(),
                llm_judgment: None,
            });
        }

        let info_prompt = format!(
            "请从以下标准答案中提取回答查询所需的关键信息点（每点一行）：\n\n查询：{query}\n\n标准答案：{ground_truth}\n\n仅列出信息点。"
        );
        let info_response = self.judge.judge(&info_prompt, "提取关键信息点").await?;
        let info_points: Vec<String> = info_response
            .content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        if info_points.is_empty() {
            return Ok(MetricScore {
                metric_name: self.name().to_string(),
                score: 1.0,
                explanation: "无关键信息点".to_string(),
                llm_judgment: None,
            });
        }

        let context_text = context.join("\n");
        let mut covered_count = 0;

        for point in &info_points {
            let prompt = format!(
                "请判断以下信息点是否在上下文中被提及：\n\n上下文：{context_text}\n\n信息点：{point}\n\n如果被提及，请输出 'yes'，否则输出 'no'。"
            );
            let check = self.judge.judge(&prompt, "检查信息覆盖").await?;
            if check.content.trim().to_lowercase().starts_with("yes") {
                covered_count += 1;
            }
        }

        #[allow(clippy::cast_precision_loss)]
        let score = f64::from(covered_count) / info_points.len() as f64;

        Ok(MetricScore {
            metric_name: self.name().to_string(),
            score,
            explanation: format!(
                "标准答案包含 {} 个关键信息点，其中 {} 个被覆盖",
                info_points.len(),
                covered_count
            ),
            llm_judgment: Some(info_response.content),
        })
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::judge::MockJudge;
    use async_trait::async_trait;

    #[tokio::test]
    async fn test_context_recall_empty_ground_truth() {
        let judge = Arc::new(MockJudge::new());
        let metric = ContextRecallMetric::new(judge);
        let result = metric.evaluate("q", &[], "a", "").await.unwrap();
        assert_eq!(result.score, 0.0);
    }

    #[tokio::test]
    async fn test_context_recall_empty_info_points() {
        struct EmptyResponseJudge;

        #[async_trait]
        impl crate::judge::LLMJudge for EmptyResponseJudge {
            async fn judge(
                &self,
                _prompt: &str,
                _description: &str,
            ) -> crate::error::Result<crate::judge::JudgeResult> {
                Ok(crate::judge::JudgeResult {
                    content: "   \n  \n  ".to_string(),
                    model: "empty-mock".to_string(),
                    latency_ms: 0,
                })
            }
        }

        let judge = Arc::new(EmptyResponseJudge);
        let metric = ContextRecallMetric::new(judge);
        let result = metric
            .evaluate("query", &["ctx".to_string()], "answer", "ground truth")
            .await
            .unwrap();
        assert_eq!(result.score, 1.0);
        assert_eq!(result.explanation, "无关键信息点");
    }

    #[tokio::test]
    async fn test_context_recall_with_mock_judge() {
        let judge = Arc::new(MockJudge::new());
        let metric = ContextRecallMetric::new(judge);
        let result = metric
            .evaluate("query", &["ctx text".to_string()], "answer", "ground truth")
            .await
            .unwrap();
        assert_eq!(result.metric_name, "context_recall");
        assert!(result.score > 0.0);
        assert!(result.llm_judgment.is_some());
    }
}
