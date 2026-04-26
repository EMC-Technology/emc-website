//! 事件类型定义
//!
//! 本模块定义事件驱动架构的核心类型体系：
//! - `SystemEvent`: 所有系统事件的统一 trait
//! - `KnowledgeEvent`: 领域事件枚举，覆盖所有业务场景
//! - 各具体事件结构体：Document/Node/Edge/Search/Embedding 等

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 所有系统事件的统一 trait
///
/// 实现此 trait 的类型可作为事件总线的消息载体。
/// 约束要求确保事件可跨线程安全传递、可序列化持久化，
/// 可通过日志与 metrics 进行观测。
pub trait SystemEvent: Send + Sync + Clone + Serialize + Deserialize<'static> + std::fmt::Debug {
    /// 事件唯一标识符（UUID v4）
    fn event_id(&self) -> Uuid;

    /// 事件发生时间戳（UTC）
    fn timestamp(&self) -> DateTime<Utc>;

    /// 事件类型名称（用于路由和过滤）
    ///
    /// 返回静态字符串，格式为 `domain.action`，如 `"document.ingested"`。
    fn event_type(&self) -> &'static str;

    /// 事件来源服务/模块名称
    fn source(&self) -> &str;

    /// 事件版本号（用于 Schema 演进与兼容性检查）
    fn version(&self) -> u32;
}

// ============================================================================
// 领域事件枚举
// ============================================================================

/// 领域事件枚举 —— 覆盖知识图谱系统的所有业务场景。
///
/// 使用 `#[serde(tag = "type")]` 自描述序列化，便于：
/// - 事件持久化后的类型自身标识
/// - 跨服务边界传输时的自动路由
/// - 事件溯源（Event Sourcing）回放
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum KnowledgeEvent {
    // ---------------------------------------------------------------------
    // 文档生命周期事件
    // ---------------------------------------------------------------------

    /// 文档已摄入（文件读取完成，哈希计算完毕）
    DocumentIngested(DocumentIngestedEvent),

    /// 文档已解析（分块/AST/Token 提取完成）
    DocumentParsed(DocumentParsedEvent),

    /// 文档已索引（搜索索引构建完成）
    DocumentIndexed(DocumentIndexedEvent),

    /// 文档已删除（含级联清理信息）
    DocumentDeleted(DocumentDeletedEvent),

    // ---------------------------------------------------------------------
    // 知识节点操作事件
    // ---------------------------------------------------------------------

    /// 知识节点已创建
    NodeCreated(NodeCreatedEvent),

    /// 知识节点已更新
    NodeUpdated(NodeUpdatedEvent),

    /// 知识节点已删除
    NodeDeleted(NodeDeletedEvent),

    /// 知识节点间已建立链接
    NodeLinked(NodeLinkedEvent),

    // ---------------------------------------------------------------------
    // 关系边操作事件
    // ---------------------------------------------------------------------

    /// 关系边已创建
    EdgeCreated(EdgeCreatedEvent),

    /// 关系边已删除
    EdgeDeleted(EdgeDeletedEvent),

    // ---------------------------------------------------------------------
    // 搜索与查询事件
    // ---------------------------------------------------------------------

    /// 搜索请求已执行
    SearchPerformed(SearchPerformedEvent),

    /// 图查询已执行
    QueryExecuted(QueryExecutedEvent),

    // ---------------------------------------------------------------------
    // Embedding 操作事件
    // ---------------------------------------------------------------------

    /// 向量嵌入已生成
    EmbeddingGenerated(EmbeddingGeneratedEvent),

    /// 向量嵌入已缓存命中
    EmbeddingCached(EmbeddingCachedEvent),

    // ---------------------------------------------------------------------
    // 用户操作事件
    // ---------------------------------------------------------------------

    /// 用户操作记录
    UserAction(UserActionEvent),

    // ---------------------------------------------------------------------
    // 系统事件
    // ---------------------------------------------------------------------

    /// 系统健康检查
    SystemHealthCheck(SystemHealthEvent),
}

impl SystemEvent for KnowledgeEvent {
    fn event_id(&self) -> Uuid {
        match self {
            KnowledgeEvent::DocumentIngested(e) => e.id,
            KnowledgeEvent::DocumentParsed(e) => e.id,
            KnowledgeEvent::DocumentIndexed(e) => e.id,
            KnowledgeEvent::DocumentDeleted(e) => e.id,
            KnowledgeEvent::NodeCreated(e) => e.id,
            KnowledgeEvent::NodeUpdated(e) => e.id,
            KnowledgeEvent::NodeDeleted(e) => e.id,
            KnowledgeEvent::NodeLinked(e) => e.id,
            KnowledgeEvent::EdgeCreated(e) => e.id,
            KnowledgeEvent::EdgeDeleted(e) => e.id,
            KnowledgeEvent::SearchPerformed(e) => e.id,
            KnowledgeEvent::QueryExecuted(e) => e.id,
            KnowledgeEvent::EmbeddingGenerated(e) => e.id,
            KnowledgeEvent::EmbeddingCached(e) => e.id,
            KnowledgeEvent::UserAction(e) => e.id,
            KnowledgeEvent::SystemHealthCheck(e) => e.id,
        }
    }

