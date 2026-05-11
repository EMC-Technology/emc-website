use std::future::Future;
use std::time::Duration;

use rand::Rng;
use tokio::time::sleep;

use crate::error::LlmError;

/// 重试条件配置
#[derive(Debug, Clone)]
pub struct RetryOn {
    /// 是否重试速率限制错误
    pub rate_limit: bool,
    /// 是否重试服务器错误
    pub server_error: bool,
    /// 是否重试网络错误
    pub network_error: bool,
    /// 是否重试超时错误
    pub timeout: bool,
}

impl Default for RetryOn {
    fn default() -> Self {
        Self {
            rate_limit: true,
            server_error: true,
            network_error: true,
            timeout: true,
        }
    }
}

impl RetryOn {
    /// 启用所有重试条件
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    /// 禁用所有重试条件
    #[must_use]
    pub fn none() -> Self {
        Self {
            rate_limit: false,
            server_error: false,
            network_error: false,
            timeout: false,
        }
    }
}

/// 重试策略
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// 最大尝试次数
    pub max_attempts: u32,
    /// 基础延迟
    pub base_delay: Duration,
    /// 最大延迟
    pub max_delay: Duration,
    /// 重试条件
    pub retry_on: RetryOn,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            retry_on: RetryOn::all(),
        }
    }
}

impl RetryPolicy {
    /// 创建新的重试策略。
    ///
    /// # Errors
    ///
    /// 当 `max_attempts` 为 0 时返回 `LlmError::InvalidRequest`。
    pub fn new(max_attempts: u32, base_delay: Duration) -> Result<Self, LlmError> {
        if max_attempts == 0 {
            return Err(LlmError::InvalidRequest {
                message: "max_attempts must be at least 1".into(),
            });
        }
        Ok(Self {
            max_attempts,
            base_delay,
            ..Default::default()
        })
    }

    // DESIGN: Timeout and ServerOverloaded are both retryable but route to
    // independent retry flags (retry_on.timeout vs retry_on.server_error).
    // This allows callers to disable timeout retries while keeping server-error
    // retries, or vice versa.  See LlmError::from_http_status INVARIANT for
    // the HTTP status → variant mapping that feeds into this method.
    /// 判断是否应对当前错误进行重试
    #[must_use]
    pub fn should_retry(&self, error: &LlmError, attempt: u32) -> bool {
        if attempt >= self.max_attempts {
            return false;
        }
        if !error.is_retryable() {
            return false;
        }
        match error {
            LlmError::RateLimitExceeded { .. } => self.retry_on.rate_limit,
            LlmError::ServerOverloaded => self.retry_on.server_error,
            LlmError::Network(_) => self.retry_on.network_error,
            LlmError::Timeout(_) => self.retry_on.timeout,
            LlmError::AuthenticationError { .. }
            | LlmError::PermissionError { .. }
            | LlmError::ContextWindowExceeded { .. }
            | LlmError::PromptTooLarge(_)
            | LlmError::ModelUnavailable(_)
            | LlmError::InvalidRequest { .. }
            | LlmError::StreamError(_)
            | LlmError::ToolCallError { .. }
            | LlmError::MissingCredentials { .. }
            | LlmError::ExpiredToken(_)
            | LlmError::JsonParse { .. }
            | LlmError::RetriesExhausted { .. }
            | LlmError::Config(_)
            | LlmError::RequestBodySizeExceeded { .. }
            | LlmError::Cancelled
            | LlmError::HttpClientInit(_)
            | LlmError::Other(_) => false,
        }
    }
}

/// 计算指数退避延迟（含抖动）
#[must_use]
pub fn backoff(base: Duration, attempt: u32) -> Duration {
    if attempt == 0 {
        return base;
    }
    let exp = 2u64.saturating_pow(attempt - 1);
    let millis = u64::try_from(base.as_millis()).unwrap_or(u64::MAX);
    let raw = millis.saturating_mul(exp);
    let jitter_pct = 90 + (rand::rng().random::<u32>() % 21);
    let jittered = raw.saturating_mul(u64::from(jitter_pct)) / 100;
    Duration::from_millis(jittered)
}

