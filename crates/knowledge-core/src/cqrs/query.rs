use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use super::aggregate::DocumentStatus;

/// Query trait - 表示读操作的请求
///
/// 所有查询（Query）必须实现此 trait，用于 CQRS 模式中的读操作。
/// 查询是只读操作，不会修改系统状态，可以针对不同场景优化读模型。
///
/// # 类型参数
///
/// - `Result`: 查询结果的类型
///
/// # 示例
///
/// ```rust
/// use knowledge_core::cqrs::Query;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Debug, Serialize)]
/// struct MyQuery {
///     id: String,
/// }
///
/// impl Query for MyQuery {
///     type Result = String;
/// }
/// ```
pub trait Query: Send + Sync + Serialize + std::fmt::Debug {
    /// 查询结果的类型
    type Result: Send + Sync;
}

/// 文档只读视图模型
///
/// 用于查询响应中的文档摘要信息，不包含完整内容。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentView {
    /// 文档唯一标识符
    pub id: String,
    /// 文档标题
    pub title: String,
    /// 内容 MIME 类型
    pub content_type: String,
    /// 文档状态
    pub status: DocumentStatus,
    /// 当前版本号
    pub version: u64,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
    /// 关联节点数量
    pub node_count: usize,
    /// 文档完整内容（仅在请求时包含）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// 节点只读视图模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeView {
    /// 节点唯一标识符
    pub id: String,
    /// 所属文档 ID
    pub document_id: String,
    /// 节点类型（如 `Heading`、`Paragraph`、`Code`）
    pub node_type: String,
    /// 节点文本内容
    pub content: String,
    /// 节点在源文件中的位置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<NodePositionView>,
}

/// 节点位置视图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePositionView {
    /// 起始行号
    pub start_line: u32,
    /// 结束行号
    pub end_line: u32,
}

/// 搜索结果条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    /// 匹配文档/节点 ID
    pub id: String,
    /// 匹配文档标题
    pub title: String,
    /// 相似度得分（0.0 ~ 1.0）
    pub score: f64,
    /// 高亮摘要片段
    pub highlight: Option<String>,
    /// 内容类型
    pub content_type: String,
}

/// 图谱统计视图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatsView {
    /// 文档总数
    pub total_documents: usize,
    /// 节点总数
    pub total_nodes: usize,
    /// 边（引用）总数
    pub total_edges: usize,
    /// 最后更新时间
    pub last_updated: DateTime<Utc>,
}

/// 文档列表条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentListItem {
    /// 文档 ID
    pub id: String,
    /// 文档标题
    pub title: String,
    /// 来源类型（`Markdown`、`Code`、`Plain`）
    pub source_type: String,
    /// 当前版本号
    pub version: u64,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
}

/// 获取文档查询
///
/// 根据文档 ID 获取文档详情，可按需包含节点和块信息。
#[derive(Debug, Serialize, Deserialize)]
pub struct GetDocumentQuery {
    /// 目标文档 ID
    pub document_id: String,
    /// 是否包含关联节点
    pub include_nodes: bool,
    /// 是否包含块摘要
    pub include_blocks: bool,
}

impl Query for GetDocumentQuery {
    type Result = Option<DocumentDetailView>;
}

/// 文档详情视图（含关联节点和块）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDetailView {
    /// 文档基本信息
    pub document: DocumentView,
    /// 关联的节点列表
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<NodeView>,
    /// 关联的块摘要列表
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<BlockSummaryView>,
}

/// 块摘要视图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSummaryView {
    /// 块 ID
    pub id: String,
    /// 块类型
    pub block_type: String,
    /// 起始行号
    pub start_line: u32,
    /// 结束行号
    pub end_line: u32,
}

/// 搜索文档查询
///
/// 基于关键词和可选过滤器执行全文/语义搜索。
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchDocumentsQuery {
    /// 搜索关键词
    pub query_string: String,
    /// 返回结果上限
    pub limit: usize,
    /// 分页偏移量
    pub offset: usize,
    /// 可选的搜索过滤器
    pub filters: Option<SearchFilters>,
}

