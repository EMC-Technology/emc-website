//! Utility functions
//!
//! This module provides utility functions for error handling and related operations.

use crate::error_object::ErrorObject;
use std::error::Error;

/// Utility functions for error handling
pub struct ErrorUtils;

impl ErrorUtils {
    /// Convert a standard error to an `ErrorObject`
    pub fn from_std_error(error: &dyn Error, source: &str, operation: &str) -> ErrorObject {
        let message = format!("{source}: {error}");
        ErrorObject::builder()
            .code(crate::error_code::registry::GENERAL_FALLBACK)
            .source(crate::classification::ErrorSource::INT)
            .severity(crate::classification::Severity::ERROR)
            .impact_scope(crate::classification::ImpactScope::OPERATION)
            .recoverability(crate::classification::Recoverability::ManualIntervention)
            .message(&message)
            .user_message("An internal error occurred")
            .module_path(source)
            .operation(operation)
            .build()
    }

    /// Extract the root cause of an error
    pub fn get_root_cause(error: &dyn Error) -> &dyn Error {
        let mut current = error;
        while let Some(cause) = current.source() {
            current = cause;
        }
        current
    }

    /// Format an error chain as a string
    pub fn format_error_chain(error: &dyn Error) -> String {
        let mut chain = Vec::new();
        let mut current = error;

        while let Some(cause) = current.source() {
            chain.push(current.to_string());
            current = cause;
        }
        chain.push(current.to_string());

        chain.join(" -> ")
    }
}

/// String utility functions
pub struct StringUtils;

impl StringUtils {
    /// Truncate a string to a maximum length
    #[must_use]
    pub fn truncate(s: &str, max_len: usize) -> String {
        if s.chars().count() <= max_len {
            s.to_string()
        } else if max_len <= 3 {
            "...".to_string()
        } else {
            let truncated: String = s.chars().take(max_len - 3).collect();
            format!("{truncated}...")
        }
    }

    /// Sanitize a string for logging
    #[must_use]
    pub fn sanitize_for_logging(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        for ch in s.chars() {
            match ch {
                '\n' | '\r' | '\t' => result.push(' '),
                c if c.is_control() => result.push(' '),
                '\\' => {
                    if result.ends_with("\\x1b") || result.ends_with("\\033") {
                        result.push(' ');
                    } else {
                        result.push(ch);
                    }
                }
                c => result.push(c),
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_utils_from_std_error() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;

        let error = TestError;
        let error_object = ErrorUtils::from_std_error(&error, "test::module", "test_operation");

        assert_eq!(error_object.code(), "ERR-INT-GEN-001_ERR_O");
        assert_eq!(error_object.module_path(), "test::module");
        assert_eq!(error_object.operation(), "test_operation");
    }

    #[test]
    fn test_string_utils_truncate() {
        let long_string = "This is a very long string that needs to be truncated";
        let truncated = StringUtils::truncate(long_string, 20);
        assert_eq!(truncated, "This is a very lo...");

        let short_string = "Short";
        let not_truncated = StringUtils::truncate(short_string, 10);
        assert_eq!(not_truncated, "Short");
    }

    #[test]
    fn test_error_utils_get_root_cause() {
        #[derive(Debug, thiserror::Error)]
        #[error("Root error")]
        struct RootError;

        #[derive(Debug, thiserror::Error)]
        #[error("Middle error")]
        struct MiddleError(#[from] RootError);

        #[derive(Debug, thiserror::Error)]
        #[error("Top error")]
        struct TopError(#[from] MiddleError);

        let error = TopError(MiddleError(RootError));
        let root_cause = ErrorUtils::get_root_cause(&error);
        assert_eq!(root_cause.to_string(), "Root error");
    }

    #[test]
    fn test_error_utils_format_error_chain() {
        #[derive(Debug, thiserror::Error)]
        #[error("Root error")]
        struct RootError;

        #[derive(Debug, thiserror::Error)]
        #[error("Middle error")]
        struct MiddleError(#[from] RootError);

        #[derive(Debug, thiserror::Error)]
        #[error("Top error")]
        struct TopError(#[from] MiddleError);

        let error = TopError(MiddleError(RootError));
        let chain = ErrorUtils::format_error_chain(&error);
        assert_eq!(chain, "Top error -> Middle error -> Root error");
    }

    #[test]
    fn test_string_utils_sanitize_for_logging() {
        let input = "This is a\nstring with\ttabs and\nnewlines";
        let sanitized = StringUtils::sanitize_for_logging(input);
        assert_eq!(sanitized, "This is a string with tabs and newlines");
    }
}
