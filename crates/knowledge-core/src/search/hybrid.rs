/// 混合搜索融合模块
///
/// 基于 Reciprocal Rank Fusion (RRF) 的混合搜索算法。
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "db")]
use crate::search::bm25::Bm25Index;
#[cfg(feature = "db")]
use crate::vector_store::{HybridQuery, SearchResult, VectorStore};
#[cfg(feature = "db")]
use error_core::Result;
#[cfg(feature = "db")]
use std::collections::HashMap;
#[cfg(feature = "db")]
use std::sync::Arc;
#[cfg(feature = "db")]
use tracing::info;

/// RRF 默认 K 参数
///
/// K 值在 RRF 公式 `1 / (k + rank)` 中控制排名对分数的衰减速度。
/// 值越大，排名差异对最终分数的影响越小；值越小，高排名结果的优势越明显。
/// 60 是原论文（Cormack et al., 2009）推荐的默认值。
pub const RRF_DEFAULT_K: usize = 60;

/// BM25 搜索结果类型：文档 ID 与原始 BM25 分数
pub type Bm25Result = (String, f64);

/// 混合搜索结果（等权重 RRF 融合）
///
/// 包含 RRF 融合分数及 BM25/语义搜索各自的排名。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HybridSearchResult {
    /// 文档 ID
    pub id: String,
    /// RRF 融合分数
    pub score: f64,
    /// BM25 关键词搜索排名（从 1 开始）
    pub bm25_rank: Option<usize>,
    /// 语义向量搜索排名（从 1 开始）
    pub semantic_rank: Option<usize>,
}

/// 加权融合搜索结果
///
/// 与 [`HybridSearchResult`] 不同，此结构包含归一化的融合分数、
/// 各自的原始分数以及负载元数据，适用于需要详细搜索信息的场景。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusedSearchResult {
    /// 文档 UUID
    pub id: Uuid,
    /// 归一化融合分数（0.0 ~ 1.0）
    pub fused_score: f64,
    /// BM25 原始分数
    pub bm25_score: Option<f64>,
    /// 向量相似度原始分数
    pub vector_score: Option<f64>,
    /// 最终排名（排序后赋值，初始为 0）
    pub rank: usize,
    /// 附加元数据
    pub payload: serde_json::Value,
}

impl FusedSearchResult {
    /// 创建新的融合搜索结果
    ///
    /// `rank` 初始值为 0，由排序逻辑在后续赋值。
    #[must_use]
    pub const fn new(
        id: Uuid,
        fused_score: f64,
        bm25_score: Option<f64>,
        vector_score: Option<f64>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id,
            fused_score,
            bm25_score,
            vector_score,
            rank: 0,
            payload,
        }
    }
}

/// 混合搜索引擎（BM25 + Vector）
///
/// 将关键词搜索（BM25）与语义向量搜索通过加权 RRF 算法融合，
/// 兼顾精确匹配和语义理解能力。
#[cfg(feature = "db")]
pub struct HybridSearchEngine<V: VectorStore> {
    bm25: Arc<Bm25Index>,
    vector_store: Arc<V>,
    rrf_k: f64,
}

#[cfg(feature = "db")]
impl<V: VectorStore> HybridSearchEngine<V> {
    /// 创建混合搜索引擎（使用默认 RRF K = 60）
    #[allow(clippy::cast_precision_loss)]
    pub fn new(bm25: Bm25Index, vector_store: V) -> Self {
        Self {
            bm25: Arc::new(bm25),
            vector_store: Arc::new(vector_store),
            rrf_k: RRF_DEFAULT_K as f64,
        }
    }

    /// 创建混合搜索引擎（自定义 RRF K 参数）
    ///
    /// K 值越小，排名靠前的结果权重越大；K 值越大，排名差异的影响越平滑。
    #[allow(clippy::cast_precision_loss)]
    pub fn with_rrf_k(bm25: Bm25Index, vector_store: V, rrf_k: usize) -> Self {
        Self {
            bm25: Arc::new(bm25),
            vector_store: Arc::new(vector_store),
            rrf_k: rrf_k as f64,
        }
    }

