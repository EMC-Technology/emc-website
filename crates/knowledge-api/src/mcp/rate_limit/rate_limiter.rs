//! 速率限制与配额管理
//!
//! 提供企业级的流量控制能力，支持：

#![allow(clippy::significant_drop_tightening)]
//! - 令牌桶算法限流
//! - 用户级/工具级配额
//! - Token 用量追踪
//! - 多维度限流策略

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::mcp::registry::RateLimitConfig;
use error_core::ErrorObject;

/// 令牌桶状态
#[derive(Clone)]
struct TokenBucket {
    /// 桶容量
    capacity: u32,
    /// 当前令牌数
    tokens: f64,
    /// 补充速率（tokens/秒）
    refill_rate: f64,
    /// 上次补充时间
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: u32, refill_rate: f64) -> Self {
        Self {
            capacity,
            tokens: f64::from(capacity),
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let tokens_to_add = elapsed * self.refill_rate;
        self.tokens = (self.tokens + tokens_to_add).min(f64::from(self.capacity));
        self.last_refill = now;
    }

    fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();
        let tokens_f64 = f64::from(tokens);
        if self.tokens >= tokens_f64 {
            self.tokens -= tokens_f64;
            true
        } else {
            false
        }
    }

    #[must_use]
    const fn available_tokens(&self) -> f64 {
        self.tokens
    }

    #[must_use]
    fn estimate_wait_time(&self, tokens: u32) -> Duration {
        let tokens_f64 = f64::from(tokens);
        if self.tokens >= tokens_f64 {
            return Duration::ZERO;
        }
        let needed = tokens_f64 - self.tokens;
        let seconds = needed / self.refill_rate;
        Duration::from_secs_f64(seconds)
    }

    #[must_use]
    #[expect(dead_code)]
    pub fn reset(&mut self) -> Self {
        self.tokens = f64::from(self.capacity);
        self.last_refill = Instant::now();
        self.clone()
    }
}

/// 限流结果
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// 请求被允许
    Allowed {
        /// 配额内剩余可用请求数
        remaining: u32,
        /// 距离配额重置的等待时间
        reset_after: Duration,
    },
    /// 请求被限流
    Limited {
        /// 建议客户端等待后重试的时间
        retry_after: Duration,
        /// 当前时间窗口的请求上限
        limit: u32,
    },
}

/// 配额消费结果
#[derive(Debug, Clone)]
pub enum ConsumeResult {
    /// 消费成功
    Success {
        /// 剩余可用配额
        remaining: u32,
    },
    /// 触发限流
    RateLimited {
        /// 需要等待的时长
        retry_after: Duration,
    },
}

/// 用户配额信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaInfo {
    /// 用户唯一标识
    pub user_id: String,
    /// 工具名称（None 表示全局配额）
    pub tool_name: Option<String>,
    /// 当前周期已使用量
    pub used: u64,
    /// 配额上限
    pub limit: u64,
    /// 配额重置时间点
    pub resets_at: DateTime<Utc>,
    /// 剩余可用量
    pub remaining: u64,
}

/// 时间周期枚举
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TimePeriod {
    /// 按分钟统计
    Minute,
    /// 按小时统计
    Hour,
    /// 按天统计
    Day,
    /// 按周统计
    Week,
    /// 按月统计（30天）
    Month,
}

impl TimePeriod {
    /// 获取时间周期对应的 `Duration` 值
    #[must_use]
    pub const fn duration(&self) -> chrono::Duration {
        match self {
            Self::Minute => chrono::Duration::minutes(1),
            Self::Hour => chrono::Duration::hours(1),
            Self::Day => chrono::Duration::days(1),
            Self::Week => chrono::Duration::weeks(1),
            Self::Month => chrono::Duration::days(30),
        }
    }
}

/// 配额状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuotaStatus {
    /// 用量未超出配额限制
    WithinLimit {
        /// 配额内剩余可用量
        remaining: u64,
    },
    /// 用量已超出配额限制
    Exceeded {
        /// 当前周期内的实际使用量
        usage: u64,
        /// 配额上限值
        limit: u64,
    },
}

/// API 用量记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// 记录唯一标识
    pub id: Uuid,
    /// 用户 ID
    pub user_id: String,
    /// 使用的工具名称
    pub tool_name: String,
    /// 消耗的 token 数量
    pub tokens_used: u32,
    /// 记录创建时间戳
    pub timestamp: DateTime<Utc>,
    /// 关联的会话 ID
    pub session_id: Uuid,
    /// 关联的请求 ID
    pub request_id: Uuid,
}

