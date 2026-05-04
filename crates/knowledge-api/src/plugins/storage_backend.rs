use async_trait::async_trait;
use std::sync::Arc;

use crate::Result;

#[cfg(feature = "db")]
use knowledge_core::EventStore;
#[cfg(feature = "db")]
use knowledge_core::VectorStore;

/// 动态分发安全的数据库客户端 trait
///
/// `DatabaseClient` 因使用泛型参数（`select<T>`、`create<T>`、`update<T>`）
/// 和 RPITIT（`impl Future<...> + Send`）返回类型而无法作为 trait object。
/// `DynDatabaseClient` 通过以下方式解决：
///
/// 1. 将泛型参数替换为 `serde_json::Value`（牺牲编译期类型安全，换取运行时多态）
/// 2. 使用 `#[async_trait]` 宏将 `async fn` 展开为 `Pin<Box<dyn Future>>`（满足 dyn-safe）
///
/// # 与 `DatabaseClient` 的关系
///
/// `DynDatabaseClient` 是 `DatabaseClient` 的 dyn-safe 投影。
/// 任何实现了 `DatabaseClient` 的类型都可以通过 [`DynDatabaseClientWrapper`]
/// 自动获得 `DynDatabaseClient` 实现。
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::storage_backend::DynDatabaseClient;
/// use std::sync::Arc;
///
/// let client: Arc<dyn DynDatabaseClient> = storage.create_database_client().await?;
/// let result = client.select_json("document:abc123").await?;
/// ```
#[cfg(feature = "db")]
#[async_trait]
pub trait DynDatabaseClient: Send + Sync {
    /// 按 `RecordId` 查询单条记录，返回 JSON 值
    async fn select_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    /// 执行参数化查询
    async fn query_json(
        &self,
        sql: &str,
        bindings: serde_json::Value,
    ) -> Result<Vec<serde_json::Value>>;

    /// 在指定表中创建记录
    async fn create_json(
        &self,
        table: &str,
        data: serde_json::Value,
    ) -> Result<Vec<serde_json::Value>>;

    /// 按 `RecordId` 更新记录
    async fn update_json(
        &self,
        id: &str,
        data: serde_json::Value,
    ) -> Result<Option<serde_json::Value>>;

    /// 按 `RecordId` 删除记录
    async fn delete_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    /// 批量插入记录
    async fn insert_batch_json(
        &self,
        table: &str,
        items: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>>;

    /// 在事务中执行多条查询
    async fn execute_transaction_json(
        &self,
        queries: Vec<String>,
        bindings: Vec<serde_json::Value>,
    ) -> Result<Vec<Vec<serde_json::Value>>>;
}

/// `DatabaseClient` → `DynDatabaseClient` 的自动适配器
///
/// 任何实现了 [`knowledge_core::DatabaseClient`] 的具体类型，
/// 都可以通过 `DynDatabaseClientWrapper<T>` 获得 `DynDatabaseClient` 实现。
///
/// # 类型擦除策略
///
/// - 输入：将 `serde_json::Value` 反序列化为具体类型 `T` 再传入
/// - 输出：将具体类型序列化为 `serde_json::Value` 返回
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::storage_backend::DynDatabaseClientWrapper;
/// use knowledge_core::database::SurrealDbClient;
///
/// let surreal = SurrealDbClient::new("ws://localhost:8000", "test", "test").await?;
/// let dyn_client = DynDatabaseClientWrapper::new(surreal);
/// assert_eq!(dyn_client.select_json("document:abc").await?.is_none(), true);
/// ```
#[cfg(feature = "db")]
pub struct DynDatabaseClientWrapper<T> {
    inner: T,
}

#[cfg(feature = "db")]
impl<T> DynDatabaseClientWrapper<T> {
    /// 创建适配器
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

#[cfg(feature = "db")]
#[async_trait]
impl<T: knowledge_core::DatabaseClient + 'static> DynDatabaseClient
    for DynDatabaseClientWrapper<T>
{
    async fn select_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let record_id = id
            .parse::<surrealdb::opt::RecordId>()
            .map_err(|()| error_core::helpers::invalid_record_id(id))?;
        let result: Option<serde_json::Value> = self.inner.select(record_id).await?;
        Ok(result)
    }

    async fn query_json(
        &self,
        sql: &str,
        bindings: serde_json::Value,
    ) -> Result<Vec<serde_json::Value>> {
        let values = self.inner.query(sql, bindings).await?;
        let json_values: Vec<serde_json::Value> =
            values.into_iter().map(surreal_value_to_json).collect();
        Ok(json_values)
    }

