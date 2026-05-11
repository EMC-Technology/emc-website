//! 统一 API 响应 DTO 定义

use knowledge_core::model::RefType;
use serde::{Deserialize, Serialize};

/// 统一 API 响应包装器
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    /// 请求是否成功
    pub success: bool,
    /// 响应数据
    pub data: T,
    /// 分页元数据（仅列表接口返回）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<PaginationMeta>,
    /// 响应时间戳（RFC 3339 格式）
    pub timestamp: String,
}

impl<T: Serialize> ApiResponse<T> {
    /// 创建成功响应
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data,
            pagination: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// 创建带分页的成功响应
    pub fn success_with_pagination(data: T, pagination: PaginationMeta) -> Self {
        Self {
            success: true,
            data,
            pagination: Some(pagination),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// 分页元数据
#[derive(Debug, Serialize, Clone)]
pub struct PaginationMeta {
    /// 总记录数
    pub total: usize,
    /// 当前偏移量
    pub offset: u32,
    /// 每页条数
    pub limit: u32,
    /// 是否还有更多记录
    pub has_more: bool,
}

impl PaginationMeta {
    /// 根据总数、偏移量和每页条数创建分页元数据
    #[must_use]
    pub const fn new(total: usize, offset: u32, limit: u32) -> Self {
        let has_more = (offset as usize).saturating_add(limit as usize) < total;
        Self {
            total,
            offset,
            limit,
            has_more,
        }
    }
}

/// Block 含 Token 列表的复合响应
#[derive(Debug, Serialize)]
pub struct BlockWithTokens {
    /// Block 实体
    pub block: knowledge_core::model::Block,
    /// Block 下的 Token 列表
    pub tokens: Vec<knowledge_core::model::Token>,
}

/// 健康检查服务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthState {
    /// 服务正常
    Healthy,
    /// 服务不健康
    Unhealthy,
    /// 服务降级运行
    Degraded,
}

/// 健康检查响应
#[derive(Debug, Serialize)]
pub struct HealthStatus {
    /// 服务状态标识
    pub status: HealthState,
    /// API 版本号
    pub version: String,
    /// 响应生成时间戳 (RFC3339 格式)
    pub timestamp: String,
}

impl HealthStatus {
    /// 创建健康状态实例
    #[must_use]
    pub fn healthy() -> Self {
        Self {
            status: HealthState::Healthy,
            version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// 文档上传请求（Multipart 替代方案的 JSON 版本）
#[derive(Debug, Deserialize)]
pub struct UploadDocumentRequest {
    /// 文档存储路径
    pub path: String,
    /// 文档标题（可选）
    pub title: Option<String>,
    /// 文档文本内容
    pub content: String,
}

/// 列表查询参数
#[derive(Debug, Deserialize)]
/// 列表查询参数
///
/// 支持分页和过滤的通用查询参数。
pub struct ListQueryParams {
    /// 分页偏移量（默认 0）
    pub offset: Option<u32>,
    /// 每页条数（默认 20，最大 100）
    pub limit: Option<u32>,
}

impl ListQueryParams {
    /// 获取分页偏移量，默认为 0
    #[must_use]
    pub fn offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }

    /// 获取每页条数，默认 20，最大 100
    #[must_use]
    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(20).min(100)
    }
}

/// 向量搜索查询参数
#[derive(Debug, Deserialize)]
/// 向量搜索查询参数
///
/// 用于基于嵌入向量的相似度搜索。
pub struct VectorSearchQuery {
    /// 近邻搜索的 k 值（返回最相似的 k 个结果）
    pub k: Option<u32>,
}

impl VectorSearchQuery {
    /// 获取近邻搜索数量 k，默认 10，最大 100
    #[must_use]
    pub fn k(&self) -> u32 {
        self.k.unwrap_or(10).min(100)
    }
}

/// 全文搜索查询参数
#[derive(Debug, Deserialize)]
/// 全文搜索查询参数
///
/// 用于基于关键词的全文检索。
pub struct FullTextSearchQuery {
    /// 搜索关键词
    pub q: String,
    /// 返回结果数量限制（默认 20，最大 100）
    pub limit: Option<u32>,
}

impl FullTextSearchQuery {
    /// 获取结果数量限制，默认 20，最大 100
    #[must_use]
    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(20).min(100)
    }

    /// # Errors
    ///
    /// 搜索关键词为空时返回验证错误。
    pub fn validate(&self) -> crate::Result<()> {
        if self.q.trim().is_empty() {
            return Err(error_core::helpers::validation_error(
                "搜索关键词不能为空",
                "validate",
            ));
        }
        Ok(())
    }
}

/// 符号 360° 上下文视图
///
/// 聚合指定符号的所有关联信息，包括调用者、被调用者、
/// 所属社区、关联流程和全部引用关系。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolContext {
    /// 符号 ID
    pub symbol_id: String,
    /// 调用者列表（引用中 `to_id` = `symbol_id` 的记录）
    pub callers: Vec<SymbolReference>,
    /// 被调用者列表（引用中 `from_id` = `symbol_id` 的记录）
    pub callees: Vec<SymbolReference>,
    /// 所属社区信息
    pub community: Option<CommunityInfo>,
    /// 关联流程名称列表
    pub processes: Vec<String>,
    /// 全部引用关系
    pub references: Vec<SymbolReference>,
}

/// 符号引用关系摘要
///
/// 表示两个符号之间的有向引用关系，用于 360° 上下文视图中
/// 的调用者和被调用者聚合。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolReference {
    /// 引用起点 ID
    pub from_id: String,
    /// 引用终点 ID
    pub to_id: String,
    /// 引用关系类型
    pub ref_type: RefType,
}

