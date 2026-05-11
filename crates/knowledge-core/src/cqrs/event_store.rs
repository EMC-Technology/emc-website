use crate::Result;
use crate::error::helpers;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};

/// 聚合根类型枚举（AP-B07 修复：替代 String 表示）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateType {
    /// 文档
    Document,
    /// 节点
    Node,
    /// 边
    Edge,
    /// 向量嵌入
    Embedding,
    /// 搜索
    Search,
    /// 用户
    User,
    /// 系统
    System,
}

impl AggregateType {
    /// 返回聚合根类型的字符串表示
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Node => "node",
            Self::Edge => "edge",
            Self::Embedding => "embedding",
            Self::Search => "search",
            Self::User => "user",
            Self::System => "system",
        }
    }
}

impl std::fmt::Display for AggregateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 事件类型枚举（AP-B07 修复：替代 String 表示）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EventType {
    /// 文档入库
    DocumentIngested,
    /// 文档解析
    DocumentParsed,
    /// 文档索引
    DocumentIndexed,
    /// 文档删除
    DocumentDeleted,
    /// 节点创建
    NodeCreated,
    /// 节点更新
    NodeUpdated,
    /// 节点删除
    NodeDeleted,
    /// 节点关联
    NodeLinked,
    /// 边创建
    EdgeCreated,
    /// 边删除
    EdgeDeleted,
    /// 搜索执行
    SearchPerformed,
    /// 查询执行
    QueryExecuted,
    /// 向量生成
    EmbeddingGenerated,
    /// 向量缓存
    EmbeddingCached,
    /// 用户操作
    UserAction,
    /// 系统健康检查
    SystemHealthCheck,
}

impl EventType {
    /// 返回事件类型的字符串表示
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::DocumentIngested => "DocumentIngested",
            Self::DocumentParsed => "DocumentParsed",
            Self::DocumentIndexed => "DocumentIndexed",
            Self::DocumentDeleted => "DocumentDeleted",
            Self::NodeCreated => "NodeCreated",
            Self::NodeUpdated => "NodeUpdated",
            Self::NodeDeleted => "NodeDeleted",
            Self::NodeLinked => "NodeLinked",
            Self::EdgeCreated => "EdgeCreated",
            Self::EdgeDeleted => "EdgeDeleted",
            Self::SearchPerformed => "SearchPerformed",
            Self::QueryExecuted => "QueryExecuted",
            Self::EmbeddingGenerated => "EmbeddingGenerated",
            Self::EmbeddingCached => "EmbeddingCached",
            Self::UserAction => "UserAction",
            Self::SystemHealthCheck => "SystemHealthCheck",
        }
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 事件存储中持久化的事件记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// 事件唯一标识符
    pub id: String,
    /// 事件类型
    pub event_type: EventType,
    /// 所属聚合根 ID
    pub aggregate_id: String,
    /// 聚合根类型
    pub aggregate_type: AggregateType,
    /// 事件负载数据（JSON）
    pub data: serde_json::Value,
    /// 事件元数据
    pub metadata: EventMetadata,
    /// 事件版本号（乐观锁）
    pub version: u64,
    /// 事件时间戳
    pub timestamp: DateTime<Utc>,
}

/// 事件触发源 —— 描述"谁触发了这个事件"
///
/// 与 `causation_id`（ID 级引用）互补，`triggered_by` 提供类型级别的
/// 触发源描述，便于审计查询和可观测性，无需回溯 ID 链即可理解因果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail")]
pub enum TriggeredBy {
    /// 由命令直接触发（CQRS 写路径的标准场景）
    Command {
        /// 命令类型名称（如 `CreateDocument`）
        command_type: String,
        /// 命令目标聚合根 ID
        aggregate_id: String,
    },
    /// 由另一个领域事件触发（Saga / Process Manager 场景）
    Event {
        /// 触发事件类型名称（如 `DocumentIngested`）
        event_type: String,
        /// 触发事件的 ID
        event_id: String,
        /// 触发事件所属聚合根 ID
        aggregate_id: String,
    },
    /// 由系统内部调度触发（定时任务、后台 Worker）
    System {
        /// 系统组件名称（如 `index-scheduler`）
        component: String,
        /// 调度原因描述
        reason: String,
    },
    /// 由外部事件触发（Webhook、消息队列消费）
    External {
        /// 外部来源标识（如 `webhook/github`）
        source: String,
        /// 外部事件 ID（用于去重和追踪）
        external_id: Option<String>,
    },
}

