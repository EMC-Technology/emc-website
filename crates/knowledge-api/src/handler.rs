//! Axum HTTP 请求处理器
//!
//! # 职责
//! - 接收 HTTP 请求并提取参数
//! - 调用 KnowledgeVM 执行业务逻辑
//! - 将结果包装为统一响应格式返回

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::path::Path as StdPath;

use crate::auth::AuthService;
use crate::auth::LoginRateLimiter;
use crate::authz::engine::AuthorizationEngine;
use crate::dto::{
    ApiResponse, BlockWithTokens, FullTextSearchQuery, HealthStatus, ListQueryParams,
    PaginationMeta, UploadDocumentRequest,
};
use crate::error_handler::ApiError;
use crate::knowledge_vm::KnowledgeVm as KnowledgeVM;
use crate::query_types::{
    TraceReferencesResponse, VectorSearchRequest, VectorSearchResponse, VectorSearchResultItem,
};
use crate::ws::WsConnectionManager;
use error_core::helpers;
use knowledge_core::model::{Block, Document, RefType, SourceType, Token};
use std::sync::Arc;

/// 上传内容大小上限（10 MB）
const MAX_UPLOAD_CONTENT_SIZE: usize = 10 * 1024 * 1024;

/// 应用状态（通过 Axum State extractor 注入到所有处理器）
#[derive(Clone)]
pub struct AppState {
    /// 知识虚拟机实例
    pub vm: Arc<KnowledgeVM>,
    /// WebSocket 连接管理器
    pub ws_manager: Arc<WsConnectionManager>,
    /// 认证服务
    pub auth_service: Arc<AuthService>,
    /// 登录限流器
    pub rate_limiter: Arc<LoginRateLimiter>,
    /// 授权引擎（可选，未配置时跳过 ABAC 检查）
    pub authz_engine: Option<Arc<AuthorizationEngine>>,
}

// ========== 文档 API 处理器 ==========

/// 分页获取文档列表
///
/// GET /api/v1/documents?offset=0&limit=20
///
/// # Errors
///
/// 当数据库查询失败时返回错误。
#[tracing::instrument(skip(state), fields(operation = "list_documents", offset, limit,))]
pub async fn list_documents(
    State(state): State<AppState>,
    Query(params): Query<ListQueryParams>,
) -> Result<Json<ApiResponse<Vec<Document>>>, ApiError> {
    let offset = params.offset();
    let limit = params.limit();

    let documents = state.vm.list_documents(offset, limit)?;

    let total = documents.len();
    let pagination = PaginationMeta::new(total, offset, limit);

    Ok(Json(ApiResponse::success_with_pagination(
        documents, pagination,
    )))
}

/// 上传新文档（触发解析流水线）
///
/// POST /api/v1/documents
/// Content-Type: application/json
///
/// # Errors
///
/// 当文件路径/内容为空、内容超限、路径遍历或数据库操作失败时返回错误。
#[tracing::instrument(skip(state), fields(operation = "upload_document", document_path,))]
pub async fn upload_document(
    State(state): State<AppState>,
    Json(req): Json<UploadDocumentRequest>,
) -> Result<Json<ApiResponse<Document>>, ApiError> {
    if req.path.is_empty() {
        return Err(helpers::validation_error("文件路径不能为空", "upload").into());
    }
    if req.content.is_empty() {
        return Err(helpers::validation_error("文件内容不能为空", "upload").into());
    }
    if req.content.len() > MAX_UPLOAD_CONTENT_SIZE {
        return Err(helpers::validation_error(
            &format!("文件内容超过大小上限 {MAX_UPLOAD_CONTENT_SIZE} 字节"),
            "upload",
        )
        .into());
    }

    if req.path.contains("..") || req.path.contains('\\') {
        return Err(helpers::validation_error("文件路径不允许包含路径遍历字符", "upload").into());
    }

    let decoded_path = urlencoding::decode(&req.path)
        .map_err(|_| helpers::validation_error("文件路径编码无效", "upload"))?;
    if decoded_path.contains("..") {
        return Err(helpers::validation_error("文件路径不允许包含编码后的路径遍历字符", "upload").into());
    }

    let source_type = detect_source_type(&req.path);

    let hash = blake3_hash(&req.content);

    if let Some(existing) = state.vm.check_idempotency(&hash)? {
        tracing::info!(
            document_path = %existing.path,
            hash = %hash,
            "文档已存在，返回幂等结果 [AUDIT-011]"
        );
        return Ok(Json(ApiResponse::success(existing)));
    }

    let doc = Document::new(
        &req.path,
        req.title.unwrap_or_else(|| {
            req.path
                .split('/')
                .next_back()
                .unwrap_or("Untitled")
                .to_string()
        }),
        source_type,
        &hash,
    )?;

    let created = state.vm.create_document(&doc)?;

    tracing::info!(
        document_path = %created.path,
        hash = %hash,
        "文档上传成功 [AUDIT-010]"
    );

    Ok(Json(ApiResponse::success(created)))
}

