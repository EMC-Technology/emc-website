//! Utils module formal verification tests using Kani

use error_core::utils::*;

// Define test error types
#[derive(Debug, thiserror::Error)]
#[error("Root error")]
struct RootError;

#[derive(Debug, thiserror::Error)]
#[error("Middle error")]
struct MiddleError(#[from] RootError);

#[derive(Debug, thiserror::Error)]
#[error("Top error")]
struct TopError(#[from] MiddleError);

#[derive(Debug, thiserror::Error)]
#[error("Test error")]
struct TestError;

// Test ErrorUtils::from_std_error
#[kani::proof]
fn test_error_utils_from_std_error() {
    let error = TestError;
    let error_object = ErrorUtils::from_std_error(&error, "test::module", "test_operation");
    
    assert_eq!(error_object.code(), "ERR-INT-GEN-001_ERR_M");
    assert_eq!(error_object.module_path(), "test::module");
    assert_eq!(error_object.operation(), "test_operation");
    assert!(error_object.message().contains("test::module: Test error"));
}

// Test ErrorUtils::get_root_cause
#[kani::proof]
fn test_error_utils_get_root_cause() {
    let error = TopError(MiddleError(RootError));
    let root_cause = ErrorUtils::get_root_cause(&error);
    assert_eq!(root_cause.to_string(), "Root error");
}

// Test ErrorUtils::format_error_chain
#[kani::proof]
fn test_error_utils_format_error_chain() {
    let error = TopError(MiddleError(RootError));
    let chain = ErrorUtils::format_error_chain(&error);
    assert_eq!(chain, "Top error -> Middle error -> Root error");
}

// Test StringUtils::truncate
#[kani::proof]
fn test_string_utils_truncate() {
    let long_string = "This is a very long string that needs to be truncated";
    let truncated = StringUtils::truncate(long_string, 20);
    assert_eq!(truncated, "This is a very lo...");

    let short_string = "Short";
    let not_truncated = StringUtils::truncate(short_string, 10);
    assert_eq!(not_truncated, "Short");

    // Test edge cases
    let empty_string = "";
    let empty_truncated = StringUtils::truncate(empty_string, 5);
    assert_eq!(empty_truncated, "");

    let exact_length = "Exactly 10 chars";
    let exact_truncated = StringUtils::truncate(exact_length, 14);
    assert_eq!(exact_truncated, "Exactly 10 chars");

    // Test max_len <= 3
    let test_string = "Test";
    let truncated_short = StringUtils::truncate(test_string, 3);
    assert_eq!(truncated_short, "...");
}

// Test StringUtils::sanitize_for_logging
#[kani::proof]
fn test_string_utils_sanitize_for_logging() {
    let input = "This is a\nstring with\ttabs and\nnewlines";
    let sanitized = StringUtils::sanitize_for_logging(input);
    assert_eq!(sanitized, "This is a string with tabs and newlines");

    // Test with no special characters
    let input_no_special = "Normal string";
    let sanitized_no_special = StringUtils::sanitize_for_logging(input_no_special);
    assert_eq!(sanitized_no_special, "Normal string");

    // Test with only special characters
    let input_only_special = "\n\t\n";
    let sanitized_only_special = StringUtils::sanitize_for_logging(input_only_special);
    assert_eq!(sanitized_only_special, "   ");
}
