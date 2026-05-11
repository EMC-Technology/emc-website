/// 向量存储核心类型定义
///
/// 定义搜索选项、过滤器、结果、距离度量等核心数据结构。
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 向量搜索选项
///
/// 控制相似性搜索的行为参数，包括返回数量、阈值、过滤条件等。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    /// 返回 Top-K 结果（默认: 10）
    pub top_k: usize,
    /// 最小相似度阈值 [0, 1]（默认: 0.7）
    pub score_threshold: f64,
    /// 元数据过滤条件（可选）
    pub filter: Option<Filter>,
    /// 是否返回原始向量数据（默认: false，节省带宽）
    pub include_vectors: bool,
    /// 是否返回原始 payload（默认: true）
    pub include_payload: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            top_k: 10,
            score_threshold: 0.7,
            filter: None,
            include_vectors: false,
            include_payload: true,
        }
    }
}

/// 元数据过滤器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    /// AND 条件：所有条件必须满足
    pub must: Vec<Condition>,
    /// OR 条件：至少满足一个
    pub should: Vec<Condition>,
    /// NOT 条件：必须不满足
    pub must_not: Vec<Condition>,
}

/// 过滤条件枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    /// 字段等于指定值
    FieldEquals {
        /// 字段名
        key: String,
        /// 匹配值
        value: serde_json::Value,
    },
    /// 字段在指定数值范围内
    FieldInRange {
        /// 字段名
        key: String,
        /// 数值范围
        range: ValueRange,
    },
    /// 字段值在指定集合中
    FieldIn {
        /// 字段名
        key: String,
        /// 候选值集合
        values: Vec<serde_json::Value>,
    },
}

/// 数值范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueRange {
    /// 范围下界（包含）
    pub min: f64,
    /// 范围上界（包含）
    pub max: f64,
}

/// 向量搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// 结果向量点的唯一标识符
    pub id: Uuid,
    /// 相似度分数
    pub score: f64,
    /// 附加元数据
    pub payload: serde_json::Value,
    /// 原始嵌入向量（仅当 `include_vectors=true` 时返回）
    pub vector: Option<Vec<f32>>,
    /// 搜索结果元数据
    pub metadata: ResultMetadata,
}

/// 搜索结果元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultMetadata {
    /// 分块 ID
    pub chunk_id: String,
    /// 所属文档 ID
    pub document_id: String,
    /// 分块在文档中的索引位置
    pub chunk_index: usize,
    /// 内容预览文本
    pub content_preview: String,
    /// 源文件类型
    pub source_type: crate::model::SourceType,
}

/// 距离度量类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DistanceMetric {
    /// 余弦相似度（默认）
    #[default]
    Cosine,
    /// 欧几里得距离
    Euclidean,
    /// 点积
    DotProduct,
    /// 曼哈顿距离
    Manhattan,
}

/// 混合查询（BM25 + Vector）
///
/// 支持纯文本、纯向量或混合检索模式。
/// 混合模式下，通过 `vector_weight` 和 `bm25_weight` 控制两种信号的权重。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridQuery {
    /// 查询文本（BM25 检索输入）
    pub text: String,
    /// 查询向量（向量检索输入）
    pub vector: Option<Vec<f32>>,
    /// 向量检索权重（默认: 0.7）
    pub vector_weight: f64,
    /// BM25 检索权重（默认: 0.3）
    pub bm25_weight: f64,
    /// 搜索选项
    pub options: SearchOptions,
}

impl Default for HybridQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            vector: None,
            vector_weight: 0.7,
            bm25_weight: 0.3,
            options: SearchOptions::default(),
        }
    }
}