impl TriggeredBy {
    /// 从命令创建触发源
    #[must_use]
    pub fn from_command(command_type: &str, aggregate_id: &str) -> Self {
        Self::Command {
            command_type: command_type.to_string(),
            aggregate_id: aggregate_id.to_string(),
        }
    }

    /// 从事件创建触发源（Saga 链式反应场景）
    #[must_use]
    pub fn from_event(event_type: &str, event_id: &str, aggregate_id: &str) -> Self {
        Self::Event {
            event_type: event_type.to_string(),
            event_id: event_id.to_string(),
            aggregate_id: aggregate_id.to_string(),
        }
    }

    /// 从系统组件创建触发源
    #[must_use]
    pub fn from_system(component: &str, reason: &str) -> Self {
        Self::System {
            component: component.to_string(),
            reason: reason.to_string(),
        }
    }

    /// 从外部来源创建触发源
    #[must_use]
    pub fn from_external(source: &str, external_id: Option<&str>) -> Self {
        Self::External {
            source: source.to_string(),
            external_id: external_id.map(std::string::ToString::to_string),
        }
    }
}

impl Default for TriggeredBy {
    fn default() -> Self {
        Self::System {
            component: "unknown".to_string(),
            reason: "unspecified".to_string(),
        }
    }
}

/// 因果链上下文 —— 在命令/事件处理链中传播因果信息
///
/// # 设计原则
///
/// 1. `correlation_id` 在业务流程入口生成，整个流程中不变
/// 2. `causation_id` 在每个处理步骤中更新为当前命令/事件的 ID
/// 3. `triggered_by` 在每个步骤中更新为当前触发源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausationContext {
    /// 业务流程关联 ID（流程入口生成，全程不变）
    pub correlation_id: String,
    /// 直接因果 ID（每步更新为触发源的 ID）
    pub causation_id: String,
    /// 触发源描述
    pub triggered_by: TriggeredBy,
    /// 操作用户
    pub user_id: Option<String>,
    /// 分布式追踪 ID
    pub trace_id: Option<String>,
}

impl CausationContext {
    /// 从 HTTP 请求入口创建新的因果上下文
    #[must_use]
    pub fn new_from_request(
        command_type: &str,
        aggregate_id: &str,
        user_id: Option<String>,
        trace_id: Option<String>,
    ) -> Self {
        let correlation_id = uuid::Uuid::new_v4().to_string();
        let causation_id = correlation_id.clone();
        Self {
            correlation_id,
            causation_id,
            triggered_by: TriggeredBy::from_command(command_type, aggregate_id),
            user_id,
            trace_id,
        }
    }

    /// 从已有事件派生新的因果上下文（Saga 链式反应）
    #[must_use]
    pub fn derive_from_event(
        parent_event_id: &str,
        parent_event_type: &str,
        parent_aggregate_id: &str,
        parent_correlation_id: &str,
        parent_user_id: Option<String>,
        parent_trace_id: Option<String>,
    ) -> Self {
        Self {
            correlation_id: parent_correlation_id.to_string(),
            causation_id: parent_event_id.to_string(),
            triggered_by: TriggeredBy::from_event(
                parent_event_type,
                parent_event_id,
                parent_aggregate_id,
            ),
            user_id: parent_user_id,
            trace_id: parent_trace_id,
        }
    }