/// 获取文档详情
///
/// GET /api/v1/documents/:id
///
/// # Errors
///
/// 当文档不存在或数据库查询失败时返回错误。
#[tracing::instrument(skip(state), fields(
    operation = "get_document",
    document_id = %id,
))]
pub async fn get_document(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Document>>, ApiError> {
    let doc = state.vm.get_document(&id)?;
    Ok(Json(ApiResponse::success(doc)))
}

/// 删除文档（级联删除所有关联数据）
///
/// DELETE /api/v1/documents/:id
///
/// # Errors
///
/// 当文档不存在或数据库删除失败时返回错误。
#[tracing::instrument(skip(state), fields(
    operation = "delete_document",
    document_id = %id,
))]
pub async fn delete_document(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    state.vm.delete_document(&id)?;

    tracing::warn!(
        document_id = %id,
        "文档删除操作 [AUDIT-060]"
    );

    Ok(Json(ApiResponse::success(())))
}

// ========== Block API 处理器 ==========

/// 获取文档的所有 Block 列表
///
/// GET /api/v1/documents/:id/blocks
///
/// # Errors
///
/// 当文档不存在或数据库查询失败时返回错误。
#[tracing::instrument(skip(state), fields(
    operation = "get_document_blocks",
    document_id = %doc_id,
))]
pub async fn get_document_blocks(
    Path(doc_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Block>>>, ApiError> {
    let blocks = state.vm.list_blocks_by_document(&doc_id)?;
    Ok(Json(ApiResponse::success(blocks)))
}

/// 获取 Block 详情（含关联 Token 列表）
///
/// GET /api/v1/blocks/:id
///
/// # Errors
///
/// 当 Block 不存在或数据库查询失败时返回错误。
#[tracing::instrument(skip(state), fields(
    operation = "get_block",
    block_id = %id,
))]
pub async fn get_block(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<BlockWithTokens>>, ApiError> {
    let (block, tokens) = state.vm.get_block(&id)?;
    Ok(Json(ApiResponse::success(BlockWithTokens {
        block,
        tokens,
    })))
}

/// 获取 Block 的 Token 列表
///
/// GET /api/v1/blocks/:id/tokens
///
/// # Errors
///
/// 当 Block 不存在或数据库查询失败时返回错误。
#[tracing::instrument(skip(state), fields(
    operation = "get_block_tokens",
    block_id = %block_id,
))]
pub async fn get_block_tokens(
    Path(block_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Token>>>, ApiError> {
    let tokens = state.vm.list_tokens_by_block(&block_id)?;
    Ok(Json(ApiResponse::success(tokens)))
}

// ========== Token API 处理器 ==========

/// 获取 Token 详情
///
/// GET /api/v1/tokens/:id
///
/// # Errors
///
/// 当 Token 不存在或数据库查询失败时返回错误。
pub fn get_token(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Token>>, ApiError> {
    let token = state.vm.get_token(&id)?;
    Ok(Json(ApiResponse::success(token)))
}

/// 获取 Token 的反向引用链（图遍历）
///
/// GET `/api/v1/tokens/:id/references?ref_type=Usage&max_depth=5`
///
/// # Errors
///
/// 当 Token 不存在、引用追踪或数据库查询失败时返回错误。
pub async fn trace_references(
    Path(token_id): Path<String>,
    Query(params): Query<TraceReferencesQuery>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<TraceReferencesResponse>>, ApiError> {
    let ref_type = params.ref_type.clone();

    let references = state.vm.trace_references(&token_id, ref_type)?;
    let total = state.vm.count_references(&token_id, params.ref_type)?;

    let response = TraceReferencesResponse { references, total };
    Ok(Json(ApiResponse::success(response)))
}

/// 引用追踪查询参数
#[derive(Debug, Deserialize)]
pub struct TraceReferencesQuery {
    /// 引用类型过滤（None 表示所有类型）
    pub ref_type: Option<RefType>,
    /// 最大追踪深度
    pub max_depth: Option<u32>,
}

// ========== 搜索 API 处理器 ==========

/// 向量相似度搜索
///
/// POST /api/v1/search/vector
/// Body: `{ "query_vec": [...], "k": 10 }`
///
/// # Errors
///
/// 当向量维度不匹配或数据库查询失败时返回错误。
#[tracing::instrument(skip(state), fields(
    operation = "vector_search",
    k = req.k,
))]
pub async fn vector_search(
    State(state): State<AppState>,
    Json(req): Json<VectorSearchRequest>,
) -> Result<Json<ApiResponse<VectorSearchResponse>>, ApiError> {
    if req.query_vec.is_empty() {
        return Err(ApiError::from_error_object(
            &error_core::helpers::validation_error("查询向量不能为空", "vector_search"),
        ));
    }
    if req.query_vec.len() > 4096 {
        return Err(ApiError::from_error_object(
            &error_core::helpers::validation_error(&format!("查询向量维度 {} 超过上限 4096", req.query_vec.len()), "vector_search"),
        ));
    }
    const MAX_K: u32 = 1000;
    if req.k == 0 || req.k > MAX_K {
        return Err(ApiError::from_error_object(
            &error_core::helpers::validation_error(&format!("k 值须在 1..={MAX_K} 范围内，当前: {}", req.k), "vector_search"),
        ));
    }
    let start = std::time::Instant::now();
    let results = state.vm.vector_search(&req.query_vec, req.k)?;
    let query_time_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let total = results.len();
    let response = VectorSearchResponse {
        results: results
            .into_iter()
            .enumerate()
            .map(|(rank, (block, distance))| VectorSearchResultItem {
                block,
                distance,
                rank: rank + 1,
            })
            .collect(),
        total,
        query_time_ms,
    };
    Ok(Json(ApiResponse::success(response)))
}

/// 全文搜索
///
/// GET /api/v1/search/fulltext?q=keyword&limit=20
///
/// # Errors
///
/// 当搜索参数无效或数据库查询失败时返回错误。
#[tracing::instrument(skip(state), fields(
    operation = "full_text_search",
    query = %params.q,
))]
pub async fn full_text_search(
    Query(params): Query<FullTextSearchQuery>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<Block>>>, ApiError> {
    params.validate()?;
    let results = state.vm.full_text_search(&params.q, params.limit())?;
    Ok(Json(ApiResponse::success(results)))
}

// ========== 系统端点 ==========

/// 健康检查端点
///
/// GET /healthz
#[tracing::instrument(fields(operation = "health_check"))]
pub async fn health_check() -> Json<HealthStatus> {
    Json(HealthStatus::healthy())
}

// ========== 辅助函数 ==========

fn detect_source_type(path: &str) -> SourceType {
    let ext = StdPath::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "md" | "markdown" => SourceType::Markdown,
        "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h" => SourceType::Code,
        _ => SourceType::Plain,
    }
}

