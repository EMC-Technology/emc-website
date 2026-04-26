use std::sync::Arc;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use tracing::{info, warn, instrument};
use crate::cqrs::event_store::{EventStore, StoredEvent, EventMetadata};
use crate::Result;
use crate::error::helpers;

/// 文档状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DocumentStatus {
    /// 草稿
    #[default]
    Draft,
    /// 已发布
    Published,
    /// 已归档
    Archived,
}

/// 聚合根操作错误
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum AggregateError {
    /// 无效状态
    #[error("Invalid state: {0}")]
    InvalidState(String),
    /// 业务规则违约
    #[error("Business rule violation: {0}")]
    BusinessRuleViolation(String),
    /// 聚合根未找到
    #[error("Aggregate not found: {0}")]
    NotFound(String),
    /// 版本冲突
    #[error("Version conflict: expected {expected}, actual {actual}")]
    VersionConflict {
        /// 期望版本号
        expected: u64,
        /// 实际版本号
        actual: u64,
    },
    /// 序列化错误
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// 文档命令枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentCommand {
    /// 创建文档
    Create(CreateDocumentData),
    /// 更新文档
    Update(UpdateDocumentData),
    /// 删除文档
    Delete(DeleteDocumentData),
}

/// 创建文档数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDocumentData {
    /// 文档 ID（由外部仓储层指定，确保聚合根身份确定性）
    pub document_id: String,
    /// 文档标题
    pub title: String,
    /// 文档内容
    pub content: String,
    /// 内容类型
    pub content_type: String,
    /// 元数据
    pub metadata: serde_json::Value,
}

/// 更新文档数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocumentData {
    /// 文档标题
    pub title: Option<String>,
    /// 文档内容
    pub content: Option<String>,
    /// 内容类型
    pub content_type: Option<String>,
    /// 元数据
    pub metadata: Option<serde_json::Value>,
}

/// 删除文档数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteDocumentData {
    /// 删除原因
    pub reason: Option<String>,
}

/// 文档领域事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentEvent {
    /// 文档已创建
    Created(DocumentCreatedData),
    /// 文档已更新
    Updated(DocumentUpdatedData),
    /// 文档已删除
    Deleted(DocumentDeletedData),
}

/// 文档创建事件携带的数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCreatedData {
    /// 文档唯一标识
    pub document_id: String,
    /// 文档标题
    pub title: String,
    /// 文档内容
    pub content: String,
    /// 内容类型（如 markdown、plain）
    pub content_type: String,
    /// 文档元数据
    pub metadata: serde_json::Value,
    /// 事件发生时间（事件溯源：重放时使用此时间戳，而非 Utc::now()）
    pub occurred_at: DateTime<Utc>,
}

/// 文档更新事件携带的数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentUpdatedData {
    /// 文档唯一标识
    pub document_id: String,
    /// 更新后的标题，None 表示未修改
    pub title: Option<String>,
    /// 更新后的内容，None 表示未修改
    pub content: Option<String>,
    /// 更新后的内容类型，None 表示未修改
    pub content_type: Option<String>,
    /// 更新后的元数据，None 表示未修改
    pub metadata: Option<serde_json::Value>,
    /// 事件发生时间（事件溯源：重放时使用此时间戳，而非 Utc::now()）
    pub occurred_at: DateTime<Utc>,
}

/// 文档删除事件携带的数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDeletedData {
    /// 文档唯一标识
    pub document_id: String,
    /// 删除原因
    pub reason: Option<String>,
    /// 事件发生时间（事件溯源：重放时使用此时间戳，而非 Utc::now()）
    pub occurred_at: DateTime<Utc>,
}