    /// 从系统调度创建因果上下文
    #[must_use]
    pub fn new_from_system(component: &str, reason: &str) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        Self {
            correlation_id: id.clone(),
            causation_id: id,
            triggered_by: TriggeredBy::from_system(component, reason),
            user_id: None,
            trace_id: None,
        }
    }

    /// 转换为 EventMetadata
    #[must_use]
    pub fn to_metadata(&self) -> EventMetadata {
        EventMetadata {
            causation_id: Some(self.causation_id.clone()),
            correlation_id: Some(self.correlation_id.clone()),
            triggered_by: self.triggered_by.clone(),
            user_id: self.user_id.clone(),
            trace_id: self.trace_id.clone(),
        }
    }
}

/// 事件元数据，用于跨服务追踪和因果链分析
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventMetadata {
    /// 因果标识符 —— 指向直接触发本事件的命令/事件 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    /// 关联标识符 —— 同一业务流程产生的所有事件共享此 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    /// 触发源描述 —— 结构化描述"谁触发了我"（Axiom-4 必须字段）
    pub triggered_by: TriggeredBy,
    /// 用户标识符
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// 分布式追踪标识符（OpenTelemetry trace_id）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

/// 事件存储抽象特征，定义领域事件的追加与查询操作
#[async_trait]
pub trait EventStore: Send + Sync {
    /// 向指定聚合根追加一批事件
    async fn append_events(
        &self,
        aggregate_id: &str,
        events: Vec<StoredEvent>,
        expected_version: Option<u64>,
    ) -> Result<()>;

    /// 加载指定聚合根的全部事件
    async fn load_events(&self, aggregate_id: &str) -> Result<Vec<StoredEvent>>;

    /// 从指定版本号开始加载事件
    async fn load_events_from_version(
        &self,
        aggregate_id: &str,
        from_version: u64,
    ) -> Result<Vec<StoredEvent>>;

    /// 按事件类型和时间范围查询事件
    async fn load_events_by_type(
        &self,
        event_type: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<StoredEvent>>;
}

/// 基于内存的事件存储实现，适用于测试和开发环境
pub struct InMemoryEventStore {
    events: Arc<tokio::sync::RwLock<Vec<StoredEvent>>>,
}

impl InMemoryEventStore {
    /// 创建空的内存事件存储实例
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// 清空所有事件
    ///
    /// 仅用于测试环境。生产环境中事件存储必须为 append-only，
    /// 调用此方法将违反 Axiom-2（事件唯一变异）。
    #[cfg(test)]
    pub async fn clear(&self) {
        let mut events = self.events.write().await;
        events.clear();
    }

    /// 返回当前事件总数
    pub async fn event_count(&self) -> usize {
        self.events.read().await.len()
    }
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    #[instrument(skip(self, events), fields(aggregate_id = %aggregate_id, event_count = events.len()))]
    async fn append_events(
        &self,
        aggregate_id: &str,
        events: Vec<StoredEvent>,
        expected_version: Option<u64>,
    ) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let mut store = self.events.write().await;

        let current_version = store
            .iter()
            .rfind(|e| e.aggregate_id == aggregate_id)
            .map_or(0, |e| e.version);

        if let Some(expected) = expected_version
            && current_version != expected
        {
            return Err(helpers::validation_error(
                &format!(
                    "Version conflict for aggregate {aggregate_id}: expected {expected}, actual {current_version}"
                ),
                "OptimisticLockError",
            ));
        }

        for event in &events {
            if event.aggregate_id != aggregate_id {
                return Err(helpers::validation_error(
                    &format!(
                        "Event aggregate_id {} does not match requested {}",
                        event.aggregate_id, aggregate_id
                    ),
                    "InvalidAggregateId",
                ));
            }
        }

        let mut validated_events = events.clone();

        for (next_version, event) in (current_version + 1..).zip(validated_events.iter_mut()) {
            event.version = next_version;

            if event.timestamp == DateTime::<Utc>::MIN_UTC {
                event.timestamp = Utc::now();
            }
        }

        store.extend(validated_events);
        drop(store);

        info!(
            aggregate_id = %aggregate_id,
            events_appended = events.len(),
            new_version = current_version + events.len() as u64,
            "Events appended successfully"
        );

        Ok(())
    }

    #[instrument(skip(self), fields(aggregate_id = %aggregate_id))]
    async fn load_events(&self, aggregate_id: &str) -> Result<Vec<StoredEvent>> {
        let events: Vec<StoredEvent> = self
            .events
            .read()
            .await
            .iter()
            .filter(|e| e.aggregate_id == aggregate_id)
            .cloned()
            .collect();

        info!(
            aggregate_id = %aggregate_id,
            event_count = events.len(),
            "Events loaded"
        );

        Ok(events)
    }

    #[instrument(skip(self), fields(aggregate_id = %aggregate_id, from_version = from_version))]
    async fn load_events_from_version(
        &self,
        aggregate_id: &str,
        from_version: u64,
    ) -> Result<Vec<StoredEvent>> {
        let events: Vec<StoredEvent> = self
            .events
            .read()
            .await
            .iter()
            .filter(|e| e.aggregate_id == aggregate_id && e.version >= from_version)
            .cloned()
            .collect();

        Ok(events)
    }

    #[instrument(skip(self), fields(event_type = %event_type, from = %from, to = %to))]
    async fn load_events_by_type(
        &self,
        event_type: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<StoredEvent>> {
        let events: Vec<StoredEvent> = self
            .events
            .read()
            .await
            .iter()
            .filter(|e| {
                e.event_type.as_str() == event_type && e.timestamp >= from && e.timestamp <= to
            })
            .cloned()
            .collect();

        Ok(events)
    }
}

/// 基于 SurrealDB 的持久化事件存储实现
#[cfg(feature = "db")]
pub struct SurrealEventStore {
    db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>,
}

#[cfg(feature = "db")]
impl SurrealEventStore {
    /// 使用指定的 SurrealDB 连接创建事件存储实例
    pub const fn new(db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>) -> Self {
        Self { db }
    }
}

#[cfg(feature = "db")]
#[async_trait]
impl EventStore for SurrealEventStore {
    #[instrument(skip(self, events), fields(aggregate_id = %aggregate_id, event_count = events.len()))]
    async fn append_events(
        &self,
        aggregate_id: &str,
        events: Vec<StoredEvent>,
        expected_version: Option<u64>,
    ) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let events_count = events.len();

        let mut queries = Vec::with_capacity(events.len() + 2);
        let mut all_bindings = serde_json::Map::new();

        all_bindings.insert("aggregate_id".to_string(), serde_json::json!(aggregate_id));

        queries.push(
            "LET $current_version = (SELECT VALUE version FROM event WHERE aggregate_id = $aggregate_id ORDER BY version DESC LIMIT 1)[0] OR 0".to_string()
        );

        if let Some(expected) = expected_version {
            queries.push(format!(
                "IF $current_version != {expected} {{ THROW 'OptimisticLockError:' + $current_version }} }}"
            ));
        }

        for (i, mut event) in events.into_iter().enumerate() {
            if event.aggregate_id != aggregate_id {
                return Err(helpers::validation_error(
                    &format!(
                        "Event aggregate_id {} does not match requested {}",
                        event.aggregate_id, aggregate_id
                    ),
                    "InvalidAggregateId",
                ));
            }

            event.version = i as u64;
            if event.timestamp == DateTime::<Utc>::MIN_UTC {
                event.timestamp = Utc::now();
            }

            let event_json = serde_json::to_value(&event).map_err(error_core::ErrorObject::from)?;
            queries.push(format!("CREATE event CONTENT $event_{i}"));
            all_bindings.insert(format!("event_{i}"), event_json);
        }

        let mut sql = String::from("BEGIN TRANSACTION;\n");
        for query in &queries {
            sql.push_str(query);
            sql.push_str(";\n");
        }
        sql.push_str("COMMIT TRANSACTION;");

        let result = self
            .db
            .query(sql)
            .bind(serde_json::Value::Object(all_bindings))
            .await
            .map_err(error_core::ErrorObject::from);

        match result {
            Ok(_) => {
                info!(aggregate_id = %aggregate_id, event_count = events_count, "事件追加成功");
                Ok(())
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("OptimisticLockError") {
                    let actual_version: u64 = err_msg
                        .split(':')
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    Err(helpers::validation_error(
                        &format!(
                            "Version conflict for aggregate {aggregate_id}: expected {}, actual {actual_version}",
                            expected_version.unwrap_or(0)
                        ),
                        "OptimisticLockError",
                    ))
                } else {
                    Err(e)
                }
            }
        }
    }

    #[instrument(skip(self), fields(aggregate_id = %aggregate_id))]
    async fn load_events(&self, aggregate_id: &str) -> Result<Vec<StoredEvent>> {
        let mut response = self
            .db
            .query("SELECT * FROM event WHERE aggregate_id = $id ORDER BY version ASC")
            .bind(("id", aggregate_id))
            .await?;

        let events: Vec<StoredEvent> = response.take(0)?;

        Ok(events)
    }

    #[instrument(skip(self), fields(aggregate_id = %aggregate_id, from_version = from_version))]
    async fn load_events_from_version(
        &self,
        aggregate_id: &str,
        from_version: u64,
    ) -> Result<Vec<StoredEvent>> {
        let mut response = self
            .db
            .query("SELECT * FROM event WHERE aggregate_id = $id AND version >= $version ORDER BY version ASC")
            .bind(("id", aggregate_id))
            .bind(("version", from_version))
            .await?;

        let events: Vec<StoredEvent> = response.take(0)?;

        Ok(events)
    }

    #[instrument(skip(self), fields(event_type = %event_type, from = %from, to = %to))]
    async fn load_events_by_type(
        &self,
        event_type: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<StoredEvent>> {
        let mut response = self
            .db
            .query("SELECT * FROM event WHERE event_type = $type AND timestamp >= $from AND timestamp <= $to ORDER BY timestamp ASC")
            .bind(("type", event_type))
            .bind(("from", from.to_rfc3339()))
            .bind(("to", to.to_rfc3339()))
            .await?;

        let events: Vec<StoredEvent> = response.take(0)?;

        Ok(events)
    }
}

/// 变更操作类型
///
/// 描述字段值变更的具体操作语义，符合 POP Axiom-2（事件唯一变异）
/// 对显式 operator 的要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOperator {
    /// 设置字段值（覆盖旧值）
    Set,
    /// 向集合追加元素
    Append,
    /// 从集合移除元素
    Remove,
    /// 替换集合中的元素
    Replace,
}

/// 字段级变更记录
///
/// 记录单个字段从 `old_value` 到 `new_value` 的变更，
/// 用于审计追踪和差异对比。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldChange {
    /// 变更前的值（None 表示字段新增）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<serde_json::Value>,
    /// 变更后的值（None 表示字段删除）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<serde_json::Value>,
    /// 变更操作类型
    #[serde(default = "default_change_operator")]
    pub operator: ChangeOperator,
}

