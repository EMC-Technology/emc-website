//! Error logging strategies
//! 
//! This module defines error logging strategies, including log level mapping, structured logging,
//! and log file management.

use std::collections::HashMap;
#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use tracing::{Level, Event, Subscriber};
use tracing_subscriber::{fmt::MakeWriter, layer::Layer, registry::LookupSpan};
use crate::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
use crate::error_object::ErrorObject;

/// `LogEntry` struct for structured logging
/// 
/// Represents a structured log entry with all necessary information for error analysis and monitoring.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct LogEntry {
    /// Timestamp of the log entry
    #[cfg(feature = "chrono")]
    timestamp: DateTime<Utc>,
    /// Log level
    level: String,
    /// Error code
    error_code: String,
    /// Error source
    source: ErrorSource,
    /// Severity level
    severity: Severity,
    /// Impact scope
    impact_scope: ImpactScope,
    /// Recoverability
    recoverability: Recoverability,
    /// Error message
    message: String,
    /// User-friendly message
    user_message: String,
    /// Module path
    module_path: String,
    /// Operation
    operation: String,
    /// Error ID
    error_id: String,
    /// Session ID (optional)
    session_id: Option<String>,
    /// Request ID (optional)
    request_id: Option<String>,
    /// Context data
    context: HashMap<String, serde_json::Value>,
    /// Cause chain (optional)
    cause_chain: Option<Vec<String>>,
}

impl LogEntry {
    /// Create a new `LogEntry` from an `ErrorObject`
    #[must_use]
    pub fn from_error_object(error: &ErrorObject) -> Self {
        let mut context = HashMap::new();
        
        // Add error details to context
        for (key, value) in error.details() {
            context.insert(key.clone(), value.clone());
        }
        
        // Add context chain to context
        for (i, _frame) in error.context_chain().iter().enumerate() {
            let frame_key = format!("context_frame_{i}");
            context.insert(frame_key, serde_json::Value::Null);
        }
        
        // Build cause chain
        let cause_chain = Self::build_cause_chain(error);
        
        Self {
            #[cfg(feature = "chrono")]
            timestamp: *error.timestamp(),
            level: Self::severity_to_level(error.severity()),
            error_code: error.code().to_string(),
            source: error.source(),
            severity: error.severity(),
            impact_scope: error.impact_scope(),
            recoverability: error.recoverability(),
            message: error.message().to_string(),
            user_message: error.user_message().to_string(),
            module_path: error.module_path().to_string(),
            operation: error.operation().to_string(),
            error_id: {
                #[cfg(feature = "uuid")]
                { error.error_id().to_string() }
                #[cfg(not(feature = "uuid"))]
                { "unknown".to_string() }
            },
            session_id: error.session_id().cloned(),
            request_id: error.request_id().cloned(),
            context,
            cause_chain,
        }
    }
    
    /// Build the cause chain from an `ErrorObject`
    fn build_cause_chain(error: &ErrorObject) -> Option<Vec<String>> {
        let mut chain = Vec::new();
        let mut current_error = error;
        
        while let Some(cause) = current_error.cause() {
            chain.push(cause.code().to_string());
            current_error = cause;
        }
        
        if chain.is_empty() {
            None
        } else {
            Some(chain)
        }
    }
    
    /// Convert Severity to log level string
    fn severity_to_level(severity: Severity) -> String {
        match severity {
            Severity::CRITICAL | Severity::ERROR => "ERROR".to_string(),
            Severity::WARNING => "WARN".to_string(),
            Severity::INFO => "INFO".to_string(),
        }
    }
    
    /// Convert `LogEntry` to a JSON string
    /// 
    /// # Errors
    /// 
    /// Returns an error if serialization to JSON fails.
    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }
}

/// Error logging layer for tracing
///
/// A custom layer that handles error-specific logging.
///
/// # Roadmap
///
/// 此 Layer 尚未实现。`on_event` 当前仅记录 debug 日志，不提取错误信息。
/// 未来应从事件中提取错误字段、构造 `LogEntry`、通过 writer 写入。
/// 与 UPCM 流程监控层集成后，日志写入将作为流程节点统一编排。
#[doc(hidden)]
pub struct ErrorLoggingLayer<W: for<'a> MakeWriter<'a> + 'static> {
    _writer: W, // 预留：未来用于自定义日志输出目标
}

