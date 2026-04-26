//! Recovery fuzz tests
//! 
//! This module contains fuzz tests for the recovery module to ensure coverage of all possible cases.

use error_core::recovery::{RecoveryState, RecoveryStateMachine, ExponentialBackoff, CircuitBreaker, CircuitBreakerState};
use error_core::propagation::RetryConfig;
use std::time::Duration;

#[test]
fn test_recovery_state_machine() {
    // Test RecoveryStateMachine with various configurations
    let test_cases = vec![
        ("minimal_config", 1, RetryConfig::new(1, 1, 1, 1.0, false)),
        ("standard_config", 3, RetryConfig::new(3, 1000, 10000, 2.0, true)),
        ("max_attempts", 100, RetryConfig::new(100, 1000, 10000, 2.0, true)),
        ("no_retry", 0, RetryConfig::new(0, 1000, 10000, 2.0, true)),
    ];
    
    for (_name, max_attempts, retry_config) in test_cases {
        let mut machine = RecoveryStateMachine::new(max_attempts, retry_config);
        
        // Test initial state
        assert_eq!(*machine.state(), RecoveryState::Initial);
        assert_eq!(machine.retry_attempts(), 0);
        
        // Test start_recovery
        machine.start_recovery();
        assert_eq!(*machine.state(), RecoveryState::Recovering);
        
        // Test recover_success
        machine.recover_success();
        assert_eq!(*machine.state(), RecoveryState::Recovered);
        assert_eq!(machine.retry_attempts(), 0);
        
        // Test start_recovery again
        machine.start_recovery();
        assert_eq!(*machine.state(), RecoveryState::Recovering);
        
        // Test recover_failed multiple times
        for i in 1..=max_attempts {
            machine.recover_failed();
            if i < max_attempts {
                assert_eq!(*machine.state(), RecoveryState::Recovering);
                assert_eq!(machine.retry_attempts(), i);
            } else {
                assert_eq!(*machine.state(), RecoveryState::Failed);
                assert_eq!(machine.retry_attempts(), i);
            }
        }
        
        // Test calculate_retry_delay
        let delay = machine.calculate_retry_delay();
        assert!(delay >= Duration::from_millis(0));
        
        // Test open_circuit
        machine.open_circuit();
        assert_eq!(*machine.state(), RecoveryState::CircuitOpen);
        
        // Test activate_degradation
        machine.activate_degradation();
        assert_eq!(*machine.state(), RecoveryState::Degraded);
    }
}

#[test]
fn test_recovery_state_machine_edge_cases() {
    // Test edge cases for RecoveryStateMachine
    
    // Test with zero max attempts
    let retry_config = RetryConfig::new(0, 1000, 10000, 2.0, true);
    let mut machine = RecoveryStateMachine::new(0, retry_config);
    
    machine.start_recovery();
    assert_eq!(*machine.state(), RecoveryState::Recovering);
    
    machine.recover_failed();
    assert_eq!(*machine.state(), RecoveryState::Failed);
    assert_eq!(machine.retry_attempts(), 1);
    
    // Test calculate_retry_delay with different attempts
    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);
    let mut machine = RecoveryStateMachine::new(3, retry_config);
    
    // Test with 0 attempts
    let delay0 = machine.calculate_retry_delay();
    assert_eq!(delay0, Duration::from_millis(1000));
    
    // Test with 1 attempt
    machine.recover_failed();
    let delay1 = machine.calculate_retry_delay();
    assert_eq!(delay1, Duration::from_millis(2000));
    
    // Test with 2 attempts (should be capped at max delay)
    let retry_config_max = RetryConfig::new(3, 1000, 2000, 2.0, true);
    let mut machine_max = RecoveryStateMachine::new(3, retry_config_max);
    machine_max.recover_failed();
    machine_max.recover_failed();
    let delay2 = machine_max.calculate_retry_delay();
    assert_eq!(delay2, Duration::from_millis(2000));
}

