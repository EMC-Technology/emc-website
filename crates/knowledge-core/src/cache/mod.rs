/// 多级缓存管理器（L1 + L2 透明代理）
pub mod cache_manager;
/// L1 本地内存缓存（基于 Moka）
pub mod l1_cache;
/// L2 分布式缓存（基于 Redis）
pub mod l2_cache;
/// 缓存策略配置
pub mod strategy;

pub use cache_manager::{CacheLookupResult, CacheManager, WritePolicy};
pub use l1_cache::{CacheStats, L1Cache, L1CacheConfig};
pub use l2_cache::{L2Cache, L2CacheError, PoolInfo};
pub use strategy::{CACHE_STRATEGIES, CacheStrategy, CacheStrategyConfig};