/// 聚合根（Aggregate）核心特征
///
/// 定义领域驱动设计中聚合根的行为契约，包括命令执行、事件应用与状态重放。
pub trait Aggregate: Send + Sync + Clone + Serialize + DeserializeOwned + Default + 'static {
    /// 聚合根接受的命令类型
    type Command;
    /// 聚合根产生的领域事件类型
    type Event: Serialize + DeserializeOwned + Send + Sync + 'static;
    /// 聚合根操作可能返回的错误类型
    type Error: std::error::Error + Send + Sync;

    /// 获取聚合根的唯一标识
    fn id(&self) -> &str;
    /// 获取聚合根的当前版本号
    fn version(&self) -> u64;

    /// 执行命令并返回产生的事件列表
    ///
    /// # Errors
    /// 当命令验证失败时返回 `AggregateError`
    fn execute(&self, command: Self::Command) -> std::result::Result<Vec<Self::Event>, Self::Error>;
    /// 将领域事件应用到聚合根，更新其内部状态
    fn apply(&mut self, event: &Self::Event);
    
    /// 通过重放事件流重建聚合根状态
    #[must_use]
    fn replay(events: Vec<Self::Event>) -> Self
    where
        Self: Sized,
    {
        let mut aggregate = Self::default();
        for event in events {
            aggregate.apply(&event);
        }
        aggregate
    }
}

/// 文档聚合根，封装文档的完整生命周期
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentAggregate {
    /// 文档唯一标识
    id: String,
    /// 文档标题
    title: String,
    /// 文档内容
    content: String,
    /// 内容类型
    content_type: String,
    /// 文档当前状态
    status: DocumentStatus,
    /// 文档元数据
    metadata: serde_json::Value,
    /// 关联的知识节点 ID 列表
    node_ids: Vec<String>,
    /// 聚合根版本号
    version: u64,
    /// 文档创建时间
    created_at: DateTime<Utc>,
    /// 文档最后更新时间
    updated_at: DateTime<Utc>,
}

impl Default for DocumentAggregate {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            content: String::new(),
            content_type: String::new(),
            status: DocumentStatus::Draft,
            metadata: serde_json::json!({}),
            node_ids: Vec::new(),
            version: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl Aggregate for DocumentAggregate {
    type Command = DocumentCommand;
    type Event = DocumentEvent;
    type Error = AggregateError;

    fn id(&self) -> &str {
        &self.id
    }

    fn version(&self) -> u64 {
        self.version
    }

    fn execute(&self, command: Self::Command) -> std::result::Result<Vec<Self::Event>, Self::Error> {
        match command {
            DocumentCommand::Create(data) => self.handle_create(data),
            DocumentCommand::Update(data) => self.handle_update(data),
            DocumentCommand::Delete(data) => self.handle_delete(data),
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            DocumentEvent::Created(data) => {
                self.id.clone_from(&data.document_id);
                self.title.clone_from(&data.title);
                self.content.clone_from(&data.content);
                self.content_type.clone_from(&data.content_type);
                self.metadata.clone_from(&data.metadata);
                self.status = DocumentStatus::Draft;
                self.created_at = data.occurred_at;
                self.updated_at = data.occurred_at;
            }
            DocumentEvent::Updated(data) => {
                if let Some(title) = &data.title {
                    self.title.clone_from(title);
                }
                if let Some(content) = &data.content {
                    self.content.clone_from(content);
                }
                if let Some(content_type) = &data.content_type {
                    self.content_type.clone_from(content_type);
                }
                if let Some(metadata) = &data.metadata {
                    self.metadata.clone_from(metadata);
                }
                self.updated_at = data.occurred_at;
            }
            DocumentEvent::Deleted(data) => {
                self.status = DocumentStatus::Archived;
                self.updated_at = data.occurred_at;
            }
        }
        self.version += 1;
    }
}

impl DocumentAggregate {
    /// 创建一个新的空文档聚合根实例
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// # Errors
    /// 当文档已存在或标题为空时返回 `AggregateError`
    pub fn handle_create(&self, data: CreateDocumentData) -> std::result::Result<Vec<DocumentEvent>, AggregateError> {
        if !self.id.is_empty() {
            return Err(AggregateError::InvalidState(
                "Document already exists".to_string(),
            ));
        }

        if data.title.trim().is_empty() {
            return Err(AggregateError::BusinessRuleViolation(
                "Title cannot be empty".to_string(),
            ));
        }

        Ok(vec![DocumentEvent::Created(DocumentCreatedData {
            document_id: data.document_id,
            title: data.title,
            content: data.content,
            content_type: data.content_type,
            metadata: data.metadata,
            occurred_at: Utc::now(),
        })])
    }