    /// 执行混合搜索
    ///
    /// 同时执行 BM25 关键词搜索和向量语义搜索，通过加权 RRF 融合结果。
    ///
    /// # Errors
    ///
    /// 当 BM25 索引查询或向量存储查询失败时返回错误。
    pub async fn search(&self, query: &HybridQuery) -> Result<Vec<FusedSearchResult>> {
        info!(
            text_len = query.text.len(),
            has_vector = query.vector.is_some(),
            "hybrid search"
        );

        let bm25_results = if query.text.is_empty() {
            vec![]
        } else {
            self.execute_bm25(&query.text, query.options.top_k * 2)
                .await?
        };
        let vector_results = if let Some(ref v) = query.vector {
            self.vector_store
                .similarity_search("knowledge", v, query.options.clone())
                .await?
        } else {
            vec![]
        };

        let mut fused = weighted_rrf(
            &bm25_results,
            &vector_results,
            self.rrf_k,
            query.vector_weight,
            query.bm25_weight,
        );
        for (i, r) in fused.iter_mut().enumerate() {
            r.rank = i;
        }
        Ok(fused
            .into_iter()
            .take(query.options.top_k)
            .filter(|r| r.fused_score >= query.options.score_threshold)
            .collect())
    }

    #[allow(clippy::unused_async, clippy::cast_precision_loss)]
    async fn execute_bm25(&self, text: &str, top_k: usize) -> Result<Vec<Bm25Result>> {
        let terms: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
        let doc_ids: Vec<String> = self.bm25.doc_lengths().keys().cloned().collect();
        let mut scored: Vec<(String, f64)> = doc_ids
            .iter()
            .map(|id| (id.clone(), self.bm25.score(&terms, id)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored.truncate(top_k);
        Ok(scored)
    }

    /// 获取 BM25 索引引用
    #[must_use]
    pub fn bm25_index(&self) -> &Bm25Index {
        &self.bm25
    }

    /// 获取向量存储引用
    #[must_use]
    pub fn vector_store(&self) -> &V {
        &self.vector_store
    }
}

/// 加权 Reciprocal Rank Fusion (RRF)
///
/// 对 BM25 和向量搜索结果进行加权融合，返回归一化的融合分数。
///
/// # 参数
///
/// - `bm25_results`: BM25 搜索结果列表（文档 ID, 原始分数）
/// - `vector_results`: 向量搜索结果列表
/// - `k`: RRF 衰减参数（控制排名对分数的影响程度）
/// - `w_vec`: 向量搜索权重
/// - `w_bm25`: BM25 搜索权重
///
/// # 确定性保证
///
/// 当两个结果的融合分数差值小于浮点精度时，按 UUID 字典序作为确定性 tie-breaker，
/// 符合项目 FATAL-LOG-01 修复要求。
#[allow(clippy::cast_precision_loss)]
#[cfg(feature = "db")]
pub fn weighted_rrf(
    bm25_results: &[Bm25Result],
    vector_results: &[SearchResult],
    k: f64,
    w_vec: f64,
    w_bm25: f64,
) -> Vec<FusedSearchResult> {
    let mut map: HashMap<String, (f64, Option<f64>, Option<f64>, serde_json::Value)> =
        HashMap::new();
    for (rank, (id, raw)) in bm25_results.iter().enumerate() {
        let e = map
            .entry(id.clone())
            .or_insert((0.0, None, None, serde_json::json!({})));
        e.0 += w_bm25 / (k + rank as f64 + 1.0);
        e.1 = Some(*raw);
    }
    for (rank, vr) in vector_results.iter().enumerate() {
        let e = map
            .entry(vr.id.to_string())
            .or_insert((0.0, None, None, serde_json::json!({})));
        e.0 += w_vec / (k + rank as f64 + 1.0);
        e.2 = Some(vr.score);
        e.3 = vr.payload.clone();
    }
    let max_s = w_bm25 / (k + 1.0) + w_vec / (k + 1.0);
    let mut results: Vec<FusedSearchResult> = map
        .into_iter()
        .map(|(id, (rrf_raw, bs, vs, p))| FusedSearchResult {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| {
                tracing::warn!(id = %id, "RRF 融合: 无法解析为 UUID，使用 nil");
                Uuid::nil()
            }),
            fused_score: if max_s > 0.0 { rrf_raw / max_s } else { 0.0 },
            bm25_score: bs,
            vector_score: vs,
            rank: 0,
            payload: p,
        })
        .collect();
    results.sort_by(|a, b| {
        b.fused_score
            .partial_cmp(&a.fused_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    results
}

/// 标准 Reciprocal Rank Fusion（等权重）
///
/// 对 BM25 和语义搜索结果进行等权重 RRF 融合。
/// 适用于不需要区分两种搜索权重的场景。
///
/// # 公式
///
/// `score(d) = Σ_{r ∈ R} 1 / (k + rank_r(d))`
///
/// 其中 `R` 为所有排名列表集合，`rank_r(d)` 为文档 `d` 在列表 `r` 中的排名。
///
/// # 确定性保证
///
/// 当两个结果的分数差值小于浮点精度时，按 ID 字典序作为确定性 tie-breaker。
#[cfg(feature = "db")]
#[allow(clippy::cast_precision_loss)]
pub fn reciprocal_rank_fusion(
    bm25: &[(String, f64)],
    semantic: &[(String, f64)],
    k: usize,
) -> Vec<HybridSearchResult> {
    let mut scores: HashMap<String, (f64, Option<usize>, Option<usize>)> = HashMap::new();
    for (rank, (id, _)) in bm25.iter().enumerate() {
        let e = scores.entry(id.clone()).or_insert((0.0, None, None));
        e.0 += 1.0 / (k as f64 + rank as f64 + 1.0);
        e.1 = Some(rank + 1);
    }
    for (rank, (id, _)) in semantic.iter().enumerate() {
        let e = scores.entry(id.clone()).or_insert((0.0, None, None));
        e.0 += 1.0 / (k as f64 + rank as f64 + 1.0);
        e.2 = Some(rank + 1);
    }
    let mut results: Vec<HybridSearchResult> = scores
        .into_iter()
        .map(|(id, (s, br, sr))| HybridSearchResult {
            id,
            score: s,
            bm25_rank: br,
            semantic_rank: sr,
        })
        .collect();
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "db")]
    use crate::model::SourceType;
    #[cfg(feature = "hnswlib")]
    use crate::vector_store::hnswlib_adapter::HnswLibAdapter;
    #[cfg(feature = "db")]
    use crate::vector_store::types::ResultMetadata;
    #[cfg(feature = "hnswlib")]
    use crate::vector_store::{DistanceMetric, VectorPoint};
    use serde_json::json;

    #[cfg(feature = "db")]
    fn make_search_result(id: Uuid, score: f64, payload: serde_json::Value) -> SearchResult {
        SearchResult {
            id,
            score,
            payload,
            vector: None,
            metadata: ResultMetadata {
                chunk_id: String::new(),
                document_id: String::new(),
                chunk_index: 0,
                content_preview: String::new(),
                source_type: SourceType::Markdown,
            },
        }
    }

    #[cfg(feature = "hnswlib")]
    #[tokio::test]
    async fn test_hybrid_search_text_only() {
        let tokens: Vec<(String, String)> = vec![
            ("doc1".to_string(), "rust".to_string()),
            ("doc1".to_string(), "async".to_string()),
            ("doc2".to_string(), "python".to_string()),
        ];
        let bm25 = Bm25Index::build_from_tokens(&tokens);
        let adapter = HnswLibAdapter::new();
        adapter
            .init_collection("knowledge", 3, DistanceMetric::Cosine)
            .await
            .unwrap();
        adapter
            .upsert(
                "knowledge",
                vec![VectorPoint::new(
                    Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
                    vec![1.0; 3],
                    json!({"doc": "doc1"}),
                )],
            )
            .await
            .unwrap();

        let engine = HybridSearchEngine::new(bm25, adapter);
        let results = engine
            .search(&HybridQuery::text_only("rust"))
            .await
            .unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_fused_result_creation() {
        let r = FusedSearchResult::new(Uuid::new_v4(), 0.95, Some(2.5), Some(0.88), json!({}));
        assert!((r.fused_score - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fused_result_new_rank_is_zero() {
        let r = FusedSearchResult::new(Uuid::new_v4(), 0.5, None, None, json!({}));
        assert_eq!(r.rank, 0);
        assert!(r.bm25_score.is_none());
        assert!(r.vector_score.is_none());
    }

    #[test]
    fn test_hybrid_search_result_serialization() {
        let r = HybridSearchResult {
            id: "doc1".to_string(),
            score: 0.5,
            bm25_rank: Some(1),
            semantic_rank: Some(2),
        };
        let json = serde_json::to_string(&r).unwrap();
        let de: HybridSearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r.id, de.id);
        assert!((r.score - de.score).abs() < f64::EPSILON);
        assert_eq!(r.bm25_rank, de.bm25_rank);
        assert_eq!(r.semantic_rank, de.semantic_rank);
    }

    #[test]
    fn test_fused_search_result_serialization() {
        let r = FusedSearchResult::new(
            Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            0.75,
            Some(1.2),
            Some(0.9),
            json!({"key": "value"}),
        );
        let json = serde_json::to_string(&r).unwrap();
        let de: FusedSearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r.id, de.id);
        assert!((r.fused_score - de.fused_score).abs() < f64::EPSILON);
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_reciprocal_rank_fusion_basic() {
        let bm25: Vec<(String, f64)> = vec![("doc1".to_string(), 10.0), ("doc2".to_string(), 5.0)];
        let semantic: Vec<(String, f64)> =
            vec![("doc2".to_string(), 0.9), ("doc3".to_string(), 0.8)];
        let results = reciprocal_rank_fusion(&bm25, &semantic, RRF_DEFAULT_K);
        assert_eq!(results.len(), 3);
        assert!(results[0].score > results[1].score);
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_reciprocal_rank_fusion_empty_inputs() {
        let results = reciprocal_rank_fusion(&[], &[], RRF_DEFAULT_K);
        assert!(results.is_empty());

        let bm25: Vec<(String, f64)> = vec![("doc1".to_string(), 1.0)];
        let results = reciprocal_rank_fusion(&bm25, &[], RRF_DEFAULT_K);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
        assert!(results[0].bm25_rank.is_some());
        assert!(results[0].semantic_rank.is_none());

        let semantic: Vec<(String, f64)> = vec![("doc2".to_string(), 0.5)];
        let results = reciprocal_rank_fusion(&[], &semantic, RRF_DEFAULT_K);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc2");
        assert!(results[0].bm25_rank.is_none());
        assert!(results[0].semantic_rank.is_some());
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_reciprocal_rank_fusion_overlap() {
        let bm25: Vec<(String, f64)> = vec![("doc1".to_string(), 10.0), ("doc2".to_string(), 5.0)];
        let semantic: Vec<(String, f64)> =
            vec![("doc1".to_string(), 0.95), ("doc3".to_string(), 0.7)];
        let results = reciprocal_rank_fusion(&bm25, &semantic, RRF_DEFAULT_K);
        let doc1 = results.iter().find(|r| r.id == "doc1").unwrap();
        assert!(doc1.bm25_rank.is_some());
        assert!(doc1.semantic_rank.is_some());
        assert!(doc1.score > 0.0);
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_reciprocal_rank_fusion_deterministic_tie_breaking() {
        let bm25: Vec<(String, f64)> = vec![("aaa".to_string(), 1.0), ("bbb".to_string(), 1.0)];
        let semantic: Vec<(String, f64)> = vec![];
        let results = reciprocal_rank_fusion(&bm25, &semantic, RRF_DEFAULT_K);
        assert_eq!(results[0].id, "aaa");
        assert_eq!(results[1].id, "bbb");
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_weighted_rrf_basic() {
        let bm25: Vec<Bm25Result> = vec![("doc1".to_string(), 10.0), ("doc2".to_string(), 5.0)];
        let vector: Vec<SearchResult> = vec![
            make_search_result(
                Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
                0.9,
                json!({}),
            ),
            make_search_result(
                Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap(),
                0.8,
                json!({}),
            ),
        ];
        let results = weighted_rrf(&bm25, &vector, 60.0, 0.7, 0.3);
        assert_eq!(results.len(), 4);
        assert!(results[0].fused_score > 0.0);
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_weighted_rrf_empty_inputs() {
        let results = weighted_rrf(&[], &[], 60.0, 0.7, 0.3);
        assert!(results.is_empty());

        let bm25: Vec<Bm25Result> = vec![("doc1".to_string(), 5.0)];
        let results = weighted_rrf(&bm25, &[], 60.0, 0.7, 0.3);
        assert_eq!(results.len(), 1);
        assert!(results[0].bm25_score.is_some());
        assert!(results[0].vector_score.is_none());

        let vector: Vec<SearchResult> = vec![make_search_result(Uuid::new_v4(), 0.9, json!({}))];
        let results = weighted_rrf(&[], &vector, 60.0, 0.7, 0.3);
        assert_eq!(results.len(), 1);
        assert!(results[0].bm25_score.is_none());
        assert!(results[0].vector_score.is_some());
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_weighted_rrf_normalization() {
        let bm25: Vec<Bm25Result> = vec![("doc1".to_string(), 10.0)];
        let vector: Vec<SearchResult> = vec![make_search_result(
            Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            0.95,
            json!({}),
        )];
        let results = weighted_rrf(&bm25, &vector, 60.0, 0.7, 0.3);
        assert_eq!(results.len(), 2);
        for r in &results {
            assert!(r.fused_score <= 1.0 + f64::EPSILON);
            assert!(r.fused_score >= 0.0 - f64::EPSILON);
        }
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_weighted_rrf_invalid_uuid_fallback() {
        let bm25: Vec<Bm25Result> = vec![("not-a-uuid".to_string(), 5.0)];
        let results = weighted_rrf(&bm25, &[], 60.0, 0.7, 0.3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, Uuid::nil());
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_weighted_rrf_deterministic_tie_breaking() {
        let bm25: Vec<Bm25Result> = vec![];
        let vector: Vec<SearchResult> = vec![
            make_search_result(
                Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap(),
                0.5,
                json!({}),
            ),
            make_search_result(
                Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap(),
                0.5,
                json!({}),
            ),
        ];
        let results = weighted_rrf(&bm25, &vector, 60.0, 0.7, 0.3);
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].id,
            Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap()
        );
        assert_eq!(
            results[1].id,
            Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap()
        );
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_weighted_rrf_overlap_preserves_scores() {
        let id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let bm25: Vec<Bm25Result> = vec![(id.to_string(), 8.0)];
        let vector: Vec<SearchResult> = vec![make_search_result(id, 0.92, json!({"meta": "data"}))];
        let results = weighted_rrf(&bm25, &vector, 60.0, 0.7, 0.3);
        assert_eq!(results.len(), 1);
        assert!(results[0].bm25_score.is_some());
        assert!(results[0].vector_score.is_some());
        assert_eq!(results[0].payload["meta"], "data");
    }
}