fn blake3_hash(content: &str) -> String {
    let hash = blake3::hash(content.as_bytes());
    hash.to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<AppState>();
    }

    #[test]
    fn test_upload_request_deserialization() {
        let json = r#"{"path":"/documents/test.md","title":"测试文档","content":"Hello World"}"#;

        let req: UploadDocumentRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.path, "/documents/test.md");
        assert_eq!(req.title.as_deref(), Some("测试文档"));
        assert_eq!(req.content, "Hello World");
    }

    #[test]
    fn test_trace_references_query_defaults() {
        let query: TraceReferencesQuery = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(query.ref_type.is_none());
        assert!(query.max_depth.is_none());
    }

    #[test]
    fn test_full_text_search_query_validation() {
        let json = r#"{"q": "Rust 编程", "limit": 50}"#;
        let params: FullTextSearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(params.q, "Rust 编程");
        assert_eq!(params.limit(), 50);
    }

    #[tokio::test]
    async fn test_health_check_returns_healthy() {
        let response = health_check().await;
        assert_eq!(response.status, "healthy");
        assert!(!response.version.is_empty());
        assert!(!response.timestamp.is_empty());
    }

    #[test]
    fn test_detect_source_type_markdown() {
        assert_eq!(detect_source_type("doc.md"), SourceType::Markdown);
        assert_eq!(detect_source_type("doc.markdown"), SourceType::Markdown);
    }

    #[test]
    fn test_detect_source_type_code() {
        assert_eq!(detect_source_type("main.rs"), SourceType::Code);
        assert_eq!(detect_source_type("app.py"), SourceType::Code);
        assert_eq!(detect_source_type("index.js"), SourceType::Code);
    }

    #[test]
    fn test_detect_source_type_plain() {
        assert_eq!(detect_source_type("readme.txt"), SourceType::Plain);
        assert_eq!(detect_source_type("data.csv"), SourceType::Plain);
    }

    #[test]
    fn test_blake3_hash_length() {
        let hash = blake3_hash("test content");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_blake3_hash_deterministic() {
        let h1 = blake3_hash("hello");
        let h2 = blake3_hash("hello");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_blake3_hash_different_inputs() {
        let h1 = blake3_hash("hello");
        let h2 = blake3_hash("world");
        assert_ne!(h1, h2);
    }
}
