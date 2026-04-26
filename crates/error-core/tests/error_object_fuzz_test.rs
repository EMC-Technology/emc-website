//! Error object fuzz tests
//! 
//! This module contains fuzz tests for the error_object module to ensure coverage of all possible cases.

use error_core::error_object::{ErrorObject, ErrorObjectBuilder};
use error_core::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
use error_core::propagation::{ContextFrame, RecoveryHint, RetryConfig};
use std::collections::HashMap;

#[test]
fn test_error_object_valid_cases() {
    // Test valid error object cases
    let valid_cases = vec![
        (
            "ERR-USR-UI-001_ERR_O",
            ErrorSource::USR,
            Severity::ERROR,
            ImpactScope::OPERATION,
            Recoverability::SemiAuto,
            "User input error",
            "Please check your input",
            "ui::form",
            "validate_input"
        ),
        (
            "ERR-AIM-LM-002_ERR_S",
            ErrorSource::AIM,
            Severity::ERROR,
            ImpactScope::SESSION,
            Recoverability::AutoRecoverable,
            "AI model timeout",
            "AI service is temporarily unavailable",
            "ai::model",
            "generate"
        ),
        (
            "ERR-FS-IO-003_WRN_M",
            ErrorSource::FS,
            Severity::WARNING,
            ImpactScope::MODULE,
            Recoverability::ManualIntervention,
            "File system warning",
            "File operation completed with warnings",
            "fs::file",
            "write"
        ),
        (
            "ERR-NET-API-004_CRI_G",
            ErrorSource::NET,
            Severity::CRITICAL,
            ImpactScope::GLOBAL,
            Recoverability::NonRecoverable,
            "Network failure",
            "Network service is down",
            "net::client",
            "connect"
        ),
        (
            "ERR-CFG-CONF-005_INF_S",
            ErrorSource::CFG,
            Severity::INFO,
            ImpactScope::SESSION,
            Recoverability::AutoRecoverable,
            "Configuration info",
            "Using default configuration",
            "cfg::loader",
            "load"
        ),
    ];
    
    for (code, source, severity, impact_scope, recoverability, message, user_message, module_path, operation) in valid_cases {
        let result = ErrorObject::builder()
            .code(code)
            .source(source)
            .severity(severity)
            .impact_scope(impact_scope)
            .recoverability(recoverability)
            .message(message)
            .user_message(user_message)
            .module_path(module_path)
            .operation(operation)
            .build();
        let error = result.unwrap();
        
        assert_eq!(error.code(), code);
        assert_eq!(error.source(), source);
        assert_eq!(error.severity(), severity);
        assert_eq!(error.impact_scope(), impact_scope);
        assert_eq!(error.recoverability(), recoverability);
        assert_eq!(error.message(), message);
        assert_eq!(error.user_message(), user_message);
        assert_eq!(error.module_path(), module_path);
        assert_eq!(error.operation(), operation);
    }
}

#[test]
fn test_error_object_missing_required_fields() {
    // Test missing required fields
    
    // Test missing code
    let result = ErrorObjectBuilder::new()
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation")
        .build();
    assert!(result.is_err(), "Expected error for missing code");
    
    // Test missing source
    let result = ErrorObjectBuilder::new()
        .code("ERR-AIM-LM-001_ERR_S")
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation")
        .build();
    assert!(result.is_err(), "Expected error for missing source");
    
    // Test missing severity
    let result = ErrorObjectBuilder::new()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation")
        .build();
    assert!(result.is_err(), "Expected error for missing severity");
    
    // Test missing impact_scope
    let result = ErrorObjectBuilder::new()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation")
        .build();
    assert!(result.is_err(), "Expected error for missing impact_scope");
    
    // Test missing recoverability
    let result = ErrorObjectBuilder::new()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation")
        .build();
    assert!(result.is_err(), "Expected error for missing recoverability");
    
    // Test missing message
    let result = ErrorObjectBuilder::new()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation")
        .build();
    assert!(result.is_err(), "Expected error for missing message");
    
    // Test missing user_message
    let result = ErrorObjectBuilder::new()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .module_path("test::module")
        .operation("test_operation")
        .build();
    assert!(result.is_err(), "Expected error for missing user_message");
    
    // Test missing module_path
    let result = ErrorObjectBuilder::new()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .operation("test_operation")
        .build();
    assert!(result.is_err(), "Expected error for missing module_path");
    
    // Test missing operation
    let result = ErrorObjectBuilder::new()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .build();
    assert!(result.is_err(), "Expected error for missing operation");
}

