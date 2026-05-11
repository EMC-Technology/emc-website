//! Recovery module formal verification tests using Kani
//!
//! 包含两类验证：
//! - 硬编码输入验证：确保特定输入路径的正确性
//! - 符号执行验证（kani::any()）：穷举所有可能的输入状态空间
#![cfg(kani)]

use error_core::propagation::RetryConfig;
use error_core::recovery::*;
use std::time::Duration;

// Test RecoveryStateMachine
#[kani::proof]
fn test_recovery_state_machine() {
    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);
    let mut machine = RecoveryStateMachine::new(3, retry_config);

    assert_eq!(*machine.state(), RecoveryState::Initial);
    assert_eq!(machine.retry_attempts(), 0);

    machine.start_recovery();
    assert_eq!(*machine.state(), RecoveryState::Recovering);

    machine.recover_failed();
    assert_eq!(*machine.state(), RecoveryState::Recovering);
    assert_eq!(machine.retry_attempts(), 1);

    machine.recover_failed();
    assert_eq!(*machine.state(), RecoveryState::Recovering);
    assert_eq!(machine.retry_attempts(), 2);

    machine.recover_failed();
    assert_eq!(*machine.state(), RecoveryState::Failed);
    assert_eq!(machine.retry_attempts(), 3);
}

// Test RecoveryStateMachine success path
#[kani::proof]
fn test_recovery_state_machine_success() {
    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);
    let mut machine = RecoveryStateMachine::new(3, retry_config);

    machine.start_recovery();
    assert_eq!(*machine.state(), RecoveryState::Recovering);

    machine.recover_success();
    assert_eq!(*machine.state(), RecoveryState::Recovered);
    assert_eq!(machine.retry_attempts(), 0);
}

// Test RecoveryStateMachine circuit open and degradation
#[kani::proof]
fn test_recovery_state_machine_circuit_and_degradation() {
    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);
    let mut machine = RecoveryStateMachine::new(3, retry_config);

    machine.open_circuit();
    assert_eq!(*machine.state(), RecoveryState::CircuitOpen);

    machine.activate_degradation();
    assert_eq!(*machine.state(), RecoveryState::Degraded);
}

// Test RecoveryStateMachine calculate_retry_delay
#[kani::proof]
fn test_recovery_state_machine_calculate_retry_delay() {
    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);
    let mut machine = RecoveryStateMachine::new(3, retry_config);

    // Test delay for attempt 0
    let delay0 = machine.calculate_retry_delay();
    assert!(delay0 >= Duration::from_millis(1000));

    // Test delay for attempt 1
    machine.recover_failed();
    let delay1 = machine.calculate_retry_delay();
    assert!(delay1 >= Duration::from_millis(2000));

    // Test delay for attempt 2
    machine.recover_failed();
    let delay2 = machine.calculate_retry_delay();
    assert!(delay2 >= Duration::from_millis(4000));
}

// Test ExponentialBackoff
#[kani::proof]
fn test_exponential_backoff() {
    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, false);
    let mut backoff = ExponentialBackoff::new(retry_config);

    let delay1 = backoff.next_delay().unwrap();
    assert!(delay1 >= Duration::from_millis(1000));

    let delay2 = backoff.next_delay().unwrap();
    assert!(delay2 >= Duration::from_millis(2000));

    let delay3 = backoff.next_delay().unwrap();
    assert!(delay3 >= Duration::from_millis(4000));

    let delay4 = backoff.next_delay();
    assert!(delay4.is_none());
}

// Test ExponentialBackoff with reset
#[kani::proof]
fn test_exponential_backoff_reset() {
    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, false);
    let mut backoff = ExponentialBackoff::new(retry_config);

    backoff.next_delay();
    backoff.next_delay();
    assert_eq!(backoff.current_attempt(), 2);

    backoff.reset();
    assert_eq!(backoff.current_attempt(), 0);

    let delay = backoff.next_delay().unwrap();
    assert!(delay >= Duration::from_millis(1000));
    assert_eq!(backoff.current_attempt(), 1);
}

// Test CircuitBreaker initial state
#[kani::proof]
fn test_circuit_breaker_initial() {
    let reset_timeout = Duration::from_millis(100);
    let mut breaker = CircuitBreaker::new(2, reset_timeout);

    assert_eq!(*breaker.state(), CircuitBreakerState::Closed);
    assert!(breaker.allow_request());
}