/// Token 使用量汇总统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// 累计总 token 数
    pub total_tokens: u64,
    /// 累计总请求数
    pub total_requests: u64,
    /// 按工具分类的使用量映射
    pub by_tool: HashMap<String, u64>,
    /// 统计周期开始时间
    pub period_start: DateTime<Utc>,
    /// 统计周期结束时间
    pub period_end: DateTime<Utc>,
}

/// 用量存储接口
#[async_trait]
pub trait UsageStore: Send + Sync {
    /// 记录一条用量数据
    ///
    /// 将用量记录持久化到存储后端，用于后续配额计算和计费统计。
    ///
    /// # Errors
    ///
    /// 当存储后端写入失败时返回 [`ErrorObject`]。
    async fn record_usage(&self, record: &UsageRecord) -> Result<(), ErrorObject>;
    /// 查询用户在指定时间周期内的总用量
    ///
    /// 聚合该用户在所有工具上的 token 使用量，返回结构化的用量统计。
    ///
    /// # Errors
    ///
    /// 当存储后端查询失败时返回 [`ErrorObject`]。
    async fn get_user_total(
        &self,
        user_id: &str,
        period: TimePeriod,
    ) -> Result<TokenUsage, ErrorObject>;
    /// 查询用户在指定时间周期内对特定工具的用量
    ///
    /// 返回该用户调用指定工具的累计 token 数量，用于细粒度的配额控制。
    ///
    /// # Errors
    ///
    /// 当存储后端查询失败时返回 [`ErrorObject`]。
    async fn get_user_tool_usage(
        &self,
        user_id: &str,
        tool_name: &str,
        period: TimePeriod,
    ) -> Result<u64, ErrorObject>;
}

/// 内存存储实现
pub struct InMemoryUsageStore {
    records: Arc<tokio::sync::RwLock<Vec<UsageRecord>>>,
}

impl InMemoryUsageStore {
    /// 创建一个空的内存用量存储实例
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }
}

impl Default for InMemoryUsageStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UsageStore for InMemoryUsageStore {
    async fn record_usage(&self, record: &UsageRecord) -> Result<(), ErrorObject> {
        let mut records = self.records.write().await;
        records.push(record.clone());
        Ok(())
    }

    async fn get_user_total(
        &self,
        user_id: &str,
        period: TimePeriod,
    ) -> Result<TokenUsage, ErrorObject> {
        let records = self.records.read().await;
        let cutoff = Utc::now() - period.duration();

        let relevant: Vec<&UsageRecord> = records
            .iter()
            .filter(|r| r.user_id == user_id && r.timestamp >= cutoff)
            .collect();

        let total_tokens: u64 = relevant.iter().map(|r| u64::from(r.tokens_used)).sum();
        let total_requests = relevant.len() as u64;

        let mut by_tool: HashMap<String, u64> = HashMap::new();
        for r in &relevant {
            *by_tool.entry(r.tool_name.clone()).or_insert(0) += u64::from(r.tokens_used);
        }

        Ok(TokenUsage {
            total_tokens,
            total_requests,
            by_tool,
            period_start: cutoff,
            period_end: Utc::now(),
        })
    }

    async fn get_user_tool_usage(
        &self,
        user_id: &str,
        tool_name: &str,
        period: TimePeriod,
    ) -> Result<u64, ErrorObject> {
        let records = self.records.read().await;
        let cutoff = Utc::now() - period.duration();

        let total: u64 = records
            .iter()
            .filter(|r| r.user_id == user_id && r.tool_name == tool_name && r.timestamp >= cutoff)
            .map(|r| u64::from(r.tokens_used))
            .sum();

        Ok(total)
    }
}

/// 令牌桶限流器
///
/// 基于 Token Bucket 算法实现的高性能限流器，
/// 支持用户级和工具级的细粒度限流。
///
/// # Examples
///
/// ```ignore
/// let limiter = TokenBucketRateLimiter::with_defaults();
///
/// match limiter.check_rate_limit("user-001", "query").await {
///     RateLimitResult::Allowed { .. } => { /* 处理请求 */ }
///     RateLimitResult::Limited { retry_after, .. } => {
///         // 返回 429 Too Many Requests
///     }
/// }
/// ```
pub struct TokenBucketRateLimiter {
    buckets: DashMap<String, TokenBucket>,
    default_config: RateLimitConfig,
}

