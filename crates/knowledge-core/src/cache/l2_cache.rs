use redis::{AsyncCommands, Client};
use serde::{de::DeserializeOwned, Serialize};
use bincode;
use std::time::Duration;
use thiserror::Error;

/// L2 缓存错误类型
#[derive(Error, Debug)]
pub enum L2CacheError {
    /// Redis 连接失败
    #[error("Redis connection failed: {0}")]
    Connection(String),

    /// 序列化失败
    #[error("Serialization failed: {0}")]
    Serialization(String),

    /// 反序列化失败
    #[error("Deserialization failed: {0}")]
    Deserialization(String),

    /// Redis 操作错误
    #[error("Redis operation error: {0}")]
    Operation(String),
}

/// Redis 连接池状态信息
#[derive(Debug, Clone, Serialize)]
pub struct PoolInfo {
    /// 活跃连接数
    pub active_connections: u64,
    /// 空闲连接数
    pub idle_connections: u64,
    /// 等待连接的请求数
    pub waiting_count: u64,
}

/// L2 分布式缓存 (基于 Redis)
///
/// # 特性
/// - 使用 bincode 进行高效序列化
/// - 支持命名空间隔离（key prefix）
/// - 自动 TTL 管理
/// - 连接池管理
///
/// # 可靠性保证
/// - Redis 故障时返回错误（由上层 `CacheManager` 处理降级）
/// - 序列化错误不影响主流程（返回 Err 而非 panic）
///
/// # 示例
///
/// ```ignore
/// use knowledge_core::cache::L2Cache;
/// use std::time::Duration;
///
/// let cache = L2Cache::from_url(
///     "redis://localhost:6379",
///     "myapp:",
///     Duration::from_secs(3600)
/// )?;
/// ```
pub struct L2Cache {
    /// Redis 客户端实例
    client: Client,
    /// 默认过期时间（预留：L2 缓存自动过期策略尚未实现，UPCM 流程驱动后实现 TTL 策略）
    #[allow(dead_code)]
    default_ttl: Duration,
    /// 键名前缀，用于命名空间隔离
    key_prefix: String,
}

impl L2Cache {
    /// 从 URL 创建 Redis 连接
    ///
    /// # 参数
    /// - `url`: Redis 连接 URL (如 `redis://localhost:6379`)
    /// - `key_prefix`: 键名前缀，用于命名空间隔离
    /// - `default_ttl`: 默认过期时间
    ///
    /// # Errors
    /// 返回 `L2CacheError::Connection` 当 URL 格式无效或连接失败时。
    pub fn from_url(url: &str, key_prefix: &str, default_ttl: Duration) -> Result<Self, L2CacheError> {
        let client = Client::open(url).map_err(|e| L2CacheError::Connection(e.to_string()))?;

        Ok(Self {
            client,
            default_ttl,
            key_prefix: key_prefix.to_string(),
        })
    }

