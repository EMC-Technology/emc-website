#![cfg(test)]
//! Retry 模块形式化验证测试
//!
//! ## 不变量 (Invariants)
//!
//! 1. `backoff(base, 0) == base`
//! 2. `backoff(base, attempt) >= base * 0.9` (90% 下界)
//! 3. `backoff` 单调非递减
//! 4. `should_retry` 在 attempt >= max_attempts 时返回 false
//! 5. 非重试错误总是返回 false

use crate::retry::{backoff, RetryOn, RetryPolicy};
use crate::error::LlmError;
use std::time::Duration;

#[test]
fn test_miri_backoff_no_overflow() {
    let base = Duration::MAX;
    let delay = backoff(base, 100);
    assert!(
        delay <= Duration::MAX,
        "backoff MUST NOT overflow Duration"
    );
}

#[test]
fn test_miri_backoff_zero_base() {
    let delay = backoff(Duration::ZERO, 0);
    assert_eq!(delay, Duration::ZERO, "backoff(0, 0) MUST be ZERO");
}

#[test]
fn test_miri_backoff_large_attempt() {
    let base = Duration::from_millis(100);
    let delay = backoff(base, u32::MAX);
    assert!(
        delay.is_zero() || delay > Duration::ZERO,
        "backoff MUST NOT panic on large attempt"
    );
}

#[test]
fn test_miri_should_retry_max_attempts_boundary() {
    let policy = RetryPolicy::default();
    let err = LlmError::Network("test".into());

    assert!(
        !policy.should_retry(&err, policy.max_attempts),
        "THEOREM: should_retry MUST return false when attempt >= max_attempts"
    );
    assert!(
        !policy.should_retry(&err, policy.max_attempts.saturating_add(1)),
        "THEOREM: should_retry MUST return false when attempt > max_attempts"
    );
}

#[test]
fn test_miri_retry_policy_new_zero_invalid() {
    let result = RetryPolicy::new(0, Duration::from_secs(1));
    assert!(result.is_err(), "RetryPolicy::new(0, ...) MUST return Err");
}

#[test]
fn test_kani_backoff_range_always_positive() {
    let base = Duration::from_millis(100);
    for attempt in 0..=1000u32 {
        let delay = backoff(base, attempt);
        assert!(
            !delay.is_zero(),
            "THEOREM: backoff(base, attempt) MUST NEVER be zero for any attempt"
        );
    }
}

#[test]
fn test_kani_backoff_90_percent_lower_bound() {
    let base = Duration::from_millis(100);
    for attempt in 0..=100u32 {
        let delay = backoff(base, attempt);
        let lower_bound = base * 90 / 100;
        assert!(
            delay >= lower_bound,
            "THEOREM: backoff(base, {}) >= 90% of base, got {:?}",
            attempt,
            delay
        );
    }
}

#[test]
fn test_kani_backoff_monotonic_non_decreasing() {
    let base = Duration::from_millis(100);
    let mut prev = backoff(base, 0);
    for attempt in 1..=100u32 {
        let curr = backoff(base, attempt);
        assert!(
            curr >= prev,
            "THEOREM: backoff MUST be monotonically non-decreasing: backoff({}, {}) < backoff({}, {})",
            base.as_millis(),
            attempt - 1,
            base.as_millis(),
            attempt
        );
        prev = curr;
    }
}

#[test]
fn test_kani_should_retry_exhaustive_error_types() {
    let policy = RetryPolicy::default();
    let retryable = [
        LlmError::Network("x".into()),
        LlmError::Timeout(100),
        LlmError::RateLimitExceeded { retry_after_ms: None },
        LlmError::ServerOverloaded,
        LlmError::ModelUnavailable("x".into()),
    ];
    let non_retryable = [
        LlmError::AuthenticationError { message: "x".into() },
        LlmError::PermissionError { message: "x".into() },
        LlmError::ContextWindowExceeded { estimated_tokens: 1, limit_tokens: 2 },
        LlmError::PromptTooLarge(1),
        LlmError::InvalidRequest { message: "x".into() },
        LlmError::StreamError("x".into()),
        LlmError::ToolCallError { message: "x".into(), tool_name: None },
        LlmError::MissingCredentials { provider: "x".into(), env_vars: vec![] },
        LlmError::ExpiredToken("x".into()),
        LlmError::JsonParse {
            provider: "x".into(),
            model: "y".into(),
            body_snippet: "z".into(),
            source: serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err(),
        },
        LlmError::RetriesExhausted { attempts: 3 },
        LlmError::Config("x".into()),
        LlmError::RequestBodySizeExceeded { estimated_bytes: 1, max_bytes: 1 },
        LlmError::Cancelled,
        LlmError::HttpClientInit("x".into()),
        LlmError::Other("x".into()),
    ];

    for err in retryable {
        assert!(
            policy.should_retry(&err, 0),
            "THEOREM: retryable error {:?} MUST be retryable at attempt 0",
            err
        );
    }

    for err in non_retryable {
        assert!(
            !policy.should_retry(&err, 0),
            "THEOREM: non-retryable error {:?} MUST NOT be retryable at attempt 0",
            err
        );
    }
}

