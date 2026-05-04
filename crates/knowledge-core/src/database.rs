use std::future::Future;
use std::sync::Arc;

use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::RecordId;
use surrealdb::sql::Value;

use crate::Result;

/// 允许的数据库表名白名单
///
/// 仅允许系统预定义的表名，防止通过外部输入注入非法表名。
const ALLOWED_TABLES: &[&str] = &[
    "document",
    "block",
    "token",
    "reference",
    "semantic_entity",
    "semantic_relation",
    "community",
    "process",
    "audit_log",
    "extraction_task",
];

/// 验证表名是否在白名单中
///
/// # Errors
///
/// 当表名不在允许列表中时返回验证错误。
pub fn validate_table_name(table: &str) -> Result<()> {
    if ALLOWED_TABLES.contains(&table) {
        Ok(())
    } else {
        Err(error_core::helpers::validation_error(
            &format!("非法表名: {table}，仅允许: {}", ALLOWED_TABLES.join(", ")),
            "validate_table_name",
        ))
    }
}

/// 数据库客户端抽象接口（依赖注入支持）
///
/// 设计原则：生产环境使用 SurrealDB，测试环境使用 Mock 替身。
/// 所有实现必须满足 `Send + Sync`，以确保可安全跨 `tokio::spawn` 边界传递。
pub trait DatabaseClient: Send + Sync {
    /// 按 RecordId 查询单条记录
    fn select<T: for<'de> serde::Deserialize<'de> + Send>(
        &self,
        id: RecordId,
    ) -> impl Future<Output = Result<Option<T>>> + Send;

    /// 执行参数化查询
    fn query(&self, sql: &str, bindings: impl serde::Serialize + Send) -> impl Future<Output = Result<Vec<Value>>> + Send;

    /// 在指定表中创建记录
    fn create<T: serde::Serialize + Send>(&self, table: &str, data: T) -> impl Future<Output = Result<Vec<Value>>> + Send;

    /// 按 RecordId 更新记录
    #[allow(clippy::manual_async_fn)]
    fn update<T: serde::Serialize + Send>(
        &self,
        id: RecordId,
        data: T,
    ) -> impl Future<Output = Result<Option<Value>>> + Send;

    /// 按 RecordId 删除记录
    fn delete(&self, id: RecordId) -> impl Future<Output = Result<Option<Value>>> + Send;

    /// 批量插入记录
    fn insert_batch(&self, table: &str, items: Vec<serde_json::Value>) -> impl Future<Output = Result<Vec<Value>>> + Send;

    /// 在事务中执行多条查询
    #[allow(clippy::manual_async_fn)]
    fn execute_transaction(
        &self,
        queries: Vec<String>,
        bindings: Vec<serde_json::Value>,
    ) -> impl Future<Output = Result<Vec<Vec<Value>>>> + Send;
}

/// SurrealDB 客户端实现
#[derive(Debug, Clone)]
pub struct SurrealDbClient {
    db: Arc<Surreal<Client>>,
}

impl SurrealDbClient {
    /// 创建 SurrealDB 客户端并连接到指定数据库
    ///
    /// # Errors
    ///
    /// 连接失败、命名空间或数据库不存在时返回错误
    pub async fn new(connection_string: &str, namespace: &str, database: &str) -> Result<Self> {
        let db = Surreal::<Client>::new::<Ws>(connection_string)
            .await
            .map_err(error_core::ErrorObject::from)?;
        db.use_ns(namespace)
            .use_db(database)
            .await
            .map_err(error_core::ErrorObject::from)?;
        Ok(Self { db: Arc::new(db) })
    }

    /// 获取底层 SurrealDB 实例的引用
    #[must_use]
    pub const fn db(&self) -> &Arc<Surreal<Client>> {
        &self.db
    }
}