    /// # Errors
    /// 当文档不存在或已归档时返回 `AggregateError`
    pub fn handle_update(&self, data: UpdateDocumentData) -> std::result::Result<Vec<DocumentEvent>, AggregateError> {
        if self.id.is_empty() {
            return Err(AggregateError::NotFound("Document does not exist".to_string()));
        }

        if matches!(self.status, DocumentStatus::Archived) {
            return Err(AggregateError::InvalidState(
                "Cannot update archived document".to_string(),
            ));
        }

        if data.title.as_ref().is_some_and(|t| t.trim().is_empty()) {
            return Err(AggregateError::BusinessRuleViolation(
                "Title cannot be empty".to_string(),
            ));
        }

        Ok(vec![DocumentEvent::Updated(DocumentUpdatedData {
            document_id: self.id.clone(),
            title: data.title,
            content: data.content,
            content_type: data.content_type,
            metadata: data.metadata,
            occurred_at: Utc::now(),
        })])
    }

    /// # Errors
    /// 当文档不存在或已归档时返回 `AggregateError`
    pub fn handle_delete(&self, data: DeleteDocumentData) -> std::result::Result<Vec<DocumentEvent>, AggregateError> {
        if self.id.is_empty() {
            return Err(AggregateError::NotFound("Document does not exist".to_string()));
        }

        if matches!(self.status, DocumentStatus::Archived) {
            return Err(AggregateError::InvalidState(
                "Document already archived".to_string(),
            ));
        }

        Ok(vec![DocumentEvent::Deleted(DocumentDeletedData {
            document_id: self.id.clone(),
            reason: data.reason,
            occurred_at: Utc::now(),
        })])
    }

    /// 获取文档标题
    #[must_use]
    pub fn get_title(&self) -> &str {
        &self.title
    }

    /// 获取文档内容
    #[must_use]
    pub fn get_content(&self) -> &str {
        &self.content
    }

    /// 获取文档状态
    #[must_use]
    pub const fn get_status(&self) -> &DocumentStatus {
        &self.status
    }

    /// 获取关联节点数量
    #[must_use]
    pub fn get_node_count(&self) -> usize {
        self.node_ids.len()
    }

    /// 判断文档是否已发布
    #[must_use]
    pub const fn is_published(&self) -> bool {
        matches!(self.status, DocumentStatus::Published)
    }

    /// 判断文档是否已归档
    #[must_use]
    pub const fn is_archived(&self) -> bool {
        matches!(self.status, DocumentStatus::Archived)
    }
}

/// 聚合根快照存储特征，用于加速事件溯源中的状态重建
#[async_trait]
pub trait SnapshotStore: Send + Sync {
    /// 保存聚合根快照
    async fn save_snapshot<A: Aggregate>(&self, aggregate: &A) -> Result<()>;
    /// 加载聚合根快照，返回聚合根实例及其版本号
    async fn load_snapshot<A: Aggregate>(&self, aggregate_id: &str) -> Result<Option<(A, u64)>>;
}

/// 基于内存的快照存储实现，适用于测试和开发环境
pub struct MemorySnapshotStore {
    snapshots: Arc<tokio::sync::RwLock<std::collections::HashMap<String, (String, u64)>>>,
}

impl MemorySnapshotStore {
    /// 创建一个新的空内存快照存储实例
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}

impl Default for MemorySnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SnapshotStore for MemorySnapshotStore {
    async fn save_snapshot<A: Aggregate>(&self, aggregate: &A) -> Result<()> {
        let json = serde_json::to_value(aggregate).map_err(error_core::ErrorObject::from)?;
        self.snapshots.write().await.insert(aggregate.id().to_string(), (json.to_string(), aggregate.version()));
        Ok(())
    }