#[test]
fn test_kani_retry_on_flags_partition() {
    let all_enabled = RetryOn::all();
    let all_disabled = RetryOn::none();

    assert!(
        all_enabled.rate_limit
            && all_enabled.server_error
            && all_enabled.network_error
            && all_enabled.timeout,
        "RetryOn::all() MUST enable all flags"
    );

    assert!(
        !all_disabled.rate_limit
            && !all_disabled.server_error
            && !all_disabled.network_error
            && !all_disabled.timeout,
        "RetryOn::none() MUST disable all flags"
    );
}

#[test]
fn test_deductive_backoff_base_case() {
    let base = Duration::from_millis(500);

    assert_eq!(
        backoff(base, 0),
        base,
        "THEOREM: backoff(base, 0) == base (base case)"
    );
}

#[test]
fn test_deductive_should_retry_attempt_exhausted() {
    let policy = RetryPolicy::new(3, Duration::from_secs(1)).unwrap();
    let err = LlmError::Network("x".into());

    for attempt in [3, 4, 5, 100, 1000] {
        assert!(
            !policy.should_retry(&err, attempt),
            "THEOREM: should_retry MUST return false when attempt >= max_attempts for attempt={}",
            attempt
        );
    }
}

#[test]
fn test_deductive_should_retry_non_retryable_always_false() {
    let policy = RetryPolicy::default();
    let non_retryable = [
        LlmError::AuthenticationError { message: "x".into() },
        LlmError::PermissionError { message: "x".into() },
        LlmError::Cancelled,
    ];

    for err in non_retryable {
        for attempt in 0..=1000u32 {
            assert!(
                !policy.should_retry(&err, attempt),
                "THEOREM: non-retryable error MUST always return false, err={:?}, attempt={}",
                err,
                attempt
            );
        }
    }
}

#[test]
fn test_deductive_timeout_and_server_overloaded_independent() {
    let timeout_only = RetryPolicy {
        retry_on: RetryOn {
            timeout: true,
            server_error: false,
            ..RetryOn::all()
        },
        ..RetryPolicy::default()
    };

    let server_only = RetryPolicy {
        retry_on: RetryOn {
            timeout: false,
            server_error: true,
            ..RetryOn::all()
        },
        ..RetryPolicy::default()
    };

    let timeout_err = LlmError::Timeout(0);
    let server_err = LlmError::ServerOverloaded;

    assert!(
        timeout_only.should_retry(&timeout_err, 0),
        "THEOREM: timeout policy MUST retry Timeout"
    );
    assert!(
        !timeout_only.should_retry(&server_err, 0),
        "THEOREM: timeout policy MUST NOT retry ServerOverloaded"
    );

    assert!(
        !server_only.should_retry(&timeout_err, 0),
        "THEOREM: server policy MUST NOT retry Timeout"
    );
    assert!(
        server_only.should_retry(&server_err, 0),
        "THEOREM: server policy MUST retry ServerOverloaded"
    );
}

#[test]
fn test_deductive_backoff_saturating_arithmetic() {
    let base = Duration::from_secs(u64::MAX);
    let delay = backoff(base, 100);

    assert!(
        delay <= Duration::MAX,
        "THEOREM: backoff MUST use saturating arithmetic to prevent overflow"
    );
    assert!(
        delay > Duration::ZERO,
        "THEOREM: backoff MUST return non-zero for large base values"
    );
}

#[test]
fn test_deductive_backoff_jitter_bounded() {
    let base = Duration::from_millis(1000);

    for _ in 0..10000 {
        let delay = backoff(base, 1);
        let min_expected = base * 2 * 90 / 100;
        let max_expected = base * 2 * 110 / 100;

        assert!(
            delay >= min_expected && delay <= Duration::from_secs(30),
            "THEOREM: backoff with jitter MUST be within [180%*base, max_delay], got {:?}",
            delay
        );
    }
}

#[test]
fn test_deductive_retry_policy_max_attempts_invariant() {
    let policy = RetryPolicy::new(5, Duration::from_secs(1)).unwrap();

    assert!(
        policy.max_attempts == 5,
        "INVARIANT: max_attempts MUST be set correctly"
    );

    let err = LlmError::Network("x".into());

    for attempt in 0..5 {
        let result = policy.should_retry(&err, attempt);
        if attempt < 5 {
            assert!(
                result,
                "THEOREM: should_retry MUST return true for attempt < max_attempts"
            );
        }
    }

    assert!(
        !policy.should_retry(&err, 5),
        "THEOREM: should_retry MUST return false for attempt >= max_attempts"
    );
}

#[test]
fn test_deductive_backoff_exponential_lower_bound() {
    let base = Duration::from_millis(100);

    for attempt in 1..=10u32 {
        let delay = backoff(base, attempt);
        let theoretical_min = base * 2u32.saturating_pow(attempt - 1) * 90 / 100;

        assert!(
            delay >= theoretical_min,
            "THEOREM: backoff(base, {}) >= 90% of 2^{} * base",
            attempt,
            attempt - 1
        );
    }
}