impl Query for SearchDocumentsQuery {
    type Result = SearchResultView;
}

/// 搜索结果视图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultView {
    /// 匹配的结果条目
    pub items: Vec<SearchResultItem>,
    /// 总匹配数（用于分页）
    pub total: usize,
    /// 查询耗时（毫秒）
    pub query_time_ms: u64,
}

/// 搜索过滤器
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchFilters {
    /// 限定内容类型列表
    pub content_types: Option<Vec<String>>,
    /// 起始日期过滤
    pub date_from: Option<DateTime<Utc>>,
    /// 结束日期过滤
    pub date_to: Option<DateTime<Utc>>,
    /// 最低相似度得分阈值
    pub min_score: Option<f64>,
}

/// 获取节点查询
///
/// 根据节点 ID 获取节点详情，可按需包含出入边引用。
#[derive(Debug, Serialize, Deserialize)]
pub struct GetNodeQuery {
    /// 目标节点 ID
    pub node_id: String,
    /// 是否包含出边和入边引用
    pub include_references: bool,
}

impl Query for GetNodeQuery {
    type Result = Option<NodeDetailView>;
}

/// 节点详情视图（含出入边引用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDetailView {
    /// 节点基本信息
    pub node: NodeView,
    /// 出边引用列表
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub outgoing_refs: Vec<ReferenceSummaryView>,
    /// 入边引用列表
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub incoming_refs: Vec<ReferenceSummaryView>,
}

/// 引用摘要视图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceSummaryView {
    /// 引用类型（如 `Usage`、`Definition`）
    pub ref_type: String,
    /// 目标节点 ID
    pub target_id: String,
    /// 目标节点内容预览
    pub target_content_preview: String,
}

/// 列出文档查询
///
/// 分页列出文档，支持排序和过滤。
#[derive(Debug, Serialize, Deserialize)]
pub struct ListDocumentsQuery {
    /// 每页条目数
    pub limit: usize,
    /// 分页偏移量
    pub offset: usize,
    /// 排序字段
    pub sort_by: SortField,
    /// 排序方向
    pub sort_order: SortOrder,
    /// 可选的过滤条件
    pub filters: Option<ListDocumentsFilter>,
}

impl Query for ListDocumentsQuery {
    type Result = ListView<DocumentListItem>;
}

/// 通用分页列表视图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListView<T> {
    /// 当前页条目
    pub items: Vec<T>,
    /// 总条目数
    pub total: usize,
    /// 是否还有更多条目
    pub has_more: bool,
}

/// 排序字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortField {
    /// 按更新时间排序
    UpdatedAt,
    /// 按创建时间排序
    CreatedAt,
    /// 按标题排序
    Title,
    /// 按版本号排序
    Version,
}

/// 排序方向
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    /// 升序
    Asc,
    /// 降序
    Desc,
}

/// 文档列表过滤条件
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListDocumentsFilter {
    /// 限定来源类型
    pub source_type: Option<String>,
    /// 限定文档状态
    pub status: Option<DocumentStatus>,
    /// 标题模糊匹配
    pub title_contains: Option<String>,
}

/// 获取图谱统计查询
///
/// 返回知识图谱的全局统计信息。
#[derive(Debug, Serialize, Deserialize)]
pub struct GetGraphStatsQuery {}

impl Query for GetGraphStatsQuery {
    type Result = GraphStatsView;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn create_test_document_view() -> DocumentView {
        DocumentView {
            id: "doc_001".to_string(),
            title: "Test Document".to_string(),
            content_type: "markdown".to_string(),
            status: DocumentStatus::Published,
            version: 5,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            node_count: 10,
            content: Some("# Hello".to_string()),
        }
    }

    #[test]
    fn test_query_trait_implementation() {
        let query = GetDocumentQuery {
            document_id: "doc_001".to_string(),
            include_nodes: true,
            include_blocks: false,
        };

        assert_eq!(query.document_id, "doc_001");
        assert!(query.include_nodes);
        assert!(!query.include_blocks);
    }

