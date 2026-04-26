//! 交叉编码器模型模块
//!
//! 提供交叉编码器的核心 trait 定义和基础实现，
//! 包括文档结构、评分文档和 Mock 实现。

use error_core::Result;
use serde::{Deserialize, Serialize};
use tracing::debug;
use uuid::Uuid;

/// 待重排序的文档
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// 文档唯一标识符
    pub id: Uuid,
    /// 文档文本内容
    pub content: String,
    /// 附加元数据
    pub metadata: serde_json::Value,
}

impl Document {
    /// 创建新文档
    #[must_use]
    pub fn new(id: Uuid, content: impl Into<String>, metadata: serde_json::Value) -> Self {
        Self {
            id,
            content: content.into(),
            metadata,
        }
    }

    /// 获取文档内容预览（截断到指定长度）
    #[must_use]
    pub fn preview(&self, max_len: usize) -> &str {
        if self.content.len() <= max_len {
            &self.content
        } else {
            &self.content[..max_len]
        }
    }
}

/// 带相关性分数的文档（重排序结果）
///
/// `Ord` 实现保证确定性排序：先按分数降序，分数相同时按 ID 字典序升序。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredDocument {
    /// 原始文档
    pub document: Document,
    /// 相关性分数（越高越相关）
    pub relevance_score: f64,
    /// 排名位置
    pub rank: usize,
}

impl ScoredDocument {
    /// 创建新的评分文档（初始排名为 0）
    #[must_use]
    pub const fn new(document: Document, relevance_score: f64) -> Self {
        Self {
            document,
            relevance_score,
            rank: 0,
        }
    }
}

impl PartialEq for ScoredDocument {
    fn eq(&self, other: &Self) -> bool {
        self.document.id == other.document.id
            && (self.relevance_score - other.relevance_score).abs() < 1e-10
    }
}
impl Eq for ScoredDocument {}
impl PartialOrd for ScoredDocument {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ScoredDocument {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .relevance_score
            .total_cmp(&self.relevance_score)
            .then_with(|| self.document.id.cmp(&other.document.id))
    }
}

/// 交叉编码器模型 trait
///
/// 交叉编码器将查询和文档联合编码，输出精确的相关性分数。
/// 与双编码器（Bi-Encoder）相比，交叉编码器精度更高但速度更慢，
/// 适用于小规模候选集的精排阶段。
#[async_trait::async_trait]
pub trait CrossEncoderModel: Send + Sync {
    /// 对文档列表进行重排序
    ///
    /// # Errors
    /// 模型推理失败时返回错误
    async fn rerank(&self, query: &str, documents: &[Document]) -> Result<Vec<ScoredDocument>>;

    /// 批量重排序
    ///
    /// # Errors
    /// 模型推理失败时返回错误
    async fn rerank_batch(
        &self,
        queries: &[&str],
        documents_list: &[Vec<Document>],
    ) -> Result<Vec<Vec<ScoredDocument>>> {
        let mut results = Vec::with_capacity(queries.len());
        for (q, docs) in queries.iter().zip(documents_list.iter()) {
            results.push(self.rerank(q, docs).await?);
        }
        Ok(results)
    }
    /// 获取模型名称
    fn model_name(&self) -> &str;

    /// 获取最大批量大小
    #[must_use]
    fn max_batch_size(&self) -> usize {
        32
    }
}

/// Mock 交叉编码器（用于测试）
///
/// 使用词袋模型（Bag of Words）计算查询与文档的 Jaccard 相似度，
/// 仅用于单元测试和集成测试，不可用于生产环境。
pub struct MockCrossEncoder {
    name: String,
}

impl MockCrossEncoder {
    /// 创建默认名称的 Mock 交叉编码器
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: "mock-cross-encoder".to_string(),
        }
    }

    /// 创建指定名称的 Mock 交叉编码器
    #[must_use]
    pub fn with_name(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    #[allow(clippy::unused_self, clippy::cast_precision_loss)]
    fn compute_similarity(&self, query: &str, doc: &str) -> f64 {
        let qt: std::collections::HashSet<&str> = query.split_whitespace().collect();
        let dt: std::collections::HashSet<&str> = doc.split_whitespace().collect();
        if qt.is_empty() || dt.is_empty() {
            return 0.0;
        }
        let inter = qt.intersection(&dt).count() as f64;
        let union = (qt.union(&dt).count() as f64).max(1.0);
        (inter / union)
            .mul_add(
                0.7,
                qt.iter()
                    .filter(|t| doc.to_lowercase().contains(&t.to_lowercase()))
                    .count() as f64
                    / qt.len().max(1) as f64
                    * 0.3,
            )
            .min(1.0)
    }
}

impl Default for MockCrossEncoder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl CrossEncoderModel for MockCrossEncoder {
    async fn rerank(&self, query: &str, documents: &[Document]) -> Result<Vec<ScoredDocument>> {
        debug!(model = %self.name, count = documents.len(), "mock rerank");
        let mut scored: Vec<ScoredDocument> = documents
            .iter()
            .map(|d| ScoredDocument::new(d.clone(), self.compute_similarity(query, &d.content)))
            .collect();
        scored.sort();
        for (i, s) in scored.iter_mut().enumerate() {
            s.rank = i;
        }
        Ok(scored)
    }
    fn model_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_mock_reranking() {
        let enc = MockCrossEncoder::new();
        let docs = vec![
            Document::new(
                Uuid::new_v4(),
                "Rust is a systems programming language",
                json!({}),
            ),
            Document::new(
                Uuid::new_v4(),
                "Python is great for data science",
                json!({}),
            ),
        ];
        let scored = enc.rerank("Rust programming", &docs).await.unwrap();
        assert_eq!(
            scored[0].document.content,
            "Rust is a systems programming language"
        );
    }
}