impl HybridQuery {
    /// 创建纯文本查询（仅 BM25）
    #[must_use]
    pub fn text_only(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    /// 创建纯向量查询
    #[must_use]
    pub fn vector_only(vector: Vec<f32>) -> Self {
        Self {
            vector: Some(vector),
            ..Default::default()
        }
    }

    /// 创建混合查询
    ///
    /// `vector_weight` 控制向量检索权重，BM25 权重自动计算为 `1.0 - vector_weight`。
    #[must_use]
    pub fn hybrid(text: impl Into<String>, vector: Vec<f32>, vector_weight: f64) -> Self {
        let bm25_weight = 1.0 - vector_weight;
        Self {
            text: text.into(),
            vector: Some(vector),
            vector_weight,
            bm25_weight,
            options: SearchOptions::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_options_default() {
        let opts = SearchOptions::default();
        assert_eq!(opts.top_k, 10);
        assert!((opts.score_threshold - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_hybrid_query_constructors() {
        let text_query = HybridQuery::text_only("hello");
        assert_eq!(text_query.text, "hello");
        assert!(text_query.vector.is_none());
    }

    #[test]
    fn test_distance_metric_default() {
        assert_eq!(DistanceMetric::default(), DistanceMetric::Cosine);
    }

    #[test]
    fn test_hybrid_query_vector_only() {
        let query = HybridQuery::vector_only(vec![1.0, 2.0, 3.0]);
        assert!(query.text.is_empty());
        assert_eq!(query.vector, Some(vec![1.0, 2.0, 3.0]));
        assert!((query.vector_weight - 0.7).abs() < f64::EPSILON);
        assert!((query.bm25_weight - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_hybrid_query_hybrid() {
        let query = HybridQuery::hybrid("test query", vec![1.0, 2.0], 0.8);
        assert_eq!(query.text, "test query");
        assert_eq!(query.vector, Some(vec![1.0, 2.0]));
        assert!((query.vector_weight - 0.8).abs() < f64::EPSILON);
        assert!((query.bm25_weight - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_hybrid_query_default() {
        let query = HybridQuery::default();
        assert!(query.text.is_empty());
        assert!(query.vector.is_none());
        assert!((query.vector_weight - 0.7).abs() < f64::EPSILON);
        assert!((query.bm25_weight - 0.3).abs() < f64::EPSILON);
        assert_eq!(query.options.top_k, 10);
    }

    #[test]
    fn test_search_options_serialization() {
        let opts = SearchOptions::default();
        let json = serde_json::to_string(&opts).unwrap();
        let de: SearchOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts.top_k, de.top_k);
        assert!((opts.score_threshold - de.score_threshold).abs() < f64::EPSILON);
    }

    #[test]
    fn test_hybrid_query_serialization() {
        let query = HybridQuery::text_only("hello world");
        let json = serde_json::to_string(&query).unwrap();
        let de: HybridQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query.text, de.text);
    }

    #[test]
    fn test_filter_serialization() {
        let filter = Filter {
            must: vec![Condition::FieldEquals {
                key: "status".to_string(),
                value: serde_json::json!("active"),
            }],
            should: vec![],
            must_not: vec![],
        };
        let json = serde_json::to_string(&filter).unwrap();
        let de: Filter = serde_json::from_str(&json).unwrap();
        assert_eq!(de.must.len(), 1);
    }

    #[test]
    fn test_condition_field_in_range() {
        let condition = Condition::FieldInRange {
            key: "score".to_string(),
            range: ValueRange { min: 0.0, max: 1.0 },
        };
        let json = serde_json::to_string(&condition).unwrap();
        let de: Condition = serde_json::from_str(&json).unwrap();
        assert!(matches!(de, Condition::FieldInRange { .. }));
    }

    #[test]
    fn test_condition_field_in() {
        let condition = Condition::FieldIn {
            key: "type".to_string(),
            values: vec![serde_json::json!("a"), serde_json::json!("b")],
        };
        let json = serde_json::to_string(&condition).unwrap();
        let de: Condition = serde_json::from_str(&json).unwrap();
        assert!(matches!(de, Condition::FieldIn { .. }));
    }

    #[test]
    fn test_search_options_with_filter() {
        let opts = SearchOptions {
            top_k: 5,
            score_threshold: 0.9,
            filter: Some(Filter {
                must: vec![],
                should: vec![],
                must_not: vec![],
            }),
            include_vectors: true,
            include_payload: false,
        };
        assert_eq!(opts.top_k, 5);
        assert!(opts.filter.is_some());
        assert!(opts.include_vectors);
        assert!(!opts.include_payload);
    }
}
