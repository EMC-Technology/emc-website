//! Propagation fuzz tests
//! 
//! This module contains fuzz tests for the propagation module to ensure coverage of all possible cases.

use error_core::propagation::{ContextFrame, RecoveryHint, RetryConfig};
use std::collections::HashMap;
use serde_json::json;

#[test]
fn test_context_frame() {
    // Test ContextFrame creation with various sources and data
    let long_source = "a".repeat(1000);
    let test_cases = vec![
        ("empty_source", "", HashMap::new()),
        ("simple_source", "api_gateway", HashMap::new()),
        ("complex_source", "ai_model::lm_manager::generate", HashMap::new()),
        ("source_with_special_chars", "api-gateway_123", HashMap::new()),
        ("long_source", &long_source, HashMap::new()),
    ];
    
    for (_name, source, mut data) in test_cases {
        // Test with different data types
        data.insert("string_value".to_string(), json!("test"));
        data.insert("number_value".to_string(), json!(42));
        data.insert("bool_value".to_string(), json!(true));
        data.insert("array_value".to_string(), json!([1, 2, 3]));
        data.insert("object_value".to_string(), json!({"key": "value"}));
        data.insert("null_value".to_string(), json!(null));
        
        let frame = ContextFrame::new(source, data.clone());
        assert_eq!(frame.source(), source);
        assert_eq!(frame.data(), &data);
    }
}

#[test]
fn test_context_frame_edge_cases() {
    // Test edge cases for ContextFrame
    
    // Test with empty data
    let frame_empty_data = ContextFrame::new("test_source", HashMap::new());
    assert_eq!(frame_empty_data.source(), "test_source");
    assert!(frame_empty_data.data().is_empty());
    
    // Test with large data
    let mut large_data = HashMap::new();
    for i in 0..100 {
        large_data.insert(format!("key_{}", i), json!(i));
    }
    let frame_large_data = ContextFrame::new("test_source", large_data.clone());
    assert_eq!(frame_large_data.source(), "test_source");
    assert_eq!(frame_large_data.data(), &large_data);
    
    // Test with nested data
    let mut nested_data = HashMap::new();
    nested_data.insert("level1".to_string(), json!({
        "level2": {
            "level3": "deep_value"
        }
    }));
    let frame_nested_data = ContextFrame::new("test_source", nested_data.clone());
    assert_eq!(frame_nested_data.source(), "test_source");
    assert_eq!(frame_nested_data.data(), &nested_data);
}

#[test]
fn test_recovery_hint() {
    // Test RecoveryHint creation with various actions, descriptions, and params
    let long_action = "a".repeat(1000);
    let test_cases = vec![
        ("empty_action", "", "Empty action", HashMap::new()),
        ("simple_action", "retry", "Retry the operation", HashMap::new()),
        ("complex_action", "retry_with_backoff", "Retry with exponential backoff", HashMap::new()),
        ("action_with_special_chars", "retry_123-abc", "Retry with special chars", HashMap::new()),
        ("long_action", &long_action, "Long action", HashMap::new()),
    ];
    
    for (_name, action, description, mut params) in test_cases {
        // Test with different params types
        params.insert("retry_delay_ms".to_string(), json!(1000));
        params.insert("max_attempts".to_string(), json!(3));
        params.insert("backoff_multiplier".to_string(), json!(2.0));
        params.insert("use_jitter".to_string(), json!(true));
        
        let hint = RecoveryHint::new(action, description, params.clone());
        assert_eq!(hint.action(), action);
        assert_eq!(hint.description(), description);
        assert_eq!(hint.params(), &params);
    }
}

