use crate::Result;
use crate::cqrs::event_store::{
    AggregateType, CausationContext, ChangeSet, EventMetadata, EventStore, EventType, FieldChange,
    StoredEvent, TriggeredBy,
};
use crate::error::helpers;
use crate::model::ContentType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::future::Future;
use std::sync::Arc;
use tracing::{info, instrument, warn};

/// 文档状态枚举
///
/// 遵循 Spec 第13章 13.4 节定义的完整生命周期：
/// `Pending → Parsing → Indexed → Active → Archived → Deleted`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DocumentStatus {
    /// 待处理（文件已上传，尚未开始解析）
    #[default]
    Pending,
    /// 解析中（解析流水线正在执行）
    Parsing,
    /// 已索引（解析完成，向量嵌入已计算）
    Indexed,
    /// 活跃（可供查询和检索）
    Active,
    /// 已归档（逻辑归档，数据仍保留）
    Archived,
    /// 已删除（软删除，等待数据保留策略清理）
    Deleted,
}

impl DocumentStatus {
    /// 验证状态转换是否合法
    ///
    /// 合法转换路径（主路径严格单调，删除为通用退出边）：
    /// - `Pending` → `Parsing` | `Deleted`
    /// - `Parsing` → `Indexed` | `Deleted`
    /// - `Indexed` → `Active` | `Deleted`
    /// - `Active` → `Archived` | `Deleted`
    /// - `Archived` → `Deleted`
    ///
    /// 非法转换：
    /// - 任何回退转换（违反严格单调性）
    /// - `Deleted` → 任何状态（终态）
    #[must_use]
    pub fn can_transition_to(&self, target: &Self) -> bool {
        matches!(
            (self, target),
            (Self::Pending, Self::Parsing | Self::Deleted)
                | (Self::Parsing, Self::Indexed | Self::Deleted)
                | (Self::Indexed, Self::Active | Self::Deleted)
                | (Self::Active, Self::Archived | Self::Deleted)
                | (Self::Archived, Self::Deleted)
        )
    }
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
    /// 发布文档（Draft → Published）
    Publish,
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
    pub content_type: ContentType,
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
    pub content_type: Option<ContentType>,
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
    /// 文档已发布
    Published(DocumentPublishedData),
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
    pub content_type: ContentType,
    /// 文档元数据
    pub metadata: serde_json::Value,
    /// 事件发生时间（事件溯源：重放时使用此时间戳，而非 Utc::now()）
    pub occurred_at: DateTime<Utc>,
    /// 触发源描述（Axiom-4: 跨域绑定 —— 所有数据变更必须包含 triggered_by）
    pub triggered_by: crate::cqrs::event_store::TriggeredBy,
}

/// 文档更新事件携带的数据
///
/// 使用 `ChangeSet` 记录字段级变更（old_value → new_value），
/// 支持审计差异对比和精确的状态回放。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentUpdatedData {
    /// 文档唯一标识
    pub document_id: String,
    /// 字段级变更集（仅包含实际变更的字段）
    pub changes: ChangeSet,
    /// 事件发生时间（事件溯源：重放时使用此时间戳，而非 Utc::now()）
    pub occurred_at: DateTime<Utc>,
    /// 触发源描述（Axiom-4: 跨域绑定）
    pub triggered_by: crate::cqrs::event_store::TriggeredBy,
}

/// 文档发布事件携带的数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentPublishedData {
    /// 文档唯一标识
    pub document_id: String,
    /// 发布前状态
    pub previous_status: DocumentStatus,
    /// 事件发生时间
    pub occurred_at: DateTime<Utc>,
    /// 触发源描述（Axiom-4: 跨域绑定）
    pub triggered_by: crate::cqrs::event_store::TriggeredBy,
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
    /// 触发源描述（Axiom-4: 跨域绑定）
    pub triggered_by: crate::cqrs::event_store::TriggeredBy,
}

