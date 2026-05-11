//! Logging property tests
#![allow(clippy::uninlined_format_args)]
//!
//! This module contains property tests for the logging module to ensure coverage of all possible cases.

use error_core::classification::{ErrorSource, ImpactScope, Recoverability, Severity};
use error_core::error_object::ErrorObject;
use error_core::logging::{ErrorLoggingLayer, LogEntry, LogFileManager, LogLevel, LoggingUtils};
use error_core::propagation::ContextFrame;
use std::collections::HashMap;

#[test]
fn test_log_entry_from_error_object() {
    // Test LogEntry creation from ErrorObject with various configurations
    let sources = [
        ErrorSource::USR,
        ErrorSource::AIM,
        ErrorSource::FS,
        ErrorSource::NET,
        ErrorSource::CFG,
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

    for source in &sources {
        for severity in &severities {
            for impact_scope in &impact_scopes {
                let code = format!(
                    "ERR-{}-LM-001_{}_{}",
                    source.as_str(),
                    severity.as_str(),
                    impact_scope.as_str()
                );

                let error = ErrorObject::builder()
                    .code(&code)
                    .source(*source)
                    .severity(*severity)
                    .impact_scope(*impact_scope)
                    .recoverability(Recoverability::AutoRecoverable)
                    .message("Test error")
                    .user_message("Test user message")
                    .module_path("test.module")
                    .operation("test_operation")
                    .build();

                // Test that LogEntry can be created without panicking
                let _log_entry = LogEntry::from_error_object(&error);
            }
        }
    }
}

#[test]
fn test_log_entry_with_context_chain() {
    // Test LogEntry with different numbers of context frames
    let frame_counts = [0, 1, 3, 5];

    for frame_count in &frame_counts {
        let mut builder = ErrorObject::builder()
            .code("ERR-AIM-LM-001_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Test error")
            .user_message("Test user message")
            .module_path("test.module")
            .operation("test_operation");

        for i in 0..*frame_count {
            let source = format!("module_{}", i);
            let mut data = HashMap::new();
            data.insert("key".to_string(), serde_json::json!(i));
            let frame = ContextFrame::new(&source, data);
            builder = builder.context_frame(frame);
        }

        let error = builder.build();
        // Test that LogEntry can be created without panicking
        let _log_entry = LogEntry::from_error_object(&error);
    }
}

#[test]
fn test_log_entry_with_cause_chain() {
    // Test LogEntry with cause chain
    let cause = ErrorObject::builder()
        .code("ERR-NET-API-001_ERR_S")
        .source(ErrorSource::NET)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Network error")
        .user_message("Network error occurred")
        .module_path("network.api")
        .operation("connect")
        .build();

    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model error")
        .user_message("AI model error occurred")
        .module_path("ai_model.lm")
        .operation("generate")
        .cause(cause)
        .build();

    // Test that LogEntry can be created without panicking
    let _log_entry = LogEntry::from_error_object(&error);
}

#[test]
fn test_log_file_manager() {
    // Test LogFileManager with different log directories
    let log_dirs = ["/var/log", "./logs", "C:\\Logs"];

    for log_dir in &log_dirs {
        let manager = LogFileManager::new(log_dir);

        // Test get_log_path
        assert_eq!(
            manager.get_log_path(&tracing::Level::ERROR),
            format!("{}/error.log", log_dir)
        );
        assert_eq!(
            manager.get_log_path(&tracing::Level::WARN),
            format!("{}/warning.log", log_dir)
        );
        assert_eq!(
            manager.get_log_path(&tracing::Level::INFO),
            format!("{}/info.log", log_dir)
        );
        assert_eq!(
            manager.get_log_path(&tracing::Level::DEBUG),
            format!("{}/debug.log", log_dir)
        );
        assert_eq!(
            manager.get_log_path(&tracing::Level::TRACE),
            format!("{}/trace.log", log_dir)
        );

        // Test get_log_path_from_level
        assert_eq!(
            manager.get_log_path_from_level(LogLevel::Error),
            format!("{}/error.log", log_dir)
        );
        assert_eq!(
            manager.get_log_path_from_level(LogLevel::Warn),
            format!("{}/warning.log", log_dir)
        );
        assert_eq!(
            manager.get_log_path_from_level(LogLevel::Info),
            format!("{}/info.log", log_dir)
        );
        assert_eq!(
            manager.get_log_path_from_level(LogLevel::Debug),
            format!("{}/debug.log", log_dir)
        );
        assert_eq!(
            manager.get_log_path_from_level(LogLevel::Trace),
            format!("{}/trace.log", log_dir)
        );

        // Test rotate_logs (should not panic)
        manager.rotate_logs();
    }
}

#[test]
fn test_logging_utils() {
    // Test LoggingUtils with different error objects
    let sources = [ErrorSource::USR, ErrorSource::AIM, ErrorSource::FS];

    let severities = [
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];

    for source in &sources {
        for severity in &severities {
            let code = format!("ERR-{}-LM-001_{}_S", source.as_str(), severity.as_str());

            let error = ErrorObject::builder()
                .code(&code)
                .source(*source)
                .severity(*severity)
                .impact_scope(ImpactScope::SESSION)
                .recoverability(Recoverability::AutoRecoverable)
                .message("Test error")
                .user_message("Test user message")
                .module_path("test.module")
                .operation("test_operation")
                .build();

            // Test log_error (should not panic)
            LoggingUtils::log_error(&error);
        }
    }

    // Test configure_logging (should not panic)
    LoggingUtils::configure_logging("./logs");
}

#[test]
fn test_error_logging_layer() {
    // Test ErrorLoggingLayer creation and methods
    let writer = || std::io::stdout();
    let _layer = ErrorLoggingLayer::new(writer);

    // Test that the layer can be created without panicking
    // We can't test the on_event method directly as it requires a real Event and Context
}

#[test]
fn test_log_entry_to_json() {
    // Test LogEntry::to_json method (when serde feature is enabled)
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("test.module")
        .operation("test_operation")
        .build();

    #[allow(unused_variables)]
    let log_entry = LogEntry::from_error_object(&error);

    // Test to_json if serde feature is enabled
    #[cfg(feature = "serde")]
    {
        let json_result = log_entry.to_json();
        assert!(json_result.is_ok());
        let json_str = json_result.unwrap();
        assert!(!json_str.is_empty());
    }
}

#[test]
fn test_log_file_manager_rotation() {
    // Test LogFileManager::rotate_logs method
    let manager = LogFileManager::new("./logs");

    // Test that rotate_logs doesn't panic
    manager.rotate_logs();
}

#[test]
fn test_log_entry_from_error_object_with_details() {
    // Test LogEntry creation with details
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("test.module")
        .operation("test_operation")
        .detail("key1", serde_json::json!("value1"))
        .detail("key2", serde_json::json!(42))
        .build();

    // Test that LogEntry can be created without panicking
    let _log_entry = LogEntry::from_error_object(&error);
}

#[test]
fn test_log_entry_from_error_object_with_session_and_request_id() {
    // Test LogEntry creation with session and request ID
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("test.module")
        .operation("test_operation")
        .session_id("session_123")
        .request_id("request_456")
        .build();

    // Test that LogEntry can be created without panicking
    let _log_entry = LogEntry::from_error_object(&error);
}

#[test]
fn test_log_entry_creation() {
    // Test that LogEntry can be created from different error objects
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("test.module")
        .operation("test_operation")
        .build();

    // Test that LogEntry can be created without panicking
    let _log_entry = LogEntry::from_error_object(&error);
}

#[test]
fn test_logging_utils_all_log_levels() {
    // Test LoggingUtils::log_error with all log levels

    // Test ERROR level
    let error_error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("test.module")
        .operation("test_operation")
        .build();
    LoggingUtils::log_error(&error_error);

    // Test WARNING level
    let error_warning = ErrorObject::builder()
        .code("ERR-AIM-LM-001_WRN_S")
        .source(ErrorSource::AIM)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test warning")
        .user_message("Test warning message")
        .module_path("test.module")
        .operation("test_operation")
        .build();
    LoggingUtils::log_error(&error_warning);

    // Test INFO level
    let error_info = ErrorObject::builder()
        .code("ERR-AIM-LM-001_INF_S")
        .source(ErrorSource::AIM)
        .severity(Severity::INFO)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test info")
        .user_message("Test info message")
        .module_path("test.module")
        .operation("test_operation")
        .build();
    LoggingUtils::log_error(&error_info);

    // Test CRITICAL level (should map to ERROR)
    let error_critical = ErrorObject::builder()
        .code("ERR-AIM-LM-001_CRI_S")
        .source(ErrorSource::AIM)
        .severity(Severity::CRITICAL)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test critical error")
        .user_message("Test critical error message")
        .module_path("test.module")
        .operation("test_operation")
        .build();
    LoggingUtils::log_error(&error_critical);
}
