//! 速率限制与配额管理模块
//!
//! 提供企业级的流量控制能力，支持令牌桶限流和用量追踪。

pub mod rate_limiter;

pub use rate_limiter::{
    TokenBucketRateLimiter,
    RateLimitResult,
    ConsumeResult,
    QuotaInfo,
    TimePeriod,
    QuotaStatus,
    UsageRecord,
    TokenUsage,
    TokenUsageTracker,
    UsageStore,
    InMemoryUsageStore,
};
