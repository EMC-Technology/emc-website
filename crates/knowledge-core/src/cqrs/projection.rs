//! 投影（Projection）：CQRS 查询端的事件流到读模型转换
//!
//! # 设计原则
//!
//! 1. **确定性**：投影函数必须是纯函数——相同的事件序列必须产生相同的读模型
//! 2. **幂等性**：对同一事件重复应用投影不会改变结果
//! 3. **可重放性**：从任意时间点重放事件流可以重建读模型
//!
//! # 参考
//!
//! - CQRS Pattern: <https://martinfowler.com/bliki/CQRS.html>
//! - Event Sourcing: <https://martinfowler.com/eaaDev/EventSourcing.html>

use crate::Result;
use crate::cqrs::aggregate::DocumentStatus;
use crate::cqrs::event_store::{AggregateType, EventType, StoredEvent};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, instrument, warn};

/// 投影 trait —— 将事件流转换为读模型
///
/// 每个投影负责维护一种读模型视图。投影函数必须是确定性的：
/// 给定相同的事件序列，必须产生相同的读模型状态。
#[async_trait]
pub trait Projection: Send + Sync {
    /// 投影名称（用于标识和日志）
    fn name(&self) -> &'static str;

    /// 此投影关注的聚合根类型
    fn interested_in(&self) -> Vec<AggregateType>;

    /// 此投影关心的事件类型
    fn event_types(&self) -> Vec<EventType>;

    /// 处理单个事件，更新投影状态
    ///
    /// # Errors
    ///
    /// 当事件处理失败时返回错误
    async fn handle(&self, event: &StoredEvent) -> Result<()>;

    /// 重建投影（从事件流重放）
    ///
    /// # Errors
    ///
    /// 当重建过程失败时返回错误
    async fn rebuild(&self, events: &[StoredEvent]) -> Result<()> {
        info!(
            projection = self.name(),
            event_count = events.len(),
            "开始重建投影"
        );
        for event in events {
            self.handle(event).await?;
        }
        info!(projection = self.name(), "投影重建完成");
        Ok(())
    }
}

/// 文档摘要读模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummaryView {
    /// 文档 ID
    pub document_id: String,
    /// 文档标题
    pub title: String,
    /// 文档状态
    pub status: DocumentStatus,
    /// 块数量
    pub block_count: usize,
    /// 最后更新时间
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 节点统计读模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatsView {
    /// 节点总数
    pub total_nodes: usize,
    /// 按类型分组的节点数
    pub nodes_by_type: HashMap<String, usize>,
    /// 边总数
    pub total_edges: usize,
    /// 按关系类型分组的边数
    pub edges_by_type: HashMap<String, usize>,
}

/// 文档投影 —— 维护文档摘要视图
pub struct DocumentProjection {
    views: tokio::sync::RwLock<HashMap<String, DocumentSummaryView>>,
}

impl DocumentProjection {
    /// 创建新的文档投影实例
    #[must_use]
    pub fn new() -> Self {
        Self {
            views: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// 获取文档摘要
    pub async fn get(&self, document_id: &str) -> Option<DocumentSummaryView> {
        let views = self.views.read().await;
        views.get(document_id).cloned()
    }

    /// 获取所有文档摘要
    pub async fn list(&self) -> Vec<DocumentSummaryView> {
        let views = self.views.read().await;
        views.values().cloned().collect()
    }
}

impl Default for DocumentProjection {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Projection for DocumentProjection {
    fn name(&self) -> &'static str {
        "DocumentProjection"
    }

    fn interested_in(&self) -> Vec<AggregateType> {
        vec![AggregateType::Document]
    }

    fn event_types(&self) -> Vec<EventType> {
        vec![
            EventType::DocumentIngested,
            EventType::DocumentIndexed,
            EventType::DocumentDeleted,
        ]
    }

    async fn handle(&self, event: &StoredEvent) -> Result<()> {
        let aggregate_id = &event.aggregate_id;
        match event.event_type {
            EventType::DocumentIngested | EventType::DocumentIndexed => {
                debug!(document_id = %aggregate_id, "文档投影：更新文档摘要");
                let title = event
                    .data
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(无标题)")
                    .to_string();

                let status = event
                    .data
                    .get("status")
                    .and_then(|v| serde_json::from_value::<DocumentStatus>(v.clone()).ok())
                    .unwrap_or_default();

                let block_count = usize::try_from(
                    event
                        .data
                        .get("block_count")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0),
                )
                .unwrap_or(0);

                let mut views = self.views.write().await;
                views.insert(
                    aggregate_id.clone(),
                    DocumentSummaryView {
                        document_id: aggregate_id.clone(),
                        title,
                        status,
                        block_count,
                        updated_at: Some(event.timestamp),
                    },
                );
            }
            EventType::DocumentDeleted => {
                debug!(document_id = %aggregate_id, "文档投影：移除文档摘要");
                let mut views = self.views.write().await;
                views.remove(aggregate_id);
            }
            _ => {
                warn!(
                    event_type = %event.event_type,
                    "文档投影收到不关心的事件类型，已忽略"
                );
            }
        }
        Ok(())
    }
}

/// 节点统计投影 —— 维护图结构统计视图
pub struct NodeStatsProjection {
    stats: tokio::sync::RwLock<NodeStatsView>,
}

impl NodeStatsProjection {
    /// 创建新的节点统计投影实例
    #[must_use]
    pub fn new() -> Self {
        Self {
            stats: tokio::sync::RwLock::new(NodeStatsView {
                total_nodes: 0,
                nodes_by_type: HashMap::new(),
                total_edges: 0,
                edges_by_type: HashMap::new(),
            }),
        }
    }