#[test]
fn test_recovery_hint_edge_cases() {
    // Test edge cases for RecoveryHint
    
    // Test with empty params
    let hint_empty_params = RecoveryHint::new("retry", "Retry operation", HashMap::new());
    assert_eq!(hint_empty_params.action(), "retry");
    assert_eq!(hint_empty_params.description(), "Retry operation");
    assert!(hint_empty_params.params().is_empty());
    
    // Test with empty description
    let hint_empty_description = RecoveryHint::new("retry", "", HashMap::new());
    assert_eq!(hint_empty_description.action(), "retry");
    assert_eq!(hint_empty_description.description(), "");
    
    // Test with large params
    let mut large_params = HashMap::new();
    for i in 0..100 {
        large_params.insert(format!("param_{}", i), json!(i));
    }
    let hint_large_params = RecoveryHint::new("retry", "Retry operation", large_params.clone());
    assert_eq!(hint_large_params.action(), "retry");
    assert_eq!(hint_large_params.params(), &large_params);
    
    // Test with nested params
    let mut nested_params = HashMap::new();
    nested_params.insert("retry_config".to_string(), json!({
        "initial_delay": 1000,
        "max_attempts": 3,
        "backoff": 2.0
    }));
    let hint_nested_params = RecoveryHint::new("retry", "Retry operation", nested_params.clone());
    assert_eq!(hint_nested_params.action(), "retry");
    assert_eq!(hint_nested_params.params(), &nested_params);
}

#[test]
fn test_retry_config() {
    // Test RetryConfig creation with various configurations
    let test_cases = vec![
        ("minimal_config", 1, 1, 1, 1.0, false),
        ("standard_config", 3, 1000, 10000, 2.0, true),
        ("max_attempts", 100, 1000, 10000, 2.0, true),
        ("short_delays", 3, 1, 100, 1.5, false),
        ("long_delays", 3, 1000, 60000, 2.0, true),
        ("no_backoff", 3, 1000, 1000, 1.0, false),
        ("high_backoff", 3, 1000, 10000, 10.0, true),
        ("no_jitter", 3, 1000, 10000, 2.0, false),
    ];
    
    for (_name, max_attempts, initial_delay_ms, max_delay_ms, backoff_multiplier, use_jitter) in test_cases {
        let config = RetryConfig::new(max_attempts, initial_delay_ms, max_delay_ms, backoff_multiplier, use_jitter);
        assert_eq!(config.max_attempts(), max_attempts);
        assert_eq!(config.initial_delay_ms(), initial_delay_ms);
        assert_eq!(config.max_delay_ms(), max_delay_ms);
        assert_eq!(config.backoff_multiplier(), backoff_multiplier);
        assert_eq!(config.use_jitter(), use_jitter);
    }
}

#[test]
fn test_retry_config_edge_cases() {
    // Test edge cases for RetryConfig
    
    // Test with zero values
    let config_zero = RetryConfig::new(0, 0, 0, 0.0, false);
    assert_eq!(config_zero.max_attempts(), 0);
    assert_eq!(config_zero.initial_delay_ms(), 0);
    assert_eq!(config_zero.max_delay_ms(), 0);
    assert_eq!(config_zero.backoff_multiplier(), 0.0);
    assert_eq!(config_zero.use_jitter(), false);
    
    // Test with very large values
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
    assert_eq!(config_large.use_jitter(), true);
    
    // Test with initial delay greater than max delay
    let config_invalid_delays = RetryConfig::new(3, 10000, 1000, 2.0, true);
    assert_eq!(config_invalid_delays.initial_delay_ms(), 10000);
    assert_eq!(config_invalid_delays.max_delay_ms(), 1000);
}