/// 聚合根（Aggregate）核心特征
///
/// 定义领域驱动设计中聚合根的行为契约，包括命令执行、事件应用与状态重放。
pub trait Aggregate:
    Send + Sync + Clone + Serialize + DeserializeOwned + Default + 'static
{
    /// 聚合根命令类型
    type Command;
    /// 聚合根事件类型
    type Event: Serialize + DeserializeOwned + Send + Sync + 'static;
    /// 聚合根错误类型
    type Error: std::error::Error + Send + Sync;

    /// 返回聚合根唯一标识
    fn id(&self) -> &str;
    /// 返回聚合根当前版本号
    fn version(&self) -> u64;

    /// 返回聚合根类型
    fn aggregate_type() -> AggregateType;
    /// 根据事件实例返回对应的事件类型
    fn event_type_for(event: &Self::Event) -> EventType;

    /// 执行命令，产生事件或错误
    fn execute(&self, command: Self::Command)
    -> std::result::Result<Vec<Self::Event>, Self::Error>;
    /// 将事件应用到聚合根以更新状态
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
    /// 文档实例唯一标识（Axiom-3: 类型化 ID，标识"哪一个文档实例"）
    id: crate::model::ids::DocumentId,
    /// 文档标题
    title: String,
    /// 文档内容
    content: String,
    /// 内容类型
    content_type: ContentType,
    /// 文档当前状态
    status: DocumentStatus,
    /// 文档元数据
    metadata: serde_json::Value,
    /// 关联的知识节点 ID 列表（Axiom-3: 类型化 ID）
    node_ids: Vec<crate::model::ids::NodeId>,
    /// 聚合根版本号
    version: u64,
    /// 文档创建时间
    created_at: DateTime<Utc>,
    /// 文档最后更新时间
    updated_at: DateTime<Utc>,
}

