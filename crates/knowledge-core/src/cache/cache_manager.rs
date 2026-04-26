use crate::cache::{L1Cache, L2Cache};
use error_core::Result;
use serde::{Serialize, de::DeserializeOwned};
use std::time::Duration;

/// 缓存管理器错误类型
#[derive(Debug, thiserror::Error)]
pub enum CacheManagerError {
    /// L2 缓存操作错误
    #[error("L2 cache error: {0}")]
    L2Error(String),
    /// 序列化/反序列化错误
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// 缓存查找结果
#[derive(Debug)]
pub enum CacheLookupResult<V> {
    /// L1 缓存命中
    Hit(V),
    /// L2 缓存命中（已自动回填 L1）
    L2Hit(V),
    /// 两级缓存均未命中
    Miss,
    /// 缓存操作错误
    Error(CacheManagerError),
}

/// 缓存写入策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritePolicy {
    /// 写穿透：同步写入 L1 和 L2，保证强一致性
    WriteThrough,
    /// 写回：先写 L1，异步批量写入 L2，最终一致性
    WriteBehind,
    /// 写绕过：绕过缓存直接写 DB，适用于大批量导入
    WriteAround,
}

/// 多级缓存管理器
///
/// 统一管理 L1（本地内存）和 L2（Redis 分布式）两级缓存，
/// 提供透明的多级查找、回填和写入策略。
///
/// # 架构
///
/// ```text
/// Client → CacheManager → L1 (Moka) → L2 (Redis) → Database
/// ```
///
/// # 写入策略
///
/// - **Write-Through**: 同步写入 L1 和 L2，保证强一致性
/// - **Write-Behind**: 先写 L1，异步批量写入 L2，最终一致性
/// - **Write-Around**: 绕过缓存直接写 DB，适用于大批量导入
pub struct CacheManager<V: Serialize + DeserializeOwned + Clone + Send + Sync + 'static> {
    /// L1 本地内存缓存
    l1: L1Cache<V>,
    /// L2 分布式缓存（可选）
    l2: Option<L2Cache>,
    /// 管理器名称
    name: String,
    /// 写入策略
    write_policy: WritePolicy,
}

impl<V: Serialize + DeserializeOwned + Clone + Send + Sync + 'static> CacheManager<V> {
    /// 创建新的缓存管理器
    ///
    /// # 参数
    /// - `l1`: L1 本地内存缓存实例
    /// - `l2`: L2 Redis 缓存实例（可选，None 表示禁用 L2）
    pub fn new(l1: L1Cache<V>, l2: Option<L2Cache>) -> Self {
        let name = format!("cache_manager_{}", l1.name());
        Self {
            l1,
            l2,
            name,
            write_policy: WritePolicy::WriteThrough,
        }
    }

    /// 设置写入策略（Builder 模式）
    #[must_use]
    pub const fn with_write_policy(mut self, policy: WritePolicy) -> Self {
        self.write_policy = policy;
        self
    }

    /// Look-Aside 模式: 先查缓存，未命中再查 DB
    ///
    /// 查找顺序：L1 → L2 → Miss（由调用方查 DB 并回填）
    ///
    /// # 降级策略
    /// - L2 故障时返回 `Miss`，由调用方决定是否降级到 DB
    /// - 序列化错误不会 panic，而是返回错误结果
    pub async fn get(&self, key: &str) -> CacheLookupResult<V> {
        if let Some(value) = self.l1.get(key).await {
            return CacheLookupResult::Hit(value);
        }

        if let Some(ref l2) = self.l2 {
            match l2.get::<V>(key).await {
                Ok(Some(value)) => {
                    self.l1.set(key, value.clone()).await;
                    return CacheLookupResult::L2Hit(value);
                }
                Ok(None) => {
                    return CacheLookupResult::Miss;
                }
                Err(e) => {
                    tracing::warn!("L2 cache error for key {}: {}", key, e);
                    return CacheLookupResult::Miss;
                }
            }
        }

        CacheLookupResult::Miss
    }

    /// Write-Through 模式: 同步写入 L1 和 L2
    ///
    /// # 写入策略行为
    ///
    /// - **Write-Through**: 先写 L2（权威源），成功后再写 L1。
    ///   L2 写入失败时返回错误，保证 Write-Through 的强一致性语义。
    /// - **Write-Behind**: 先写 L1，L2 写入失败仅记录日志（最终一致性）。
    /// - **Write-Around**: 绕过缓存直接写 DB。
    ///
    /// # Errors
    ///
    /// Write-Through 模式下 L2 写入失败时返回 `CacheManagerError::L2Error`
    pub async fn set(&self, key: &str, value: &V, ttl: Duration) -> Result<()> {
        match self.write_policy {
            WritePolicy::WriteThrough => {
                if let Some(ref l2) = self.l2 {
                    if let Err(e) = l2.set(key, value, ttl).await {
                        return Err(error_core::helpers::internal_error(&format!(
                            "L2 cache write error for key {key}: {e}"
                        )));
                    }
                }
                self.l1.set(key, value.clone()).await;
            }
            WritePolicy::WriteBehind => {
                self.l1.set(key, value.clone()).await;
                if let Some(ref l2) = self.l2 {
                    if let Err(e) = l2.set(key, value, ttl).await {
                        tracing::warn!("L2 cache write error for key {}: {}", key, e);
                    }
                }
            }
            WritePolicy::WriteAround => {
                // Write-Around: 不写缓存，由调用方直接写 DB
            }
        }

        Ok(())
    }

    /// 删除缓存项（两级缓存同步删除）
    ///
    /// # 删除顺序
    ///
    /// 先删 L2（权威源），再删 L1。若 L2 删除失败，
    /// 不删除 L1（避免 L2 旧数据回填 L1）。
    ///
    /// # Errors
    ///
    /// 此方法始终返回 `Ok(())`，L2 删除失败仅记录日志
    pub async fn invalidate(&self, key: &str) -> Result<()> {
        if let Some(ref l2) = self.l2 {
            if let Err(e) = l2.delete(key).await {
                tracing::warn!("L2 cache delete error for key {}: {}", key, e);
            }
        }

        self.l1.invalidate(key).await;

        Ok(())
    }

    /// 批量删除（按前缀，两级缓存同步）
    ///
    /// 先删 L2（权威源），再删 L1。若 L2 删除失败，
    /// 不删除 L1（避免 L2 旧数据回填 L1）。
    ///
    /// # Errors
    /// 此方法始终返回 `Ok(())`，L2 删除失败仅记录日志
    pub async fn invalidate_prefix(&self, prefix: &str) -> Result<()> {
        if let Some(ref l2) = self.l2 {
            if let Err(e) = l2.delete_pattern(prefix).await {
                tracing::warn!("L2 cache pattern delete error for prefix {}: {}", prefix, e);
            }
        }

        self.l1.invalidate_prefix(prefix).await;

        Ok(())
    }

    /// 清空所有缓存
    ///
    /// # Errors
    /// 此方法始终返回 `Ok(())`，L2 清空失败仅记录日志
    pub async fn clear(&self) -> Result<()> {
        self.l1.clear();

        if let Some(ref l2) = self.l2 {
            if let Err(e) = l2.flush().await {
                tracing::warn!("L2 cache flush error: {}", e);
            }
        }

        Ok(())
    }

    /// 获取管理器名称
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> crate::cache::CacheStats {
        self.l1.stats()
    }
}
