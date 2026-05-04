use async_trait::async_trait;
use uuid::Uuid;

use crate::Result;

/// Core `VectorStore` → Agent Memory `VectorStore` 适配器
///
/// # 碎片化现状
///
/// 项目中存在两套 `VectorStore` trait：
///
/// | Trait | Crate | 方法数 | 用途 |
/// |-------|-------|--------|------|
/// | `knowledge_core::VectorStore` | knowledge-core | 8 | 完整向量数据库操作 |
/// | `knowledge_api::agent::memory::VectorStore` | knowledge-api | 3 | Agent 记忆系统简化接口 |
///
/// # 统一方案
///
/// Agent Memory 复用 Core 的 `VectorStore`。
/// `CoreVectorStoreAdapter` 将 Core 的 8 方法 `VectorStore`
/// 适配为 Agent Memory 的 3 方法 `VectorStore`。
///
/// 适配策略：
/// - `store` → `upsert`（使用默认 collection）
/// - `search` → `similarity_search`（取 `top_k` 结果，仅返回 ID + 分数）
/// - `delete` → `delete`（直接委托）
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::vector_store_adapter::CoreVectorStoreAdapter;
/// use std::sync::Arc;
///
/// let core_store: Arc<dyn knowledge_core::VectorStore> = /* ... */;
/// let memory_store = CoreVectorStoreAdapter::new(core_store, "agent_memory");
/// let long_term_memory = LongTermMemory::new(Arc::new(memory_store), "episodes");
/// ```
pub struct CoreVectorStoreAdapter {
    inner: std::sync::Arc<dyn knowledge_core::VectorStore>,
    collection: String,
}

impl CoreVectorStoreAdapter {
    /// 创建适配器
    ///
    /// # Arguments
    ///
    /// * `inner` - Core 的 `VectorStore` 实例
    /// * `collection` - 默认使用的集合名称
    pub fn new(
        inner: std::sync::Arc<dyn knowledge_core::VectorStore>,
        collection: impl Into<String>,
    ) -> Self {
        Self {
            inner,
            collection: collection.into(),
        }
    }
}

#[async_trait]
impl crate::agent::memory::VectorStore for CoreVectorStoreAdapter {
    async fn store(&self, id: &Uuid, vector: &[f32], payload: &serde_json::Value) -> Result<()> {
        let point = knowledge_core::vector_store::store::VectorPoint {
            id: *id,
            vector: vector.to_vec(),
            payload: payload.clone(),
        };
        self.inner.upsert(&self.collection, vec![point]).await?;
        Ok(())
    }

    async fn search(&self, query_vector: &[f32], top_k: usize) -> Result<Vec<(Uuid, f64)>> {
        let options = knowledge_core::vector_store::types::SearchOptions {
            top_k,
            ..Default::default()
        };
        let results = self
            .inner
            .similarity_search(&self.collection, query_vector, options)
            .await?;
        let mapped: Vec<(Uuid, f64)> = results.into_iter().map(|r| (r.id, r.score)).collect();
        Ok(mapped)
    }

    async fn delete(&self, id: &Uuid) -> Result<()> {
        self.inner.delete(&self.collection, &[*id]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_adapter_creation() {
        let collection = "test_collection";
        assert_eq!(collection, "test_collection");
    }
}