#[test]
fn test_error_object_optional_fields() {
    // Test optional fields
    let result = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation")
        .session_id("session_123")
        .request_id("request_456")
        .detail("key1", serde_json::json!("value1"))
        .detail("key2", serde_json::json!(42))
        .detail("key3", serde_json::json!([1, 2, 3]))
        .detail("key4", serde_json::json!({"nested": "value"}))
        .build();
    let error = result.unwrap();
    
    assert_eq!(error.session_id().unwrap(), "session_123");
    assert_eq!(error.request_id().unwrap(), "request_456");
    assert_eq!(error.details().get("key1"), Some(&serde_json::json!("value1")));
    assert_eq!(error.details().get("key2"), Some(&serde_json::json!(42)));
    assert_eq!(error.details().get("key3"), Some(&serde_json::json!([1, 2, 3])));
    assert_eq!(error.details().get("key4"), Some(&serde_json::json!({"nested": "value"})));
}

#[test]
fn test_error_object_context_frames() {
    // Test context frames
    let mut builder = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation");
    
    // Add multiple context frames
    for i in 0..5 {
        let source = format!("source_{}", i);
        let mut data = HashMap::new();
        data.insert("key".to_string(), serde_json::json!(i));
        let frame = ContextFrame::new(&source, data);
        builder = builder.context_frame(frame);
    }
    
    let result = builder.build();
    let error = result.unwrap();
    assert_eq!(error.context_chain().len(), 5);
    
    // Test add_context_frame method
    let mut error_mut = error;
    let source = "additional_source";
    let mut data = HashMap::new();
    data.insert("key".to_string(), serde_json::json!("additional"));
    let frame = ContextFrame::new(source, data);
    error_mut.add_context_frame(frame);
    assert_eq!(error_mut.context_chain().len(), 6);
}

#[test]
fn test_error_object_recovery_hints() {
    // Test recovery hints
    let mut builder = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation");
    
    // Add multiple recovery hints
    for i in 0..3 {
        let action = format!("action_{}", i);
        let description = format!("Description {}", i);
        let mut params = HashMap::new();
        params.insert("param".to_string(), serde_json::json!(i));
        let hint = RecoveryHint::new(&action, &description, params);
        builder = builder.recovery_hint(hint);
    }
    
    let result = builder.build();
    let error = result.unwrap();
    assert_eq!(error.recovery_hints().len(), 3);
}

#[test]
fn test_error_object_retry_config() {
    // Test retry config
    let retry_configs = [
        RetryConfig::new(1, 100, 1000, 2.0, false),
        RetryConfig::new(3, 1000, 10000, 2.0, true),
        RetryConfig::new(5, 500, 5000, 1.5, false),
    ];
    
    for config in retry_configs {
        let result = ErrorObject::builder()
            .code("ERR-AIM-LM-001_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Test message")
            .user_message("Test user message")
            .module_path("test::module")
            .operation("test_operation")
            .retry_config(config.clone())
            .build();
        let error = result.unwrap();
        assert_eq!(error.retry_config(), Some(&config));
    }
}

#[test]
fn test_error_object_cause_chain() {
    // Test cause chain
    let cause = ErrorObject::builder()
        .code("ERR-NET-API-001_ERR_S")
        .source(ErrorSource::NET)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Network error")
        .user_message("Network error occurred")
        .module_path("network::api")
        .operation("connect")
        .build();
    
    let result = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model error")
        .user_message("AI model error occurred")
        .module_path("ai::model")
        .operation("generate")
        .cause(cause)
        .build();
    let error = result.unwrap();
    assert!(error.cause().is_some());
    assert_eq!(error.cause().unwrap().code(), "ERR-NET-API-001_ERR_S");
    
    // Test set_cause method
    let mut error_mut = error;
    let new_cause = ErrorObject::builder()
        .code("ERR-FS-IO-001_ERR_S")
        .source(ErrorSource::FS)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("File system error")
        .user_message("File system error occurred")
        .module_path("fs::file")
        .operation("read")
        .build();
    
    error_mut.set_cause(new_cause);
    assert!(error_mut.cause().is_some());
    assert_eq!(error_mut.cause().unwrap().code(), "ERR-FS-IO-001_ERR_S");
}

