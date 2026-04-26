//! Propagation module formal verification tests using Kani

use error_core::propagation::*;
use std::collections::HashMap;
use serde_json::json;

// Test ContextFrame
#[kani::proof]
fn test_context_frame() {
    let mut data = HashMap::new();
    data.insert("user_id".to_string(), json!("12345"));
    data.insert("operation".to_string(), json!("generate_code"));
    
    let frame = ContextFrame::new("ai_model::lm_manager", data);
    assert_eq!(frame.source(), "ai_model::lm_manager");
    assert_eq!(frame.data().get("user_id").unwrap(), &json!("12345"));
    assert_eq!(frame.data().get("operation").unwrap(), &json!("generate_code"));
}

// Test ContextFrame with empty data
#[kani::proof]
fn test_context_frame_empty_data() {
    let data = HashMap::new();
    let frame = ContextFrame::new("test_module", data);
    assert_eq!(frame.source(), "test_module");
    assert!(frame.data().is_empty());
}

// Test RecoveryHint
#[kani::proof]
fn test_recovery_hint() {
    let mut params = HashMap::new();
    params.insert("retry_delay_ms".to_string(), json!(1000));
    params.insert("max_attempts".to_string(), json!(3));
    
    let hint = RecoveryHint::new(
        "retry",
        "Retry the AI model call with exponential backoff",
        params,
    );
    assert_eq!(hint.action(), "retry");
    assert_eq!(hint.description(), "Retry the AI model call with exponential backoff");
    assert_eq!(hint.params().get("retry_delay_ms").unwrap(), &json!(1000));
    assert_eq!(hint.params().get("max_attempts").unwrap(), &json!(3));
}

// Test RecoveryHint with empty params
#[kani::proof]
fn test_recovery_hint_empty_params() {
    let params = HashMap::new();
    let hint = RecoveryHint::new("retry", "Retry the operation", params);
    assert_eq!(hint.action(), "retry");
    assert_eq!(hint.description(), "Retry the operation");
    assert!(hint.params().is_empty());
}

// Test RetryConfig
#[kani::proof]
fn test_retry_config() {
    let config = RetryConfig::new(3, 1000, 10000, 2.0, true);
    assert_eq!(config.max_attempts(), 3);
    assert_eq!(config.initial_delay_ms(), 1000);
    assert_eq!(config.max_delay_ms(), 10000);
    assert_eq!(config.backoff_multiplier(), 2.0);
    assert_eq!(config.use_jitter(), true);
}

// Test RetryConfig with different values
#[kani::proof]
fn test_retry_config_different_values() {
    let config = RetryConfig::new(5, 500, 5000, 1.5, false);
    assert_eq!(config.max_attempts(), 5);
    assert_eq!(config.initial_delay_ms(), 500);
    assert_eq!(config.max_delay_ms(), 5000);
    assert_eq!(config.backoff_multiplier(), 1.5);
    assert_eq!(config.use_jitter(), false);
}

// Test RetryConfig with boundary values
#[kani::proof]
fn test_retry_config_boundary_values() {
    // Test minimum values
    let config_min = RetryConfig::new(1, 0, 0, 1.0, false);
    assert_eq!(config_min.max_attempts(), 1);
    assert_eq!(config_min.initial_delay_ms(), 0);
    assert_eq!(config_min.max_delay_ms(), 0);
    assert_eq!(config_min.backoff_multiplier(), 1.0);
    assert_eq!(config_min.use_jitter(), false);
    
    // Test large values
    let config_max = RetryConfig::new(100, 10000, 60000, 10.0, true);
    assert_eq!(config_max.max_attempts(), 100);
    assert_eq!(config_max.initial_delay_ms(), 10000);
    assert_eq!(config_max.max_delay_ms(), 60000);
    assert_eq!(config_max.backoff_multiplier(), 10.0);
    assert_eq!(config_max.use_jitter(), true);
}