    async fn load_snapshot<A: Aggregate>(&self, aggregate_id: &str) -> Result<Option<(A, u64)>> {
        let guard = self.snapshots.read().await;
        
        match guard.get(aggregate_id) {
            Some((json_str, version)) => {
                let json_str = json_str.clone();
                let version = *version;
                drop(guard);
                let aggregate: A =
                    serde_json::from_str(&json_str).map_err(|e| helpers::internal_error(&format!("Failed to deserialize snapshot: {e}")))?;
                Ok(Some((aggregate, version)))
            }
            None => Ok(None),
        }
    }
}

/// 聚合根仓储，封装聚合根的加载与命令执行逻辑
pub struct AggregateRepository<A: Aggregate> {
    event_store: Arc<dyn EventStore>,
    snapshot_store: Option<Arc<MemorySnapshotStore>>,
    _marker: std::marker::PhantomData<A>,
}

impl<A: Aggregate> AggregateRepository<A> {
    /// 创建仅使用事件存储的仓储实例
    pub fn new(event_store: Arc<dyn EventStore>) -> Self {
        Self {
            event_store,
            snapshot_store: None,
            _marker: std::marker::PhantomData,
        }
    }

    /// 创建同时使用事件存储和快照存储的仓储实例
    pub fn with_snapshots(event_store: Arc<dyn EventStore>, snapshot_store: Arc<MemorySnapshotStore>) -> Self {
        Self {
            event_store,
            snapshot_store: Some(snapshot_store),
            _marker: std::marker::PhantomData,
        }
    }

    /// # Errors
    /// 当快照加载失败或事件反序列化失败时返回错误
    #[instrument(skip(self), fields(aggregate_id = %aggregate_id))]
    pub async fn load(&self, aggregate_id: &str) -> Result<A> {
        let mut aggregate = None;
        let mut from_version = 0u64;

        if let Some(ref ss) = self.snapshot_store {
            if let Ok(Some((snapshot, version))) = ss.load_snapshot::<A>(aggregate_id).await {
                from_version = version + 1;
                aggregate = Some(snapshot);
                info!(
                    aggregate_id = %aggregate_id,
                    snapshot_version = version,
                    "Loaded aggregate from snapshot"
                );
            }
        }

        let events = self.event_store.load_events_from_version(aggregate_id, from_version).await?;

        if let Some(mut agg) = aggregate {
            for event_data in &events {
                let event: A::Event = serde_json::from_value(event_data.data.clone())
                    .map_err(|e| helpers::internal_error(&format!("Failed to deserialize event: {e}")))?;
                agg.apply(&event);
            }
            info!(
                aggregate_id = %aggregate_id,
                total_events = events.len(),
                final_version = agg.version(),
                "Aggregate reconstructed from snapshot and events"
            );
            Ok(agg)
        } else {
            let deserialized_events: Vec<A::Event> = events
                .iter()
                .map(|e| {
                    serde_json::from_value(e.data.clone())
                        .map_err(|err| helpers::internal_error(&format!("Event deserialization failed: {err}")))
                })
                .collect::<Result<Vec<_>>>()?;

            let aggregate = A::replay(deserialized_events);

            info!(
                aggregate_id = %aggregate_id,
                event_count = events.len(),
                final_version = aggregate.version(),
                "Aggregate replayed from event stream"
            );

            Ok(aggregate)
        }
    }