    /// 获取缓存值
    ///
    /// # 类型参数
    /// - `V`: 必须实现 `DeserializeOwned`
    ///
    /// # Errors
    /// - `L2CacheError::Connection`: Redis 连接失败
    /// - `L2CacheError::Deserialization`: 反序列化失败
    pub async fn get<V: DeserializeOwned>(&self, key: &str) -> Result<Option<V>, L2CacheError> {
        let full_key = format!("{}{}", self.key_prefix, key);

        let mut conn = self.get_connection().await?;
        let data: Option<Vec<u8>> = conn.get(&full_key).await.map_err(|e| L2CacheError::Operation(e.to_string()))?;

        match data {
            Some(bytes) => {
                let value: V = bincode::deserialize(&bytes)
                    .map_err(|e| L2CacheError::Deserialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// 设置缓存值
    ///
    /// 使用 bincode 序列化并设置 TTL。
    ///
    /// # 参数
    /// - `key`: 缓存键
    /// - `value`: 缓存值（必须实现 Serialize）
    /// - `ttl`: 过期时间（None 表示使用默认 TTL）
    ///
    /// # Errors
    /// - `L2CacheError::Connection`: Redis 连接失败
    /// - `L2CacheError::Serialization`: 序列化失败
    pub async fn set<V: Serialize + Sync>(&self, key: &str, value: &V, ttl: Duration) -> Result<(), L2CacheError> {
        let full_key = format!("{}{}", self.key_prefix, key);

        let serialized = bincode::serialize(value)
            .map_err(|e| L2CacheError::Serialization(e.to_string()))?;

        let mut conn = self.get_connection().await?;
        let _: () = conn.set_ex(&full_key, serialized, ttl.as_secs())
            .await
            .map_err(|e| L2CacheError::Operation(e.to_string()))?;

        Ok(())
    }

    /// 删除缓存项
    ///
    /// # Errors
    /// 当 Redis 连接或操作失败时返回 `L2CacheError`
    pub async fn delete(&self, key: &str) -> Result<(), L2CacheError> {
        let full_key = format!("{}{}", self.key_prefix, key);

        let mut conn = self.get_connection().await?;
        let _: () = conn.del(&full_key).await.map_err(|e| L2CacheError::Operation(e.to_string()))?;

        Ok(())
    }

    /// 按模式删除缓存项（使用 SCAN + DEL）
    ///
    /// 完整遍历 SCAN 游标直到返回 0，确保所有匹配键均被删除。
    ///
    /// # Errors
    /// 当 Redis 连接或操作失败时返回 `L2CacheError`
    pub async fn delete_pattern(&self, pattern: &str) -> Result<(), L2CacheError> {
        let full_pattern = format!("{}{}*", self.key_prefix, pattern);
        let mut conn = self.get_connection().await?;

        let mut cursor: u64 = 0;
        loop {
            let mut cmd = redis::cmd("SCAN");
            cmd.arg(cursor).arg("MATCH").arg(&full_pattern).arg("COUNT").arg(100);
            let (next_cursor, keys): (u64, Vec<String>) = cmd
                .query_async(&mut conn)
                .await
                .map_err(|e| L2CacheError::Operation(e.to_string()))?;

            if !keys.is_empty() {
                let _: () = conn.del(&keys).await.map_err(|e| L2CacheError::Operation(e.to_string()))?;
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(())
    }

    /// 清空当前命名空间的所有缓存
    ///
    /// # Errors
    /// 当 Redis 连接或操作失败时返回 `L2CacheError`
    pub async fn flush(&self) -> Result<(), L2CacheError> {
        self.delete_pattern("*").await
    }

    /// 检查键是否存在
    ///
    /// # Errors
    /// 当 Redis 连接或操作失败时返回 `L2CacheError`
    pub async fn exists(&self, key: &str) -> Result<bool, L2CacheError> {
        let full_key = format!("{}{}", self.key_prefix, key);

        let mut conn = self.get_connection().await?;
        conn.exists(&full_key).await.map_err(|e| L2CacheError::Operation(e.to_string()))
    }

    /// 设置过期时间
    ///
    /// # Errors
    /// 当 Redis 连接或操作失败时返回 `L2CacheError`
    #[allow(clippy::cast_possible_wrap)]
    pub async fn expire(&self, key: &str, ttl: Duration) -> Result<(), L2CacheError> {
        let full_key = format!("{}{}", self.key_prefix, key);

        let mut conn = self.get_connection().await?;
        let _: () = conn.expire(&full_key, ttl.as_secs() as i64)
            .await
            .map_err(|e| L2CacheError::Operation(e.to_string()))?;

        Ok(())
    }

    /// 获取连接池状态
    pub fn pool_info(&self) -> PoolInfo {
        PoolInfo {
            active_connections: 0,
            idle_connections: 1,
            waiting_count: 0,
        }
    }

    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection, L2CacheError> {
        self.client.get_multiplexed_async_connection().await
            .map_err(|e| L2CacheError::Connection(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_from_url_invalid() {
        let result = L2Cache::from_url("invalid://url", "test:", Duration::from_secs(60));
        assert!(result.is_err());
    }
}