#[allow(clippy::manual_async_fn)]
impl DatabaseClient for SurrealDbClient {
    #[allow(clippy::manual_async_fn)]
    fn select<T: for<'de> serde::Deserialize<'de> + Send>(
        &self,
        id: RecordId,
    ) -> impl Future<Output = Result<Option<T>>> + Send {
        async move {
            self.db
                .select(id)
                .await
                .map_err(error_core::ErrorObject::from)
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn query(&self, sql: &str, bindings: impl serde::Serialize + Send) -> impl Future<Output = Result<Vec<Value>>> + Send {
        async move {
            let mut response = self
                .db
                .query(sql)
                .bind(bindings)
                .await
                .map_err(error_core::ErrorObject::from)?;
            response.take(0).map_err(error_core::ErrorObject::from)
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn create<T: serde::Serialize + Send>(&self, table: &str, data: T) -> impl Future<Output = Result<Vec<Value>>> + Send {
        async move {
            validate_table_name(table)?;
            self.db
                .create(table)
                .content(data)
                .await
                .map_err(error_core::ErrorObject::from)
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn update<T: serde::Serialize + Send>(
        &self,
        id: RecordId,
        data: T,
    ) -> impl Future<Output = Result<Option<Value>>> + Send {
        async move {
            self.db
                .update(id)
                .content(data)
                .await
                .map_err(error_core::ErrorObject::from)
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn delete(&self, id: RecordId) -> impl Future<Output = Result<Option<Value>>> + Send {
        async move {
            self.db
                .delete(id)
                .await
                .map_err(error_core::ErrorObject::from)
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn insert_batch(&self, table: &str, items: Vec<serde_json::Value>) -> impl Future<Output = Result<Vec<Value>>> + Send {
        async move {
            validate_table_name(table)?;
            let mut results = Vec::with_capacity(items.len());
            for item in items {
                results.extend(self.create(table, item).await?);
            }
            Ok(results)
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn execute_transaction(
        &self,
        queries: Vec<String>,
        bindings: Vec<serde_json::Value>,
    ) -> impl Future<Output = Result<Vec<Vec<Value>>>> + Send {
        async move {
            let mut full_sql = String::from("BEGIN TRANSACTION");
            for sql in &queries {
                full_sql.push_str("; ");
                full_sql.push_str(sql);
            }
            full_sql.push_str("; COMMIT TRANSACTION");

            let mut merged = serde_json::Map::new();
            for b in &bindings {
                if let serde_json::Value::Object(map) = b {
                    for (k, v) in map {
                        merged.insert(k.clone(), v.clone());
                    }
                }
            }

            let mut response = self
                .db
                .query(&full_sql)
                .bind(serde_json::Value::Object(merged))
                .await
                .map_err(error_core::ErrorObject::from)?;
            let mut results = Vec::with_capacity(queries.len());
            for i in 0..queries.len() {
                results.push(
                    response
                        .take(i + 1)
                        .map_err(error_core::ErrorObject::from)?,
                );
            }
            Ok(results)
        }
    }
}

#[cfg(test)]
fn json_to_surreal_value(json: &serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::None,
        serde_json::Value::Bool(b) => Value::from(*b),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Value::from)
            .or_else(|| n.as_f64().map(Value::from))
            .unwrap_or(Value::None),
        serde_json::Value::String(s) => Value::from(s.as_str()),
        serde_json::Value::Array(arr) => {
            Value::from(arr.iter().map(json_to_surreal_value).collect::<Vec<_>>())
        }
        serde_json::Value::Object(map) => {
            let mut obj = std::collections::BTreeMap::new();
            for (k, v) in map {
                obj.insert(k.clone(), json_to_surreal_value(v));
            }
            Value::from(obj)
        }
    }
}

#[cfg(test)]
/// 数据库客户端的 Mock 实现，用于测试
///
/// 设计原则：在测试环境中模拟数据库行为，避免真实数据库依赖。
/// 使用内存哈希表存储预定义的响应，支持错误注入。
pub struct MockDbClient {
    responses: std::sync::Mutex<std::collections::HashMap<String, serde_json::Value>>,
    #[allow(dead_code)]
    error_queries: std::sync::Mutex<std::collections::HashSet<String>>,
}

#[cfg(test)]
impl Default for MockDbClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
use std::collections::{HashMap, HashSet};

#[cfg(test)]
impl MockDbClient {
    #[must_use]
    /// 创建一个新的 MockDbClient 实例
    ///
    /// # Returns
    /// 返回一个空的 MockDbClient，需要通过 register_response 方法注册响应
    pub fn new() -> Self {
        Self {
            responses: std::sync::Mutex::new(HashMap::new()),
            error_queries: std::sync::Mutex::new(HashSet::new()),
        }
    }
    /// 注册一个查询响应
    ///
    /// # Parameters
    /// - `key`: 查询键，通常是 SQL 语句或 RecordId 的字符串表示
    /// - `resp`: 对应的响应值，将在查询时返回
    pub fn register_response(&self, key: impl Into<String>, resp: serde_json::Value) {
        self.responses
            .lock()
            .unwrap_or_else(|e| {
                tracing::error!("MockDbClient Mutex poisoned: {e}");
                e.into_inner()
            })
            .insert(key.into(), resp);
    }
}

#[cfg(test)]
#[allow(clippy::manual_async_fn)]
impl DatabaseClient for MockDbClient {
    #[allow(clippy::manual_async_fn)]
    fn select<T: for<'de> serde::Deserialize<'de> + Send>(
        &self,
        id: RecordId,
    ) -> impl Future<Output = Result<Option<T>>> + Send {
        async move {
            let raw = id.to_string();
            let guard = self.responses.lock().unwrap_or_else(|e| {
                tracing::error!("MockDbClient Mutex poisoned: {e}");
                e.into_inner()
            });
            if let Some(v) = guard.get(&raw) {
                Ok(serde_json::from_value(v.clone()).map_err(error_core::ErrorObject::from)?)
            } else {
                Ok(None)
            }
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn query(
        &self,
        sql: &str,
        _bindings: impl serde::Serialize + Send,
    ) -> impl Future<Output = Result<Vec<Value>>> + Send {
        async move {
            let responses = self.responses.lock().unwrap_or_else(|e| {
                tracing::error!("MockDbClient Mutex poisoned: {e}");
                e.into_inner()
            });
            if let Some(v) = responses.get(sql) {
                if let Some(e) = v.get("error").and_then(|x| x.as_str()) {
                    return Err(error_core::ErrorObject::from(surrealdb::Error::Db(
                        surrealdb::error::Db::Thrown(e.to_string()),
                    )));
                }
                return Ok(v.as_array().map_or_else(
                    || vec![json_to_surreal_value(v)],
                    |arr| arr.iter().map(json_to_surreal_value).collect(),
                ));
            }
            Ok(Vec::new())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn create<T: serde::Serialize + Send>(&self, table: &str, data: T) -> impl Future<Output = Result<Vec<Value>>> + Send {
        async move {
            let json = serde_json::to_value(data).map_err(error_core::ErrorObject::from)?;
            let key = format!("CREATE {table} CONTENT");
            let responses = self.responses.lock().unwrap_or_else(|e| {
                tracing::error!("MockDbClient Mutex poisoned: {e}");
                e.into_inner()
            });
            if let Some(r) = responses.get(&key) {
                return Ok(r.as_array().map_or_else(
                    || vec![json_to_surreal_value(r)],
                    |a| a.iter().map(json_to_surreal_value).collect(),
                ));
            }
            Ok(vec![json_to_surreal_value(&json)])
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn update<T: serde::Serialize + Send>(
        &self,
        _id: RecordId,
        data: T,
    ) -> impl Future<Output = Result<Option<Value>>> + Send {
        async move {
            Ok(Some(json_to_surreal_value(
                &serde_json::to_value(data).map_err(error_core::ErrorObject::from)?,
            )))
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn delete(&self, _id: RecordId) -> impl Future<Output = Result<Option<Value>>> + Send {
        async move { Ok(Some(Value::None)) }
    }

    #[allow(clippy::manual_async_fn)]
    fn insert_batch(&self, table: &str, items: Vec<serde_json::Value>) -> impl Future<Output = Result<Vec<Value>>> + Send {
        async move {
            let mut r = Vec::with_capacity(items.len());
            for i in items {
                r.extend(self.create(table, i).await?);
            }
            Ok(r)
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn execute_transaction(
        &self,
        queries: Vec<String>,
        bindings: Vec<serde_json::Value>,
    ) -> impl Future<Output = Result<Vec<Vec<Value>>>> + Send {
        async move {
            let mut res = Vec::with_capacity(queries.len());
            for (q, b) in queries.into_iter().zip(bindings) {
                res.push(self.query(&q, b).await?);
            }
            Ok(res)
        }
    }
}