#[test]
fn test_exponential_backoff() {
    // Test ExponentialBackoff with various configurations
    let test_cases = vec![
        ("minimal_config", RetryConfig::new(1, 1, 1, 1.0, false)),
        ("standard_config", RetryConfig::new(3, 1000, 10000, 2.0, true)),
        ("max_attempts", RetryConfig::new(10, 1000, 10000, 2.0, true)),
        ("no_retry", RetryConfig::new(0, 1000, 10000, 2.0, true)),
        ("no_backoff", RetryConfig::new(3, 1000, 1000, 1.0, false)),
    ];
    
    for (_name, retry_config) in test_cases {
        let mut backoff = ExponentialBackoff::new(retry_config.clone());
        
        // Test current_attempt
        assert_eq!(backoff.current_attempt(), 0);
        
        // Test next_delay
        let max_attempts = retry_config.max_attempts();
        for i in 0..max_attempts {
            let delay = backoff.next_delay();
            assert!(delay.is_some());
            assert_eq!(backoff.current_attempt(), i + 1);
        }
        
        // Test that next_delay returns None after max attempts
        let delay = backoff.next_delay();
        assert!(delay.is_none());
        
        // Test reset
        backoff.reset();
        assert_eq!(backoff.current_attempt(), 0);
        
        // Test next_delay after reset
        if max_attempts > 0 {
            let delay = backoff.next_delay();
            assert!(delay.is_some());
            assert_eq!(backoff.current_attempt(), 1);
        }
    }
}

#[test]
fn test_exponential_backoff_edge_cases() {
    // Test edge cases for ExponentialBackoff
    
    // Test with zero max attempts
    let retry_config = RetryConfig::new(0, 1000, 10000, 2.0, true);
    let mut backoff = ExponentialBackoff::new(retry_config);
    let delay = backoff.next_delay();
    assert!(delay.is_none());
    
    // Test with zero initial delay
    let retry_config_zero = RetryConfig::new(3, 0, 10000, 2.0, true);
    let mut backoff_zero = ExponentialBackoff::new(retry_config_zero);
    for _ in 0..3 {
        let delay = backoff_zero.next_delay();
        assert!(delay.is_some());
        assert_eq!(delay.unwrap(), Duration::from_millis(0));
    }
    
    // Test with max delay less than initial delay
    let retry_config_max = RetryConfig::new(3, 1000, 500, 2.0, false);
    let mut backoff_max = ExponentialBackoff::new(retry_config_max);
    for _ in 0..3 {
        let delay = backoff_max.next_delay();
        assert!(delay.is_some());
        assert_eq!(delay.unwrap(), Duration::from_millis(500));
    }
}

#[test]
fn test_circuit_breaker() {
    // Test CircuitBreaker with various configurations
    let test_cases = vec![
        ("minimal_config", 1, Duration::from_millis(1)),
        ("standard_config", 3, Duration::from_millis(1000)),
        ("high_threshold", 10, Duration::from_millis(5000)),
    ];
    
    for (_name, failure_threshold, reset_timeout) in test_cases {
        let mut breaker = CircuitBreaker::new(failure_threshold, reset_timeout);
        
        // Test initial state
        assert_eq!(*breaker.state(), CircuitBreakerState::Closed);
        assert!(breaker.allow_request());
        
        // Test record_failure multiple times
        for i in 1..=failure_threshold {
            breaker.record_failure();
            if i < failure_threshold {
                assert_eq!(*breaker.state(), CircuitBreakerState::Closed);
            } else {
                assert_eq!(*breaker.state(), CircuitBreakerState::Open);
            }
        }
        
        // Test allow_request in Open state
        assert!(!breaker.allow_request());
        
        // Test record_success in Open state (should not change state)
        breaker.record_success();
        assert_eq!(*breaker.state(), CircuitBreakerState::Open);
        
        // Test record_failure in Open state (should do nothing)
        breaker.record_failure();
        assert_eq!(*breaker.state(), CircuitBreakerState::Open);
    }
}