    /// # Panics
    /// 当事件序列化失败时会 panic（理论上不应发生）
    ///
    /// # Errors
    /// 当聚合加载失败或命令执行失败时返回错误
    #[instrument(skip(self, command), fields(aggregate_id = %aggregate_id))]
    pub async fn execute(&self, aggregate_id: &str, command: A::Command) -> Result<Vec<StoredEvent>> {
        let aggregate = self.load(aggregate_id).await?;

        let current_version = aggregate.version();
        let events = aggregate.execute(command).map_err(|e| {
            helpers::internal_error(&format!("Command execution failed: {e}"))
        })?;

        if events.is_empty() {
            return Ok(Vec::new());
        }

        let stored_events: Result<Vec<StoredEvent>> = events
            .into_iter()
            .enumerate()
            .map(|(i, event)| {
                let event_json = serde_json::to_value(&event)
                    .map_err(|e| helpers::internal_error(&format!("事件序列化失败: {e}")))?;
                
                Ok(StoredEvent {
                    id: format!("event:{}:{}", aggregate_id, current_version + i as u64 + 1),
                    event_type: std::any::type_name::<A::Event>()
                        .rsplit("::")
                        .next()
                        .unwrap_or("Unknown")
                        .to_string(),
                    aggregate_id: aggregate_id.to_string(),
                    aggregate_type: std::any::type_name::<A>()
                        .rsplit("::")
                        .next()
                        .unwrap_or("Unknown")
                        .to_string(),
                    data: event_json,
                    metadata: EventMetadata::default(),
                    version: current_version + i as u64 + 1,
                    timestamp: DateTime::<Utc>::MIN_UTC,
                })
            })
            .collect();

        let stored_events = stored_events?;

        self.event_store
            .append_events(aggregate_id, stored_events.clone(), Some(current_version))
            .await?;

        if let Some(ref ss) = self.snapshot_store {
            let updated_aggregate = self.load(aggregate_id).await?;
            if let Err(e) = ss.save_snapshot(&updated_aggregate).await {
                warn!(
                    error = %e,
                    aggregate_id = %aggregate_id,
                    "Failed to save snapshot after event persistence"
                );
            } else {
                info!(aggregate_id = %aggregate_id, "Snapshot saved successfully");
            }
        }

        Ok(stored_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_aggregate_creation() {
        let aggregate = DocumentAggregate::new();
        
        assert_eq!(aggregate.version(), 0);
        assert!(aggregate.id().is_empty());
        assert!(!aggregate.is_published());
        assert!(!aggregate.is_archived());
    }

    #[test]
    fn test_document_create_command_success() {
        let aggregate = DocumentAggregate::new();
        
        let result = aggregate.execute(DocumentCommand::Create(CreateDocumentData {
            document_id: "doc_test".to_string(),
            title: "Test Document".to_string(),
            content: "# Hello".to_string(),
            content_type: "markdown".to_string(),
            metadata: serde_json::json!({}),
        }));

        assert!(result.is_ok());
        let events = result.unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            DocumentEvent::Created(data) => {
                assert_eq!(data.title, "Test Document");
                assert_eq!(data.content_type, "markdown");
                assert!(!data.document_id.is_empty());
            }
            _ => panic!("Expected Created event"),
        }
    }

    #[test]
    fn test_document_create_empty_title_error() {
        let aggregate = DocumentAggregate::new();
        
        let result = aggregate.execute(DocumentCommand::Create(CreateDocumentData {
            document_id: "doc_empty_title".to_string(),
            title: String::new(),
            content: "# Hello".to_string(),
            content_type: "markdown".to_string(),
            metadata: serde_json::json!({}),
        }));

        assert!(result.is_err());
        match result.unwrap_err() {
            AggregateError::BusinessRuleViolation(msg) => {
                assert!(msg.contains("empty"));
            }
            _ => panic!("Expected BusinessRuleViolation"),
        }
    }

    #[test]
    fn test_document_update_command_success() {
        let mut aggregate = DocumentAggregate::new();
        
        let create_event = DocumentEvent::Created(DocumentCreatedData {
            document_id: "doc_001".to_string(),
            title: "Original Title".to_string(),
            content: "Original Content".to_string(),
            content_type: "markdown".to_string(),
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
        });
        aggregate.apply(&create_event);

        let result = aggregate.execute(DocumentCommand::Update(UpdateDocumentData {
            title: Some("Updated Title".to_string()),
            content: None,
            content_type: None,
            metadata: None,
        }));

        assert!(result.is_ok());
        let events = result.unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            DocumentEvent::Updated(data) => {
                assert_eq!(data.title.as_deref(), Some("Updated Title"));
                assert!(data.content.is_none());
            }
            _ => panic!("Expected Updated event"),
        }
    }

    #[test]
    fn test_document_delete_command_success() {
        let mut aggregate = DocumentAggregate::new();
        
        aggregate.apply(&DocumentEvent::Created(DocumentCreatedData {
            document_id: "doc_002".to_string(),
            title: "To Delete".to_string(),
            content: "Content".to_string(),
            content_type: "plain".to_string(),
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
        }));

        let result = aggregate.execute(DocumentCommand::Delete(DeleteDocumentData {
            reason: Some("No longer needed".to_string()),
        }));

        assert!(result.is_ok());
        let events = result.unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            DocumentEvent::Deleted(data) => {
                assert_eq!(data.reason.as_deref(), Some("No longer needed"));
            }
            _ => panic!("Expected Deleted event"),
        }
    }