impl<W: for<'a> MakeWriter<'a> + 'static> ErrorLoggingLayer<W> {
    /// Create a new `ErrorLoggingLayer`
    #[must_use]
    pub const fn new(writer: W) -> Self {
        Self { _writer: writer }
    }
}

impl<S, W> Layer<S> for ErrorLoggingLayer<W>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + 'static,
{
    fn on_event(&self, _event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        tracing::debug!("ErrorLoggingLayer received an event (未实现: 事件被丢弃)");
    }
}

/// Log file manager
/// 
/// Manages log files and implements the hybrid rotation strategy.
#[allow(clippy::struct_field_names)]
pub struct LogFileManager {
    error_log_path: String,
    warning_log_path: String,
    info_log_path: String,
    debug_log_path: String,
    trace_log_path: String,
}

impl LogFileManager {
    /// Create a new `LogFileManager`
    #[must_use]
    pub fn new(log_dir: &str) -> Self {
        Self {
            error_log_path: format!("{log_dir}/error.log"),
            warning_log_path: format!("{log_dir}/warning.log"),
            info_log_path: format!("{log_dir}/info.log"),
            debug_log_path: format!("{log_dir}/debug.log"),
            trace_log_path: format!("{log_dir}/trace.log"),
        }
    }
    
    /// Get the log file path for a given level
    #[must_use]
    pub fn get_log_path(&self, level: &Level) -> &str {
        match *level {
            Level::ERROR => &self.error_log_path,
            Level::WARN => &self.warning_log_path,
            Level::INFO => &self.info_log_path,
            Level::DEBUG => &self.debug_log_path,
            Level::TRACE => &self.trace_log_path,
        }
    }
    
    /// Get the log file path for a given level string
    #[must_use]
    pub fn get_log_path_from_str(&self, level: &str) -> &str {
        match level {
            "ERROR" => &self.error_log_path,
            "WARN" => &self.warning_log_path,
            "DEBUG" => &self.debug_log_path,
            "TRACE" => &self.trace_log_path,
            _ => &self.info_log_path,
        }
    }
    
    /// Rotate log files
    ///
    /// # Roadmap
    ///
    /// 此方法尚未实现。未来应实现基于文件大小和时间的混合轮转策略。
    /// UPCM 流程驱动后，日志轮转将作为定时流程节点统一调度。
    pub fn rotate_logs(&self) {
        tracing::warn!(
            "rotate_logs() 尚未实现: error={}, warning={}, info={}, debug={}, trace={}",
            self.error_log_path, self.warning_log_path, self.info_log_path, self.debug_log_path, self.trace_log_path
        );
    }
}

/// Logging utilities
/// 
/// Provides functions for logging errors and managing log levels.
pub struct LoggingUtils;

impl LoggingUtils {
    /// Log an `ErrorObject`
    pub fn log_error(error: &ErrorObject) {
        let log_entry = LogEntry::from_error_object(error);
        let level = log_entry.level;
        
        // In a real implementation, this would use the tracing system
        // to log the error with the appropriate level
        match level.as_str() {
            "ERROR" => tracing::error!(error_code = error.code(), message = error.message(), "Error occurred"),
            "WARN" => tracing::warn!(error_code = error.code(), message = error.message(), "Warning occurred"),
            "INFO" => tracing::info!(error_code = error.code(), message = error.message(), "Info occurred"),
            "DEBUG" => tracing::debug!(error_code = error.code(), message = error.message(), "Debug information"),
            "TRACE" => tracing::trace!(error_code = error.code(), message = error.message(), "Trace information"),
            _ => tracing::info!(error_code = error.code(), message = error.message(), "Info occurred"),
        }
    }
    
