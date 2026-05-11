//! 社区摘要存储适配器

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use error_core::helpers;
use knowledge_core::model::{CommunitySummary, RecordIdType};

/// 社区摘要存储 trait
///
/// 详见文档: §5.1 | 用例: UC-052
pub trait CommunitySummaryStore: Send + Sync {
    fn save(&self, summary: &CommunitySummary) -> impl Future<Output = crate::Result<()>> + Send;
    fn save_batch(
        &self,
        summaries: &[CommunitySummary],
    ) -> impl Future<Output = crate::Result<()>> + Send;
    fn get_by_community(
        &self,
        community_id: &RecordIdType,
    ) -> impl Future<Output = crate::Result<Option<CommunitySummary>>> + Send;
    fn get_all(&self) -> impl Future<Output = crate::Result<Vec<CommunitySummary>>> + Send;
    fn delete_by_community(
        &self,
        community_id: &RecordIdType,
    ) -> impl Future<Output = crate::Result<()>> + Send;
}

/// 内存存储（测试用）
///
/// 详见文档: §5.1 | 用例: UC-052
pub struct InMemoryStore {
    summaries: tokio::sync::RwLock<HashMap<RecordIdType, CommunitySummary>>,
}

impl InMemoryStore {
    /// 创建内存存储
    ///
    /// 详见文档: §5.1 | 用例: UC-052 | 方法: M-075
    #[must_use]
    pub fn new() -> Self {
        Self {
            summaries: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::manual_async_fn)]
impl CommunitySummaryStore for InMemoryStore {
    #[allow(clippy::manual_async_fn)]
    fn save(&self, summary: &CommunitySummary) -> impl Future<Output = crate::Result<()>> + Send {
        async move {
            let mut map = self.summaries.write().await;
            map.insert(summary.community_id.clone(), summary.clone());
            Ok(())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn save_batch(
        &self,
        summaries: &[CommunitySummary],
    ) -> impl Future<Output = crate::Result<()>> + Send {
        async move {
            let mut map = self.summaries.write().await;
            for summary in summaries {
                map.insert(summary.community_id.clone(), summary.clone());
            }
            Ok(())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn get_by_community(
        &self,
        community_id: &RecordIdType,
    ) -> impl Future<Output = crate::Result<Option<CommunitySummary>>> + Send {
        async move {
            let map = self.summaries.read().await;
            Ok(map.get(community_id).cloned())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn get_all(&self) -> impl Future<Output = crate::Result<Vec<CommunitySummary>>> + Send {
        async move {
            let map = self.summaries.read().await;
            Ok(map.values().cloned().collect())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn delete_by_community(
        &self,
        community_id: &RecordIdType,
    ) -> impl Future<Output = crate::Result<()>> + Send {
        async move {
            let mut map = self.summaries.write().await;
            map.remove(community_id);
            Ok(())
        }
    }
}

/// `SurrealDB` 存储适配器
///
/// 详见文档: §5.2 | 用例: UC-053
#[cfg(feature = "db")]
pub struct SurrealSummaryStore {
    db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>,
}

#[cfg(feature = "db")]
impl SurrealSummaryStore {
    /// 创建 `SurrealDB` 存储适配器
    ///
    /// 详见文档: §5.2 | 用例: UC-053 | 方法: M-076
    #[must_use]
    pub fn new(db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>) -> Self {
        Self { db }
    }
}

#[cfg(feature = "db")]
#[allow(clippy::manual_async_fn)]
impl CommunitySummaryStore for SurrealSummaryStore {
    #[allow(clippy::manual_async_fn)]
    fn save(&self, summary: &CommunitySummary) -> impl Future<Output = crate::Result<()>> + Send {
        async move {
            let _: Vec<CommunitySummary> = self
                .db
                .create("community_summary")
                .content(summary)
                .await
                .map_err(|e| helpers::db_error(&format!("SurrealDB 写入失败: {e}")))?;
            Ok(())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn save_batch(
        &self,
        summaries: &[CommunitySummary],
    ) -> impl Future<Output = crate::Result<()>> + Send {
        async move {
            for summary in summaries {
                self.save(summary).await?;
            }
            Ok(())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn get_by_community(
        &self,
        community_id: &RecordIdType,
    ) -> impl Future<Output = crate::Result<Option<CommunitySummary>>> + Send {
        async move {
            let mut results: surrealdb::Response = self
                .db
                .query("SELECT * FROM community_summary WHERE community_id = $community_id LIMIT 1")
                .bind(("community_id", community_id.clone()))
                .await
                .map_err(|e| helpers::db_error(&format!("SurrealDB 查询失败: {e}")))?;
            let summary: Option<CommunitySummary> = results.take(0)?;
            Ok(summary)
        }
    }

    fn get_all(&self) -> impl Future<Output = crate::Result<Vec<CommunitySummary>>> + Send {
        async move {
            let summaries: Vec<CommunitySummary> = self
                .db
                .select("community_summary")
                .await
                .map_err(|e| helpers::db_error(&format!("SurrealDB 查询失败: {e}")))?;
            Ok(summaries)
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn delete_by_community(
        &self,
        community_id: &RecordIdType,
    ) -> impl Future<Output = crate::Result<()>> + Send {
        async move {
            let _: surrealdb::Response = self
                .db
                .query("DELETE FROM community_summary WHERE community_id = $community_id")
                .bind(("community_id", community_id.clone()))
                .await
                .map_err(|e| helpers::db_error(&format!("SurrealDB 删除失败: {e}")))?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rid(s: &str) -> RecordIdType {
        #[cfg(feature = "db")]
        {
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() == 2 {
                surrealdb::sql::Thing::from((parts[0], parts[1]))
            } else {
                surrealdb::sql::Thing::from((s, ""))
            }
        }
        #[cfg(not(feature = "db"))]
        {
            s.to_string()
        }
    }

    fn make_summary(community_id: &str, text: &str) -> CommunitySummary {
        CommunitySummary::new(
            make_rid(community_id),
            text.to_string(),
            vec!["concept".to_string()],
            5,
            3,
            0.8,
            "test-model".to_string(),
        )
    }

    #[tokio::test]
    async fn test_in_memory_store_save_and_get() {
        let store = InMemoryStore::new();
        let summary = make_summary("community:c1", "测试摘要");

        store.save(&summary).await.unwrap();
        let result = store
            .get_by_community(&make_rid("community:c1"))
            .await
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().summary_text, "测试摘要");
    }

    #[tokio::test]
    async fn test_in_memory_store_get_nonexistent() {
        let store = InMemoryStore::new();
        let result = store
            .get_by_community(&make_rid("community:missing"))
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_store_save_batch() {
        let store = InMemoryStore::new();
        let summaries = vec![
            make_summary("community:c1", "摘要1"),
            make_summary("community:c2", "摘要2"),
        ];

        store.save_batch(&summaries).await.unwrap();
        let all = store.get_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_store_delete() {
        let store = InMemoryStore::new();
        let summary = make_summary("community:c1", "测试摘要");

        store.save(&summary).await.unwrap();
        store
            .delete_by_community(&make_rid("community:c1"))
            .await
            .unwrap();
        let result = store
            .get_by_community(&make_rid("community:c1"))
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_store_overwrite() {
        let store = InMemoryStore::new();
        let s1 = make_summary("community:c1", "旧摘要");
        let s2 = make_summary("community:c1", "新摘要");

        store.save(&s1).await.unwrap();
        store.save(&s2).await.unwrap();
        let result = store
            .get_by_community(&make_rid("community:c1"))
            .await
            .unwrap();
        assert_eq!(result.unwrap().summary_text, "新摘要");
    }

    #[test]
    fn test_in_memory_store_default() {
        let store = InMemoryStore::default();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let all = rt.block_on(store.get_all()).unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_in_memory_store_save_batch_empty() {
        let store = InMemoryStore::new();
        store.save_batch(&[]).await.unwrap();
        let all = store.get_all().await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_in_memory_store_delete_nonexistent() {
        let store = InMemoryStore::new();
        store
            .delete_by_community(&make_rid("community:nonexistent"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_in_memory_store_get_all_multiple() {
        let store = InMemoryStore::new();
        let s1 = make_summary("community:c1", "摘要1");
        let s2 = make_summary("community:c2", "摘要2");
        let s3 = make_summary("community:c3", "摘要3");
        store.save_batch(&[s1, s2, s3]).await.unwrap();
        let all = store.get_all().await.unwrap();
        assert_eq!(all.len(), 3);
    }
}