    #[test]
    fn test_apply_created_event_updates_state() {
        let mut aggregate = DocumentAggregate::new();
        
        aggregate.apply(&DocumentEvent::Created(DocumentCreatedData {
            document_id: "doc_003".to_string(),
            title: "Applied Doc".to_string(),
            content: "Applied Content".to_string(),
            content_type: "code".to_string(),
            metadata: serde_json::json!({"key": "value"}),
            occurred_at: Utc::now(),
        }));

        assert_eq!(aggregate.id(), "doc_003");
        assert_eq!(aggregate.get_title(), "Applied Doc");
        assert_eq!(aggregate.get_content(), "Applied Content");
        assert_eq!(aggregate.version(), 1);
        assert!(!aggregate.is_archived());
    }

    #[test]
    fn test_apply_updated_event_modifies_state() {
        let mut aggregate = DocumentAggregate::new();
        
        aggregate.apply(&DocumentEvent::Created(DocumentCreatedData {
            document_id: "doc_004".to_string(),
            title: "Original".to_string(),
            content: "Original Content".to_string(),
            content_type: "plain".to_string(),
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
        }));
        
        aggregate.apply(&DocumentEvent::Updated(DocumentUpdatedData {
            document_id: "doc_004".to_string(),
            title: Some("Modified".to_string()),
            content: Some("New Content".to_string()),
            content_type: None,
            metadata: None,
            occurred_at: Utc::now(),
        }));

        assert_eq!(aggregate.get_title(), "Modified");
        assert_eq!(aggregate.get_content(), "New Content");
        assert_eq!(aggregate.version(), 2);
    }

    #[test]
    fn test_apply_deleted_event_archives_document() {
        let mut aggregate = DocumentAggregate::new();
        
        aggregate.apply(&DocumentEvent::Created(DocumentCreatedData {
            document_id: "doc_005".to_string(),
            title: "To Archive".to_string(),
            content: "Content".to_string(),
            content_type: "plain".to_string(),
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
        }));
        
        aggregate.apply(&DocumentEvent::Deleted(DocumentDeletedData {
            document_id: "doc_005".to_string(),
            reason: None,
            occurred_at: Utc::now(),
        }));

        assert!(aggregate.is_archived());
        assert_eq!(aggregate.version(), 2);
    }

    #[test]
    fn test_replay_reconstructs_state() {
        let events = vec![
            DocumentEvent::Created(DocumentCreatedData {
                document_id: "doc_006".to_string(),
                title: "Replayed Doc".to_string(),
                content: "Content".to_string(),
                content_type: "markdown".to_string(),
                metadata: serde_json::json!({}),
                occurred_at: Utc::now(),
            }),
            DocumentEvent::Updated(DocumentUpdatedData {
                document_id: "doc_006".to_string(),
                title: Some("Replayed Updated".to_string()),
                content: None,
                content_type: None,
                metadata: None,
                occurred_at: Utc::now(),
            }),
        ];

        let aggregate = DocumentAggregate::replay(events);

        assert_eq!(aggregate.id(), "doc_006");
        assert_eq!(aggregate.get_title(), "Replayed Updated");
        assert_eq!(aggregate.version(), 2);
    }

    #[tokio::test]
    async fn test_aggregate_repository_load_and_execute() {
        use crate::cqrs::event_store::InMemoryEventStore;
        
        let event_store = Arc::new(InMemoryEventStore::new());
        let repository = AggregateRepository::<DocumentAggregate>::new(event_store);

        let events = repository
            .execute(
                "doc_007",
                DocumentCommand::Create(CreateDocumentData {
                    document_id: "doc_007".to_string(),
                    title: "Repository Test".to_string(),
                    content: "# Test".to_string(),
                    content_type: "markdown".to_string(),
                    metadata: serde_json::json!({}),
                }),
            )
            .await;

        assert!(events.is_ok());
        let stored_events = events.unwrap();
        assert_eq!(stored_events.len(), 1);
        assert_eq!(stored_events[0].version, 1);

        let loaded = repository.load("doc_007").await.unwrap();
        assert_eq!(loaded.id(), "doc_007");
        assert_eq!(loaded.get_title(), "Repository Test");
        assert_eq!(loaded.version(), 1);
    }

