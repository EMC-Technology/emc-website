use crate::Result;
use crate::error::helpers;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};

/// 事件存储中持久化的事件记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// 事件唯一标识符
    pub id: String,
    /// 事件类型名称
    pub event_type: String,
    /// 所属聚合根 ID
    pub aggregate_id: String,
    /// 聚合根类型名称
    pub aggregate_type: String,
    /// 事件负载数据（JSON）
    pub data: serde_json::Value,
    /// 事件元数据
    pub metadata: EventMetadata,
    /// 事件版本号（乐观锁）
    pub version: u64,
    /// 事件时间戳
    pub timestamp: DateTime<Utc>,
}

/// 事件元数据，用于跨服务追踪和因果链分析
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventMetadata {
    /// 因果标识符
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    /// 关联标识符
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    /// 用户标识符
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// 分布式追踪标识符
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

        if let Some(expected) = expected_version {
            if current_version != expected {
                return Err(helpers::validation_error(
                    &format!(
                        "Version conflict for aggregate {aggregate_id}: expected {expected}, actual {current_version}"
                    ),
                    "OptimisticLockError",
                ));
            }
        }

        for event in &events {
            if event.aggregate_id != aggregate_id {
                return Err(helpers::validation_error(
                    "InvalidAggregateId",
                    &format!(
                        "Event aggregate_id {} does not match requested {}",
                        event.aggregate_id, aggregate_id
                    ),
                ));
            }
        }

        let mut next_version = current_version + 1;
        let mut validated_events = events.clone();

        for event in &mut validated_events {
            event.version = next_version;
            next_version += 1;

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
        let events: Vec<StoredEvent> = self.events.read().await
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
        let events: Vec<StoredEvent> = self.events.read().await
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
        let events: Vec<StoredEvent> = self.events.read().await
            .iter()
            .filter(|e| e.event_type == event_type && e.timestamp >= from && e.timestamp <= to)
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

        let mut version_response = self
            .db
            .query("SELECT VALUE version FROM event WHERE aggregate_id = $id ORDER BY version DESC LIMIT 1")
            .bind(("id", aggregate_id.to_owned()))
            .await
            .map_err(error_core::ErrorObject::from)?;

        let versions: Vec<u64> = version_response
            .take(0)
            .map_err(error_core::ErrorObject::from)?;
        let current_version = versions.into_iter().next();

        let current = current_version.unwrap_or(0);

        if let Some(expected) = expected_version {
            if current != expected {
                return Err(helpers::validation_error(
                    &format!(
                        "Version conflict for aggregate {aggregate_id}: expected {expected}, actual {current}"
                    ),
                    "OptimisticLockError",
                ));
            }
        }

        let mut next_version = current + 1;
        let mut queries = Vec::with_capacity(events.len());
        let mut bindings = Vec::with_capacity(events.len());

        let events_count = events.len();

        for (i, mut event) in events.into_iter().enumerate() {
            if event.aggregate_id != aggregate_id {
                return Err(helpers::validation_error(
                    "InvalidAggregateId",
                    &format!(
                        "Event aggregate_id {} does not match requested {}",
                        event.aggregate_id, aggregate_id
                    ),
                ));
            }

            event.version = next_version;
            next_version += 1;

            if event.timestamp == DateTime::<Utc>::MIN_UTC {
                event.timestamp = Utc::now();
            }

            let event_json = serde_json::to_value(&event).map_err(error_core::ErrorObject::from)?;
            queries.push(format!("CREATE event CONTENT $event_{i}"));
            bindings.push(serde_json::json!({ format!("event_{i}"): event_json }));
        }

        let mut sql = String::from("BEGIN TRANSACTION;\n");
        let mut all_bindings = serde_json::Map::new();
        for (i, query) in queries.iter().enumerate() {
            sql.push_str(query);
            sql.push_str(";\n");
            if let serde_json::Value::Object(map) = &bindings[i] {
                for (k, v) in map {
                    all_bindings.insert(k.clone(), v.clone());
                }
            }
        }
        sql.push_str("COMMIT TRANSACTION;");

        self.db
            .query(sql)
            .bind(serde_json::Value::Object(all_bindings))
            .await
            .map_err(error_core::ErrorObject::from)?;

        info!(
            aggregate_id = %aggregate_id,
            events_appended = events_count,
            new_version = current + events_count as u64,
            "Events appended to SurrealDB"
        );

        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_test_event(aggregate_id: &str, event_type: &str, version: u64) -> StoredEvent {
        StoredEvent {
            id: format!("event:{}", Uuid::new_v4()),
            event_type: event_type.to_string(),
            aggregate_id: aggregate_id.to_string(),
            aggregate_type: "document".to_string(),
            data: serde_json::json!({"title": "Test"}),
            metadata: EventMetadata {
                causation_id: Some(Uuid::new_v4().to_string()),
                correlation_id: Some(Uuid::new_v4().to_string()),
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
        assert_eq!(events[0].event_type, "TitleUpdated");
        assert_eq!(events[1].event_type, "ContentUpdated");
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
                    event_type: "DocumentCreated".to_string(),
                    aggregate_id: "doc_001".to_string(),
                    aggregate_type: "document".to_string(),
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
                    event_type: "NodeCreated".to_string(),
                    aggregate_id: "doc_002".to_string(),
                    aggregate_type: "node".to_string(),
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
            .load_events_by_type("DocumentCreated", from, to)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "DocumentCreated");
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
    async fn test_event_metadata_default_skips_none_fields() {
        let metadata = EventMetadata::default();
        let json = serde_json::to_value(&metadata).expect("序列化失败");

        assert!(json.get("causation_id").is_none());
        assert!(json.get("correlation_id").is_none());
        assert!(json.get("user_id").is_none());
        assert!(json.get("trace_id").is_none());
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