fn default_change_operator() -> ChangeOperator {
    ChangeOperator::Set
}

impl FieldChange {
    /// 创建字段变更记录
    #[must_use]
    pub fn new(old: Option<serde_json::Value>, new: Option<serde_json::Value>) -> Self {
        let operator = match (&old, &new) {
            (Some(_), None) => ChangeOperator::Remove,
            _ => ChangeOperator::Set,
        };
        Self {
            old_value: old,
            new_value: new,
            operator,
        }
    }

    /// 从旧值和新值创建（值都存在）
    #[must_use]
    pub fn changed(old: serde_json::Value, new: serde_json::Value) -> Self {
        Self {
            old_value: Some(old),
            new_value: Some(new),
            operator: ChangeOperator::Set,
        }
    }

    /// 字段新增
    #[must_use]
    pub fn added(new: serde_json::Value) -> Self {
        Self {
            old_value: None,
            new_value: Some(new),
            operator: ChangeOperator::Set,
        }
    }

    /// 字段删除
    #[must_use]
    pub fn removed(old: serde_json::Value) -> Self {
        Self {
            old_value: Some(old),
            new_value: None,
            operator: ChangeOperator::Remove,
        }
    }

    /// 追加操作
    #[must_use]
    pub fn appended(new: serde_json::Value) -> Self {
        Self {
            old_value: None,
            new_value: Some(new),
            operator: ChangeOperator::Append,
        }
    }

