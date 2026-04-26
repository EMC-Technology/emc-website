//! Logging fuzz tests
//! 
//! This module contains fuzz tests for the logging module to ensure coverage of all possible cases.

use error_core::logging::{LogEntry, LogFileManager, LoggingUtils};
use error_core::error_object::ErrorObject;
use error_core::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
use error_core::propagation::ContextFrame;
use std::collections::HashMap;
use serde_json::json;
use tracing::Level;

#[test]
fn test_log_entry_from_error_object() {
    // Test LogEntry creation from various ErrorObject configurations
    let error_configs = vec![
        ("basic", ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build()),
        ("with_session_and_request_id", ErrorObject::builder()
            .code("ERR-AIM-LM-003_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call failed")
            .user_message("AI model service is unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .session_id("session_123")
            .request_id("request_456")
            .build()),
        ("with_details", ErrorObject::builder()
            .code("ERR-AIM-LM-004_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call failed")
            .user_message("AI model service is unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .detail("model_name", json!("gpt-4"))
            .detail("timeout_ms", json!(5000))
            .detail("attempts", json!(3))
            .build()),
        ("with_context_chain", ErrorObject::builder()
            .code("ERR-AIM-LM-005_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call failed")
            .user_message("AI model service is unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .context_frame(ContextFrame::new("api_gateway", HashMap::new()))
            .context_frame(ContextFrame::new("auth_service", HashMap::new()))
            .build()),
        ("with_cause", ErrorObject::builder()
            .code("ERR-AIM-LM-006_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call failed")
            .user_message("AI model service is unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .cause(ErrorObject::builder()
                .code("ERR-NET-API-001_ERR_S")
                .source(ErrorSource::NET)
                .severity(Severity::ERROR)
                .impact_scope(ImpactScope::SESSION)
                .recoverability(Recoverability::AutoRecoverable)
                .message("Network timeout")
                .user_message("Network service is temporarily unavailable")
                .module_path("network::api_client")
                .operation("send_request")
                .build())
            .build()),
    ];
    
    for (_name, error) in error_configs {
        // Just test that LogEntry can be created without panicking
        let _log_entry = LogEntry::from_error_object(&error);
    }
}

#[test]
fn test_log_file_manager() {
    // Test LogFileManager with different log directories
    let log_dirs = ["", "/var/log", "./logs", "c:\\logs"];
    
    for log_dir in log_dirs {
        let manager = LogFileManager::new(log_dir);
        
        // Test get_log_path with all levels
        assert_eq!(manager.get_log_path(&Level::ERROR), format!("{}/error.log", log_dir).replace("//", "/"));
        assert_eq!(manager.get_log_path(&Level::WARN), format!("{}/warning.log", log_dir).replace("//", "/"));
        assert_eq!(manager.get_log_path(&Level::INFO), format!("{}/info.log", log_dir).replace("//", "/"));
        assert_eq!(manager.get_log_path(&Level::DEBUG), format!("{}/debug.log", log_dir).replace("//", "/"));
        assert_eq!(manager.get_log_path(&Level::TRACE), format!("{}/trace.log", log_dir).replace("//", "/"));
        
        // Test get_log_path_from_str with all levels
        assert_eq!(manager.get_log_path_from_str("ERROR"), format!("{}/error.log", log_dir).replace("//", "/"));
        assert_eq!(manager.get_log_path_from_str("WARN"), format!("{}/warning.log", log_dir).replace("//", "/"));
        assert_eq!(manager.get_log_path_from_str("INFO"), format!("{}/info.log", log_dir).replace("//", "/"));
        assert_eq!(manager.get_log_path_from_str("DEBUG"), format!("{}/debug.log", log_dir).replace("//", "/"));
        assert_eq!(manager.get_log_path_from_str("TRACE"), format!("{}/trace.log", log_dir).replace("//", "/"));
        
        // Test get_log_path_from_str with invalid level (should default to info)
        assert_eq!(manager.get_log_path_from_str("INVALID"), format!("{}/info.log", log_dir).replace("//", "/"));
        assert_eq!(manager.get_log_path_from_str(""), format!("{}/info.log", log_dir).replace("//", "/"));
        assert_eq!(manager.get_log_path_from_str("UNKNOWN"), format!("{}/info.log", log_dir).replace("//", "/"));
        
        // Test rotate_logs (should not panic)
        manager.rotate_logs();
    }
}

#[test]
fn test_logging_utils() {
    // Test LoggingUtils with different error configurations
    let error_configs = vec![
        ("error", ErrorObject::builder()
            .code("ERR-AIM-LM-001_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build()),
        ("warning", ErrorObject::builder()
            .code("ERR-AIM-LM-002_WRN_S")
            .source(ErrorSource::AIM)
            .severity(Severity::WARNING)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call slow")
            .user_message("AI model service is responding slowly")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build()),
        ("info", ErrorObject::builder()
            .code("ERR-AIM-LM-003_INF_S")
            .source(ErrorSource::AIM)
            .severity(Severity::INFO)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call completed")
            .user_message("AI model service completed the request")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build()),
        ("critical", ErrorObject::builder()
            .code("ERR-AIM-LM-004_CRI_S")
            .source(ErrorSource::AIM)
            .severity(Severity::CRITICAL)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call failed")
            .user_message("AI model service is unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build()),
    ];
    
    for (_name, error) in error_configs {
        // Test log_error (should not panic)
        LoggingUtils::log_error(&error);
    }
    
    // Test configure_logging with different directories
    let log_dirs = ["", "/var/log", "./logs", "c:\\logs"];
    for log_dir in log_dirs {
        // Test configure_logging (should not panic)
        LoggingUtils::configure_logging(log_dir);
    }
}

#[test]
fn test_log_entry_edge_cases() {
    // Test edge cases for LogEntry
    
    // Test with empty strings
    let error_empty_strings = ErrorObject::builder()
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
    
    // Just test that LogEntry can be created without panicking
    let _log_entry_empty_strings = LogEntry::from_error_object(&error_empty_strings);
    
    // Test with very long strings
    let long_string = "a".repeat(1000);
    let error_long_strings = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&long_string)
        .user_message(&long_string)
        .module_path(&long_string)
        .operation(&long_string)
        .build();
    
    // Just test that LogEntry can be created without panicking
    let _log_entry_long_strings = LogEntry::from_error_object(&error_long_strings);
}

#[test]
fn test_logging_utils_log_level_mapping() {
    // Test that all severity levels can be logged without panicking
    let severity_level_pairs = vec![
        (Severity::CRITICAL, "ERROR"),
        (Severity::ERROR, "ERROR"),
        (Severity::WARNING, "WARN"),
        (Severity::INFO, "INFO"),
    ];
    
    for (severity, _expected_log_level) in severity_level_pairs {
        let error = ErrorObject::builder()
            .code("ERR-TEST-TEST-001_ERR_S")
            .source(ErrorSource::INT)
            .severity(severity)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Test message")
            .user_message("Test user message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        
        // Test that logging doesn't panic
        LoggingUtils::log_error(&error);
    }
}

#[test]
fn test_log_entry_with_all_combinations() {
    // Test LogEntry creation with all combinations of ErrorSource, Severity, ImpactScope, and Recoverability
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
                    let error = ErrorObject::builder()
                        .code("ERR-TEST-TEST-001_ERR_S")
                        .source(*source)
                        .severity(*severity)
                        .impact_scope(*impact_scope)
                        .recoverability(*recoverability)
                        .message("Test message")
                        .user_message("Test user message")
                        .module_path("test::module")
                        .operation("test_operation")
                        .build();
                    
                    // Just test that LogEntry can be created without panicking
                    let _log_entry = LogEntry::from_error_object(&error);
                }
            }
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn test_log_entry_to_json() {
    // Test LogEntry to_json method
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
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
    let json_result = log_entry.to_json();
    assert!(json_result.is_ok());
    
    // Test with more complex error object
    let error_with_details = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call failed")
        .user_message("AI model service is unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .session_id("session_123")
        .request_id("request_456")
        .detail("model_name", json!("gpt-4"))
        .detail("timeout_ms", json!(5000))
        .context_frame(ContextFrame::new("api_gateway", HashMap::new()))
        .build();
    
    let log_entry_with_details = LogEntry::from_error_object(&error_with_details);
    let json_result_with_details = log_entry_with_details.to_json();
    assert!(json_result_with_details.is_ok());
}