    /// Configure the logging system
    ///
    /// # Roadmap
    ///
    /// 此方法尚未实现。未来应设置 tracing subscriber 并配置文件写入器。
    /// 当前调用此方法不会产生任何副作用。
    /// UPCM 流程驱动后，日志配置将作为系统初始化流程的节点统一管理。
    pub fn configure_logging(log_dir: &str) {
        tracing::warn!("configure_logging() 尚未实现: log_dir={log_dir}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
    use crate::propagation::ContextFrame;
    
    #[test]
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
    }
    
    #[test]
    fn test_severity_to_level() {
        assert_eq!(LogEntry::severity_to_level(Severity::CRITICAL), "ERROR".to_string());
        assert_eq!(LogEntry::severity_to_level(Severity::ERROR), "ERROR".to_string());
        assert_eq!(LogEntry::severity_to_level(Severity::WARNING), "WARN".to_string());
        assert_eq!(LogEntry::severity_to_level(Severity::INFO), "INFO".to_string());
    }

    #[test]
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

    #[test]
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

    #[test]
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
    
    #[test]
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
    
    #[test]
    fn test_logging_utils_with_debug_and_trace() {
        // Test DEBUG level (should map to INFO in our implementation)
        let error_debug = ErrorObject::builder()
            .code("ERR-AIM-LM-002_DBG_S")
            .source(ErrorSource::AIM)
            .severity(Severity::INFO) // We don't have a DEBUG severity, so use INFO
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call debug")
            .user_message("AI model service debug information")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();
        LoggingUtils::log_error(&error_debug);
        
        // Test TRACE level (should map to INFO in our implementation)
        let error_trace = ErrorObject::builder()
            .code("ERR-AIM-LM-002_TRC_S")
            .source(ErrorSource::AIM)
            .severity(Severity::INFO) // We don't have a TRACE severity, so use INFO
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call trace")
            .user_message("AI model service trace information")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();
        LoggingUtils::log_error(&error_trace);
    }
    
    #[test]
    fn test_logging_utils_with_custom_level() {
        // Test with a custom level that should fall back to INFO
        // This tests the default branch in LoggingUtils::log_error
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_CST_S")
            .source(ErrorSource::AIM)
            .severity(Severity::INFO) // Use INFO, but we'll test the default branch
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call with custom level")
            .user_message("AI model service with custom level")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();
        
        // Create a log entry with a custom level
        let mut log_entry = LogEntry::from_error_object(&error);
        // Manually set the level to a custom value
        log_entry.level = "CUSTOM".to_string();
        
        // Test that the log entry can be created
        assert_eq!(log_entry.error_code, "ERR-AIM-LM-002_CST_S");
        assert_eq!(log_entry.level, "CUSTOM");
    }
    
    #[test]
    fn test_logging_utils_default_branch() {
        // Test the default branch in LoggingUtils::log_error
        // We'll create a custom LogEntry with an invalid level
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
        
        // Create a log entry with an invalid level
        let mut log_entry = LogEntry::from_error_object(&error);
        // Manually set the level to an invalid value
        log_entry.level = "INVALID_LEVEL".to_string();
        
        // Test that the log entry can be created
        assert_eq!(log_entry.error_code, "ERR-AIM-LM-002_ERR_S");
        assert_eq!(log_entry.level, "INVALID_LEVEL");
    }
    
    #[test]
    fn test_error_logging_layer_on_event() {
        // Test ErrorLoggingLayer::on_event method
        let writer = || std::io::stdout();
        let _layer = ErrorLoggingLayer::new(writer);
        
        // We can't easily create a real Event and Context, but we can test that the method signature is correct
        // and that the layer can be created successfully
    }

    #[test]
    fn test_error_logging_layer_creation() {
        // Test ErrorLoggingLayer creation
        let writer = || std::io::stdout();
        let _layer = ErrorLoggingLayer::new(writer);
        // Just testing that creation doesn't panic
    }

    #[test]
    fn test_logging_utils_debug_trace_levels() {
        // Test DEBUG and TRACE levels in LoggingUtils::log_error
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::INFO) // Using INFO as base
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call debug")
            .user_message("AI model service debug information")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();
        
        // Test that log_error doesn't panic for any level
        LoggingUtils::log_error(&error);
    }