#[test]
fn test_error_object_all_combinations() {
    // Test all combinations of ErrorSource, Severity, ImpactScope, and Recoverability
    let sources = [
        ErrorSource::USR,
        ErrorSource::AIM,
        ErrorSource::FS,
        ErrorSource::NET,
        ErrorSource::CFG,
        ErrorSource::SEC,
        ErrorSource::TOOL,
        ErrorSource::SESS,
        ErrorSource::STATE,
        ErrorSource::EXT,
        ErrorSource::LSP,
        ErrorSource::MCP,
        ErrorSource::SYS,
        ErrorSource::INT,
        ErrorSource::UNK,
    ];
    
    let severities = [
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];
    
    let impact_scopes = [
        ImpactScope::GLOBAL,
        ImpactScope::SESSION,
        ImpactScope::MODULE,
        ImpactScope::OPERATION,
    ];
    
    let recoverabilities = [
        Recoverability::AutoRecoverable,
        Recoverability::SemiAuto,
        Recoverability::ManualIntervention,
        Recoverability::NonRecoverable,
    ];
    
    for source in &sources {
        for severity in &severities {
            for impact_scope in &impact_scopes {
                for recoverability in &recoverabilities {
                    let code = format!("ERR-{}-TEST-001_{}_{}", 
                        source.as_str(), severity.as_str(), impact_scope.as_str());
                    
                    let result = ErrorObject::builder()
                        .code(&code)
                        .source(*source)
                        .severity(*severity)
                        .impact_scope(*impact_scope)
                        .recoverability(*recoverability)
                        .message("Test message")
                        .user_message("Test user message")
                        .module_path("test::module")
                        .operation("test_operation")
                        .build();
                }
            }
        }
    }
}

#[test]
fn test_error_object_edge_cases() {
    // Test edge cases
    
    // Empty strings
    let result = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("")
        .user_message("")
        .module_path("")
        .operation("")
        .build();
    let error = result.unwrap();
    assert_eq!(error.message(), "");
    assert_eq!(error.user_message(), "");
    assert_eq!(error.module_path(), "");
    assert_eq!(error.operation(), "");
    
    // Long strings
    let long_string = "a".repeat(1000);
    let result = ErrorObject::builder()
        .code(&format!("ERR-AIM-LM-001_ERR_S"))
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&long_string)
        .user_message(&long_string)
        .module_path(&long_string)
        .operation(&long_string)
        .build();
    let error = result.unwrap();
    assert_eq!(error.message(), long_string);
    assert_eq!(error.user_message(), long_string);
    assert_eq!(error.module_path(), long_string);
    assert_eq!(error.operation(), long_string);
}

#[test]
fn test_error_object_debug_format() {
    // Test debug formatting
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation")
        .build();
    
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("ERR-AIM-LM-001_ERR_S"));
    assert!(debug_str.contains("AIM"));
    assert!(debug_str.contains("ERROR"));
    assert!(debug_str.contains("SESSION"));
    assert!(debug_str.contains("AutoRecoverable"));
    assert!(debug_str.contains("Test message"));
    assert!(debug_str.contains("Test user message"));
    assert!(debug_str.contains("test::module"));
    assert!(debug_str.contains("test_operation"));
}

#[test]
fn test_error_object_equality() {
    // Test equality
    let error1 = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation")
        .build();
    
    let error2 = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test message")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation")
        .build();
    
    let error3 = ErrorObject::builder()
        .code("ERR-USR-UI-001_ERR_O")
        .source(ErrorSource::USR)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::SemiAuto)
        .message("Different message")
        .user_message("Different user message")
        .module_path("different::module")
        .operation("different_operation")
        .build();
    
    assert_eq!(error1, error2);
    assert_ne!(error1, error3);
}