// Test CircuitBreaker failure threshold
#[kani::proof]
fn test_circuit_breaker_failure_threshold() {
    let reset_timeout = Duration::from_millis(100);
    let mut breaker = CircuitBreaker::new(2, reset_timeout);

    breaker.record_failure();
    assert_eq!(*breaker.state(), CircuitBreakerState::Closed);

    breaker.record_failure();
    assert_eq!(*breaker.state(), CircuitBreakerState::Open);
    assert!(!breaker.allow_request());
}

// Test CircuitBreaker record_success in HalfOpen state
#[kani::proof]
fn test_circuit_breaker_record_success() {
    let reset_timeout = Duration::from_millis(100);
    let mut breaker = CircuitBreaker::new(1, reset_timeout);

    breaker.record_failure();
    assert_eq!(*breaker.state(), CircuitBreakerState::Open);

    breaker.set_state(CircuitBreakerState::HalfOpen);

    breaker.record_success();
    assert_eq!(*breaker.state(), CircuitBreakerState::Closed);
}

// Test CircuitBreaker record_failure in HalfOpen state
#[kani::proof]
fn test_circuit_breaker_record_failure_half_open() {
    let reset_timeout = Duration::from_millis(100);
    let mut breaker = CircuitBreaker::new(1, reset_timeout);

    breaker.set_state(CircuitBreakerState::HalfOpen);

    breaker.record_failure();
    assert_eq!(*breaker.state(), CircuitBreakerState::Open);
}

// ============================================================
// 符号执行验证（kani::any()）— 穷举所有可能的输入状态空间
// ============================================================

/// 验证：任意合法 max_attempts (1..=10)，状态机初始状态始终为 Initial
#[kani::proof]
fn kani_proof_state_machine_initial_state_any() {
    let max_attempts: u32 = kani::any();
    kani::assume(max_attempts >= 1);
    kani::assume(max_attempts <= 10);
    let config = RetryConfig::new(max_attempts, 1000, 10000, 2.0, false);
    let machine = RecoveryStateMachine::new(max_attempts, config);
    assert_eq!(machine.state(), &RecoveryState::Initial);
    assert_eq!(machine.retry_attempts(), 0);
}

/// 验证：任意合法 failure_threshold (1..=10)，断路器初始状态始终为 Closed
#[kani::proof]
fn kani_proof_circuit_breaker_initial_state_any() {
    let failure_threshold: u32 = kani::any();
    kani::assume(failure_threshold >= 1);
    kani::assume(failure_threshold <= 10);
    let mut breaker = CircuitBreaker::new(failure_threshold, Duration::from_secs(1));
    assert_eq!(breaker.state(), &CircuitBreakerState::Closed);
    assert!(breaker.allow_request());
}

/// 验证：任意合法 failure_threshold，恰好达到阈值时断路器必定 Open
#[kani::proof]
fn kani_proof_circuit_breaker_opens_at_threshold_any() {
    let failure_threshold: u32 = kani::any();
    kani::assume(failure_threshold >= 1);
    kani::assume(failure_threshold <= 10);
    let mut breaker = CircuitBreaker::new(failure_threshold, Duration::from_secs(1));
    for _ in 0..failure_threshold {
        breaker.record_failure();
    }
    assert_eq!(breaker.state(), &CircuitBreakerState::Open);
    assert!(!breaker.allow_request());
}

/// 验证：任意合法 failure_threshold，未达到阈值时断路器保持 Closed
#[kani::proof]
fn kani_proof_circuit_breaker_closed_below_threshold_any() {
    let failure_threshold: u32 = kani::any();
    kani::assume(failure_threshold >= 2);
    kani::assume(failure_threshold <= 10);
    let mut breaker = CircuitBreaker::new(failure_threshold, Duration::from_secs(1));
    for _ in 0..(failure_threshold - 1) {
        breaker.record_failure();
    }
    assert_eq!(breaker.state(), &CircuitBreakerState::Closed);
    assert!(breaker.allow_request());
}

/// 验证：任意合法 max_attempts，恰好达到最大重试次数时状态机必定 Failed
#[kani::proof]
fn kani_proof_state_machine_failed_at_max_attempts_any() {
    let max_attempts: u32 = kani::any();
    kani::assume(max_attempts >= 1);
    kani::assume(max_attempts <= 10);
    let config = RetryConfig::new(max_attempts, 100, 10000, 2.0, false);
    let mut machine = RecoveryStateMachine::new(max_attempts, config);
    machine.start_recovery();
    for _ in 0..max_attempts {
        machine.recover_failed();
    }
    assert_eq!(machine.state(), &RecoveryState::Failed);
    assert_eq!(machine.retry_attempts(), max_attempts);
}