    /// 是否为实际变更（排除值相同的伪变更）
    #[must_use]
    pub fn is_actual_change(&self) -> bool {
        match (&self.old_value, &self.new_value) {
            (Some(old), Some(new)) => old != new,
            (None, None) => false,
            _ => true,
        }
    }
}

/// 变更集 —— 多字段的变更汇总
///
/// 以字段名为键，记录每个字段的变更前后值。
/// 仅包含实际发生变更的字段，未变更的字段不出现在集合中。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChangeSet {
    /// 字段变更映射
    pub changes: std::collections::HashMap<String, FieldChange>,
}

impl ChangeSet {
    /// 创建空变更集
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加字段变更
    pub fn add_change(&mut self, field: impl Into<String>, change: FieldChange) {
        if change.is_actual_change() {
            self.changes.insert(field.into(), change);
        }
    }

    /// 是否包含任何变更
    #[must_use]
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    /// 获取变更字段数量
    #[must_use]
    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    /// 获取指定字段的变更
    #[must_use]
    pub fn get_change(&self, field: &str) -> Option<&FieldChange> {
        self.changes.get(field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_test_event(aggregate_id: &str, event_type: &str, version: u64) -> StoredEvent {
        let et = match event_type {
            "NodeCreated" => EventType::NodeCreated,
            _ => EventType::DocumentIngested,
        };
        StoredEvent {
            id: format!("event:{}", Uuid::new_v4()),
            event_type: et,
            aggregate_id: aggregate_id.to_string(),
            aggregate_type: AggregateType::Document,
            data: serde_json::json!({"title": "Test"}),
            metadata: EventMetadata {
                causation_id: Some(Uuid::new_v4().to_string()),
                correlation_id: Some(Uuid::new_v4().to_string()),
                triggered_by: TriggeredBy::Command {
                    command_type: format!("{event_type}Command"),
                    aggregate_id: aggregate_id.to_string(),
                },
                user_id: Some("user_001".to_string()),
                trace_id: Some("trace-123".to_string()),
            },
            version,
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_append_single_event() {
        let store = InMemoryEventStore::new();
        let event = create_test_event("doc_001", "DocumentCreated", 0);

        let result = store.append_events("doc_001", vec![event], None).await;
        assert!(result.is_ok());

        assert_eq!(store.event_count().await, 1);
    }

    #[tokio::test]
    async fn test_append_multiple_events_sequentially() {
        let store = InMemoryEventStore::new();

        let event1 = create_test_event("doc_001", "DocumentCreated", 0);
        let event2 = create_test_event("doc_001", "TitleUpdated", 0);

        store
            .append_events("doc_001", vec![event1], None)
            .await
            .unwrap();
        store
            .append_events("doc_001", vec![event2], None)
            .await
            .unwrap();

        assert_eq!(store.event_count().await, 2);
    }

    #[tokio::test]
    async fn test_load_events_for_aggregate() {
        let store = InMemoryEventStore::new();

        store
            .append_events(
                "doc_001",
                vec![create_test_event("doc_001", "DocumentCreated", 0)],
                None,
            )
            .await
            .unwrap();

        store
            .append_events(
                "doc_002",
                vec![create_test_event("doc_002", "DocumentCreated", 0)],
                None,
            )
            .await
            .unwrap();

        let events = store.load_events("doc_001").await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].aggregate_id, "doc_001");
    }

    #[tokio::test]
    async fn test_optimistic_lock_success() {
        let store = InMemoryEventStore::new();

        store
            .append_events(
                "doc_001",
                vec![create_test_event("doc_001", "DocumentCreated", 0)],
                None,
            )
            .await
            .unwrap();

        let result = store
            .append_events(
                "doc_001",
                vec![create_test_event("doc_001", "TitleUpdated", 0)],
                Some(1),
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(store.event_count().await, 2);
    }

    #[tokio::test]
    async fn test_optimistic_lock_conflict() {
        let store = InMemoryEventStore::new();

        store
            .append_events(
                "doc_001",
                vec![create_test_event("doc_001", "DocumentCreated", 0)],
                None,
            )
            .await
            .unwrap();

        let result = store
            .append_events(
                "doc_001",
                vec![create_test_event("doc_001", "TitleUpdated", 0)],
                Some(0),
            )
            .await;

        assert!(result.is_err());
        let err_msg = format!("{:?}", result.err());
        assert!(err_msg.contains("Version conflict") || err_msg.contains("OptimisticLockError"));
    }

    #[tokio::test]
    async fn test_load_events_from_version() {
        let store = InMemoryEventStore::new();

        store
            .append_events(
                "doc_001",
                vec![create_test_event("doc_001", "DocumentCreated", 0)],
                None,
            )
            .await
            .unwrap();

        store
            .append_events(
                "doc_001",
                vec![
                    create_test_event("doc_001", "TitleUpdated", 0),
                    create_test_event("doc_001", "ContentUpdated", 0),
                ],
                None,
            )
            .await
            .unwrap();

        let events = store.load_events_from_version("doc_001", 2).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, EventType::DocumentIngested);
        assert_eq!(events[1].event_type, EventType::DocumentIngested);
    }

    #[tokio::test]
    async fn test_load_events_by_type() {
        let store = InMemoryEventStore::new();
        let now = Utc::now();

        store
            .append_events(
                "doc_001",
                vec![StoredEvent {
                    id: format!("event:{}", Uuid::new_v4()),
                    event_type: EventType::DocumentIngested,
                    aggregate_id: "doc_001".to_string(),
                    aggregate_type: AggregateType::Document,
                    data: serde_json::json!({}),
                    metadata: EventMetadata::default(),
                    version: 1,
                    timestamp: now,
                }],
                None,
            )
            .await
            .unwrap();

        store
            .append_events(
                "doc_002",
                vec![StoredEvent {
                    id: format!("event:{}", Uuid::new_v4()),
                    event_type: EventType::NodeCreated,
                    aggregate_id: "doc_002".to_string(),
                    aggregate_type: AggregateType::Node,
                    data: serde_json::json!({}),
                    metadata: EventMetadata::default(),
                    version: 1,
                    timestamp: now,
                }],
                None,
            )
            .await
            .unwrap();

        let from = now - chrono::Duration::seconds(1);
        let to = now + chrono::Duration::seconds(1);

        let events = store
            .load_events_by_type("DocumentIngested", from, to)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::DocumentIngested);
    }

    #[tokio::test]
    async fn test_empty_events_list_noop() {
        let store = InMemoryEventStore::new();

        let result = store.append_events("doc_001", vec![], None).await;
        assert!(result.is_ok());
        assert_eq!(store.event_count().await, 0);
    }

    #[tokio::test]
    async fn test_mismatched_aggregate_id_error() {
        let store = InMemoryEventStore::new();

        let event = create_test_event("doc_001", "DocumentCreated", 0);
        let result = store.append_events("doc_002", vec![event], None).await;

        assert!(result.is_err());
        let err_msg = format!("{:?}", result.err());
        assert!(err_msg.contains("InvalidAggregateId"));
    }

    #[tokio::test]
    async fn test_version_auto_incrementing() {
        let store = InMemoryEventStore::new();

        store
            .append_events(
                "doc_001",
                vec![create_test_event("doc_001", "DocumentCreated", 0)],
                None,
            )
            .await
            .unwrap();

        store
            .append_events(
                "doc_001",
                vec![
                    create_test_event("doc_001", "Event2", 0),
                    create_test_event("doc_001", "Event3", 0),
                ],
                None,
            )
            .await
            .unwrap();

        let events = store.load_events("doc_001").await.unwrap();
        assert_eq!(events[0].version, 1);
        assert_eq!(events[1].version, 2);
        assert_eq!(events[2].version, 3);
    }

    #[tokio::test]
    async fn test_clear_store() {
        let store = InMemoryEventStore::new();

        store
            .append_events(
                "doc_001",
                vec![create_test_event("doc_001", "DocumentCreated", 0)],
                None,
            )
            .await
            .unwrap();

        assert_eq!(store.event_count().await, 1);

        store.clear().await;
        assert_eq!(store.event_count().await, 0);
    }

    #[tokio::test]
    async fn test_event_metadata_serialization() {
        let metadata = EventMetadata {
            causation_id: Some("cmd-123".to_string()),
            correlation_id: Some("corr-456".to_string()),
            triggered_by: TriggeredBy::Command {
                command_type: "TestCommand".to_string(),
                aggregate_id: "doc_001".to_string(),
            },
            user_id: Some("user-789".to_string()),
            trace_id: Some("trace-abc".to_string()),
        };

        let json = serde_json::to_value(&metadata).expect("序列化失败");
        assert_eq!(
            json.get("causation_id").unwrap().as_str().unwrap(),
            "cmd-123"
        );
        assert_eq!(
            json.get("correlation_id").unwrap().as_str().unwrap(),
            "corr-456"
        );

        let deserialized: EventMetadata = serde_json::from_value(json).expect("反序列化失败");
        assert_eq!(deserialized.causation_id, metadata.causation_id);
        assert_eq!(deserialized.user_id, metadata.user_id);
    }

    #[tokio::test]
    async fn test_event_metadata_default_has_triggered_by() {
        let metadata = EventMetadata::default();
        let json = serde_json::to_value(&metadata).expect("序列化失败");

        assert!(json.get("causation_id").is_none());
        assert!(json.get("correlation_id").is_none());
        assert!(json.get("user_id").is_none());
        assert!(json.get("trace_id").is_none());
        assert!(
            json.get("triggered_by").is_some(),
            "triggered_by 是 Axiom-4 必须字段，不应为 None"
        );
    }

    #[test]
    fn test_in_memory_event_store_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<InMemoryEventStore>();
    }

    #[tokio::test]
    async fn test_concurrent_append_same_aggregate() {
        let store = Arc::new(InMemoryEventStore::new());
        let store_clone1 = Arc::clone(&store);
        let store_clone2 = Arc::clone(&store);

        let handle1 = tokio::spawn(async move {
            store_clone1
                .append_events(
                    "doc_001",
                    vec![create_test_event("doc_001", "Event1", 0)],
                    None,
                )
                .await
        });

        let handle2 = tokio::spawn(async move {
            store_clone2
                .append_events(
                    "doc_001",
                    vec![create_test_event("doc_001", "Event2", 0)],
                    None,
                )
                .await
        });

        let (result1, result2) = tokio::join!(handle1, handle2);

        assert!(result1.expect("Task 1 panicked").is_ok());
        assert!(result2.expect("Task 2 panicked").is_ok());
        assert_eq!(store.event_count().await, 2);
    }
}
