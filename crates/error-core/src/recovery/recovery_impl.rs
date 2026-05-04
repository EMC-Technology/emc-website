//! Error recovery mechanisms
//!
//! This module defines error recovery mechanisms, including the error recovery state machine
//! and exponential backoff retry strategy.

use crate::propagation::RetryConfig;
use std::time::Duration;

/// Error recovery state
///
/// Represents the state of an error recovery process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryState {
    /// Initial state, no recovery attempted yet
    Initial,
    /// Recovery is in progress
    Recovering,
    /// Recovery succeeded
    Recovered,
    /// Recovery failed
    Failed,
    /// Circuit breaker is open
    CircuitOpen,
    /// Graceful degradation activated
    Degraded,
}

/// Error recovery state machine
///
/// Manages the state transitions for error recovery processes.
pub struct RecoveryStateMachine {
    state: RecoveryState,
    retry_attempts: u32,
    max_attempts: u32,
    retry_config: RetryConfig,
}

impl RecoveryStateMachine {
    /// Create a new `RecoveryStateMachine`
    #[must_use]
    pub const fn new(max_attempts: u32, retry_config: RetryConfig) -> Self {
        Self {
            state: RecoveryState::Initial,
            retry_attempts: 0,
            max_attempts,
            retry_config,
        }
    }

    /// Get the current state
    #[must_use]
    pub const fn state(&self) -> &RecoveryState {
        &self.state
    }

    /// Get the number of retry attempts
    #[must_use]
    pub const fn retry_attempts(&self) -> u32 {
        self.retry_attempts
    }

    /// Start the recovery process
    ///
    /// # Note
    ///
    /// 可从任意状态调用。调用者应确保状态转换的语义合理性。
    pub const fn start_recovery(&mut self) {
        self.state = RecoveryState::Recovering;
    }

    /// Record a successful recovery
    ///
    /// # Note
    ///
    /// 可从任意状态调用。调用者应确保状态转换的语义合理性。
    pub const fn recover_success(&mut self) {
        self.state = RecoveryState::Recovered;
        self.retry_attempts = 0;
    }

    /// Record a failed recovery attempt
    pub const fn recover_failed(&mut self) {
        self.retry_attempts = self.retry_attempts.saturating_add(1);

        if self.retry_attempts >= self.max_attempts {
            self.state = RecoveryState::Failed;
        } else {
            self.state = RecoveryState::Recovering;
        }
    }

    /// Open the circuit breaker
    pub const fn open_circuit(&mut self) {
        self.state = RecoveryState::CircuitOpen;
    }

    /// Activate graceful degradation
    pub const fn activate_degradation(&mut self) {
        self.state = RecoveryState::Degraded;
    }

    /// Calculate the next retry delay
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn calculate_retry_delay(&self) -> Duration {
        let attempt = self.retry_attempts;
        let initial_delay = self.retry_config.initial_delay_ms();
        let multiplier = self.retry_config.backoff_multiplier();
        let max_delay = self.retry_config.max_delay_ms();

        let delay_ms = initial_delay as f64 * (multiplier.powi(attempt as i32));
        let delay_ms = delay_ms.min(max_delay as f64);

        let delay_ms = if self.retry_config.use_jitter() {
            let jitter = deterministic_jitter(attempt);
            delay_ms * (1.0 + jitter)
        } else {
            delay_ms
        };

        let delay_ms = delay_ms.round().clamp(0.0, u64::MAX as f64) as u64;

        Duration::from_millis(delay_ms)
    }
}

/// Exponential backoff retry strategy
///
/// Implements an exponential backoff algorithm for retries.
pub struct ExponentialBackoff {
    config: RetryConfig,
    current_attempt: u32,
}

impl ExponentialBackoff {
    /// Create a new `ExponentialBackoff`
    #[must_use]
    pub const fn new(config: RetryConfig) -> Self {
        Self {
            config,
            current_attempt: 0,
        }
    }

    /// Get the next retry delay
    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.current_attempt >= self.config.max_attempts() {
            return None;
        }

        let delay = self.calculate_delay(self.current_attempt);
        self.current_attempt = self.current_attempt.saturating_add(1);
        Some(delay)
    }

    /// Calculate the delay for a given attempt
    ///
    /// # 确定性保证
    ///
    /// 当 `use_jitter` 启用时，使用基于 attempt 的确定性抖动
    /// （FNV-1a 哈希），而非 `rand::random()`，确保相同输入
    /// 在不同硬件/运行时环境下产生字节级一致的输出。
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let initial_delay = self.config.initial_delay_ms();
        let multiplier = self.config.backoff_multiplier();
        let max_delay = self.config.max_delay_ms();

        let delay_ms = initial_delay as f64 * (multiplier.powi(attempt as i32));
        let delay_ms = delay_ms.min(max_delay as f64);

        let delay_ms = if self.config.use_jitter() {
            let jitter = deterministic_jitter(attempt);
            delay_ms * (1.0 + jitter)
        } else {
            delay_ms
        };

        let delay_ms = delay_ms.round().clamp(0.0, u64::MAX as f64) as u64;

        Duration::from_millis(delay_ms)
    }

    /// Reset the retry counter
    pub const fn reset(&mut self) {
        self.current_attempt = 0;
    }

    /// Get the current attempt count
    #[must_use]
    pub const fn current_attempt(&self) -> u32 {
        self.current_attempt
    }
}