    #[tokio::test]
    async fn test_aggregate_repository_optimistic_lock() {
        use crate::cqrs::event_store::InMemoryEventStore;
        
        let event_store = Arc::new(InMemoryEventStore::new());
        let repository = AggregateRepository::<DocumentAggregate>::new(event_store);

        repository
            .execute(
                "doc_008",
                DocumentCommand::Create(CreateDocumentData {
                    document_id: "doc_multi".to_string(),
                    title: "First".to_string(),
                    content: "# First".to_string(),
                    content_type: "markdown".to_string(),
                    metadata: serde_json::json!({}),
                }),
            )
            .await
            .unwrap();

        let result = repository
            .execute(
                "doc_008",
                DocumentCommand::Update(UpdateDocumentData {
                    title: Some("Updated".to_string()),
                    content: None,
                    content_type: None,
                    metadata: None,
                }),
            )
            .await;

        assert!(result.is_ok());

        let loaded = repository.load("doc_008").await.unwrap();
        assert_eq!(loaded.get_title(), "Updated");
        assert_eq!(loaded.version(), 2);
    }

    #[tokio::test]
    async fn test_memory_snapshot_store_save_and_load() {
        let store = MemorySnapshotStore::new();
        
        let mut aggregate = DocumentAggregate::new();
        aggregate.apply(&DocumentEvent::Created(DocumentCreatedData {
            document_id: "snap_doc_001".to_string(),
            title: "Snapshot Test".to_string(),
            content: "Content".to_string(),
            content_type: "plain".to_string(),
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
        }));

        store.save_snapshot(&aggregate).await.unwrap();

        let (loaded, version) = store.load_snapshot::<DocumentAggregate>("snap_doc_001")
            .await
            .unwrap()
            .expect("Snapshot should exist");

        assert_eq!(loaded.id(), "snap_doc_001");
        assert_eq!(loaded.get_title(), "Snapshot Test");
        assert_eq!(version, 1);
    }

    #[tokio::test]
    async fn test_memory_snapshot_store_not_found() {
        let store = MemorySnapshotStore::new();
        
        let result = store.load_snapshot::<DocumentAggregate>("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_aggregate_error_display() {
        let err = AggregateError::InvalidState("Bad state".to_string());
        assert!(format!("{err}").contains("Bad state"));

        let err = AggregateError::VersionConflict { expected: 5, actual: 3 };
        let display = format!("{err}");
        assert!(display.contains('5') && display.contains('3'));
    }

    #[test]
    fn test_document_status_serialization() {
        let statuses = vec![
            DocumentStatus::Draft,
            DocumentStatus::Published,
            DocumentStatus::Archived,
        ];

        for status in &statuses {
            let json = serde_json::to_string(status).expect("序列化失败");
            let deserialized: DocumentStatus =
                serde_json::from_str(&json).expect("反序列化失败");
            assert_eq!(*status, deserialized);
        }
    }

    #[tokio::test]
    async fn test_aggregate_with_snapshots_integration() {
        use crate::cqrs::event_store::InMemoryEventStore;
        
        let event_store = Arc::new(InMemoryEventStore::new());
        let snapshot_store = Arc::new(MemorySnapshotStore::new());
        let repository = AggregateRepository::<DocumentAggregate>::with_snapshots(
            event_store,
            snapshot_store,
        );

        repository
            .execute(
                "doc_snapshot_001",
                DocumentCommand::Create(CreateDocumentData {
                    document_id: "doc_snapshot".to_string(),
                    title: "With Snapshot".to_string(),
                    content: "# Snapshot".to_string(),
                    content_type: "markdown".to_string(),
                    metadata: serde_json::json!({}),
                }),
            )
            .await
            .unwrap();

        let loaded = repository.load("doc_snapshot_001").await.unwrap();
        assert_eq!(loaded.get_title(), "With Snapshot");
        assert_eq!(loaded.version(), 1);
    }
}