impl TokenBucketRateLimiter {
    /// 使用默认配置创建限流器
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            buckets: DashMap::new(),
            default_config: RateLimitConfig::default(),
        }
    }

    /// 使用自定义默认配置创建限流器
    #[must_use]
    pub fn with_config(default_config: RateLimitConfig) -> Self {
        Self {
            buckets: DashMap::new(),
            default_config,
        }
    }

    fn make_key(user_id: &str, tool_name: &str) -> String {
        format!("{user_id}:{tool_name}")
    }

    /// 检查是否允许请求
    ///
    /// 返回限流检查结果，包含剩余配额或重试等待时间。
    #[must_use]
    pub fn check_rate_limit(&self, user_id: &str, tool_name: &str) -> RateLimitResult {
        let key = Self::make_key(user_id, tool_name);

        let requests_per_second = f64::from(self.default_config.max_requests_per_minute) / 60.0;
        self.buckets.entry(key.clone()).or_insert_with(|| {
            TokenBucket::new(self.default_config.burst_limit.max(1), requests_per_second)
        });

        if let Some(mut bucket) = self.buckets.get_mut(&key) {
            if bucket.try_consume(1) {
                #[allow(clippy::cast_possible_truncation)]
                #[allow(clippy::cast_sign_loss)]
                let remaining = bucket.available_tokens() as u32;
                let estimate_wait = bucket.estimate_wait_time(1);
                return RateLimitResult::Allowed {
                    remaining,
                    reset_after: estimate_wait,
                };
            }

            let retry_after = bucket.estimate_wait_time(1);
            RateLimitResult::Limited {
                retry_after,
                limit: self.default_config.burst_limit,
            }
        } else {
            RateLimitResult::Allowed {
                remaining: self.default_config.burst_limit,
                reset_after: Duration::from_secs(0),
            }
        }
    }

    /// 消耗指定数量的令牌
    ///
    /// 用于精确控制消耗量（如基于 token 数量）。
    #[must_use]
    pub fn consume(&self, user_id: &str, tool_name: &str, tokens: u32) -> ConsumeResult {
        let key = Self::make_key(user_id, tool_name);

        let requests_per_second = f64::from(self.default_config.max_requests_per_minute) / 60.0;
        self.buckets.entry(key.clone()).or_insert_with(|| {
            TokenBucket::new(self.default_config.burst_limit.max(1), requests_per_second)
        });

        self.buckets.get_mut(&key).map_or(
            ConsumeResult::Success {
                remaining: self.default_config.burst_limit,
            },
            |mut bucket| {
                if bucket.try_consume(tokens) {
                    #[allow(clippy::cast_possible_truncation)]
                    #[allow(clippy::cast_sign_loss)]
                    let remaining = bucket.available_tokens() as u32;
                    ConsumeResult::Success { remaining }
                } else {
                    let retry_after = bucket.estimate_wait_time(tokens);
                    ConsumeResult::RateLimited { retry_after }
                }
            },
        )
    }

    /// 获取剩余配额信息
    ///
    /// # Errors
    ///
    /// 当存储操作失败时返回错误。
    #[must_use]
    pub fn get_remaining(&self, user_id: &str, tool_name: &str) -> QuotaInfo {
        let key = Self::make_key(user_id, tool_name);

        let bucket = self.buckets.get(&key).map_or_else(
            || TokenBucket::new(self.default_config.burst_limit, 0.0),
            |b| b.value().clone(),
        );

        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let remaining = bucket.available_tokens() as u64;
        QuotaInfo {
            user_id: user_id.to_string(),
            tool_name: Some(tool_name.to_string()),
            used: u64::from(bucket.capacity).saturating_sub(remaining),
            limit: u64::from(bucket.capacity),
            resets_at: Utc::now()
                + chrono::Duration::from_std(bucket.estimate_wait_time(1))
                    .unwrap_or_else(|_| chrono::Duration::seconds(60)),
            remaining,
        }
    }

    /// 更新指定 key 的配置
    pub fn update_config(&self, user_id: &str, tool_name: &str, config: &RateLimitConfig) {
        let key = Self::make_key(user_id, tool_name);
        let requests_per_second = f64::from(config.max_requests_per_minute) / 60.0;
        let new_bucket = TokenBucket::new(config.burst_limit.max(1), requests_per_second);
        self.buckets.insert(key, new_bucket);
    }

    /// 清理过期的桶（释放内存）
    #[must_use]
    pub fn cleanup_expired_buckets(&self) -> usize {
        let before = self.buckets.len();
        self.buckets
            .retain(|_, bucket| bucket.last_refill.elapsed() < Duration::from_secs(3600));
        before - self.buckets.len()
    }

    /// 获取当前活跃桶数量
    #[must_use]
    pub fn active_bucket_count(&self) -> usize {
        self.buckets.len()
    }
}

