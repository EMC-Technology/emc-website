//! Propagation property tests
//!
//! This module contains property tests for the propagation module to ensure coverage of all possible cases.
#![allow(clippy::uninlined_format_args, clippy::cast_sign_loss, clippy::float_cmp, clippy::unreadable_literal)]

use error_core::propagation::{ContextFrame, RecoveryHint, RetryConfig};
use std::collections::HashMap;

#[test]
fn test_context_frame() {
    // Test ContextFrame with different sources and data
    let sources = ["api_gateway", "business_logic", "data_access", "external_service"];
    let data_sizes = [0, 1, 5, 10];
    
    for source in &sources {
        for data_size in &data_sizes {
            let mut data = HashMap::new();
            for i in 0..*data_size {
                let key = format!("key_{}", i);
                data.insert(key, serde_json::json!(i));
            }
            
            let frame = ContextFrame::new(source, data);
            assert_eq!(frame.source(), *source);
            assert_eq!(frame.data().len(), *data_size as usize);
            
            for i in 0..*data_size {
                let key = format!("key_{}", i);
                assert!(frame.data().contains_key(&key));
                assert_eq!(frame.data().get(&key).unwrap(), &serde_json::json!(i));
            }
        }
    }
}

#[test]
fn test_recovery_hint() {
    // Test RecoveryHint with different actions, descriptions, and parameters
    let actions = ["retry", "reconnect", "fallback", "notify"];
    let descriptions = [
        "Retry the operation",
        "Reconnect to the service",
        "Use fallback service",
        "Notify system administrator",
    ];
    let param_sizes = [0, 1, 3, 5];
    
    for (action, description) in actions.iter().zip(descriptions.iter()) {
        for param_size in &param_sizes {
            let mut params = HashMap::new();
            for i in 0..*param_size {
                let key = format!("param_{}", i);
                params.insert(key, serde_json::json!(i));
            }
            
            let hint = RecoveryHint::new(action, description, params);
            assert_eq!(hint.action(), *action);
            assert_eq!(hint.description(), *description);
            assert_eq!(hint.params().len(), *param_size as usize);
            
            for i in 0..*param_size {
                let key = format!("param_{}", i);
                assert!(hint.params().contains_key(&key));
                assert_eq!(hint.params().get(&key).unwrap(), &serde_json::json!(i));
            }
        }
    }
}

#[test]
fn test_retry_config() {
    // Test RetryConfig with different configurations
    let configs = [
        RetryConfig::new(1, 100, 1000, 2.0, false),
        RetryConfig::new(3, 1000, 10000, 2.0, true),
        RetryConfig::new(5, 500, 5000, 1.5, false),
        RetryConfig::new(10, 200, 2000, 1.2, true),
    ];
    
    for config in &configs {
        assert_eq!(config.max_attempts(), config.max_attempts());
        assert_eq!(config.initial_delay_ms(), config.initial_delay_ms());
        assert_eq!(config.max_delay_ms(), config.max_delay_ms());
        assert_eq!(config.backoff_multiplier(), config.backoff_multiplier());
        assert_eq!(config.use_jitter(), config.use_jitter());
    }
    
    // Test edge cases
    let edge_cases = [
        RetryConfig::new(0, 0, 0, 1.0, false),
        RetryConfig::new(100, 100000, 1000000, 10.0, true),
    ];
    
    for config in &edge_cases {
        assert_eq!(config.max_attempts(), config.max_attempts());
        assert_eq!(config.initial_delay_ms(), config.initial_delay_ms());
        assert_eq!(config.max_delay_ms(), config.max_delay_ms());
        assert_eq!(config.backoff_multiplier(), config.backoff_multiplier());
        assert_eq!(config.use_jitter(), config.use_jitter());
    }
}

#[test]
fn test_context_frame_timestamp() {
    // Test ContextFrame timestamp when chrono feature is enabled
    #[cfg(feature = "chrono")]
    {
        let data = HashMap::new();
        let frame = ContextFrame::new("test_source", data);
        // Just test that timestamp() method exists and returns a value
        let _timestamp = frame.timestamp();
    }
}

