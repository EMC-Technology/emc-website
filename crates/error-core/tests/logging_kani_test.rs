//! Logging module formal verification tests using Kani

use error_core::logging::*;
use error_core::error_object::*;
use error_core::classification::*;
use tracing::Level;
use std::collections::HashMap;
use serde_json::json;

// Test LogEntry::from_error_object
#[kani::proof]
fn test_log_entry_from_error_object() {
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
        .build();
    
    let log_entry = LogEntry::from_error_object(&error);
    assert_eq!(log_entry.error_code, "ERR-AIM-LM-002_ERR_S");
    assert_eq!(log_entry.source, ErrorSource::AIM);
    assert_eq!(log_entry.severity, Severity::ERROR);
    assert_eq!(log_entry.impact_scope, ImpactScope::SESSION);
    assert_eq!(log_entry.recoverability, Recoverability::AutoRecoverable);
    assert_eq!(log_entry.message, "AI model call timed out");
    assert_eq!(log_entry.user_message, "AI model service is temporarily unavailable");
    assert_eq!(log_entry.module_path, "ai_model::lm_manager");
    assert_eq!(log_entry.operation, "generate_code_completion");
    assert_eq!(log_entry.level, "ERROR");
}

// Test LogEntry::severity_to_level
#[kani::proof]
fn test_severity_to_level() {
    assert_eq!(LogEntry::severity_to_level(Severity::CRITICAL), "ERROR".to_string());
    assert_eq!(LogEntry::severity_to_level(Severity::ERROR), "ERROR".to_string());
    assert_eq!(LogEntry::severity_to_level(Severity::WARNING), "WARN".to_string());
    assert_eq!(LogEntry::severity_to_level(Severity::INFO), "INFO".to_string());
}

// Test LogEntry::build_cause_chain
#[kani::proof]
fn test_log_entry_build_cause_chain() {
    // Create a cause error
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
    
    // Create an error with the cause
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
        .cause(cause)
        .build();
    
    let log_entry = LogEntry::from_error_object(&error);
    assert!(log_entry.cause_chain.is_some());
    assert_eq!(log_entry.cause_chain.unwrap(), vec!["ERR-NET-API-001_ERR_S"]);
}

// Test LogFileManager
#[kani::proof]
fn test_log_file_manager() {
    let manager = LogFileManager::new("/var/log");
    
    // Test get_log_path
    assert_eq!(manager.get_log_path(&Level::ERROR), "/var/log/error.log");
    assert_eq!(manager.get_log_path(&Level::WARN), "/var/log/warning.log");
    assert_eq!(manager.get_log_path(&Level::INFO), "/var/log/info.log");
    assert_eq!(manager.get_log_path(&Level::DEBUG), "/var/log/debug.log");
    assert_eq!(manager.get_log_path(&Level::TRACE), "/var/log/trace.log");
    
    // Test get_log_path_from_str
    assert_eq!(manager.get_log_path_from_str("ERROR"), "/var/log/error.log");
    assert_eq!(manager.get_log_path_from_str("WARN"), "/var/log/warning.log");
    assert_eq!(manager.get_log_path_from_str("INFO"), "/var/log/info.log");
    assert_eq!(manager.get_log_path_from_str("DEBUG"), "/var/log/debug.log");
    assert_eq!(manager.get_log_path_from_str("TRACE"), "/var/log/trace.log");
    assert_eq!(manager.get_log_path_from_str("INVALID"), "/var/log/info.log");
    
    // Test rotate_logs (should not panic)
    manager.rotate_logs();
}

// Test LoggingUtils
#[kani::proof]
fn test_logging_utils() {
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
        .build();
    
    // Test log_error (should not panic)
    LoggingUtils::log_error(&error);
    
    // Test configure_logging (should not panic)
    LoggingUtils::configure_logging("/var/log");
}

// Test LoggingUtils with different severity levels
#[kani::proof]
fn test_logging_utils_all_levels() {
    // Test ERROR level
    let error_error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build();
    LoggingUtils::log_error(&error_error);
    
    // Test WARNING level
    let error_warning = ErrorObject::builder()
        .code("ERR-AIM-LM-002_WRN_S")
        .source(ErrorSource::AIM)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call slow")
        .user_message("AI model service is responding slowly")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build();
    LoggingUtils::log_error(&error_warning);
    
    // Test INFO level
    let error_info = ErrorObject::builder()
        .code("ERR-AIM-LM-002_INF_S")
        .source(ErrorSource::AIM)
        .severity(Severity::INFO)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call completed")
        .user_message("AI model service completed the request")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build();
    LoggingUtils::log_error(&error_info);
    
    // Test CRITICAL level (should map to ERROR)
    let error_critical = ErrorObject::builder()
        .code("ERR-AIM-LM-002_CRI_S")
        .source(ErrorSource::AIM)
        .severity(Severity::CRITICAL)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call failed")
        .user_message("AI model service is unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build();
    LoggingUtils::log_error(&error_critical);
}

// Test LogEntry with context chain and details
#[kani::proof]
fn test_log_entry_with_context_and_details() {
    // Test LogEntry with context chain
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
        .detail("model_name", serde_json::json!("gpt-4"))
        .detail("timeout_ms", serde_json::json!(5000))
        .session_id("session_123")
        .request_id("request_456")
        .build();
    
    let log_entry = LogEntry::from_error_object(&error);
    assert_eq!(log_entry.error_code, "ERR-AIM-LM-002_ERR_S");
    assert!(log_entry.context.contains_key("model_name"));
    assert!(log_entry.context.contains_key("timeout_ms"));
    assert_eq!(log_entry.session_id, Some("session_123".to_string()));
    assert_eq!(log_entry.request_id, Some("request_456".to_string()));
    assert_eq!(log_entry.level, "ERROR");
}

// Test ErrorLoggingLayer creation
#[kani::proof]
fn test_error_logging_layer_creation() {
    // Test ErrorLoggingLayer creation
    let writer = || std::io::stdout();
    let _layer = ErrorLoggingLayer::new(writer);
    // Just testing that creation doesn't panic
}

// Test LogEntry from error object with empty fields
#[kani::proof]
fn test_log_entry_from_error_object_empty_fields() {
    // Test with empty details, empty context chain, and no cause
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
        .build();
    
    let log_entry = LogEntry::from_error_object(&error);
    assert_eq!(log_entry.error_code, "ERR-AIM-LM-002_ERR_S");
    assert!(log_entry.cause_chain.is_none());
    assert_eq!(log_entry.level, "ERROR");
}
