//! LLM 判决器实现
//!
//! 详见文档: §4 | 用例: UC-032

use async_trait::async_trait;
use std::sync::Arc;
use tracing::debug;

use error_core::Result;

use crate::reranker::cross_encoder::ScoredDocument;
use crate::reranker::pipeline::LLMJudger;

/// LLM 判决器语言模型抽象
///
/// 封装 LLM 文本生成能力，支持依赖注入和测试替身。
#[async_trait]
pub trait LlmLanguageModel: Send + Sync {
    /// 根据提示词生成文本
    ///
    /// # Errors
    ///
    /// 当 LLM 调用失败时返回错误消息
    async fn generate(&self, prompt: &str) -> Result<String>;
}

/// 基于 LLM 的相关性判决器
///
/// 使用大语言模型对重排序结果进行最终判决，
/// 通过 prompt 让 LLM 评估每个文档与查询的相关性。
///
/// 详见文档: §4 | 用例: UC-032
pub struct LlmJudgerImpl {
    llm: Arc<dyn LlmLanguageModel>,
    model_name: String,
}

impl LlmJudgerImpl {
    /// 创建 LLM 判决器
    ///
    /// 详见文档: §4.1 | 用例: UC-032 | 方法: M-047
    pub fn new(llm: Arc<dyn LlmLanguageModel>, model_name: impl Into<String>) -> Self {
        Self {
            llm,
            model_name: model_name.into(),
        }
    }

    /// 构建 LLM 判决提示词
    ///
    /// 详见文档: §4.2 | 用例: UC-032 | 方法: M-048
    fn build_judge_prompt(query: &str, documents: &[ScoredDocument]) -> String {
        let doc_list: String = documents
            .iter()
            .enumerate()
            .map(|(i, d)| {
                format!(
                    "{}. [score={:.4}] {}",
                    i + 1,
                    d.relevance_score,
                    d.document.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Given the query: "{query}"

Rank the following documents by relevance to the query. Output a JSON array of objects with fields "index" (1-based) and "relevance" (0.0-1.0):

{doc_list}

Output format: [{{"index": 1, "relevance": 0.95}}, ...]"#
        )
    }
}

#[async_trait]
impl LLMJudger for LlmJudgerImpl {
    async fn judge_relevance(
        &self,
        query: &str,
        documents: &[ScoredDocument],
    ) -> Result<Vec<ScoredDocument>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        debug!(
            model = %self.model_name,
            count = documents.len(),
            "LLM judge start"
        );

        let prompt = Self::build_judge_prompt(query, documents);
        let response = self.llm.generate(&prompt).await?;

        let judged_scores = Self::parse_judge_response(&response, documents.len());

        let mut judged: Vec<ScoredDocument> = documents
            .iter()
            .zip(judged_scores)
            .map(|(doc, score)| {
                let mut s = ScoredDocument::new(doc.document.clone(), score);
                s.rank = 0;
                s
            })
            .collect();

        judged.sort();
        for (i, s) in judged.iter_mut().enumerate() {
            s.rank = i;
        }

        Ok(judged)
    }
}

impl LlmJudgerImpl {
    /// 解析 LLM 判决响应
    ///
    /// 从 LLM 返回的 JSON 数组中提取每个文档的相关性分数。
    /// 若解析失败，回退到原始分数乘以衰减因子 0.95。
    fn parse_judge_response(response: &str, doc_count: usize) -> Vec<f64> {
        let json_str = extract_json_array(response);

        #[derive(serde::Deserialize)]
        struct JudgeEntry {
            #[allow(dead_code)]
            index: usize,
            relevance: f64,
        }

        match serde_json::from_str::<Vec<JudgeEntry>>(json_str) {
            Ok(entries) if entries.len() == doc_count => entries
                .into_iter()
                .map(|e| e.relevance.clamp(0.0, 1.0))
                .collect(),
            _ => {
                debug!("LLM judge response parse failed, falling back to decay scores");
                (0..doc_count)
                    .map(|i| {
                        #[allow(clippy::cast_precision_loss)]
                        let decay = i as f64;
                        (1.0 - decay * 0.05).max(0.0)
                    })
                    .collect()
            }
        }
    }
}

/// 从 LLM 响应中提取 JSON 数组部分
fn extract_json_array(response: &str) -> &str {
    let start = response.find('[').unwrap_or(0);
    let end = response
        .rfind(']')
        .map_or_else(|| response.len(), |i| i + 1);
    &response[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLlm {
        response: String,
    }

    #[async_trait]
    impl LlmLanguageModel for MockLlm {
        async fn generate(&self, _prompt: &str) -> Result<String> {
            Ok(self.response.clone())
        }
    }

    fn make_scored_doc(content: &str, score: f64) -> ScoredDocument {
        use serde_json::json;
        use uuid::Uuid;
        ScoredDocument::new(
            crate::reranker::cross_encoder::Document::new(Uuid::new_v4(), content, json!({})),
            score,
        )
    }

    #[test]
    fn test_build_judge_prompt_contains_query() {
        let docs = vec![make_scored_doc("test doc", 0.9)];

        let prompt = LlmJudgerImpl::build_judge_prompt("test query", &docs);
        assert!(prompt.contains("test query"));
        assert!(prompt.contains("test doc"));
    }

    #[test]
    fn test_parse_judge_response_valid_json() {
        let response = r#"[{"index": 1, "relevance": 0.9}, {"index": 2, "relevance": 0.5}]"#;
        let scores = LlmJudgerImpl::parse_judge_response(response, 2);
        assert_eq!(scores.len(), 2);
        assert!((scores[0] - 0.9).abs() < 1e-6);
        assert!((scores[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_parse_judge_response_invalid_json_fallback() {
        let scores = LlmJudgerImpl::parse_judge_response("not json", 3);
        assert_eq!(scores.len(), 3);
        assert!((scores[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_parse_judge_response_clamps_scores() {
        let response = r#"[{"index": 1, "relevance": 1.5}, {"index": 2, "relevance": -0.3}]"#;
        let scores = LlmJudgerImpl::parse_judge_response(response, 2);
        assert!((scores[0] - 1.0).abs() < 1e-6);
        assert!((scores[1] - 0.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_judge_relevance_empty_docs() {
        let llm = Arc::new(MockLlm {
            response: "[]".to_string(),
        });
        let judger = LlmJudgerImpl::new(llm, "test-llm");

        let result = judger.judge_relevance("query", &[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_judge_relevance_with_valid_response() {
        let llm = Arc::new(MockLlm {
            response: r#"[{"index": 1, "relevance": 0.8}, {"index": 2, "relevance": 0.3}]"#
                .to_string(),
        });
        let judger = LlmJudgerImpl::new(llm, "test-llm");

        let docs = vec![make_scored_doc("doc a", 0.9), make_scored_doc("doc b", 0.7)];

        let result = judger.judge_relevance("query", &docs).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].rank, 0);
        assert!(result[0].relevance_score > result[1].relevance_score);
    }

    #[test]
    fn test_extract_json_array() {
        assert_eq!(extract_json_array("[1,2,3]"), "[1,2,3]");
        assert_eq!(extract_json_array("```json\n[1,2,3]\n```"), "[1,2,3]");
    }
}