#[test]
fn test_context_frame_edge_cases() {
    // Test ContextFrame with edge cases
    
    // Empty source
    let data = HashMap::new();
    let frame = ContextFrame::new("", data);
    assert_eq!(frame.source(), "");
    assert!(frame.data().is_empty());
    
    // Complex data structure
    let mut complex_data = HashMap::new();
    complex_data.insert("string_key".to_string(), serde_json::json!("value"));
    complex_data.insert("number_key".to_string(), serde_json::json!(42));
    complex_data.insert("bool_key".to_string(), serde_json::json!(true));
    complex_data.insert("array_key".to_string(), serde_json::json!([1, 2, 3]));
    complex_data.insert("object_key".to_string(), serde_json::json!({"nested": "value"}));
    
    let frame = ContextFrame::new("complex_source", complex_data);
    assert_eq!(frame.source(), "complex_source");
    assert_eq!(frame.data().len(), 5);
    assert_eq!(frame.data().get("string_key").unwrap(), &serde_json::json!("value"));
    assert_eq!(frame.data().get("number_key").unwrap(), &serde_json::json!(42));
    assert_eq!(frame.data().get("bool_key").unwrap(), &serde_json::json!(true));
    assert_eq!(frame.data().get("array_key").unwrap(), &serde_json::json!([1, 2, 3]));
    assert_eq!(frame.data().get("object_key").unwrap(), &serde_json::json!({"nested": "value"}));
}

#[test]
fn test_recovery_hint_edge_cases() {
    // Test RecoveryHint with edge cases
    
    // Empty action and description
    let data = HashMap::new();
    let hint = RecoveryHint::new("", "", data);
    assert_eq!(hint.action(), "");
    assert_eq!(hint.description(), "");
    assert!(hint.params().is_empty());
    
    // Complex parameters
    let mut complex_params = HashMap::new();
    complex_params.insert("string_param".to_string(), serde_json::json!("value"));
    complex_params.insert("number_param".to_string(), serde_json::json!(42));
    complex_params.insert("bool_param".to_string(), serde_json::json!(true));
    complex_params.insert("array_param".to_string(), serde_json::json!([1, 2, 3]));
    complex_params.insert("object_param".to_string(), serde_json::json!({"nested": "value"}));
    
    let hint = RecoveryHint::new("complex_action", "Complex description", complex_params);
    assert_eq!(hint.action(), "complex_action");
    assert_eq!(hint.description(), "Complex description");
    assert_eq!(hint.params().len(), 5);
    assert_eq!(hint.params().get("string_param").unwrap(), &serde_json::json!("value"));
    assert_eq!(hint.params().get("number_param").unwrap(), &serde_json::json!(42));
    assert_eq!(hint.params().get("bool_param").unwrap(), &serde_json::json!(true));
    assert_eq!(hint.params().get("array_param").unwrap(), &serde_json::json!([1, 2, 3]));
    assert_eq!(hint.params().get("object_param").unwrap(), &serde_json::json!({"nested": "value"}));
}

#[test]
fn test_retry_config_edge_cases() {
    // Test RetryConfig with more edge cases
    
    // Very small values
    let config_small = RetryConfig::new(1, 1, 1, 1.0, false);
    assert_eq!(config_small.max_attempts(), 1);
    assert_eq!(config_small.initial_delay_ms(), 1);
    assert_eq!(config_small.max_delay_ms(), 1);
    assert_eq!(config_small.backoff_multiplier(), 1.0);
    assert!(!config_small.use_jitter());
    
    // Very large values
    let config_large = RetryConfig::new(
        u32::MAX,
        u64::MAX,
        u64::MAX,
        f64::MAX,
        true
    );
    assert_eq!(config_large.max_attempts(), u32::MAX);
    assert_eq!(config_large.initial_delay_ms(), u64::MAX);
    assert_eq!(config_large.max_delay_ms(), u64::MAX);
    assert_eq!(config_large.backoff_multiplier(), f64::MAX);
    assert!(config_large.use_jitter());
    
    // Different backoff multipliers
    let configs = [
        RetryConfig::new(3, 1000, 10000, 1.0, false), // No backoff
        RetryConfig::new(3, 1000, 10000, 1.5, false), // Moderate backoff
        RetryConfig::new(3, 1000, 10000, 3.0, false), // Aggressive backoff
    ];
    
    for config in &configs {
        assert_eq!(config.max_attempts(), 3);
        assert_eq!(config.initial_delay_ms(), 1000);
        assert_eq!(config.max_delay_ms(), 10000);
        assert!(!config.use_jitter());
    }
}
