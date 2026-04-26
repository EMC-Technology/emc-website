//! Recovery property tests — 使用 proptest 进行属性测试
//!
//! 验证恢复模块的核心属性：
//! - 状态机转换不变量
//! - 指数退避单调递增（无 jitter 时）
//! - 断路器状态转换合法性
//! - 随机配置不 panic

#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use error_core::recovery::{RecoveryStateMachine, ExponentialBackoff, CircuitBreaker, RecoveryState, CircuitBreakerState};
use error_core::propagation::RetryConfig;
use std::time::Duration;
use proptest::prelude::*;

prop_compose! {
    fn any_retry_config()(
        max_attempts in 1u32..=10,
        initial_delay_ms in 100u64..=5000,
        max_delay_ms in 5000u64..=60000,
        backoff_multiplier in 1.0f64..=5.0,
        use_jitter in proptest::bool::ANY,
    ) -> RetryConfig {
        let initial_delay_ms = initial_delay_ms.min(max_delay_ms);
        RetryConfig::new(
            max_attempts,
            initial_delay_ms,
            max_delay_ms,
            backoff_multiplier,
            use_jitter,
        )
    }
}

proptest! {
    #[test]
    fn proptest_recovery_state_machine_initial_state(config in any_retry_config()) {
        let machine = RecoveryStateMachine::new(config.max_attempts(), config);
        prop_assert_eq!(machine.state(), &RecoveryState::Initial);
        prop_assert_eq!(machine.retry_attempts(), 0);
    }

    #[test]
    fn proptest_recovery_state_machine_start_recovery(config in any_retry_config()) {
        let mut machine = RecoveryStateMachine::new(config.max_attempts(), config);
        machine.start_recovery();
        prop_assert_eq!(machine.state(), &RecoveryState::Recovering);
    }

    #[test]
    fn proptest_recovery_state_machine_recover_success_resets_attempts(config in any_retry_config()) {
        let mut machine = RecoveryStateMachine::new(config.max_attempts(), config);
        machine.start_recovery();
        machine.recover_failed();
        machine.recover_failed();
        let attempts_before = machine.retry_attempts();
        prop_assert!(attempts_before > 0);
        machine.recover_success();
        prop_assert_eq!(machine.state(), &RecoveryState::Recovered);
        prop_assert_eq!(machine.retry_attempts(), 0);
    }

    #[test]
    fn proptest_recovery_state_machine_max_attempts_leads_to_failed(
        max_attempts in 1u32..=5,
    ) {
        let config = RetryConfig::new(max_attempts, 100, 10000, 2.0, false);
        let mut machine = RecoveryStateMachine::new(max_attempts, config);
        machine.start_recovery();
        for _ in 0..max_attempts {
            machine.recover_failed();
        }
        prop_assert_eq!(machine.state(), &RecoveryState::Failed);
        prop_assert_eq!(machine.retry_attempts(), max_attempts);
    }

    #[test]
    fn proptest_exponential_backoff_delay_monotonically_non_decreasing(config in any_retry_config()) {
        if config.use_jitter() { return Ok(()); }
        let max_attempts = config.max_attempts();
        let mut backoff = ExponentialBackoff::new(config);
        let mut prev_delay = Duration::ZERO;
        for _ in 0..max_attempts {
            if let Some(delay) = backoff.next_delay() {
                prop_assert!(delay >= prev_delay, "退避延迟应单调非递减: {delay:?} < {prev_delay:?}");
                prev_delay = delay;
            }
        }
    }

    #[test]
    fn proptest_exponential_backoff_respects_max_delay(config in any_retry_config()) {
        if config.use_jitter() { return Ok(()); }
        let max_delay = Duration::from_millis(config.max_delay_ms());
        let max_attempts = config.max_attempts();
        let mut backoff = ExponentialBackoff::new(config);
        for _ in 0..max_attempts {
            if let Some(delay) = backoff.next_delay() {
                prop_assert!(delay <= max_delay, "退避延迟不应超过最大值: {delay:?} > {max_delay:?}");
            }
        }
    }

    #[test]
    fn proptest_exponential_backoff_returns_none_after_max_attempts(config in any_retry_config()) {
        let mut backoff = ExponentialBackoff::new(config.clone());
        for _ in 0..config.max_attempts() {
            let _ = backoff.next_delay();
        }
        prop_assert!(backoff.next_delay().is_none());
    }

    #[test]
    fn proptest_exponential_backoff_reset_works(config in any_retry_config()) {
        let mut backoff = ExponentialBackoff::new(config.clone());
        for _ in 0..config.max_attempts() {
            let _ = backoff.next_delay();
        }
        backoff.reset();
        prop_assert_eq!(backoff.current_attempt(), 0);
        prop_assert!(backoff.next_delay().is_some());
    }

    #[test]
    fn proptest_circuit_breaker_initial_state(failure_threshold in 1u32..=10) {
        let mut breaker = CircuitBreaker::new(failure_threshold, Duration::from_secs(1));
        prop_assert_eq!(breaker.state(), &CircuitBreakerState::Closed);
        prop_assert!(breaker.allow_request());
    }

    #[test]
    fn proptest_circuit_breaker_opens_after_threshold(failure_threshold in 1u32..=5) {
        let mut breaker = CircuitBreaker::new(failure_threshold, Duration::from_secs(1));
        for _ in 0..failure_threshold {
            breaker.record_failure();
        }
        prop_assert_eq!(breaker.state(), &CircuitBreakerState::Open);
        prop_assert!(!breaker.allow_request());
    }

    #[test]
    fn proptest_circuit_breaker_stays_closed_below_threshold(failure_threshold in 2u32..=5) {
        let mut breaker = CircuitBreaker::new(failure_threshold, Duration::from_secs(1));
        for _ in 0..(failure_threshold - 1) {
            breaker.record_failure();
        }
        prop_assert_eq!(breaker.state(), &CircuitBreakerState::Closed);
        prop_assert!(breaker.allow_request());
    }

    #[test]
    fn proptest_circuit_breaker_record_failure_in_open_does_not_change_state(failure_threshold in 1u32..=5) {
        let mut breaker = CircuitBreaker::new(failure_threshold, Duration::from_secs(1));
        for _ in 0..failure_threshold {
            breaker.record_failure();
        }
        prop_assert_eq!(breaker.state(), &CircuitBreakerState::Open);
        breaker.record_failure();
        prop_assert_eq!(breaker.state(), &CircuitBreakerState::Open);
    }
}