impl Default for DocumentAggregate {
    fn default() -> Self {
        use crate::model::ids::DocumentId;
        Self {
            id: DocumentId::new(""),
            title: String::new(),
            content: String::new(),
            content_type: ContentType::Markdown,
            status: DocumentStatus::Pending,
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
        self.id.as_str()
    }

    fn version(&self) -> u64 {
        self.version
    }

    fn aggregate_type() -> AggregateType {
        AggregateType::Document
    }

    fn event_type_for(event: &Self::Event) -> EventType {
        match event {
            DocumentEvent::Created(_) => EventType::DocumentIngested,
            DocumentEvent::Updated(_) | DocumentEvent::Published(_) => EventType::DocumentIndexed,
            DocumentEvent::Deleted(_) => EventType::DocumentDeleted,
        }
    }

    fn execute(
        &self,
        command: Self::Command,
    ) -> std::result::Result<Vec<Self::Event>, Self::Error> {
        match command {
            DocumentCommand::Create(data) => self.handle_create(data),
            DocumentCommand::Update(data) => self.handle_update(&data),
            DocumentCommand::Publish => self.handle_publish(),
            DocumentCommand::Delete(data) => self.handle_delete(data),
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            DocumentEvent::Created(data) => {
                self.id = crate::model::ids::DocumentId::new(&data.document_id);
                self.title.clone_from(&data.title);
                self.content.clone_from(&data.content);
                self.content_type.clone_from(&data.content_type);
                self.metadata.clone_from(&data.metadata);
                self.status = DocumentStatus::Pending;
                self.created_at = data.occurred_at;
                self.updated_at = data.occurred_at;
            }
            DocumentEvent::Updated(data) => {
                for (field, change) in &data.changes.changes {
                    match field.as_str() {
                        "title" => {
                            if let Some(ref new) = change.new_value {
                                self.title = new.as_str().unwrap_or_default().to_string();
                            }
                        }
                        "content" => {
                            if let Some(ref new) = change.new_value {
                                self.content = new.as_str().unwrap_or_default().to_string();
                            }
                        }
                        "content_type" => {
                            if let Some(ref new) = change.new_value
                                && let Some(s) = new.as_str()
                            {
                                if let Ok(ct) = ContentType::from_mime(s) {
                                    self.content_type = ct;
                                } else {
                                    tracing::warn!(value = %s, "无法解析的 ContentType 值，跳过");
                                }
                            }
                        }
                        "metadata" => {
                            if let Some(ref new) = change.new_value {
                                self.metadata = new.clone();
                            }
                        }
                        other => {
                            tracing::warn!(
                                field = %other,
                                "未识别的字段变更在 DocumentUpdated 事件中，跳过"
                            );
                        }
                    }
                }
                self.updated_at = data.occurred_at;
            }
            DocumentEvent::Published(data) => {
                self.status = DocumentStatus::Active;
                self.updated_at = data.occurred_at;
                let _ = &data.previous_status;
            }
            DocumentEvent::Deleted(data) => {
                self.status = DocumentStatus::Deleted;
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
    pub fn handle_create(
        &self,
        data: CreateDocumentData,
    ) -> std::result::Result<Vec<DocumentEvent>, AggregateError> {
        if !self.id.as_str().is_empty() {
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
            document_id: data.document_id.clone(),
            title: data.title,
            content: data.content,
            content_type: data.content_type,
            metadata: data.metadata,
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("CreateDocument", &data.document_id),
        })])
    }

    /// # Errors
    /// 当文档不存在或已归档时返回 `AggregateError`
    pub fn handle_update(
        &self,
        data: &UpdateDocumentData,
    ) -> std::result::Result<Vec<DocumentEvent>, AggregateError> {
        if self.id.as_str().is_empty() {
            return Err(AggregateError::NotFound(
                "Document does not exist".to_string(),
            ));
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

        let mut changes = ChangeSet::new();

        if let Some(ref new_title) = data.title {
            changes.add_change(
                "title",
                FieldChange::changed(
                    serde_json::Value::String(self.title.clone()),
                    serde_json::Value::String(new_title.clone()),
                ),
            );
        }

        if let Some(ref new_content) = data.content {
            changes.add_change(
                "content",
                FieldChange::changed(
                    serde_json::Value::String(self.content.clone()),
                    serde_json::Value::String(new_content.clone()),
                ),
            );
        }

        if let Some(ref new_content_type) = data.content_type {
            changes.add_change(
                "content_type",
                FieldChange::changed(
                    serde_json::Value::String(self.content_type.to_mime().to_string()),
                    serde_json::Value::String(new_content_type.to_mime().to_string()),
                ),
            );
        }

        if let Some(ref new_metadata) = data.metadata {
            changes.add_change(
                "metadata",
                FieldChange::changed(self.metadata.clone(), new_metadata.clone()),
            );
        }

        if !changes.has_changes() {
            return Ok(Vec::new());
        }

        Ok(vec![DocumentEvent::Updated(DocumentUpdatedData {
            document_id: self.id.to_string(),
            changes,
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("UpdateDocument", self.id.as_str()),
        })])
    }

    /// 发布文档（Draft → Published）
    ///
    /// # Errors
    /// 当文档不存在、已归档或不在 Draft 状态时返回 `AggregateError`
    pub fn handle_publish(&self) -> std::result::Result<Vec<DocumentEvent>, AggregateError> {
        if self.id.as_str().is_empty() {
            return Err(AggregateError::NotFound(
                "Document does not exist".to_string(),
            ));
        }

        if !self.status.can_transition_to(&DocumentStatus::Active) {
            return Err(AggregateError::InvalidState(format!(
                "Cannot publish document in {:?} state, only Indexed can be published",
                self.status
            )));
        }

        Ok(vec![DocumentEvent::Published(DocumentPublishedData {
            document_id: self.id.to_string(),
            previous_status: self.status.clone(),
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("PublishDocument", self.id.as_str()),
        })])
    }

    /// # Errors
    /// 当文档不存在、已删除、或当前状态不允许转换到 `Deleted` 时返回 `AggregateError`
    pub fn handle_delete(
        &self,
        data: DeleteDocumentData,
    ) -> std::result::Result<Vec<DocumentEvent>, AggregateError> {
        if self.id.as_str().is_empty() {
            return Err(AggregateError::NotFound(
                "Document does not exist".to_string(),
            ));
        }

        if matches!(self.status, DocumentStatus::Deleted) {
            return Err(AggregateError::InvalidState(
                "Document already deleted".to_string(),
            ));
        }

        if !self.status.can_transition_to(&DocumentStatus::Deleted) {
            return Err(AggregateError::InvalidState(format!(
                "Cannot delete document in {:?} state",
                self.status
            )));
        }

        Ok(vec![DocumentEvent::Deleted(DocumentDeletedData {
            document_id: self.id.to_string(),
            reason: data.reason,
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("DeleteDocument", self.id.as_str()),
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
        matches!(self.status, DocumentStatus::Active)
    }

    /// 判断文档是否已归档
    #[must_use]
    pub const fn is_archived(&self) -> bool {
        matches!(self.status, DocumentStatus::Archived)
    }
}

/// 聚合根快照存储特征，用于加速事件溯源中的状态重建
pub trait SnapshotStore: Send + Sync {
    /// 保存聚合根快照
    fn save_snapshot<A: Aggregate>(&self, aggregate: &A)
    -> impl Future<Output = Result<()>> + Send;
    /// 加载聚合根快照，返回聚合根实例及其版本号
    fn load_snapshot<A: Aggregate>(
        &self,
        aggregate_id: &str,
    ) -> impl Future<Output = Result<Option<(A, u64)>>> + Send;
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

#[allow(clippy::manual_async_fn)]
impl SnapshotStore for MemorySnapshotStore {
    #[allow(clippy::manual_async_fn)]
    fn save_snapshot<A: Aggregate>(
        &self,
        aggregate: &A,
    ) -> impl Future<Output = Result<()>> + Send {
        async move {
            let json = serde_json::to_value(aggregate).map_err(error_core::ErrorObject::from)?;
            self.snapshots.write().await.insert(
                aggregate.id().to_string(),
                (json.to_string(), aggregate.version()),
            );
            Ok(())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn load_snapshot<A: Aggregate>(
        &self,
        aggregate_id: &str,
    ) -> impl Future<Output = Result<Option<(A, u64)>>> + Send {
        async move {
            let guard = self.snapshots.read().await;

            match guard.get(aggregate_id) {
                Some((json_str, version)) => {
                    let json_str = json_str.clone();
                    let version = *version;
                    drop(guard);
                    let aggregate: A = serde_json::from_str(&json_str).map_err(|e| {
                        helpers::aggregate_serialization_error(&format!(
                            "Failed to deserialize snapshot: {e}"
                        ))
                    })?;
                    Ok(Some((aggregate, version)))
                }
                None => Ok(None),
            }
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
    pub fn with_snapshots(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<MemorySnapshotStore>,
    ) -> Self {
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

        if let Some(ref ss) = self.snapshot_store
            && let Ok(Some((snapshot, version))) = ss.load_snapshot::<A>(aggregate_id).await
        {
            from_version = version + 1;
            aggregate = Some(snapshot);
            info!(
                aggregate_id = %aggregate_id,
                snapshot_version = version,
                "Loaded aggregate from snapshot"
            );
        }

        let events = self
            .event_store
            .load_events_from_version(aggregate_id, from_version)
            .await?;

        if let Some(mut agg) = aggregate {
            for event_data in &events {
                let event: A::Event =
                    serde_json::from_value(event_data.data.clone()).map_err(|e| {
                        helpers::aggregate_serialization_error(&format!(
                            "Failed to deserialize event: {e}"
                        ))
                    })?;
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
                    serde_json::from_value(e.data.clone()).map_err(|err| {
                        helpers::aggregate_serialization_error(&format!(
                            "Event deserialization failed: {err}"
                        ))
                    })
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
    pub async fn execute(
        &self,
        aggregate_id: &str,
        command: A::Command,
    ) -> Result<Vec<StoredEvent>> {
        self.execute_with_metadata(aggregate_id, command, EventMetadata::default())
            .await
    }

    /// 带因果上下文的命令执行
    ///
    /// 通过 `CausationContext` 自动传播 `correlation_id`、`causation_id`、
    /// `triggered_by` 等因果元数据，确保事件链可审计可追溯。
    ///
    /// # Errors
    /// 当聚合加载失败或命令执行失败时返回错误
    #[instrument(skip(self, command, context), fields(aggregate_id = %aggregate_id))]
    pub async fn execute_with_context(
        &self,
        aggregate_id: &str,
        command: A::Command,
        context: CausationContext,
    ) -> Result<Vec<StoredEvent>> {
        self.execute_with_metadata(aggregate_id, command, context.to_metadata())
            .await
    }

    /// 带自定义元数据的命令执行（内部实现）
    async fn execute_with_metadata(
        &self,
        aggregate_id: &str,
        command: A::Command,
        metadata: EventMetadata,
    ) -> Result<Vec<StoredEvent>> {
        let aggregate = self.load(aggregate_id).await?;

        let current_version = aggregate.version();
        let events = aggregate.execute(command).map_err(|e| {
            helpers::aggregate_business_rule(&format!("Command execution failed: {e}"))
        })?;

        if events.is_empty() {
            return Ok(Vec::new());
        }

        let stored_events: Result<Vec<StoredEvent>> = events
            .into_iter()
            .enumerate()
            .map(|(i, event)| {
                let event_json = serde_json::to_value(&event).map_err(|e| {
                    helpers::aggregate_serialization_error(&format!("事件序列化失败: {e}"))
                })?;

                Ok(StoredEvent {
                    id: format!("event:{}:{}", aggregate_id, current_version + i as u64 + 1),
                    event_type: A::event_type_for(&event),
                    aggregate_id: aggregate_id.to_string(),
                    aggregate_type: A::aggregate_type(),
                    data: event_json,
                    metadata: metadata.clone(),
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
            content_type: ContentType::Markdown,
            metadata: serde_json::json!({}),
        }));

        assert!(result.is_ok());
        let events = result.unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            DocumentEvent::Created(data) => {
                assert_eq!(data.title, "Test Document");
                assert_eq!(data.content_type, ContentType::Markdown);
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
            content_type: ContentType::Markdown,
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
            content_type: ContentType::Markdown,
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("Test", "doc_001"),
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
                assert!(data.changes.get_change("title").is_some());
                assert!(data.changes.get_change("content").is_none());
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
            content_type: ContentType::Plain,
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("Test", "doc_002"),
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
            content_type: ContentType::Code,
            metadata: serde_json::json!({"key": "value"}),
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("Test", "doc_003"),
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
            content_type: ContentType::Plain,
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("Test", "doc_004"),
        }));

        aggregate.apply(&DocumentEvent::Updated(DocumentUpdatedData {
            document_id: "doc_004".to_string(),
            changes: {
                let mut cs = ChangeSet::new();
                cs.add_change(
                    "title",
                    FieldChange::changed(
                        serde_json::Value::String("Original".to_string()),
                        serde_json::Value::String("Modified".to_string()),
                    ),
                );
                cs.add_change(
                    "content",
                    FieldChange::changed(
                        serde_json::Value::String("Original Content".to_string()),
                        serde_json::Value::String("New Content".to_string()),
                    ),
                );
                cs
            },
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("Test", "doc_004"),
        }));

        assert_eq!(aggregate.get_title(), "Modified");
        assert_eq!(aggregate.get_content(), "New Content");
        assert_eq!(aggregate.version(), 2);
    }

    #[test]
    fn test_apply_deleted_event_marks_document_deleted() {
        let mut aggregate = DocumentAggregate::new();

        aggregate.apply(&DocumentEvent::Created(DocumentCreatedData {
            document_id: "doc_005".to_string(),
            title: "To Delete".to_string(),
            content: "Content".to_string(),
            content_type: ContentType::Plain,
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("Test", "doc_005"),
        }));

        aggregate.apply(&DocumentEvent::Deleted(DocumentDeletedData {
            document_id: "doc_005".to_string(),
            reason: None,
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("Test", "doc_005"),
        }));

        assert!(matches!(aggregate.status, DocumentStatus::Deleted));
        assert_eq!(aggregate.version(), 2);
    }

    #[test]
    fn test_replay_reconstructs_state() {
        let events = vec![
            DocumentEvent::Created(DocumentCreatedData {
                document_id: "doc_006".to_string(),
                title: "Replayed Doc".to_string(),
                content: "Content".to_string(),
                content_type: ContentType::Markdown,
                metadata: serde_json::json!({}),
                occurred_at: Utc::now(),
                triggered_by: TriggeredBy::from_command("Test", "doc_006"),
            }),
            DocumentEvent::Updated(DocumentUpdatedData {
                document_id: "doc_006".to_string(),
                changes: {
                    let mut cs = ChangeSet::new();
                    cs.add_change(
                        "title",
                        FieldChange::changed(
                            serde_json::Value::String("Replayed Doc".to_string()),
                            serde_json::Value::String("Replayed Updated".to_string()),
                        ),
                    );
                    cs
                },
                occurred_at: Utc::now(),
                triggered_by: TriggeredBy::from_command("Test", "doc_006"),
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
                    content_type: ContentType::Markdown,
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
                    content_type: ContentType::Markdown,
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
            content_type: ContentType::Plain,
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
            triggered_by: TriggeredBy::from_command("Test", "snap_doc_001"),
        }));

        store.save_snapshot(&aggregate).await.unwrap();

        let (loaded, version) = store
            .load_snapshot::<DocumentAggregate>("snap_doc_001")
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

        let result = store
            .load_snapshot::<DocumentAggregate>("nonexistent")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_aggregate_error_display() {
        let err = AggregateError::InvalidState("Bad state".to_string());
        assert!(format!("{err}").contains("Bad state"));

        let err = AggregateError::VersionConflict {
            expected: 5,
            actual: 3,
        };
        let display = format!("{err}");
        assert!(display.contains('5') && display.contains('3'));
    }

    #[test]
    fn test_document_status_serialization() {
        let statuses = vec![
            DocumentStatus::Pending,
            DocumentStatus::Parsing,
            DocumentStatus::Indexed,
            DocumentStatus::Active,
            DocumentStatus::Archived,
            DocumentStatus::Deleted,
        ];

        for status in &statuses {
            let json = serde_json::to_string(status).expect("序列化失败");
            let deserialized: DocumentStatus = serde_json::from_str(&json).expect("反序列化失败");
            assert_eq!(*status, deserialized);
        }
    }

    #[tokio::test]
    async fn test_aggregate_with_snapshots_integration() {
        use crate::cqrs::event_store::InMemoryEventStore;

        let event_store = Arc::new(InMemoryEventStore::new());
        let snapshot_store = Arc::new(MemorySnapshotStore::new());
        let repository =
            AggregateRepository::<DocumentAggregate>::with_snapshots(event_store, snapshot_store);

        repository
            .execute(
                "doc_snapshot_001",
                DocumentCommand::Create(CreateDocumentData {
                    document_id: "doc_snapshot".to_string(),
                    title: "With Snapshot".to_string(),
                    content: "# Snapshot".to_string(),
                    content_type: ContentType::Markdown,
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