/// 基于尝试次数的确定性抖动计算
///
/// 使用 FNV-1a 哈希将 attempt 映射到 [-0.1, +0.1] 区间，
/// 确保相同 attempt 值始终产生相同的抖动因子。
/// 这消除了 `rand::random()` 的非确定性，同时保持抖动的
/// 防雷暴（thundering herd）效果。
///
/// # 确定性保证
///
/// 给定相同的 `attempt` 值，此函数始终返回相同的抖动因子。
fn deterministic_jitter(attempt: u32) -> f64 {
    let mut hash: u32 = 2_166_136_261;
    let bytes = attempt.to_le_bytes();
    for &byte in &bytes {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    let normalized = f64::from(hash % 1000) / 1000.0;
    normalized.mul_add(0.2, -0.1)
}

/// Circuit breaker state
///
/// Represents the state of a circuit breaker.
#[derive(Debug, PartialEq, Eq)]
pub enum CircuitBreakerState {
    /// Closed: requests are allowed
    Closed,
    /// Open: requests are blocked
    Open,
    /// Half-open: limited requests are allowed to test recovery
    HalfOpen,
}

/// Circuit breaker
///
/// Implements a circuit breaker pattern to prevent cascading failures.
pub struct CircuitBreaker {
    state: CircuitBreakerState,
    failure_count: u32,
    failure_threshold: u32,
    reset_timeout: Duration,
    last_failure: Option<std::time::Instant>,
}

impl CircuitBreaker {
    /// Create a new `CircuitBreaker`
    #[must_use]
    pub const fn new(failure_threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            failure_threshold,
            reset_timeout,
            last_failure: None,
        }
    }

    /// Check if a request is allowed
    #[allow(clippy::match_same_arms)]
    /// 检查是否允许请求通过
    ///
    /// # Side Effect
    ///
    /// 当断路器处于 `Open` 状态且重置超时已过时，
    /// 此方法会将状态转换为 `HalfOpen`（命令-查询分离例外）。
    pub fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Check if the reset timeout has elapsed
                if let Some(last_failure) = self.last_failure {
                    if last_failure.elapsed() > self.reset_timeout {
                        // Transition to HalfOpen state
                        self.state = CircuitBreakerState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }

    /// Record a successful request
    pub fn record_success(&mut self) {
        if self.state == CircuitBreakerState::HalfOpen {
            self.state = CircuitBreakerState::Closed;
            self.failure_count = 0;
        }
    }

    /// Record a failed request
    #[allow(clippy::match_wildcard_for_single_variants)]
    pub fn record_failure(&mut self) {
        match self.state {
            CircuitBreakerState::Closed => {
                self.failure_count = self.failure_count.saturating_add(1);
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitBreakerState::Open;
                    self.last_failure = Some(std::time::Instant::now());
                }
            }
            CircuitBreakerState::HalfOpen => {
                self.state = CircuitBreakerState::Open;
                self.last_failure = Some(std::time::Instant::now());
            }
            _ => {}
        }
    }

    /// Get the current state
    #[must_use]
    pub const fn state(&self) -> &CircuitBreakerState {
        &self.state
    }

    /// Set the state directly (test-only)
    ///
    /// 此方法仅用于测试目的，允许直接设置断路器状态，
    /// 避免在测试代码中使用 `unsafe` 指针操作。
    ///
    /// # Safety (design)
    ///
    /// 此方法绕过正常的状态转换逻辑，仅应在测试中使用。
    /// 生产代码应通过 `record_failure()`/`record_success()`/`allow_request()`
    /// 触发状态转换。
    #[cfg(any(test, feature = "testing"))]
    pub const fn set_state(&mut self, state: CircuitBreakerState) {
        self.state = state;
    }

    /// Set the `last_failure` timestamp directly (test-only)
    ///
    /// 此方法仅用于测试目的，允许直接设置最后失败时间戳。
    #[cfg(any(test, feature = "testing"))]
    pub const fn set_last_failure(&mut self, last_failure: Option<std::time::Instant>) {
        self.last_failure = last_failure;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_state_machine() {
        let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);
        let mut machine = RecoveryStateMachine::new(3, retry_config);

        assert_eq!(*machine.state(), RecoveryState::Initial);

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

    #[test]
    fn test_exponential_backoff() {
        let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, false);
        let mut backoff = ExponentialBackoff::new(retry_config);

        let delay1 = backoff.next_delay().unwrap();
        assert_eq!(delay1, Duration::from_secs(1));

        let delay2 = backoff.next_delay().unwrap();
        assert_eq!(delay2, Duration::from_secs(2));

        let delay3 = backoff.next_delay().unwrap();
        assert_eq!(delay3, Duration::from_secs(4));

        let delay4 = backoff.next_delay();
        assert!(delay4.is_none());
    }

    #[test]
    fn test_recovery_state_machine_full() {
        let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);
        let mut machine = RecoveryStateMachine::new(3, retry_config);

        // Test state transitions
        assert_eq!(*machine.state(), RecoveryState::Initial);
        assert_eq!(machine.retry_attempts(), 0);

        machine.start_recovery();
        assert_eq!(*machine.state(), RecoveryState::Recovering);

        machine.recover_success();
        assert_eq!(*machine.state(), RecoveryState::Recovered);
        assert_eq!(machine.retry_attempts(), 0);

        machine.start_recovery();
        machine.recover_failed();
        assert_eq!(machine.retry_attempts(), 1);

        // Test calculate_retry_delay
        let delay = machine.calculate_retry_delay();
        assert!(delay >= Duration::from_secs(1));

        // Test open_circuit
        machine.open_circuit();
        assert_eq!(*machine.state(), RecoveryState::CircuitOpen);

        // Test activate_degradation
        machine.activate_degradation();
        assert_eq!(*machine.state(), RecoveryState::Degraded);
    }

    #[test]
    fn test_exponential_backoff_with_jitter() {
        let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);
        let mut backoff = ExponentialBackoff::new(retry_config);

        // Test next_delay with jitter
        let delay1 = backoff.next_delay().unwrap();
        assert!(delay1 >= Duration::from_millis(900) && delay1 <= Duration::from_millis(1100));

        let delay2 = backoff.next_delay().unwrap();
        assert!(delay2 >= Duration::from_millis(1800) && delay2 <= Duration::from_millis(2200));

        // Test reset
        backoff.reset();
        assert_eq!(backoff.current_attempt(), 0);

        let delay_reset = backoff.next_delay().unwrap();
        assert!(
            delay_reset >= Duration::from_millis(900) && delay_reset <= Duration::from_millis(1100)
        );
    }

    #[test]
    fn test_circuit_breaker() {
        let reset_timeout = Duration::from_millis(100);
        let mut breaker = CircuitBreaker::new(2, reset_timeout);

        // Test initial state
        assert_eq!(*breaker.state(), CircuitBreakerState::Closed);
        assert!(breaker.allow_request());

        // Test record_failure
        breaker.record_failure();
        assert_eq!(*breaker.state(), CircuitBreakerState::Closed);

        breaker.record_failure();
        assert_eq!(*breaker.state(), CircuitBreakerState::Open);

        // Test allow_request in Open state
        assert!(!breaker.allow_request());

        // Test record_success in Open state (should not change state)
        breaker.record_success();
        assert_eq!(*breaker.state(), CircuitBreakerState::Open);

        // Test allow_request after reset timeout
        std::thread::sleep(reset_timeout * 2);
        assert!(breaker.allow_request());

        // Test record_success in HalfOpen state
        breaker.record_success();
        assert_eq!(*breaker.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_circuit_breaker_half_open() {
        let reset_timeout = Duration::from_millis(100);
        let mut breaker = CircuitBreaker::new(1, reset_timeout);

        // Open the circuit
        breaker.record_failure();
        assert_eq!(*breaker.state(), CircuitBreakerState::Open);

        // Wait for reset timeout
        std::thread::sleep(reset_timeout * 2);

        // Should be in HalfOpen state now
        assert!(breaker.allow_request());

        // Test record_failure in HalfOpen state
        breaker.record_failure();
        assert_eq!(*breaker.state(), CircuitBreakerState::Open);
    }

    #[test]
    fn test_circuit_breaker_open_no_last_failure() {
        let reset_timeout = Duration::from_millis(100);
        let mut breaker = CircuitBreaker::new(1, reset_timeout);

        breaker.set_state(CircuitBreakerState::Open);
        breaker.set_last_failure(None);

        assert!(!breaker.allow_request());
    }

    #[test]
    fn test_circuit_breaker_open_record_failure() {
        // Test record_failure when in Open state
        let reset_timeout = Duration::from_millis(100);
        let mut breaker = CircuitBreaker::new(1, reset_timeout);

        // Open the circuit
        breaker.record_failure();
        assert_eq!(*breaker.state(), CircuitBreakerState::Open);

        // Test record_failure in Open state (should do nothing)
        breaker.record_failure();
        assert_eq!(*breaker.state(), CircuitBreakerState::Open);
    }

    #[test]
    fn test_circuit_breaker_half_open_allow_request() {
        let reset_timeout = Duration::from_millis(100);
        let mut breaker = CircuitBreaker::new(1, reset_timeout);

        breaker.set_state(CircuitBreakerState::HalfOpen);

        assert!(breaker.allow_request());
    }
}