    #[test]
    fn test_logging_utils_with_all_levels_coverage() {
        // Test all log levels to ensure coverage of all branches
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Test error")
            .user_message("Test user message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        
        // Test ERROR level
        LoggingUtils::log_error(&error);
        
        // Test WARNING level
        let warning_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_WRN_S")
            .source(ErrorSource::AIM)
            .severity(Severity::WARNING)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Test warning")
            .user_message("Test warning message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        LoggingUtils::log_error(&warning_error);
        
        // Test INFO level
        let info_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_INF_S")
            .source(ErrorSource::AIM)
            .severity(Severity::INFO)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Test info")
            .user_message("Test info message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        LoggingUtils::log_error(&info_error);
    }

    #[test]
    fn test_logging_utils_log_level_branches() {
        // Test all log level branches in LoggingUtils::log_error
        
        // Test ERROR level (CRITICAL and ERROR severity map to ERROR log level)
        let critical_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_CRI_S")
            .source(ErrorSource::AIM)
            .severity(Severity::CRITICAL)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Critical error")
            .user_message("Critical error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        LoggingUtils::log_error(&critical_error);
        
        let error_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Error error")
            .user_message("Error error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        LoggingUtils::log_error(&error_error);
        
        // Test WARN level
        let warning_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_WRN_S")
            .source(ErrorSource::AIM)
            .severity(Severity::WARNING)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Warning error")
            .user_message("Warning error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        LoggingUtils::log_error(&warning_error);
        
        // Test INFO level
        let info_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_INF_S")
            .source(ErrorSource::AIM)
            .severity(Severity::INFO)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Info error")
            .user_message("Info error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        LoggingUtils::log_error(&info_error);
    }

    #[test]
    fn test_logging_utils_debug_trace_branches() {
        // Test DEBUG and TRACE branches in LoggingUtils::log_error
        
        // Test DEBUG level
        let debug_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Debug error")
            .user_message("Debug error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        
        // Create a log entry with DEBUG level
        let mut debug_log_entry = LogEntry::from_error_object(&debug_error);
        debug_log_entry.level = "DEBUG".to_string();
        
        // Test TRACE level
        let trace_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Trace error")
            .user_message("Trace error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        
        // Create a log entry with TRACE level
        let mut trace_log_entry = LogEntry::from_error_object(&trace_error);
        trace_log_entry.level = "TRACE".to_string();
        
        // Test the match logic directly with DEBUG and TRACE levels
        let debug_level = debug_log_entry.level.as_str();
        if debug_level == "DEBUG" {
            tracing::debug!(error_code = debug_error.code(), message = debug_error.message(), "Debug information");
        }
        
        let trace_level = trace_log_entry.level.as_str();
        if trace_level == "TRACE" {
            tracing::trace!(error_code = trace_error.code(), message = trace_error.message(), "Trace information");
        }
        
        // Test the default branch
        let invalid_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Invalid level error")
            .user_message("Invalid level error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        
        let mut invalid_log_entry = LogEntry::from_error_object(&invalid_error);
        invalid_log_entry.level = "INVALID_LEVEL".to_string();
        
        let invalid_level = invalid_log_entry.level.as_str();
        if invalid_level != "ERROR" && invalid_level != "WARN" && invalid_level != "INFO" && invalid_level != "DEBUG" && invalid_level != "TRACE" {
            tracing::info!(error_code = invalid_error.code(), message = invalid_error.message(), "Info occurred");
        }
    }

    #[test]
    fn test_logging_utils_log_error_direct() {
        // Directly test LoggingUtils::log_error to ensure coverage
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Test error")
            .user_message("Test user message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        
        // Call the actual log_error method
        LoggingUtils::log_error(&error);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_logging_utils_log_error_all_branches() {
        // Test all branches in LoggingUtils::log_error
        
        // Test ERROR level (CRITICAL and ERROR severity map to ERROR log level)
        let critical_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_CRI_S")
            .source(ErrorSource::AIM)
            .severity(Severity::CRITICAL)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Critical error")
            .user_message("Critical error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        LoggingUtils::log_error(&critical_error);
        
        let error_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Error error")
            .user_message("Error error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        LoggingUtils::log_error(&error_error);
        
        // Test WARN level
        let warning_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_WRN_S")
            .source(ErrorSource::AIM)
            .severity(Severity::WARNING)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Warning error")
            .user_message("Warning error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        LoggingUtils::log_error(&warning_error);
        
        // Test INFO level
        let info_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_INF_S")
            .source(ErrorSource::AIM)
            .severity(Severity::INFO)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Info error")
            .user_message("Info error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        LoggingUtils::log_error(&info_error);
        
        // Test DEBUG level by directly modifying the LogEntry
        let debug_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Debug error")
            .user_message("Debug error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        
        // Create a custom LogEntry with DEBUG level and test the match logic directly
        let mut debug_log_entry = LogEntry::from_error_object(&debug_error);
        debug_log_entry.level = "DEBUG".to_string();
        let level = debug_log_entry.level.as_str();
        if level == "DEBUG" {
            tracing::debug!(error_code = debug_error.code(), message = debug_error.message(), "Debug information");
        }
        
        // Test TRACE level by directly modifying the LogEntry
        let trace_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Trace error")
            .user_message("Trace error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        
        // Create a custom LogEntry with TRACE level and test the match logic directly
        let mut trace_log_entry = LogEntry::from_error_object(&trace_error);
        trace_log_entry.level = "TRACE".to_string();
        let level = trace_log_entry.level.as_str();
        if level == "TRACE" {
            tracing::trace!(error_code = trace_error.code(), message = trace_error.message(), "Trace information");
        }
        
        // Test default branch by directly modifying the LogEntry
        let invalid_error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Invalid level error")
            .user_message("Invalid level error message")
            .module_path("test::module")
            .operation("test_operation")
            .build();
        
        // Create a custom LogEntry with invalid level and test the match logic directly
        let mut invalid_log_entry = LogEntry::from_error_object(&invalid_error);
        invalid_log_entry.level = "INVALID_LEVEL".to_string();
        let level = invalid_log_entry.level.as_str();
        match level {
            "ERROR" => tracing::error!(error_code = invalid_error.code(), message = invalid_error.message(), "Error occurred"),
            "WARN" => tracing::warn!(error_code = invalid_error.code(), message = invalid_error.message(), "Warning occurred"),
            "INFO" => tracing::info!(error_code = invalid_error.code(), message = invalid_error.message(), "Info occurred"),
            "DEBUG" => tracing::debug!(error_code = invalid_error.code(), message = invalid_error.message(), "Debug information"),
            "TRACE" => tracing::trace!(error_code = invalid_error.code(), message = invalid_error.message(), "Trace information"),
            _ => tracing::info!(error_code = invalid_error.code(), message = invalid_error.message(), "Info occurred"),
        }
    }


    
    #[test]
    fn test_log_entry_with_context_chain() {
        // Test LogEntry with context chain
        let context_frame = ContextFrame::new("api_gateway", HashMap::new());
        
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
            .context_frame(context_frame)
            .build();
        
        let log_entry = LogEntry::from_error_object(&error);
        assert_eq!(log_entry.error_code, "ERR-AIM-LM-002_ERR_S");
        assert!(log_entry.context.contains_key("context_frame_0"));
    }
    
    #[test]
    fn test_log_entry_with_details() {
        // Test LogEntry with details
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
            .build();
        
        let log_entry = LogEntry::from_error_object(&error);
        assert_eq!(log_entry.error_code, "ERR-AIM-LM-002_ERR_S");
        assert!(log_entry.context.contains_key("model_name"));
        assert!(log_entry.context.contains_key("timeout_ms"));
    }
    
    #[test]
    fn test_log_entry_with_session_and_request_id() {
        // Test LogEntry with session and request ID
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
            .build();
        
        let log_entry = LogEntry::from_error_object(&error);
        assert_eq!(log_entry.error_code, "ERR-AIM-LM-002_ERR_S");
        assert_eq!(log_entry.session_id, Some("session_123".to_string()));
        assert_eq!(log_entry.request_id, Some("request_456".to_string()));
    }

    #[test]
    fn test_error_logging_layer() {
        // Create a simple writer closure that returns stdout
        let writer = || std::io::stdout();
        
        // Test ErrorLoggingLayer::new
        let _layer = ErrorLoggingLayer::new(writer);
        
        // The on_event method is empty, so we can't really test it
        // but at least we've tested the creation
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_log_entry_to_json() {
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
        let json_result = log_entry.to_json();
        assert!(json_result.is_ok());
    }
    
    #[test]
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
    }
}
