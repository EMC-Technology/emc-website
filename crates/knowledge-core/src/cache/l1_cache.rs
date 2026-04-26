use moka::future::Cache as MokaCache;
use serde::{Serialize, de::DeserializeOwned};
use std::sync::RwLock as StdRwLock;
use std::time::Duration;

/// L1 缓存配置
#[derive(Debug, Clone)]
pub struct L1CacheConfig {
    /// 最大缓存条目数
    pub max_capacity: u64,
    /// 生存时间（Time-To-Live），条目创建后在此时间后过期
    pub ttl: Duration,
    /// 空闲超时（Time-To-Idle），条目在此时间内未被访问则过期
    pub time_to_idle: Duration,
    /// 是否启用命中率统计
    pub enable_stats: bool,
}

impl Default for L1CacheConfig {
    fn default() -> Self {
        Self {
            max_capacity: 10_000,
            ttl: Duration::from_secs(300),
            time_to_idle: Duration::from_secs(60),
            enable_stats: true,
        }
    }
}

/// L1 本地内存缓存 (基于 Moka - 高性能 concurrent cache)
///
/// # 性能特征
/// - 命中延迟: < 1ms (P99)
/// - 线程安全: 基于 Moka 的并发设计
/// - 自动过期: 支持 TTL 和 TTI (Time To Idle)
///
/// # 示例
///
/// ```ignore
/// use knowledge_core::cache::{L1Cache, L1CacheConfig};
/// use std::time::Duration;
///
/// let config = L1CacheConfig {
///     max_capacity: 1000,
///     ttl: Duration::from_secs(60),
///     ..Default::default()
/// };
///
/// let cache: L1Cache<String> = L1Cache::new("my_cache", config);
/// ```
pub struct L1Cache<V: Serialize + DeserializeOwned + Clone + Send + Sync + 'static> {
    /// Moka 并发缓存实例
    inner: MokaCache<String, V>,
    /// 缓存配置（预留：运行时动态调整缓存参数尚未实现，UPCM 流程驱动后实现热更新）
    #[allow(dead_code)]
    config: L1CacheConfig,
    /// 缓存实例名称（用于 metrics 标识）
    name: String,
    /// 命中计数器
    hits: StdRwLock<u64>,
    /// 未命中计数器
    misses: StdRwLock<u64>,
}

impl<V: Serialize + DeserializeOwned + Clone + Send + Sync + 'static> L1Cache<V> {
    /// 创建新的 L1 缓存实例
    ///
    /// # 参数
    /// - `name`: 缓存实例名称（用于 metrics 标识）
    /// - `config`: 缓存配置参数
    #[must_use]
    pub fn new(name: &str, config: L1CacheConfig) -> Self {
        let cache = MokaCache::builder()
            .max_capacity(config.max_capacity)
            .time_to_live(config.ttl)
            .time_to_idle(config.time_to_idle)
            .build();

        Self {
            inner: cache,
            config,
            name: name.to_string(),
            hits: StdRwLock::new(0),
            misses: StdRwLock::new(0),
        }
    }

    /// 获取缓存实例名称
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 获取缓存值
    ///
    /// 返回 `Some(value)` 表示命中，`None` 表示未命中。
    /// 每次调用都会更新 hit/miss 计数器和命中率 gauge。
    pub async fn get(&self, key: &str) -> Option<V> {
        let start = std::time::Instant::now();

        let result = if let Some(value) = self.inner.get(key).await {
            *self
                .hits
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
            Some(value)
        } else {
            *self
                .misses
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
            None
        };

        let elapsed = start.elapsed().as_secs_f64();
        let _ = elapsed;
        result
    }

    /// 设置缓存值
    ///
    /// 使用配置中的默认 TTL。
    pub async fn set(&self, key: &str, value: V) {
        self.inner.insert(key.to_string(), value).await;
    }

    /// 删除缓存项
    pub async fn invalidate(&self, key: &str) {
        self.inner.invalidate(key).await;
    }

    /// 批量删除 (按前缀)
    ///
    /// 遍历所有条目并删除匹配前缀的项。
    /// 注意：Moka 不直接支持前缀索引，此操作时间复杂度为 O(n)。
    pub async fn invalidate_prefix(&self, prefix: &str) {
        let keys_to_remove: Vec<String> = self
            .inner
            .iter()
            .filter_map(|(k, _)| {
                if k.starts_with(prefix) {
                    Some(k.to_string())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            self.inner.invalidate(&key).await;
        }
    }

    /// 清空整个缓存并重置统计信息
    pub fn clear(&self) {
        self.inner.invalidate_all();
        *self
            .hits
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = 0;
        *self
            .misses
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = 0;
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            name: self.name.clone(),
            entries: self.inner.entry_count(),
            hits: *self
                .hits
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            misses: *self
                .misses
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            hit_rate: self.hit_rate(),
        }
    }

    /// 获取当前缓存条目数
    pub fn entry_count(&self) -> u64 {
        self.inner.entry_count()
    }

    fn hit_rate(&self) -> f64 {
        let hits = *self
            .hits
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let misses = *self
            .misses
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            let h = hits as f64;
            #[allow(clippy::cast_precision_loss)]
            let t = total as f64;
            h / t
        }
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// 缓存实例名称
    pub name: String,
    /// 当前缓存条目数
    pub entries: u64,
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 命中率（0.0 ~ 1.0）
    pub hit_rate: f64,
}