    fn timestamp(&self) -> DateTime<Utc> {
        match self {
            KnowledgeEvent::DocumentIngested(e) => e.timestamp,
            KnowledgeEvent::DocumentParsed(e) => e.timestamp,
            KnowledgeEvent::DocumentIndexed(e) => e.timestamp,
            KnowledgeEvent::DocumentDeleted(e) => e.timestamp,
            KnowledgeEvent::NodeCreated(e) => e.timestamp,
            KnowledgeEvent::NodeUpdated(e) => e.timestamp,
            KnowledgeEvent::NodeDeleted(e) => e.timestamp,
            KnowledgeEvent::NodeLinked(e) => e.timestamp,
            KnowledgeEvent::EdgeCreated(e) => e.timestamp,
            KnowledgeEvent::EdgeDeleted(e) => e.timestamp,
            KnowledgeEvent::SearchPerformed(e) => e.timestamp,
            KnowledgeEvent::QueryExecuted(e) => e.timestamp,
            KnowledgeEvent::EmbeddingGenerated(e) => e.timestamp,
            KnowledgeEvent::EmbeddingCached(e) => e.timestamp,
            KnowledgeEvent::UserAction(e) => e.timestamp,
            KnowledgeEvent::SystemHealthCheck(e) => e.timestamp,
        }
    }

    fn event_type(&self) -> &'static str {
        match self {
            KnowledgeEvent::DocumentIngested(_) => "document.ingested",
            KnowledgeEvent::DocumentParsed(_) => "document.parsed",
            KnowledgeEvent::DocumentIndexed(_) => "document.indexed",
            KnowledgeEvent::DocumentDeleted(_) => "document.deleted",
            KnowledgeEvent::NodeCreated(_) => "node.created",
            KnowledgeEvent::NodeUpdated(_) => "node.updated",
            KnowledgeEvent::NodeDeleted(_) => "node.deleted",
            KnowledgeEvent::NodeLinked(_) => "node.linked",
            KnowledgeEvent::EdgeCreated(_) => "edge.created",
            KnowledgeEvent::EdgeDeleted(_) => "edge.deleted",
            KnowledgeEvent::SearchPerformed(_) => "search.performed",
            KnowledgeEvent::QueryExecuted(_) => "query.executed",
            KnowledgeEvent::EmbeddingGenerated(_) => "embedding.generated",
            KnowledgeEvent::EmbeddingCached(_) => "embedding.cached",
            KnowledgeEvent::UserAction(_) => "user.action",
            KnowledgeEvent::SystemHealthCheck(_) => "system.health_check",
        }
    }

    fn source(&self) -> &str {
        match self {
            KnowledgeEvent::DocumentIngested(e) => &e.source,
            KnowledgeEvent::DocumentParsed(e) => &e.source,
            KnowledgeEvent::DocumentIndexed(e) => &e.source,
            KnowledgeEvent::DocumentDeleted(e) => &e.source,
            KnowledgeEvent::NodeCreated(e) => &e.source,
            KnowledgeEvent::NodeUpdated(e) => &e.source,
            KnowledgeEvent::NodeDeleted(e) => &e.source,
            KnowledgeEvent::NodeLinked(e) => &e.source,
            KnowledgeEvent::EdgeCreated(e) => &e.source,
            KnowledgeEvent::EdgeDeleted(e) => &e.source,
            KnowledgeEvent::SearchPerformed(e) => &e.source,
            KnowledgeEvent::QueryExecuted(e) => &e.source,
            KnowledgeEvent::EmbeddingGenerated(e) => &e.source,
            KnowledgeEvent::EmbeddingCached(e) => &e.source,
            KnowledgeEvent::UserAction(e) => &e.source,
            KnowledgeEvent::SystemHealthCheck(e) => &e.source,
        }
    }

    fn version(&self) -> u32 {
        match self {
            KnowledgeEvent::DocumentIngested(e) => e.version,
            KnowledgeEvent::DocumentParsed(e) => e.version,
            KnowledgeEvent::DocumentIndexed(e) => e.version,
            KnowledgeEvent::DocumentDeleted(e) => e.version,
            KnowledgeEvent::NodeCreated(e) => e.version,
            KnowledgeEvent::NodeUpdated(e) => e.version,
            KnowledgeEvent::NodeDeleted(e) => e.version,
            KnowledgeEvent::NodeLinked(e) => e.version,
            KnowledgeEvent::EdgeCreated(e) => e.version,
            KnowledgeEvent::EdgeDeleted(e) => e.version,
            KnowledgeEvent::SearchPerformed(e) => e.version,
            KnowledgeEvent::QueryExecuted(e) => e.version,
            KnowledgeEvent::EmbeddingGenerated(e) => e.version,
            KnowledgeEvent::EmbeddingCached(e) => e.version,
            KnowledgeEvent::UserAction(e) => e.version,
            KnowledgeEvent::SystemHealthCheck(e) => e.version,
        }
    }
}