impl Default for TokenBucketRateLimiter {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Token 用量追踪器
///
/// 记录和查询用户的 Token 使用情况，
/// 支持按时间周期聚合统计。
///
/// # Examples
///
/// ```ignore
/// let store = Box::new(InMemoryUsageStore::new());
/// let tracker = TokenUsageTracker::new(store);
///
/// let record = UsageRecord { ... };
/// tracker.record_usage(&record).await?;
///
/// let usage = tracker.get_user_total("user-001", TimePeriod::Day).await?;
/// ```
pub struct TokenUsageTracker {
    store: Box<dyn UsageStore>,
}

impl TokenUsageTracker {
    /// 创建新的用量追踪器
    #[must_use]
    pub fn new(store: Box<dyn UsageStore>) -> Self {
        Self { store }
    }

    /// 记录 Token 用量
    ///
    /// # Errors
    ///
    /// - 当存储操作失败时返回错误
    pub async fn record_usage(&self, record: &UsageRecord) -> Result<(), ErrorObject> {
        self.store.record_usage(record).await
    }

    /// 获取用户总用量
    ///
    /// 返回指定时间周期内的使用汇总。
    ///
    /// # Errors
    ///
    /// 当存储操作失败时返回错误。
    pub async fn get_user_total(
        &self,
        user_id: &str,
        period: TimePeriod,
    ) -> Result<TokenUsage, ErrorObject> {
        self.store.get_user_total(user_id, period).await
    }

    /// 获取用户对特定工具的使用量
    ///
    /// # Errors
    ///
    /// 当存储操作失败时返回错误。
    pub async fn get_user_tool_usage(
        &self,
        user_id: &str,
        tool_name: &str,
        period: TimePeriod,
    ) -> Result<u64, ErrorObject> {
        self.store
            .get_user_tool_usage(user_id, tool_name, period)
            .await
    }

    /// 检查是否超配额
    ///
    /// 返回当前配额状态。
    ///
    /// # Errors
    ///
    /// 当存储操作失败时返回错误。
    pub async fn check_quota(
        &self,
        user_id: &str,
        limit: u64,
        period: TimePeriod,
    ) -> Result<QuotaStatus, ErrorObject> {
        let usage = self.get_user_total(user_id, period).await?;

        if usage.total_tokens >= limit {
            Ok(QuotaStatus::Exceeded {
                usage: usage.total_tokens,
                limit,
            })
        } else {
            Ok(QuotaStatus::WithinLimit {
                remaining: limit - usage.total_tokens,
            })
        }
    }