    async fn create_json(
        &self,
        table: &str,
        data: serde_json::Value,
    ) -> Result<Vec<serde_json::Value>> {
        let values = self.inner.create(table, data).await?;
        let json_values: Vec<serde_json::Value> =
            values.into_iter().map(surreal_value_to_json).collect();
        Ok(json_values)
    }

    async fn update_json(
        &self,
        id: &str,
        data: serde_json::Value,
    ) -> Result<Option<serde_json::Value>> {
        let record_id = id
            .parse::<surrealdb::opt::RecordId>()
            .map_err(|()| error_core::helpers::invalid_record_id(id))?;
        let result = self.inner.update(record_id, data).await?;
        Ok(result.map(surreal_value_to_json))
    }

    async fn delete_json(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let record_id = id
            .parse::<surrealdb::opt::RecordId>()
            .map_err(|()| error_core::helpers::invalid_record_id(id))?;
        let result = self.inner.delete(record_id).await?;
        Ok(result.map(surreal_value_to_json))
    }

    async fn insert_batch_json(
        &self,
        table: &str,
        items: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        let values = self.inner.insert_batch(table, items).await?;
        let json_values: Vec<serde_json::Value> =
            values.into_iter().map(surreal_value_to_json).collect();
        Ok(json_values)
    }

    async fn execute_transaction_json(
        &self,
        queries: Vec<String>,
        bindings: Vec<serde_json::Value>,
    ) -> Result<Vec<Vec<serde_json::Value>>> {
        let results = self.inner.execute_transaction(queries, bindings).await?;
        let json_results: Vec<Vec<serde_json::Value>> = results
            .into_iter()
            .map(|inner| inner.into_iter().map(surreal_value_to_json).collect())
            .collect();
        Ok(json_results)
    }
}

/// 将 `surrealdb::sql::Value` 转换为 `serde_json::Value`
///
/// 此函数用于 `DynDatabaseClientWrapper` 的输出类型擦除。
#[cfg(feature = "db")]
#[allow(clippy::needless_pass_by_value)]
fn surreal_value_to_json(value: surrealdb::sql::Value) -> serde_json::Value {
    serde_json::to_value(&value).unwrap_or(serde_json::Value::Null)
}

/// 存储后端插件 — `LightField` 的"插座"
///
/// 开源默认实现：SurrealDb 适配
/// 闭源增强实现：`LightFieldBackend`（自研内存堆图对象数据库）
///
/// # 架构角色
///
/// `StorageBackend` 将存储层的创建权交给插件。
/// 闭源 `LightField` 可以提供更高性能的存储实现，
/// 而开源版本默认使用 `SurrealDB`。
///
/// # Feature Gate
///
/// `db` feature 启用时，此 trait 包含 `create_database_client`、
/// `create_vector_store`、`create_event_store` 三个工厂方法。
/// 未启用 `db` feature 时，仅包含 `backend_id` 和 `health_check`。
///
/// # `DatabaseClient` 的 dyn-safe 包装
///
/// 原始 `DatabaseClient` trait 因泛型方法和 RPITIT 返回类型而无法作为
/// trait object。`StorageBackend::create_database_client` 返回
/// `Arc<dyn DynDatabaseClient>`——一个 dyn-safe 的投影 trait，
/// 通过 `serde_json::Value` 替代泛型参数实现运行时多态。
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::StorageBackend;
/// use std::sync::Arc;
///
/// let storage: Arc<dyn StorageBackend> = Arc::new(LightFieldBackend::from_config(&config)?);
/// registry.register_storage(storage);
/// ```
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// 后端唯一标识
    fn backend_id(&self) -> &str;

    /// 创建 [`DynDatabaseClient`] 实例
    ///
    /// 返回 dyn-safe 的数据库客户端，通过 `serde_json::Value` 进行类型擦除。
    #[cfg(feature = "db")]
    async fn create_database_client(&self) -> Result<Arc<dyn DynDatabaseClient>>;

    /// 创建 [`VectorStore`] 实例
    #[cfg(feature = "db")]
    async fn create_vector_store(&self) -> Result<Arc<dyn VectorStore>>;

    /// 创建 [`EventStore`] 实例
    #[cfg(feature = "db")]
    async fn create_event_store(&self) -> Result<Arc<dyn EventStore>>;

    /// 健康检查
    async fn health_check(&self) -> Result<bool>;
}
