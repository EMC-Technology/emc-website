//! 上下文精确率指标
//!
//! 详见文档: §3.4 | 用例: UC-038

use std::sync::Arc;

use async_trait::async_trait;

use crate::dataset::sample::{MetricScore, RAGMetric};
use crate::error::Result;
use crate::judge::LLMJudge;

/// 上下文精确率指标
///
/// 详见文档: §3.4 | 用例: UC-038
pub struct ContextPrecisionMetric {
    judge: Arc<dyn LLMJudge>,
}

impl ContextPrecisionMetric {
    /// 创建上下文精确率指标
    #[must_use]
    pub fn new(judge: Arc<dyn LLMJudge>) -> Self {
        Self { judge }
    }
}

#[async_trait]
impl RAGMetric for ContextPrecisionMetric {
    fn name(&self) -> &'static str {
        "context_precision"
    }

    /// 详见文档: §3.4 | 用例: UC-038 | 方法: M-053
    async fn evaluate(
        &self,
        query: &str,
        context: &[String],
        _answer: &str,
        _ground_truth: &str,
    ) -> Result<MetricScore> {
        if context.is_empty() {
            return Ok(MetricScore {
                metric_name: self.name().to_string(),
                score: 0.0,
                explanation: "无检索上下文".to_string(),
                llm_judgment: None,
            });
        }

        let mut weighted_precision_sum = 0.0;
        let mut relevant_count = 0;
        for (i, ctx) in context.iter().enumerate() {
            let k = i + 1;
            let prompt = format!(
                "请判断以下上下文片段是否与查询相关：\n\n查询：{query}\n\n上下文片段 {k}：{ctx}\n\n如果包含对回答查询有用的信息，请输出 'yes'，否则输出 'no'。",
            );
            let judgment = self.judge.judge(&prompt, "检查上下文相关性").await?;
            if judgment.content.trim().to_lowercase().starts_with("yes") {
                relevant_count += 1;
                #[allow(clippy::cast_precision_loss)]
                let precision_at_k = f64::from(relevant_count) / k as f64;
                weighted_precision_sum += precision_at_k;
            }
        }

        #[allow(clippy::cast_precision_loss)]
        let score = if relevant_count > 0 {
            weighted_precision_sum / f64::from(relevant_count)
        } else {
            0.0
        };

        Ok(MetricScore {
            metric_name: self.name().to_string(),
            score,
            explanation: format!(
                "检索到 {} 个上下文片段，其中 {} 个与查询相关",
                context.len(),
                relevant_count
            ),
            llm_judgment: None,
        })
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::judge::MockJudge;

    #[tokio::test]
    async fn test_context_precision_empty() {
        let judge = Arc::new(MockJudge::new());
        let metric = ContextPrecisionMetric::new(judge);
        let result = metric.evaluate("q", &[], "a", "gt").await.unwrap();
        assert_eq!(result.score, 0.0);
    }
}
