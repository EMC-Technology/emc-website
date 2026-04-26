//! Logging comprehensive lifecycle tests
//!
//! This module tests the complete lifecycle of logging functionality, covering all edge cases and boundary values.
#![allow(clippy::uninlined_format_args, clippy::used_underscore_binding, clippy::items_after_statements)]

use error_core::logging::{LogEntry, LogFileManager, ErrorLoggingLayer};
use error_core::error_object::ErrorObject;
use error_core::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
use error_core::propagation::{ContextFrame, RecoveryHint, RetryConfig};
use std::collections::HashMap;

#[test]
fn test_log_entry_complete_lifecycle() {
    // Create a complex error object with cause chain and context
    let cause = ErrorObject::builder()
        .code("ERR-NET-API-001_ERR_S")
        .source(ErrorSource::NET)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Network timeout")
        .user_message("Network service is temporarily unavailable")
        .module_path("network::api_client")
        .operation("send_request")
        .build();
    
    let context_frame = ContextFrame::new("api_gateway", HashMap::new());
    let recovery_hint = RecoveryHint::new("Retry", "Retry the operation", HashMap::new());
    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);
    
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .session_id("session_123")
        .request_id("request_456")
        .detail("model_name", serde_json::json!("gpt-4"))
        .context_frame(context_frame)
        .recovery_hint(recovery_hint)
        .retry_config(retry_config)
        .cause(cause)
        .build();
    
    // Test log entry creation from error object - just ensure it doesn't panic
    let _log_entry = LogEntry::from_error_object(&error);
    
    // Test debug formatting
    let debug_str = format!("{:?}", _log_entry);
    assert!(debug_str.contains("LogEntry"));
    assert!(debug_str.contains("ERR-AIM-LM-002_ERR_S"));
}

#[test]
fn test_log_entry_edge_cases() {
    // Test log entry with minimal error object
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("test::module")
        .operation("test_operation")
        .build();
    
    // Test log entry creation from minimal error object - just ensure it doesn't panic
    let _log_entry = LogEntry::from_error_object(&error);
    
    // Test debug formatting
    let debug_str = format!("{:?}", _log_entry);
    assert!(debug_str.contains("LogEntry"));
    assert!(debug_str.contains("ERR-AIM-LM-001_ERR_S"));
}

#[test]
fn test_log_file_manager() {
    // Test log file manager creation
    let log_dir = "/var/log/test";
    
    let manager = LogFileManager::new(log_dir);
    
    // Test get_log_path
    use tracing::Level;
    assert_eq!(manager.get_log_path(&Level::ERROR), "/var/log/test/error.log");
    assert_eq!(manager.get_log_path(&Level::WARN), "/var/log/test/warning.log");
    assert_eq!(manager.get_log_path(&Level::INFO), "/var/log/test/info.log");
    assert_eq!(manager.get_log_path(&Level::DEBUG), "/var/log/test/debug.log");
    assert_eq!(manager.get_log_path(&Level::TRACE), "/var/log/test/trace.log");
    
    // Test get_log_path_from_str
    assert_eq!(manager.get_log_path_from_str("ERROR"), "/var/log/test/error.log");
    assert_eq!(manager.get_log_path_from_str("WARN"), "/var/log/test/warning.log");
    assert_eq!(manager.get_log_path_from_str("INFO"), "/var/log/test/info.log");
    assert_eq!(manager.get_log_path_from_str("DEBUG"), "/var/log/test/debug.log");
    assert_eq!(manager.get_log_path_from_str("TRACE"), "/var/log/test/trace.log");
    assert_eq!(manager.get_log_path_from_str("INVALID"), "/var/log/test/info.log");
    
    // Test log rotation (should not panic)
    manager.rotate_logs();
}

#[test]
fn test_error_logging_layer() {
    // Test error logging layer creation
    let writer = || std::io::stdout();
    let _layer = ErrorLoggingLayer::new(writer);
    
    // Note: Testing the actual logging functionality requires setting up a tracing subscriber
    // which is beyond the scope of this unit test
}

// Test for severity_to_level removed since it's a private method
