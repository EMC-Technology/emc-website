//! 搜索与图遍历请求/响应类型定义
//!
//! 本模块定义了向量搜索和引用追踪相关的 DTO 类型，
//! 用于 API 层与 VM 层之间的数据传输。

#[allow(clippy::wildcard_imports)]
use knowledge_core::model::*;
use serde::{Deserialize, Serialize};

/// 向量搜索请求
///
/// 通过向量相似度搜索最近的 Block。
///
/// # Example
///
/// ```json
/// {
///   "query_vec": [0.1, 0.2, ...],
///   "k": 10
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchRequest {
    /// 查询向量（维度须与系统配置一致，默认 1536）
    pub query_vec: Vec<f32>,
    /// 返回最近邻的数量上限
    pub k: u32,
}

/// 向量搜索响应（含确定性排序信息）
///
/// 确定性保证：通过 `ORDER BY distance, id` 双重排序确保
/// 相同输入产生字节级一致的输出（FATAL-LOG-01 修复）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResponse {
    /// 搜索结果列表（按距离升序排列）
    pub results: Vec<VectorSearchResultItem>,
    /// 结果总数
    pub total: usize,
    /// 查询耗时（毫秒）
    pub query_time_ms: u64,
}

/// 向量搜索单条结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResultItem {
    /// 匹配的 Block
    pub block: Block,
    /// 与查询向量的距离（越小越相似）
    pub distance: f64,
    /// 排名（从 1 开始）
    pub rank: usize,
}

/// 图遍历请求
///
/// 从指定 Token 出发，沿引用关系追踪关联节点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceReferencesRequest {
    /// 起始 Token ID
    pub token_id: String,
    /// 引用类型过滤（None 表示所有类型）
    pub ref_type: Option<RefType>,
    /// 最大遍历深度
    pub max_depth: u32,
}

/// 图遍历响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceReferencesResponse {
    /// 追踪到的引用列表
    pub references: Vec<Reference>,
    /// 引用总数
    pub total: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_search_request_serialization() {
        let req = VectorSearchRequest {
            query_vec: vec![0.1, 0.2, 0.3],
            k: 5,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"query_vec\""));
        assert!(json.contains("\"k\":5"));

        let de: VectorSearchRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(de.k, 5);
        assert_eq!(de.query_vec.len(), 3);
    }

    #[test]
    fn test_vector_search_response_serialization() {
        let resp = VectorSearchResponse {
            results: vec![],
            total: 0,
            query_time_ms: 42,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"query_time_ms\":42"));
        assert!(json.contains("\"total\":0"));
    }

    #[test]
    fn test_trace_references_request_defaults() {
        let req = TraceReferencesRequest {
            token_id: "token:abc".to_string(),
            ref_type: None,
            max_depth: 5,
        };
        assert!(req.ref_type.is_none());
        assert_eq!(req.max_depth, 5);
    }

    #[test]
    fn test_trace_references_response_empty() {
        let resp = TraceReferencesResponse {
            references: vec![],
            total: 0,
        };
        assert!(resp.references.is_empty());
        assert_eq!(resp.total, 0);
    }
}
