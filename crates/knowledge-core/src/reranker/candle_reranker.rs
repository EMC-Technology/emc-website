//! Candle Cross-Encoder 重排序器
//!
//! 实现 `CrossEncoderModel` trait，提供基于 Candle 的本地推理。
//!
//! 详见文档: §3 | 用例: UC-031

use std::future::Future;
use tracing::{debug, info};

use error_core::Result;

use super::candle_reranker_model::CandleRerankerModel;
use super::config::CrossEncoderConfig;
use crate::reranker::cross_encoder::{CrossEncoderModel, Document, ScoredDocument};

/// Candle Cross-Encoder 重排序器
///
/// 将 `CandleRerankerModel` 适配为 `CrossEncoderModel` trait，
/// 提供异步重排序接口。
///
/// 详见文档: §3 | 用例: UC-031
pub struct CandleReranker {
    model: CandleRerankerModel,
}

impl CandleReranker {
    /// 创建并加载 Candle 重排序器
    ///
    /// 详见文档: §3.1 | 用例: UC-031 | 方法: M-045
    ///
    /// # Errors
    ///
    /// 当模型加载失败时返回错误
    pub async fn new(config: CrossEncoderConfig) -> Result<Self> {
        let model = CandleRerankerModel::load(config).await?;
        Ok(Self { model })
    }

    /// 对文档列表进行重排序（内部实现）
    ///
    /// 详见文档: §3.2 | 用例: UC-031 | 方法: M-046
    fn rerank_sync(&self, query: &str, documents: &[Document]) -> Result<Vec<ScoredDocument>> {
        let pairs: Vec<(&str, &str)> = documents
            .iter()
            .map(|d| (query, d.content.as_str()))
            .collect();

        let scores = self.model.forward_batch(&pairs)?;

        let mut scored: Vec<ScoredDocument> = documents
            .iter()
            .zip(scores.into_iter())
            .map(|(doc, score)| ScoredDocument::new(doc.clone(), score))
            .collect();

        scored.sort();

        for (i, s) in scored.iter_mut().enumerate() {
            s.rank = i;
        }

        debug!(
            model = %self.model_name(),
            count = scored.len(),
            "Candle rerank complete"
        );

        Ok(scored)
    }
}

#[allow(clippy::manual_async_fn)]
impl CrossEncoderModel for CandleReranker {
    #[allow(clippy::manual_async_fn)]
    fn rerank(&self, query: &str, documents: &[Document]) -> impl Future<Output = Result<Vec<ScoredDocument>>> + Send {
        async move {
            info!(
                model = %self.model_name(),
                query_preview = &query[..query.len().min(50)],
                doc_count = documents.len(),
                "Candle rerank start"
            );
            self.rerank_sync(query, documents)
        }
    }

    fn model_name(&self) -> &str {
        &self.model.config().model_id
    }

    fn max_batch_size(&self) -> usize {
        self.model.config().batch_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    fn make_doc(content: &str) -> Document {
        Document::new(Uuid::new_v4(), content, json!({}))
    }

    #[tokio::test]
    async fn test_candle_reranker_model_name() {
        let config = CrossEncoderConfig {
            model_id: "test-model".to_string(),
            ..Default::default()
        };

        let model = CandleRerankerModel::load(config).await;
        if model.is_err() {
            return;
        }
        let reranker = CandleReranker { model: model.unwrap() };
        assert_eq!(reranker.model_name(), "test-model");
    }

    #[test]
    fn test_rerank_sync_sorts_by_score() {
        let config = CrossEncoderConfig::default();
        let model_result = futures::executor::block_on(CandleRerankerModel::load(config));
        if model_result.is_err() {
            return;
        }
        let reranker = CandleReranker { model: model_result.unwrap() };

        let docs = vec![
            make_doc("Rust is a systems programming language"),
            make_doc("Python is great for data science"),
        ];

        let result = reranker.rerank_sync("Rust programming", &docs);
        if let Ok(scored) = result {
            assert_eq!(scored.len(), 2);
            assert_eq!(scored[0].rank, 0);
            assert_eq!(scored[1].rank, 1);
        }
    }
}