/// 使用重试策略执行异步操作。
///
/// # Errors
///
/// 当所有重试次数耗尽后仍失败时返回 `LlmError::RetriesExhausted`；当错误不可重试时直接返回原始错误。
pub async fn run_with_retry<T, F, Fut>(
    policy: &RetryPolicy,
    mut operation: F,
) -> Result<T, LlmError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, LlmError>>,
{
    for attempt in 0..policy.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                if !policy.should_retry(&err, attempt) {
                    return Err(err);
                }
                let delay = backoff(policy.base_delay, attempt);
                let delay = delay.min(policy.max_delay);
                sleep(delay).await;
            }
        }
    }
    Err(LlmError::RetriesExhausted {
        attempts: policy.max_attempts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_on_all() {
        let on = RetryOn::all();
        assert!(on.rate_limit);
        assert!(on.server_error);
        assert!(on.network_error);
        assert!(on.timeout);
    }

    #[test]
    fn test_retry_on_none() {
        let on = RetryOn::none();
        assert!(!on.rate_limit);
        assert!(!on.server_error);
        assert!(!on.network_error);
        assert!(!on.timeout);
    }

    #[test]
    fn test_default_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.base_delay, Duration::from_secs(1));
        assert_eq!(policy.max_delay, Duration::from_secs(30));
    }

    #[test]
    fn test_should_retry_rate_limit() {
        let policy = RetryPolicy::default();
        let err = LlmError::RateLimitExceeded {
            retry_after_ms: None,
        };
        assert!(policy.should_retry(&err, 0));
        assert!(!policy.should_retry(&err, 3));
    }

    #[test]
    fn test_should_retry_network() {
        let policy = RetryPolicy::default();
        let err = LlmError::Network("conn refused".into());
        assert!(policy.should_retry(&err, 0));
    }

    #[test]
    fn test_should_not_retry_auth() {
        let policy = RetryPolicy::default();
        let err = LlmError::AuthenticationError {
            message: "bad key".into(),
        };
        assert!(!policy.should_retry(&err, 0));
    }

    #[test]
    fn test_should_not_retry_when_disabled() {
        let policy = RetryPolicy {
            retry_on: RetryOn {
                network_error: false,
                ..RetryOn::all()
            },
            ..Default::default()
        };
        let err = LlmError::Network("conn refused".into());
        assert!(!policy.should_retry(&err, 0));
    }

    #[test]
    fn test_backoff_increases() {
        let base = Duration::from_secs(1);
        let d0 = backoff(base, 0);
        assert_eq!(d0, base);
        let d1 = backoff(base, 1);
        assert!(d1 >= Duration::from_millis(900));
        let d2 = backoff(base, 2);
        assert!(d2 >= Duration::from_millis(1800));
    }

    #[test]
    fn test_backoff_exponential_growth() {
        let base = Duration::from_secs(1);
        let d3 = backoff(base, 3);
        assert!(d3 >= Duration::from_millis(3600));
        let d4 = backoff(base, 4);
        assert!(d4 >= Duration::from_millis(7200));
    }

    #[tokio::test]
    async fn test_run_with_retry_success() {
        let policy = RetryPolicy::new(3, Duration::from_millis(1)).unwrap();
        let mut count = 0;
        let result = run_with_retry(&policy, || {
            count += 1;
            async move { Ok::<i32, LlmError>(42) }
        })
        .await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_run_with_retry_eventual_success() {
        let policy = RetryPolicy::new(3, Duration::from_millis(1)).unwrap();
        let mut count = 0;
        let result = run_with_retry(&policy, || {
            count += 1;
            async move {
                if count < 3 {
                    Err(LlmError::Network("retry".into()))
                } else {
                    Ok(42)
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_run_with_retry_exhausted() {
        let policy = RetryPolicy::new(2, Duration::from_millis(1)).unwrap();
        let result: Result<i32, LlmError> = run_with_retry(&policy, || async {
            Err(LlmError::Network("always fail".into()))
        })
        .await;
        assert!(matches!(
            result,
            Err(LlmError::RetriesExhausted { attempts: 2 })
        ));
    }

    #[tokio::test]
    async fn test_run_with_retry_non_retryable() {
        let policy = RetryPolicy::new(3, Duration::from_millis(1)).unwrap();
        let result: Result<i32, LlmError> = run_with_retry(&policy, || async {
            Err(LlmError::AuthenticationError {
                message: "bad key".into(),
            })
        })
        .await;
        assert!(matches!(result, Err(LlmError::AuthenticationError { .. })));
    }

    #[test]
    fn test_should_retry_server_error() {
        let policy = RetryPolicy::default();
        let err = LlmError::ServerOverloaded;
        assert!(policy.should_retry(&err, 0));
    }

    #[test]
    fn test_should_not_retry_server_error_when_disabled() {
        let policy = RetryPolicy {
            retry_on: RetryOn {
                server_error: false,
                ..RetryOn::all()
            },
            ..Default::default()
        };
        let err = LlmError::ServerOverloaded;
        assert!(!policy.should_retry(&err, 0));
    }

    #[test]
    fn test_should_not_retry_rate_limit_when_disabled() {
        let policy = RetryPolicy {
            retry_on: RetryOn {
                rate_limit: false,
                ..RetryOn::all()
            },
            ..Default::default()
        };
        let err = LlmError::RateLimitExceeded {
            retry_after_ms: None,
        };
        assert!(!policy.should_retry(&err, 0));
    }

    #[test]
    fn test_should_not_retry_non_retryable_errors() {
        let policy = RetryPolicy::default();
        let err = LlmError::InvalidRequest {
            message: "bad".into(),
        };
        assert!(!policy.should_retry(&err, 0));
        let err = LlmError::PermissionError {
            message: "denied".into(),
        };
        assert!(!policy.should_retry(&err, 0));
    }

    #[test]
    fn test_retry_policy_with_retry_on() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(30),
            retry_on: RetryOn {
                network_error: true,
                rate_limit: false,
                server_error: false,
                timeout: false,
            },
        };
        assert_eq!(policy.max_attempts, 5);
        assert!(policy.retry_on.network_error);
        assert!(!policy.retry_on.rate_limit);
    }

    #[tokio::test]
    async fn test_run_with_retry_rate_limit_with_retry_after() {
        let policy = RetryPolicy::new(3, Duration::from_millis(1)).unwrap();
        let mut count = 0;
        let result = run_with_retry(&policy, || {
            count += 1;
            async move {
                if count < 2 {
                    Err(LlmError::RateLimitExceeded {
                        retry_after_ms: Some(1),
                    })
                } else {
                    Ok(42)
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_retry_policy_new_zero_attempts() {
        let result = RetryPolicy::new(0, Duration::from_secs(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_timeout_and_server_overloaded_route_to_different_retry_flags() {
        let policy = RetryPolicy {
            retry_on: RetryOn {
                timeout: true,
                server_error: false,
                ..RetryOn::all()
            },
            ..Default::default()
        };
        let timeout_err = LlmError::Timeout(0);
        let server_err = LlmError::ServerOverloaded;
        assert!(
            policy.should_retry(&timeout_err, 0),
            "Timeout MUST be retryable when retry_on.timeout = true"
        );
        assert!(
            !policy.should_retry(&server_err, 0),
            "ServerOverloaded MUST NOT be retried when retry_on.server_error = false"
        );

        let policy_flipped = RetryPolicy {
            retry_on: RetryOn {
                timeout: false,
                server_error: true,
                ..RetryOn::all()
            },
            ..Default::default()
        };
        assert!(
            !policy_flipped.should_retry(&timeout_err, 0),
            "Timeout MUST NOT be retried when retry_on.timeout = false"
        );
        assert!(
            policy_flipped.should_retry(&server_err, 0),
            "ServerOverloaded MUST be retryable when retry_on.server_error = true"
        );
    }

    #[test]
    fn test_kani_backoff_always_ge_base() {
        let base = Duration::from_millis(100);
        for attempt in 0..=10u32 {
            let delay = backoff(base, attempt);
            assert!(
                delay >= base * 90 / 100,
                "backoff(base, {attempt}) = {delay:?} must be >= 90% of base = {base:?}"
            );
        }
    }

    #[test]
    fn test_kani_backoff_monotonically_non_decreasing() {
        let base = Duration::from_millis(100);
        for attempt in 1..=10u32 {
            let delay = backoff(base, attempt);
            let theoretical_min = base * 2u32.saturating_pow(attempt - 1);
            let lower_bound = theoretical_min * 90 / 100;
            assert!(
                delay >= lower_bound,
                "backoff(base, {attempt}) = {delay:?} must be >= 90% of theoretical {theoretical_min:?}"
            );
        }
    }

    #[test]
    fn test_kani_backoff_never_zero() {
        let base = Duration::from_millis(100);
        for attempt in 0..=20u32 {
            let delay = backoff(base, attempt);
            assert!(!delay.is_zero(), "backoff must never return zero duration");
        }
    }

    #[test]
    fn test_deductive_should_retry_exhausted_implies_false() {
        let policy = RetryPolicy::default();
        let retryable_errors: Vec<LlmError> = vec![
            LlmError::Network("x".into()),
            LlmError::Timeout(100),
            LlmError::RateLimitExceeded {
                retry_after_ms: None,
            },
            LlmError::ServerOverloaded,
            LlmError::ModelUnavailable("x".into()),
        ];
        for err in retryable_errors {
            assert!(
                !policy.should_retry(&err, policy.max_attempts),
                "should_retry MUST return false when attempt >= max_attempts for {err:?}"
            );
            assert!(
                !policy.should_retry(&err, policy.max_attempts + 100),
                "should_retry MUST return false when attempt >> max_attempts for {err:?}"
            );
        }
    }

    #[test]
    fn test_deductive_should_retry_non_retryable_always_false() {
        let policy = RetryPolicy::default();
        let non_retryable_errors: Vec<LlmError> = vec![
            LlmError::AuthenticationError {
                message: "x".into(),
            },
            LlmError::PermissionError {
                message: "x".into(),
            },
            LlmError::InvalidRequest {
                message: "x".into(),
            },
            LlmError::Config("x".into()),
            LlmError::Cancelled,
            LlmError::RetriesExhausted { attempts: 3 },
        ];
        for err in non_retryable_errors {
            for attempt in 0..=5u32 {
                assert!(
                    !policy.should_retry(&err, attempt),
                    "non-retryable error MUST always return false: err={err:?}, attempt={attempt}"
                );
            }
        }
    }

    #[test]
    fn test_deductive_retry_policy_new_zero_attempts_invariant() {
        let result = RetryPolicy::new(0, Duration::from_secs(1));
        assert!(result.is_err(), "RetryPolicy::new(0, ...) MUST return Err");
    }

    #[test]
    fn test_deductive_backoff_attempt_zero_equals_base() {
        let base = Duration::from_millis(500);
        let delay = backoff(base, 0);
        assert_eq!(delay, base, "backoff(base, 0) MUST equal base exactly");
    }

    #[test]
    fn test_deductive_backoff_saturating_no_overflow() {
        let base = Duration::from_secs(u64::MAX);
        let delay = backoff(base, 100);
        assert!(
            delay > Duration::ZERO,
            "backoff must not panic on large base"
        );
        assert!(
            delay <= Duration::from_secs(u64::MAX),
            "backoff must not overflow"
        );
    }
}