    #[test]
    fn test_get_document_query_serialization() {
        let query = GetDocumentQuery {
            document_id: "doc_002".to_string(),
            include_nodes: true,
            include_blocks: true,
        };

        let json = serde_json::to_string(&query).expect("序列化失败");
        let deserialized: GetDocumentQuery =
            serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(deserialized.document_id, query.document_id);
        assert_eq!(deserialized.include_nodes, query.include_nodes);
    }

    #[test]
    fn test_search_documents_query_with_filters() {
        let query = SearchDocumentsQuery {
            query_string: "Rust async".to_string(),
            limit: 20,
            offset: 0,
            filters: Some(SearchFilters {
                content_types: Some(vec!["markdown".to_string()]),
                date_from: None,
                date_to: None,
                min_score: Some(0.5),
            }),
        };

        let json = serde_json::to_value(&query).expect("序列化失败");
        assert!(json.get("filters").is_some());

        let filters = json.get("filters").unwrap();
        assert!(filters.get("content_types").is_some());
        assert!(filters.get("min_score").is_some());
    }

    #[test]
    fn test_list_documents_query_pagination() {
        let query = ListDocumentsQuery {
            limit: 10,
            offset: 20,
            sort_by: SortField::UpdatedAt,
            sort_order: SortOrder::Desc,
            filters: Some(ListDocumentsFilter {
                source_type: Some("Markdown".to_string()),
                ..Default::default()
            }),
        };

        assert_eq!(query.limit, 10);
        assert_eq!(query.offset, 20);
        matches!(query.sort_by, SortField::UpdatedAt);
        matches!(query.sort_order, SortOrder::Desc);
    }

    #[test]
    fn test_document_view_serialization_skips_none_fields() {
        let view = DocumentView {
            id: "doc_003".to_string(),
            title: "No Content Doc".to_string(),
            content_type: "plain".to_string(),
            status: DocumentStatus::Draft,
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            node_count: 0,
            content: None,
        };

        let json = serde_json::to_value(&view).expect("序列化失败");
        assert!(json.get("content").is_none());
    }

    #[test]
    fn test_search_result_item_structure() {
        let item = SearchResultItem {
            id: "doc_004".to_string(),
            title: "Async Rust Guide".to_string(),
            score: 0.95,
            highlight: Some("**async** is powerful".to_string()),
            content_type: "markdown".to_string(),
        };

        assert!((item.score - 0.95).abs() < f64::EPSILON);
        assert!(item.highlight.is_some());
    }

    #[test]
    fn test_graph_stats_view_completeness() {
        let stats = GraphStatsView {
            total_documents: 100,
            total_nodes: 1000,
            total_edges: 2500,
            last_updated: Utc::now(),
        };

        assert_eq!(stats.total_documents, 100);
        assert_eq!(stats.total_nodes, 1000);
        assert_eq!(stats.total_edges, 2500);
    }

    #[test]
    fn test_node_view_with_optional_position() {
        let with_pos = NodeView {
            id: "node_001".to_string(),
            document_id: "doc_001".to_string(),
            node_type: "Heading".to_string(),
            content: "# Introduction".to_string(),
            position: Some(NodePositionView {
                start_line: 1,
                end_line: 1,
            }),
        };

        let without_pos = NodeView {
            id: "node_002".to_string(),
            document_id: "doc_001".to_string(),
            node_type: "Paragraph".to_string(),
            content: "Some text".to_string(),
            position: None,
        };

        let json_with = serde_json::to_value(&with_pos).expect("序列化失败");
        let json_without = serde_json::to_value(&without_pos).expect("序列化失败");

        assert!(json_with.get("position").is_some());
        assert!(json_without.get("position").is_none());
    }

    #[tokio::test]
    async fn test_query_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<GetDocumentQuery>();
        assert_send_sync::<SearchDocumentsQuery>();
        assert_send_sync::<GetNodeQuery>();
        assert_send_sync::<ListDocumentsQuery>();
        assert_send_sync::<GetGraphStatsQuery>();
    }
}