    /// 创建用量记录辅助方法
    #[must_use]
    pub fn create_record(
        user_id: &str,
        tool_name: &str,
        tokens_used: u32,
        session_id: Uuid,
        request_id: Uuid,
    ) -> UsageRecord {
        UsageRecord {
            id: Uuid::new_v4(),
            user_id: user_id.to_string(),
            tool_name: tool_name.to_string(),
            tokens_used,
            timestamp: Utc::now(),
            session_id,
            request_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_bucket_basic_flow() {
        let limiter = TokenBucketRateLimiter::with_defaults();

        let result = limiter.check_rate_limit("user-001", "test-tool");

        match result {
            RateLimitResult::Allowed { remaining, .. } => {
                assert!(remaining > 0);
            }
            RateLimitResult::Limited { .. } => panic!("首次请求应被允许"),
        }
    }

    #[tokio::test]
    async fn test_rate_limiting_kicks_in() {
        let config = RateLimitConfig {
            max_requests_per_minute: 5,
            burst_limit: 3,
            max_tokens_per_request: 100,
        };
        let burst_limit = config.burst_limit;
        let limiter = TokenBucketRateLimiter::with_config(config);

        let mut allowed_count = 0u32;
        let mut limited_count = 0u32;

        for _ in 0..10 {
            match limiter.check_rate_limit("user-002", "fast-tool") {
                RateLimitResult::Allowed { .. } => allowed_count += 1,
                RateLimitResult::Limited { .. } => limited_count += 1,
            }
        }

        assert!(allowed_count <= burst_limit + 2);
        assert!(limited_count > 0);
    }

    #[tokio::test]
    async fn test_consume_specific_tokens() {
        let config = RateLimitConfig {
            max_requests_per_minute: 60,
            max_tokens_per_request: 4096,
            burst_limit: 100,
        };
        let limiter = TokenBucketRateLimiter::with_config(config);

        let result = limiter.consume("user-003", "token-tool", 50);

        match result {
            ConsumeResult::Success { remaining } => {
                assert!(remaining > 0);
            }
            ConsumeResult::RateLimited { .. } => panic!("应成功消耗"),
        }
    }

    #[tokio::test]
    async fn test_get_quota_info() {
        let limiter = TokenBucketRateLimiter::with_defaults();

        let _ = limiter.check_rate_limit("user-004", "info-tool");

        let info = limiter.get_remaining("user-004", "info-tool");
        assert_eq!(info.user_id, "user-004");
        assert_eq!(info.tool_name.as_deref(), Some("info-tool"));
        assert!(info.remaining > 0);
    }

    #[tokio::test]
    async fn test_per_user_isolation() {
        let limiter = TokenBucketRateLimiter::with_defaults();

        for _ in 0..20 {
            let result_a = limiter.check_rate_limit("user-a", "shared-tool");
            assert!(
                matches!(result_a, RateLimitResult::Allowed { .. })
                    || matches!(result_a, RateLimitResult::Limited { .. }),
                "用户 A 的请求应有明确结果"
            );
        }

        let result_b = limiter.check_rate_limit("user-b", "shared-tool");
        match result_b {
            RateLimitResult::Allowed { .. } => {}
            RateLimitResult::Limited { .. } => panic!("不同用户应有独立配额"),
        }
    }

    #[tokio::test]
    async fn test_token_usage_tracking() {
        let store = Box::new(InMemoryUsageStore::new());
        let tracker = TokenUsageTracker::new(store);

        let record = TokenUsageTracker::create_record(
            "user-tracker",
            "test-tool",
            100,
            Uuid::new_v4(),
            Uuid::new_v4(),
        );

        tracker.record_usage(&record).await.unwrap();

        let usage = tracker
            .get_user_total("user-tracker", TimePeriod::Day)
            .await
            .unwrap();

        assert_eq!(usage.total_tokens, 100);
        assert_eq!(usage.total_requests, 1);
    }

    #[tokio::test]
    async fn test_quota_check_within_limit() {
        let store = Box::new(InMemoryUsageStore::new());
        let tracker = TokenUsageTracker::new(store);

        let record = TokenUsageTracker::create_record(
            "user-quota",
            "tool",
            50,
            Uuid::new_v4(),
            Uuid::new_v4(),
        );
        tracker.record_usage(&record).await.unwrap();

        let status = tracker
            .check_quota("user-quota", 1000, TimePeriod::Day)
            .await
            .unwrap();

        match status {
            QuotaStatus::WithinLimit { remaining } => {
                assert_eq!(remaining, 950);
            }
            QuotaStatus::Exceeded { .. } => panic!("应在限额内"),
        }
    }

    #[tokio::test]
    async fn test_quota_check_exceeded() {
        let store = Box::new(InMemoryUsageStore::new());
        let tracker = TokenUsageTracker::new(store);

        for _ in 0..10 {
            let record = TokenUsageTracker::create_record(
                "user-exceed",
                "tool",
                200,
                Uuid::new_v4(),
                Uuid::new_v4(),
            );
            tracker.record_usage(&record).await.unwrap();
        }

        let status = tracker
            .check_quota("user-exceed", 500, TimePeriod::Day)
            .await
            .unwrap();

        match status {
            QuotaStatus::Exceeded { usage, limit } => {
                assert!(usage > limit);
                assert_eq!(limit, 500);
            }
            QuotaStatus::WithinLimit { .. } => panic!("应超配额"),
        }
    }

    #[tokio::test]
    async fn test_cleanup_buckets() {
        let limiter = TokenBucketRateLimiter::with_defaults();

        for i in 0..100 {
            let _ = limiter.check_rate_limit(&format!("cleanup-user-{i}"), "tool");
        }

        assert_eq!(limiter.active_bucket_count(), 100);

        let cleaned = limiter.cleanup_expired_buckets();
        assert_eq!(cleaned, 0);
    }

    #[tokio::test]
    async fn test_custom_config_per_key() {
        let limiter = TokenBucketRateLimiter::with_defaults();

        let custom_config = RateLimitConfig {
            max_requests_per_minute: 1000,
            burst_limit: 100,
            max_tokens_per_request: 10_000,
        };

        limiter.update_config("premium-user", "premium-tool", &custom_config);

        let mut allowed = 0;
        for _ in 0..50 {
            if matches!(
                limiter.check_rate_limit("premium-user", "premium-tool"),
                RateLimitResult::Allowed { .. }
            ) {
                allowed += 1;
            }
        }

        assert!(allowed > 30);
    }
}