#[test]
fn test_propagation_all_combinations() {
    // Test all combinations of propagation structures
    
    // Test ContextFrame with different sources and data
    let sources = ["", "api_gateway", "ai_model", "database"];
    let data_configs = [
        HashMap::new(),
        { let mut m = HashMap::new(); m.insert("key".to_string(), json!("value")); m },
        { let mut m = HashMap::new(); m.insert("number".to_string(), json!(42)); m },
    ];
    
    for source in sources {
        for data in &data_configs {
            let frame = ContextFrame::new(source, data.clone());
            assert_eq!(frame.source(), source);
            assert_eq!(frame.data(), data);
        }
    }
    
    // Test RecoveryHint with different actions and params
    let actions = ["", "retry", "retry_with_backoff", "fallback"];
    let descriptions = ["", "Retry operation", "Retry with backoff", "Use fallback service"];
    let params_configs = [
        HashMap::new(),
        { let mut m = HashMap::new(); m.insert("delay".to_string(), json!(1000)); m },
    ];
    
    for action in actions {
        for &description in &descriptions {
            for params in &params_configs {
                let hint = RecoveryHint::new(action, description, params.clone());
                assert_eq!(hint.action(), action);
                assert_eq!(hint.description(), description);
                assert_eq!(hint.params(), params);
            }
        }
    }
    
    // Test RetryConfig with different configurations
    let max_attempts_values = [1, 3, 5];
    let initial_delay_values = [100, 1000, 5000];
    let use_jitter_values = [true, false];
    
    for max_attempts in max_attempts_values {
        for initial_delay in initial_delay_values {
            for use_jitter in use_jitter_values {
                let config = RetryConfig::new(
                    max_attempts,
                    initial_delay,
                    initial_delay * 10,
                    2.0,
                    use_jitter
                );
                assert_eq!(config.max_attempts(), max_attempts);
                assert_eq!(config.initial_delay_ms(), initial_delay);
                assert_eq!(config.use_jitter(), use_jitter);
            }
        }
    }
}

#[test]
fn test_propagation_equality() {
    // Test equality for all propagation structures
    
    // Test ContextFrame equality
    let mut data1 = HashMap::new();
    data1.insert("key".to_string(), json!("value"));
    let frame1 = ContextFrame::new("source", data1.clone());
    let frame2 = ContextFrame::new("source", data1);
    assert_eq!(frame1, frame2);
    
    let mut data2 = HashMap::new();
    data2.insert("key".to_string(), json!("different"));
    let frame3 = ContextFrame::new("source", data2);
    assert_ne!(frame1, frame3);
    
    let frame4 = ContextFrame::new("different_source", HashMap::new());
    assert_ne!(frame1, frame4);
    
    // Test RecoveryHint equality
    let mut params1 = HashMap::new();
    params1.insert("delay".to_string(), json!(1000));
    let hint1 = RecoveryHint::new("retry", "Retry operation", params1.clone());
    let hint2 = RecoveryHint::new("retry", "Retry operation", params1);
    assert_eq!(hint1, hint2);
    
    let mut params2 = HashMap::new();
    params2.insert("delay".to_string(), json!(2000));
    let hint3 = RecoveryHint::new("retry", "Retry operation", params2);
    assert_ne!(hint1, hint3);
    
    let hint4 = RecoveryHint::new("different", "Retry operation", HashMap::new());
    assert_ne!(hint1, hint4);
    
    let hint5 = RecoveryHint::new("retry", "Different description", HashMap::new());
    assert_ne!(hint1, hint5);
    
    // Test RetryConfig equality
    let config1 = RetryConfig::new(3, 1000, 10000, 2.0, true);
    let config2 = RetryConfig::new(3, 1000, 10000, 2.0, true);
    assert_eq!(config1, config2);
    
    let config3 = RetryConfig::new(5, 1000, 10000, 2.0, true);
    assert_ne!(config1, config3);
    
    let config4 = RetryConfig::new(3, 2000, 10000, 2.0, true);
    assert_ne!(config1, config4);
    
    let config5 = RetryConfig::new(3, 1000, 20000, 2.0, true);
    assert_ne!(config1, config5);
    
    let config6 = RetryConfig::new(3, 1000, 10000, 1.5, true);
    assert_ne!(config1, config6);
    
    let config7 = RetryConfig::new(3, 1000, 10000, 2.0, false);
    assert_ne!(config1, config7);
}
