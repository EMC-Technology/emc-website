//! Utils fuzz tests
//! 
//! This module contains fuzz tests for the utils module to ensure coverage of all possible cases.

use error_core::utils::{ErrorUtils, StringUtils};
use std::error::Error;

#[test]
fn test_error_utils_from_std_error() {
    // Test ErrorUtils::from_std_error with various error types
    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;
    
    #[derive(Debug, thiserror::Error)]
    #[error("Empty error")]
    struct EmptyError;
    
    #[derive(Debug, thiserror::Error)]
    #[error("Error with long message: {0}")]
    struct LongError(String);
    
    let long_error = LongError("a".repeat(1000));
    let long_source = "a".repeat(1000);
    let long_operation = "b".repeat(1000);
    
    let test_cases = vec![
        ("simple_error", &TestError as &dyn Error, "test::module", "test_operation"),
        ("empty_error", &EmptyError as &dyn Error, "", ""),
        ("long_error", &long_error as &dyn Error, &long_source, &long_operation),
    ];
    
    for (_name, error, source, operation) in test_cases {
        let error_object = ErrorUtils::from_std_error(error, source, operation);
        assert_eq!(error_object.code(), "ERR-INT-GEN-001_ERR_M");
        assert_eq!(error_object.module_path(), source);
        assert_eq!(error_object.operation(), operation);
    }
}