    /// 获取当前统计
    pub async fn get_stats(&self) -> NodeStatsView {
        self.stats.read().await.clone()
    }
}

impl Default for NodeStatsProjection {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Projection for NodeStatsProjection {
    fn name(&self) -> &'static str {
        "NodeStatsProjection"
    }

    fn interested_in(&self) -> Vec<AggregateType> {
        vec![AggregateType::Node, AggregateType::Edge]
    }

    fn event_types(&self) -> Vec<EventType> {
        vec![
            EventType::NodeCreated,
            EventType::NodeDeleted,
            EventType::EdgeCreated,
            EventType::EdgeDeleted,
        ]
    }

    async fn handle(&self, event: &StoredEvent) -> Result<()> {
        let mut stats = self.stats.write().await;
        match event.event_type {
            EventType::NodeCreated => {
                stats.total_nodes = stats.total_nodes.saturating_add(1);
                let node_type = event
                    .data
                    .get("entity_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                *stats.nodes_by_type.entry(node_type).or_insert(0) += 1;
            }
            EventType::NodeDeleted => {
                stats.total_nodes = stats.total_nodes.saturating_sub(1);
            }
            EventType::EdgeCreated => {
                stats.total_edges = stats.total_edges.saturating_add(1);
                let edge_type = event
                    .data
                    .get("relation_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                *stats.edges_by_type.entry(edge_type).or_insert(0) += 1;
            }
            EventType::EdgeDeleted => {
                stats.total_edges = stats.total_edges.saturating_sub(1);
            }
            _ => {}
        }
        Ok(())
    }
}

/// 投影注册表 —— 管理所有投影实例
pub struct ProjectionRegistry {
    projections: Vec<Box<dyn Projection>>,
}

impl ProjectionRegistry {
    /// 创建空的投影注册表
    #[must_use]
    pub fn new() -> Self {
        Self {
            projections: Vec::new(),
        }
    }

    /// 注册投影
    pub fn register(&mut self, projection: Box<dyn Projection>) {
        info!(projection = projection.name(), "注册投影");
        self.projections.push(projection);
    }

    /// 获取所有投影
    pub fn projections(&self) -> &[Box<dyn Projection>] {
        &self.projections
    }

    /// 将事件分发给所有关心的投影
    ///
    /// # Errors
    ///
    /// 当任一投影处理失败时返回错误
    #[instrument(skip(self, event))]
    pub async fn dispatch(&self, event: &StoredEvent) -> Result<()> {
        for projection in &self.projections {
            if projection.interested_in().contains(&event.aggregate_type)
                && projection.event_types().contains(&event.event_type)
            {
                debug!(projection = projection.name(), "分发事件到投影");
                projection.handle(event).await?;
            }
        }
        Ok(())
    }
}

impl Default for ProjectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cqrs::event_store::{AggregateType, EventMetadata, EventType};
    use chrono::Utc;
    use serde_json::json;

    fn create_test_stored_event(
        event_type: EventType,
        aggregate_type: AggregateType,
    ) -> StoredEvent {
        StoredEvent {
            id: format!("event:{}", uuid::Uuid::new_v4()),
            event_type,
            aggregate_id: "test_doc_001".to_string(),
            aggregate_type,
            data: json!({
                "title": "测试文档",
                "status": "active",
                "block_count": 5
            }),
            metadata: EventMetadata::default(),
            version: 1,
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_document_projection_handle_ingested() {
        let projection = DocumentProjection::new();
        let event = create_test_stored_event(EventType::DocumentIngested, AggregateType::Document);
        projection.handle(&event).await.unwrap();

        let view = projection.get("test_doc_001").await.unwrap();
        assert_eq!(view.title, "测试文档");
        assert_eq!(view.status, DocumentStatus::Pending);
        assert_eq!(view.block_count, 5);
    }

    #[tokio::test]
    async fn test_document_projection_handle_deleted() {
        let projection = DocumentProjection::new();
        let event = create_test_stored_event(EventType::DocumentIngested, AggregateType::Document);
        projection.handle(&event).await.unwrap();
        assert!(projection.get("test_doc_001").await.is_some());

        let delete_event =
            create_test_stored_event(EventType::DocumentDeleted, AggregateType::Document);
        projection.handle(&delete_event).await.unwrap();
        assert!(projection.get("test_doc_001").await.is_none());
    }

    #[tokio::test]
    async fn test_node_stats_projection() {
        let projection = NodeStatsProjection::new();

        let node_event = StoredEvent {
            id: format!("event:{}", uuid::Uuid::new_v4()),
            event_type: EventType::NodeCreated,
            aggregate_id: "node_001".to_string(),
            aggregate_type: AggregateType::Node,
            data: json!({"entity_type": "concept"}),
            metadata: EventMetadata::default(),
            version: 1,
            timestamp: Utc::now(),
        };
        projection.handle(&node_event).await.unwrap();

        let stats = projection.get_stats().await;
        assert_eq!(stats.total_nodes, 1);
        assert_eq!(*stats.nodes_by_type.get("concept").unwrap(), 1);
    }

    #[tokio::test]
    async fn test_projection_registry_dispatch() {
        let mut registry = ProjectionRegistry::new();
        registry.register(Box::new(DocumentProjection::new()));
        registry.register(Box::new(NodeStatsProjection::new()));

        let event = create_test_stored_event(EventType::DocumentIngested, AggregateType::Document);
        registry.dispatch(&event).await.unwrap();
    }

    #[tokio::test]
    async fn test_projection_rebuild() {
        let projection = DocumentProjection::new();
        let events = vec![create_test_stored_event(
            EventType::DocumentIngested,
            AggregateType::Document,
        )];
        projection.rebuild(&events).await.unwrap();

        let view = projection.get("test_doc_001").await.unwrap();
        assert_eq!(view.title, "测试文档");
    }

    #[test]
    fn test_projection_interested_in() {
        let doc_proj = DocumentProjection::new();
        assert!(doc_proj.interested_in().contains(&AggregateType::Document));

        let node_proj = NodeStatsProjection::new();
        assert!(node_proj.interested_in().contains(&AggregateType::Node));
        assert!(node_proj.interested_in().contains(&AggregateType::Edge));
    }

    #[tokio::test]
    async fn test_document_projection_list() {
        let projection = DocumentProjection::new();
        let event1 = create_test_stored_event(EventType::DocumentIngested, AggregateType::Document);
        projection.handle(&event1).await.unwrap();

        let mut event2 =
            create_test_stored_event(EventType::DocumentIngested, AggregateType::Document);
        event2.aggregate_id = "test_doc_002".to_string();
        event2.data = json!({"title": "第二个文档", "status": "active", "block_count": 3});
        projection.handle(&event2).await.unwrap();

        let list = projection.list().await;
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_document_projection_default() {
        let projection = DocumentProjection::default();
        let list = projection.list().await;
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_node_stats_projection_default() {
        let projection = NodeStatsProjection::default();
        let stats = projection.get_stats().await;
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.total_edges, 0);
    }

    #[tokio::test]
    async fn test_node_stats_projection_edge_events() {
        let projection = NodeStatsProjection::new();

        let edge_event = StoredEvent {
            id: format!("event:{}", uuid::Uuid::new_v4()),
            event_type: EventType::EdgeCreated,
            aggregate_id: "edge_001".to_string(),
            aggregate_type: AggregateType::Edge,
            data: json!({"relation_type": "depends_on"}),
            metadata: EventMetadata::default(),
            version: 1,
            timestamp: Utc::now(),
        };
        projection.handle(&edge_event).await.unwrap();

        let stats = projection.get_stats().await;
        assert_eq!(stats.total_edges, 1);
        assert_eq!(*stats.edges_by_type.get("depends_on").unwrap(), 1);

        let edge_delete = StoredEvent {
            id: format!("event:{}", uuid::Uuid::new_v4()),
            event_type: EventType::EdgeDeleted,
            aggregate_id: "edge_001".to_string(),
            aggregate_type: AggregateType::Edge,
            data: json!({}),
            metadata: EventMetadata::default(),
            version: 2,
            timestamp: Utc::now(),
        };
        projection.handle(&edge_delete).await.unwrap();
        let stats = projection.get_stats().await;
        assert_eq!(stats.total_edges, 0);
    }

    #[tokio::test]
    async fn test_node_stats_projection_node_delete() {
        let projection = NodeStatsProjection::new();

        let node_create = StoredEvent {
            id: format!("event:{}", uuid::Uuid::new_v4()),
            event_type: EventType::NodeCreated,
            aggregate_id: "node_001".to_string(),
            aggregate_type: AggregateType::Node,
            data: json!({"entity_type": "technology"}),
            metadata: EventMetadata::default(),
            version: 1,
            timestamp: Utc::now(),
        };
        projection.handle(&node_create).await.unwrap();
        assert_eq!(projection.get_stats().await.total_nodes, 1);

        let node_delete = StoredEvent {
            id: format!("event:{}", uuid::Uuid::new_v4()),
            event_type: EventType::NodeDeleted,
            aggregate_id: "node_001".to_string(),
            aggregate_type: AggregateType::Node,
            data: json!({}),
            metadata: EventMetadata::default(),
            version: 2,
            timestamp: Utc::now(),
        };
        projection.handle(&node_delete).await.unwrap();
        assert_eq!(projection.get_stats().await.total_nodes, 0);
    }

    #[tokio::test]
    async fn test_document_projection_indexed_event() {
        let projection = DocumentProjection::new();
        let event = create_test_stored_event(EventType::DocumentIndexed, AggregateType::Document);
        projection.handle(&event).await.unwrap();

        let view = projection.get("test_doc_001").await.unwrap();
        assert_eq!(view.title, "测试文档");
    }

    #[tokio::test]
    async fn test_projection_registry_default() {
        let registry = ProjectionRegistry::default();
        assert!(registry.projections().is_empty());
    }

    #[tokio::test]
    async fn test_projection_registry_dispatch_unrelated_event() {
        let mut registry = ProjectionRegistry::new();
        registry.register(Box::new(DocumentProjection::new()));

        let unrelated_event = StoredEvent {
            id: format!("event:{}", uuid::Uuid::new_v4()),
            event_type: EventType::NodeCreated,
            aggregate_type: AggregateType::Node,
            aggregate_id: "node_001".to_string(),
            data: json!({}),
            metadata: EventMetadata::default(),
            version: 1,
            timestamp: Utc::now(),
        };
        registry.dispatch(&unrelated_event).await.unwrap();
    }

    #[tokio::test]
    async fn test_document_summary_view_serialization() {
        let view = DocumentSummaryView {
            document_id: "doc_001".to_string(),
            title: "Test".to_string(),
            status: DocumentStatus::Active,
            block_count: 5,
            updated_at: Some(Utc::now()),
        };
        let json = serde_json::to_string(&view).unwrap();
        let de: DocumentSummaryView = serde_json::from_str(&json).unwrap();
        assert_eq!(view.document_id, de.document_id);
    }

    #[tokio::test]
    async fn test_node_stats_view_serialization() {
        let view = NodeStatsView {
            total_nodes: 10,
            nodes_by_type: HashMap::from([("concept".to_string(), 5)]),
            total_edges: 3,
            edges_by_type: HashMap::from([("depends_on".to_string(), 2)]),
        };
        let json = serde_json::to_string(&view).unwrap();
        let de: NodeStatsView = serde_json::from_str(&json).unwrap();
        assert_eq!(view.total_nodes, de.total_nodes);
    }
}