// ============================================================================
// 具体事件结构 —— 文档生命周期
// ============================================================================

/// 文档摄入事件
///
/// 当原始文件被系统读取并完成 哈希计算后触发。
/// 此事件标志着文档进入处理流水线的入口点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentIngestedEvent {
    pub id: Uuid,
    pub document_id: String,
    pub file_path: String,
    pub file_size: u64,
    pub source_type: String,
    pub hash: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl DocumentIngestedEvent {
    pub fn new(
        document_id: impl Into<String>,
        file_path: impl Into<String>,
        file_size: u64,
        source_type: impl Into<String>,
        hash: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            document_id: document_id.into(),
            file_path: file_path.into(),
            file_size,
            source_type: source_type.into(),
            hash: hash.into(),
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

/// 文档解析完成事件
///
/// 当文档经过 处理（分词/ 解析/Token 提取）后触发。
/// 包含解析产出的统计信息，供下游消费者决策。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentParsedEvent {
    pub id: Uuid,
    pub document_id: String,
    pub block_count: usize,
    pub token_count: usize,
    pub parse_duration_ms: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl DocumentParsedEvent {
    pub fn new(
        document_id: impl Into<String>,
        block_count: usize,
        token_count: usize,
        parse_duration_ms: u64,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            document_id: document_id.into(),
            block_count,
            token_count,
            parse_duration_ms,
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

/// 文档索引完成事件
///
/// 当文档的所有 已写入 倒排索引后触发。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentIndexedEvent {
    pub id: Uuid,
    pub document_id: String,
    pub indexed_block_count: usize,
    pub index_build_duration_ms: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl DocumentIndexedEvent {
    pub fn new(
        document_id: impl Into<String>,
        indexed_block_count: usize,
        index_build_duration_ms: u64,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            document_id: document_id.into(),
            indexed_block_count,
            index_build_duration_ms,
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

/// 文档删除事件
///
/// 当文档及其所有关联数据（Block/Token/Reference）被级联删除后触发。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDeletedEvent {
    pub id: Uuid,
    pub document_id: String,
    pub cascaded_blocks: usize,
    pub cascaded_tokens: usize,
    pub cascaded_references: usize,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl DocumentDeletedEvent {
    pub fn new(
        document_id: impl Into<String>,
        cascaded_blocks: usize,
        cascaded_tokens: usize,
        cascaded_references: usize,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            document_id: document_id.into(),
            cascaded_blocks,
            cascaded_tokens,
            cascaded_references,
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

// ============================================================================
// 具体事件结构 —— 知识节点操作
// ============================================================================

/// 知识节点创建事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCreatedEvent {
    pub id: Uuid,
    pub node_id: String,
    pub node_type: String,
    pub document_id: Option<String>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl NodeCreatedEvent {
    pub fn new(
        node_id: impl Into<String>,
        node_type: impl Into<String>,
        document_id: Option<impl Into<String>>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id: node_id.into(),
            node_type: node_type.into(),
            document_id: document_id.map(|s| s.into()),
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

/// 知识节点更新事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeUpdatedEvent {
    pub id: Uuid,
    pub node_id: String,
    pub updated_fields: Vec<String>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl NodeUpdatedEvent {
    pub fn new(
        node_id: impl Into<String>,
        updated_fields: Vec<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id: node_id.into(),
            updated_fields,
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

/// 知识节点删除事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDeletedEvent {
    pub id: Uuid,
    pub node_id: String,
    pub node_type: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl NodeDeletedEvent {
    pub fn new(
        node_id: impl Into<String>,
        node_type: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id: node_id.into(),
            node_type: node_type.into(),
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

/// 知识节点链接事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLinkedEvent {
    pub id: Uuid,
    pub from_node_id: String,
    pub to_node_id: String,
    pub relation_type: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl NodeLinkedEvent {
    pub fn new(
        from_node_id: impl Into<String>,
        to_node_id: impl Into<String>,
        relation_type: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_node_id: from_node_id.into(),
            to_node_id: to_node_id.into(),
            relation_type: relation_type.into(),
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

// ============================================================================
// 具体事件结构 —— 关系边操作
// ============================================================================

/// 关系边创建事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeCreatedEvent {
    pub id: Uuid,
    pub edge_id: String,
    pub ref_type: String,
    pub from_id: String,
    pub to_id: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl EdgeCreatedEvent {
    pub fn new(
        edge_id: impl Into<String>,
        ref_type: impl Into<String>,
        from_id: impl Into<String>,
        to_id: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            edge_id: edge_id.into(),
            ref_type: ref_type.into(),
            from_id: from_id.into(),
            to_id: to_id.into(),
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

/// 关系边删除事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDeletedEvent {
    pub id: Uuid,
    pub edge_id: String,
    pub ref_type: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl EdgeDeletedEvent {
    pub fn new(
        edge_id: impl Into<String>,
        ref_type: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            edge_id: edge_id.into(),
            ref_type: ref_type.into(),
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

// ============================================================================
// 具体事件结构 —— 搜索与查询
// ============================================================================

/// 搜索执行事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPerformedEvent {
    pub id: Uuid,
    pub query: String,
    pub result_count: usize,
    pub duration_ms: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl SearchPerformedEvent {
    pub fn new(
        query: impl Into<String>,
        result_count: usize,
        duration_ms: u64,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            query: query.into(),
            result_count,
            duration_ms,
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

/// 图查询执行事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryExecutedEvent {
    pub id: Uuid,
    pub query_type: String,
    pub query_body: String,
    pub result_count: usize,
    pub duration_ms: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl QueryExecutedEvent {
    pub fn new(
        query_type: impl Into<String>,
        query_body: impl Into<String>,
        result_count: usize,
        duration_ms: u64,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            query_type: query_type.into(),
            query_body: query_body.into(),
            result_count,
            duration_ms,
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

// ============================================================================
// 具体事件结构 —— Embedding 操作
// ============================================================================

/// 向量嵌入生成事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingGeneratedEvent {
    pub id: Uuid,
    pub entity_id: String,
    pub entity_type: String,
    pub embedding_dim: usize,
    pub model_name: String,
    pub generation_duration_ms: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl EmbeddingGeneratedEvent {
    pub fn new(
        entity_id: impl Into<String>,
        entity_type: impl Into<String>,
        embedding_dim: usize,
        model_name: impl Into<String>,
        generation_duration_ms: u64,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            entity_id: entity_id.into(),
            entity_type: entity_type.into(),
            embedding_dim,
            model_name: model_name.into(),
            generation_duration_ms,
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

/// 向量嵌入缓存命中事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingCachedEvent {
    pub id: Uuid,
    pub entity_id: String,
    pub entity_type: String,
    pub cache_key: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl EmbeddingCachedEvent {
    pub fn new(
        entity_id: impl Into<String>,
        entity_type: impl Into<String>,
        cache_key: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            entity_id: entity_id.into(),
            entity_type: entity_type.into(),
            cache_key: cache_key.into(),
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

// ============================================================================
// 具体事件结构 —— 用户操作
// ============================================================================

/// 用户操作事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserActionEvent {
    pub id: Uuid,
    pub user_id: String,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl UserActionEvent {
    pub fn new(
        user_id: impl Into<String>,
        action: impl Into<String>,
        resource_type: Option<impl Into<String>>,
        resource_id: Option<impl Into<String>>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            action: action.into(),
            resource_type: resource_type.map(|s| s.into()),
            resource_id: resource_id.map(|s| s.into()),
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

// ============================================================================
// 具体事件结构 —— 系统事件
// ============================================================================

/// 系统健康检查事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthEvent {
    pub id: Uuid,
    pub component: String,
    pub status: String,
    pub details: Option<String>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub source: String,
}

impl SystemHealthEvent {
    pub fn new(
        component: impl Into<String>,
        status: impl Into<String>,
        details: Option<impl Into<String>>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            component: component.into(),
            status: status.into(),
            details: details.map(|s| s.into()),
            timestamp: Utc::now(),
            version: 1,
            source: source.into(),
        }
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_ingested_event_serialization() {
        let event = DocumentIngestedEvent::new(
            "doc-001",
            "/test.md",
            1024,
            "Markdown",
            &"a".repeat(64),
            "file-ingester",
        );

        let json = serde_json::to_string(&event).expect("序列化失败");
        let de_event: DocumentIngestedEvent =
            serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(event.document_id, de_event.document_id);
        assert_eq!(event.file_path, de_event.file_path);
        assert_eq!(event.file_size, de_event.file_size);
        assert_eq!(event.version, 1);
    }

    #[test]
    fn test_knowledge_event_system_trait() {
        let event = KnowledgeEvent::DocumentIngested(DocumentIngestedEvent::new(
            "doc-002",
            "/api.rs",
            2048,
            "Code",
            &"b".repeat(64),
            "parser",
        ));

        assert_eq!(event.event_type(), "document.ingested");
        assert_eq!(event.source(), "parser");
        assert_eq!(event.version(), 1);
        assert!(event.timestamp() <= Utc::now());
    }

    #[test]
    fn test_knowledge_event_tagged_serialization() {
        let events = vec![
            KnowledgeEvent::DocumentParsed(DocumentParsedEvent::new("doc-003", 10, 200, 50, "parser")),
            KnowledgeEvent::NodeCreated(NodeCreatedEvent::new("node-001", "Block", Some("doc-003"), "graph-builder")),
            KnowledgeEvent::EmbeddingGenerated(EmbeddingGeneratedEvent::new(
                "block-001", "Block", 1536, "text-embedding-ada-002", 120, "embedding-service",
            )),
        ];

        for event in &events {
            let json = serde_json::to_string(event).expect("序列化失败");
            assert!(json.contains("\"type\":"), "序列化结果应包含 type 标签");
            let de_event: KnowledgeEvent =
                serde_json::from_str(&json).expect("反序列化失败");
            assert_eq!(event.event_type(), de_event.event_type());
        }
    }

    #[test]
    fn test_all_event_types_have_valid_metadata() {
        let event_variants: Vec<KnowledgeEvent> = vec![
            KnowledgeEvent::DocumentIngested(DocumentIngestedEvent::new("d", "/", 0, "", "", "s")),
            KnowledgeEvent::DocumentParsed(DocumentParsedEvent::new("d", 0, 0, 0, "s")),
            KnowledgeEvent::DocumentIndexed(DocumentIndexedEvent::new("d", 0, 0, "s")),
            KnowledgeEvent::DocumentDeleted(DocumentDeletedEvent::new("d", 0, 0, 0, "s")),
            KnowledgeEvent::NodeCreated(NodeCreatedEvent::new("n", "T", None::<String>, "s")),
            KnowledgeEvent::NodeUpdated(NodeUpdatedEvent::new("n", vec![], "s")),
            KnowledgeEvent::NodeDeleted(NodeDeletedEvent::new("n", "T", "s")),
            KnowledgeEvent::NodeLinked(NodeLinkedEvent::new("a", "b", "r", "s")),
            KnowledgeEvent::EdgeCreated(EdgeCreatedEvent::new("e", "r", "a", "b", "s")),
            KnowledgeEvent::EdgeDeleted(EdgeDeletedEvent::new("e", "r", "s")),
            KnowledgeEvent::SearchPerformed(SearchPerformedEvent::new("q", 0, 0, "s")),
            KnowledgeEvent::QueryExecuted(QueryExecutedEvent::new("t", "b", 0, 0, "s")),
            KnowledgeEvent::EmbeddingGenerated(EmbeddingGeneratedEvent::new("e", "t", 0, "m", 0, "s")),
            KnowledgeEvent::EmbeddingCached(EmbeddingCachedEvent::new("e", "t", "k", "s")),
            KnowledgeEvent::UserAction(UserActionEvent::new("u", "a", None::<String>, None::<String>, "s")),
            KnowledgeEvent::SystemHealthCheck(SystemHealthEvent::new("c", "ok", None::<String>, "s")),
        ];

        for event in event_variants {
            assert!(!event.event_id().is_nil(), "事件 ID 不应为 nil");
            assert!(!event.event_type().is_empty(), "事件类型不应为空");
            assert!(!event.source().is_empty(), "事件来源不应为空");
            assert_eq!(event.version(), 1, "默认版本号应为 1");
        }
    }

    #[test]
    fn test_timestamp_chrono_milliseconds_roundtrip() {
        let event = NodeCreatedEvent::new("node-ts", "Token", None::<String>, "test");
        let original_ts = event.timestamp;

        let json = serde_json::to_string(&event).expect("序列化失败");
        let de_event: NodeCreatedEvent = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(original_ts, de_event.timestamp, "时间戳毫秒精度序列化应无损");
    }
}