#[test]
fn test_error_utils_get_root_cause() {
    // Test ErrorUtils::get_root_cause with various error chains
    #[derive(Debug, thiserror::Error)]
    #[error("Root error")]
    struct RootError;
    
    #[derive(Debug, thiserror::Error)]
    #[error("Middle error")]
    struct MiddleError(#[from] RootError);
    
    #[derive(Debug, thiserror::Error)]
    #[error("Top error")]
    struct TopError(#[from] MiddleError);
    
    // Test with single error
    let root_error = RootError;
    let root_cause = ErrorUtils::get_root_cause(&root_error);
    assert_eq!(root_cause.to_string(), "Root error");
    
    // Test with two-level error chain
    let middle_error = MiddleError(RootError);
    let root_cause = ErrorUtils::get_root_cause(&middle_error);
    assert_eq!(root_cause.to_string(), "Root error");
    
    // Test with three-level error chain
    let top_error = TopError(MiddleError(RootError));
    let root_cause = ErrorUtils::get_root_cause(&top_error);
    assert_eq!(root_cause.to_string(), "Root error");
}

#[test]
fn test_error_utils_format_error_chain() {
    // Test ErrorUtils::format_error_chain with various error chains
    #[derive(Debug, thiserror::Error)]
    #[error("Root error")]
    struct RootError;
    
    #[derive(Debug, thiserror::Error)]
    #[error("Middle error")]
    struct MiddleError(#[from] RootError);
    
    #[derive(Debug, thiserror::Error)]
    #[error("Top error")]
    struct TopError(#[from] MiddleError);
    
    // Test with single error
    let root_error = RootError;
    let chain = ErrorUtils::format_error_chain(&root_error);
    assert_eq!(chain, "Root error");
    
    // Test with two-level error chain
    let middle_error = MiddleError(RootError);
    let chain = ErrorUtils::format_error_chain(&middle_error);
    assert_eq!(chain, "Middle error -> Root error");
    
    // Test with three-level error chain
    let top_error = TopError(MiddleError(RootError));
    let chain = ErrorUtils::format_error_chain(&top_error);
    assert_eq!(chain, "Top error -> Middle error -> Root error");
}

#[test]
fn test_string_utils_truncate() {
    // Test StringUtils::truncate with various inputs
    let very_long_string = "a".repeat(1000);
    let expected_very_long = format!("{}{}", &"a".repeat(47), "...");
    
    let test_cases = vec![
        ("empty_string", "", 0, ""),
        ("empty_string_with_max", "", 10, ""),
        ("short_string", "Short", 10, "Short"),
        ("exact_length", "Exactly 10 chars", 10, "Exactly..."),
        ("long_string", "This is a very long string", 20, "This is a very lo..."),
        ("min_length", "Test", 3, "..."),
        ("very_long_string", &very_long_string, 50, &expected_very_long),
    ];
    
    for (_name, input, max_len, expected) in test_cases {
        let result = StringUtils::truncate(input, max_len);
        assert_eq!(result, expected);
    }
}

#[test]
fn test_string_utils_truncate_edge_cases() {
    // Test edge cases for StringUtils::truncate
    
    // Test with max_len = 0
    let result = StringUtils::truncate("Test", 0);
    assert_eq!(result, "...");
    
    // Test with max_len = 1
    let result = StringUtils::truncate("Test", 1);
    assert_eq!(result, "...");
    
    // Test with max_len = 2
    let result = StringUtils::truncate("Test", 2);
    assert_eq!(result, "...");
    
    // Test with max_len = 3
    let result = StringUtils::truncate("Test", 3);
    assert_eq!(result, "...");
    
    // Test with max_len = 4
    let result = StringUtils::truncate("Test", 4);
    assert_eq!(result, "Test");
    
    // Test with max_len = 5
    let result = StringUtils::truncate("Test", 5);
    assert_eq!(result, "Test");
}

#[test]
fn test_string_utils_sanitize_for_logging() {
    // Test StringUtils::sanitize_for_logging with various inputs
    let long_string_input = "a\nb\nc\td\te".repeat(100);
    let long_string_expected = "a b c d e".repeat(100);
    
    let test_cases = vec![
        ("empty_string", "", ""),
        ("simple_string", "Hello world", "Hello world"),
        ("string_with_newlines", "Hello\nworld\n", "Hello world "),
        ("string_with_tabs", "Hello\tworld\t", "Hello world "),
        ("string_with_mixed_whitespace", "Hello\n\tworld\t\n", "Hello  world  "),
        ("long_string", &long_string_input, &long_string_expected),
    ];
    
    for (_name, input, expected) in test_cases {
        let result = StringUtils::sanitize_for_logging(input);
        assert_eq!(result, expected);
    }
}

#[test]
fn test_string_utils_sanitize_for_logging_edge_cases() {
    // Test edge cases for StringUtils::sanitize_for_logging
    
    // Test with only newlines
    let result = StringUtils::sanitize_for_logging("\n\n\n");
    assert_eq!(result, "   ");
    
    // Test with only tabs
    let result = StringUtils::sanitize_for_logging("\t\t\t");
    assert_eq!(result, "   ");
    
    // Test with mixed whitespace at beginning and end
    let result = StringUtils::sanitize_for_logging("\n\tHello world\t\n");
    assert_eq!(result, "  Hello world  ");
}

#[test]
fn test_utils_all_combinations() {
    // Test all combinations of utility functions
    
    // Test ErrorUtils with different error types
    #[derive(Debug, thiserror::Error)]
    #[error("Test error {0}")]
    struct TestError(i32);
    
    let error = TestError(42);
    
    // Test from_std_error
    let error_object = ErrorUtils::from_std_error(&error, "test::module", "test_operation");
    assert_eq!(error_object.code(), "ERR-INT-GEN-001_ERR_M");
    
    // Test get_root_cause
    let root_cause = ErrorUtils::get_root_cause(&error);
    assert_eq!(root_cause.to_string(), "Test error 42");
    
    // Test format_error_chain
    let chain = ErrorUtils::format_error_chain(&error);
    assert_eq!(chain, "Test error 42");
    
    // Test StringUtils with different inputs
    let test_string = "Hello\nworld\t";
    
    // Test truncate
    let truncated = StringUtils::truncate(test_string, 10);
    assert_eq!(truncated, "Hello\nw...");
    
    // Test sanitize_for_logging
    let sanitized = StringUtils::sanitize_for_logging(test_string);
    assert_eq!(sanitized, "Hello world ");
}

#[test]
fn test_utils_error_chain_with_multiple_levels() {
    // Test error chain with multiple levels
    #[derive(Debug, thiserror::Error)]
    #[error("Level 4 error")]
    struct Level4Error;
    
    #[derive(Debug, thiserror::Error)]
    #[error("Level 3 error")]
    struct Level3Error(#[from] Level4Error);
    
    #[derive(Debug, thiserror::Error)]
    #[error("Level 2 error")]
    struct Level2Error(#[from] Level3Error);
    
    #[derive(Debug, thiserror::Error)]
    #[error("Level 1 error")]
    struct Level1Error(#[from] Level2Error);
    
    let error = Level1Error(Level2Error(Level3Error(Level4Error)));
    
    // Test get_root_cause
    let root_cause = ErrorUtils::get_root_cause(&error);
    assert_eq!(root_cause.to_string(), "Level 4 error");
    
    // Test format_error_chain
    let chain = ErrorUtils::format_error_chain(&error);
    assert_eq!(chain, "Level 1 error -> Level 2 error -> Level 3 error -> Level 4 error");
}