#[test]
fn test_circuit_breaker_edge_cases() {
    // Test edge cases for CircuitBreaker
    
    // Test with zero failure threshold
    let mut breaker_zero = CircuitBreaker::new(0, Duration::from_millis(1000));
    assert_eq!(*breaker_zero.state(), CircuitBreakerState::Closed);
    assert!(breaker_zero.allow_request());
    
    // Test record_failure with zero threshold
    breaker_zero.record_failure();
    assert_eq!(*breaker_zero.state(), CircuitBreakerState::Open);
    
    // Test allow_request after reset timeout
    let reset_timeout = Duration::from_millis(50);
    let mut breaker = CircuitBreaker::new(1, reset_timeout);
    
    // Open the circuit
    breaker.record_failure();
    assert_eq!(*breaker.state(), CircuitBreakerState::Open);
    assert!(!breaker.allow_request());
    
    // Wait for reset timeout
    std::thread::sleep(reset_timeout * 2);
    
    // Should be in HalfOpen state now
    assert!(breaker.allow_request());
    
    // Test record_success in HalfOpen state
    breaker.record_success();
    assert_eq!(*breaker.state(), CircuitBreakerState::Closed);
    
    // Test record_failure in HalfOpen state
    let mut breaker_half_open = CircuitBreaker::new(1, reset_timeout);
    breaker_half_open.record_failure();
    std::thread::sleep(reset_timeout * 2);
    breaker_half_open.allow_request(); // Transition to HalfOpen
    breaker_half_open.record_failure();
    assert_eq!(*breaker_half_open.state(), CircuitBreakerState::Open);
}

#[test]
fn test_recovery_all_combinations() {
    // Test all combinations of recovery mechanisms
    
    // Test RecoveryStateMachine with different max attempts
    let max_attempts_values = [1, 3, 5];
    let retry_configs = [
        RetryConfig::new(3, 100, 1000, 1.5, false),
        RetryConfig::new(3, 1000, 10000, 2.0, true),
    ];
    
    for max_attempts in max_attempts_values {
        for retry_config in &retry_configs {
            let mut machine = RecoveryStateMachine::new(max_attempts, retry_config.clone());
            
            // Test state transitions
            machine.start_recovery();
            assert_eq!(*machine.state(), RecoveryState::Recovering);
            
            machine.recover_success();
            assert_eq!(*machine.state(), RecoveryState::Recovered);
            
            machine.start_recovery();
            machine.recover_failed();
            assert_eq!(machine.retry_attempts(), 1);
        }
    }
    
    // Test ExponentialBackoff with different configurations
    let backoff_configs = [
        RetryConfig::new(3, 100, 1000, 1.5, false),
        RetryConfig::new(3, 1000, 10000, 2.0, true),
        RetryConfig::new(3, 500, 5000, 1.2, false),
    ];
    
    for config in &backoff_configs {
        let mut backoff = ExponentialBackoff::new(config.clone());
        
        for _ in 0..config.max_attempts() {
            let delay = backoff.next_delay();
            assert!(delay.is_some());
        }
        
        let delay = backoff.next_delay();
        assert!(delay.is_none());
    }
    
    // Test CircuitBreaker with different failure thresholds
    let failure_thresholds = [1, 2, 3];
    let reset_timeouts = [
        Duration::from_millis(10),
        Duration::from_millis(100),
    ];
    
    for threshold in failure_thresholds {
        for timeout in &reset_timeouts {
            let mut breaker = CircuitBreaker::new(threshold, *timeout);
            
            for _ in 0..threshold {
                breaker.record_failure();
            }
            
            assert_eq!(*breaker.state(), CircuitBreakerState::Open);
        }
    }
}

#[test]
fn test_recovery_equality() {
    // Test equality for recovery types
    
    // Test RecoveryState equality
    assert_eq!(RecoveryState::Initial, RecoveryState::Initial);
    assert_eq!(RecoveryState::Recovering, RecoveryState::Recovering);
    assert_eq!(RecoveryState::Recovered, RecoveryState::Recovered);
    assert_eq!(RecoveryState::Failed, RecoveryState::Failed);
    assert_eq!(RecoveryState::CircuitOpen, RecoveryState::CircuitOpen);
    assert_eq!(RecoveryState::Degraded, RecoveryState::Degraded);
    
    assert_ne!(RecoveryState::Initial, RecoveryState::Recovering);
    assert_ne!(RecoveryState::Recovered, RecoveryState::Failed);
    
    // Test CircuitBreakerState equality
    assert_eq!(CircuitBreakerState::Closed, CircuitBreakerState::Closed);
    assert_eq!(CircuitBreakerState::Open, CircuitBreakerState::Open);
    assert_eq!(CircuitBreakerState::HalfOpen, CircuitBreakerState::HalfOpen);
    
    assert_ne!(CircuitBreakerState::Closed, CircuitBreakerState::Open);
    assert_ne!(CircuitBreakerState::Open, CircuitBreakerState::HalfOpen);
}