/// 社区信息摘要
///
/// 表示符号所属社区的名称与内聚度分数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityInfo {
    /// 社区名称
    pub name: String,
    /// 内聚度分数（0.0 ~ 1.0）
    pub cohesion_score: f64,
}

/// 按社区分组的搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunitySearchGroup {
    /// 社区名称
    pub community_name: String,
    /// 社区内聚度分数（0.0 ~ 1.0）
    pub cohesion_score: f64,
    /// 社区内的 Block 列表
    pub blocks: Vec<knowledge_core::model::Block>,
}

/// 按执行流程分组的搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSearchGroup {
    /// 流程名称
    pub process_name: String,
    /// 流程入口点 ID
    pub entry_point: String,
    /// 流程内的 Block 列表
    pub blocks: Vec<knowledge_core::model::Block>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse::success(vec![1, 2, 3]);
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"data\":[1,2,3]"));
        assert!(json.contains("\"timestamp\""));
    }

    #[test]
    fn test_pagination_meta_fields() {
        let meta = PaginationMeta::new(100, 0, 20);
        assert_eq!(meta.total, 100);
        assert_eq!(meta.offset, 0);
        assert_eq!(meta.limit, 20);
        assert!(meta.has_more);

        let meta_last = PaginationMeta::new(100, 80, 20);
        assert!(!meta_last.has_more);
    }

    #[test]
    fn test_health_status_healthy() {
        let health = HealthStatus::healthy();
        assert_eq!(health.status, HealthState::Healthy);
        assert!(!health.version.is_empty());
        assert!(!health.timestamp.is_empty());
    }

    #[test]
    fn test_list_query_params_defaults() {
        let params = ListQueryParams {
            offset: None,
            limit: None,
        };
        assert_eq!(params.offset(), 0);
        assert_eq!(params.limit(), 20);

        let params_limited = ListQueryParams {
            offset: Some(10),
            limit: Some(200),
        };
        assert_eq!(params_limited.offset(), 10);
        assert_eq!(params_limited.limit(), 100); // 限制最大值
    }

    #[test]
    fn test_community_info_serialization() {
        let info = CommunityInfo {
            name: "Rust 异步运行时".to_string(),
            cohesion_score: 0.87,
        };

        let json = serde_json::to_string(&info).expect("序列化失败");
        let de_info: CommunityInfo = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(info.name, de_info.name);
        assert!((info.cohesion_score - de_info.cohesion_score).abs() < f64::EPSILON);
    }

    #[test]
    fn test_symbol_reference_ref_type_classification() {
        let ref_types = vec![
            RefType::Definition,
            RefType::Usage,
            RefType::Link,
            RefType::Inherit,
            RefType::Implement,
            RefType::Constrain,
        ];

        for rt in ref_types {
            let sym_ref = SymbolReference {
                from_id: "token:a".to_string(),
                to_id: "token:b".to_string(),
                ref_type: rt.clone(),
            };

            let json = serde_json::to_string(&sym_ref).expect("序列化失败");
            let de_ref: SymbolReference = serde_json::from_str(&json).expect("反序列化失败");

            assert_eq!(sym_ref.from_id, de_ref.from_id);
            assert_eq!(sym_ref.to_id, de_ref.to_id);
            assert_eq!(sym_ref.ref_type, de_ref.ref_type);
        }
    }

    #[test]
    fn test_vector_search_query_k_default() {
        let query = VectorSearchQuery { k: None };
        assert_eq!(query.k(), 10);
    }

    #[test]
    fn test_vector_search_query_k_custom() {
        let query = VectorSearchQuery { k: Some(50) };
        assert_eq!(query.k(), 50);
    }

    #[test]
    fn test_vector_search_query_k_max_cap() {
        let query = VectorSearchQuery { k: Some(200) };
        assert_eq!(query.k(), 100);
    }

    #[test]
    fn test_full_text_search_query_validate_success() {
        let query = FullTextSearchQuery {
            q: "test query".to_string(),
            limit: None,
        };
        assert!(query.validate().is_ok());
    }

    #[test]
    fn test_full_text_search_query_validate_empty() {
        let query = FullTextSearchQuery {
            q: "   ".to_string(),
            limit: None,
        };
        assert!(query.validate().is_err());
    }

    #[test]
    fn test_full_text_search_query_limit_default() {
        let query = FullTextSearchQuery {
            q: "test".to_string(),
            limit: None,
        };
        assert_eq!(query.limit(), 20);
    }

    #[test]
    fn test_full_text_search_query_limit_capped() {
        let query = FullTextSearchQuery {
            q: "test".to_string(),
            limit: Some(200),
        };
        assert_eq!(query.limit(), 100);
    }

    #[test]
    fn test_api_response_with_pagination() {
        let meta = PaginationMeta::new(100, 0, 20);
        let response = ApiResponse::success_with_pagination(vec![1, 2], meta);
        assert!(response.success);
        assert!(response.pagination.is_some());
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("pagination"));
    }

    #[test]
    fn test_health_state_serialization() {
        let states = [
            HealthState::Healthy,
            HealthState::Unhealthy,
            HealthState::Degraded,
        ];
        let json = serde_json::to_string(&states).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("unhealthy"));
        assert!(json.contains("degraded"));
    }

    #[test]
    fn test_symbol_context_serialization() {
        let ctx = SymbolContext {
            symbol_id: "sym1".to_string(),
            callers: vec![],
            callees: vec![],
            community: Some(CommunityInfo {
                name: "core".to_string(),
                cohesion_score: 0.9,
            }),
            processes: vec!["main".to_string()],
            references: vec![],
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let de: SymbolContext = serde_json::from_str(&json).unwrap();
        assert_eq!(de.symbol_id, "sym1");
        assert!(de.community.is_some());
    }

    #[test]
    fn test_pagination_meta_no_more() {
        let meta = PaginationMeta::new(20, 0, 20);
        assert!(!meta.has_more);
    }

    #[test]
    fn test_upload_document_request_deserialization() {
        let json = r#"{"path": "/test.md", "title": "Test", "content": "hello"}"#;
        let req: UploadDocumentRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.path, "/test.md");
        assert_eq!(req.title, Some("Test".to_string()));
    }
}
