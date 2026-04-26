//! Utils property tests
//!
//! This module contains property tests for the utils module to ensure coverage of all possible cases.
#![allow(clippy::single_char_pattern)]

use error_core::utils::{ErrorUtils, StringUtils};

#[test]
fn test_error_utils_from_std_error() {
    // Test ErrorUtils::from_std_error with different error types
    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;
    
    #[derive(Debug, thiserror::Error)]
    #[error("Another error")]
    struct AnotherError;
    
    let sources = ["test::module", "another::module"];
    let operations = ["test_operation", "another_operation"];
    
    // Test with TestError
    let test_error = TestError;
    for source in &sources {
        for operation in &operations {
            let error_object = ErrorUtils::from_std_error(&test_error, source, operation);
            assert_eq!(error_object.code(), "ERR-INT-GEN-001_ERR_O");
            assert_eq!(error_object.module_path(), *source);
            assert_eq!(error_object.operation(), *operation);
            assert!(error_object.message().contains(source));
        }
    }
    
    // Test with AnotherError
    let another_error = AnotherError;
    for source in &sources {
        for operation in &operations {
            let error_object = ErrorUtils::from_std_error(&another_error, source, operation);
            assert_eq!(error_object.code(), "ERR-INT-GEN-001_ERR_O");
            assert_eq!(error_object.module_path(), *source);
            assert_eq!(error_object.operation(), *operation);
            assert!(error_object.message().contains(source));
        }
    }
}

#[test]
fn test_error_utils_get_root_cause() {
    // Test ErrorUtils::get_root_cause with different error chains
    #[derive(Debug, thiserror::Error)]
    #[error("Root error")]
    struct RootError;

    #[derive(Debug, thiserror::Error)]
    #[error("Middle error")]
    struct MiddleError(#[from] RootError);

    #[derive(Debug, thiserror::Error)]
    #[error("Top error")]
    struct TopError(#[from] MiddleError);

    // Test with single error (no cause)
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
    // Test ErrorUtils::format_error_chain with different error chains
    #[derive(Debug, thiserror::Error)]
    #[error("Root error")]
    struct RootError;

    #[derive(Debug, thiserror::Error)]
    #[error("Middle error")]
    struct MiddleError(#[from] RootError);

    #[derive(Debug, thiserror::Error)]
    #[error("Top error")]
    struct TopError(#[from] MiddleError);

    // Test with single error (no cause)
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
    // Test StringUtils::truncate with different lengths
    let test_string = "This is a test string";
    
    // Test with length greater than string length
    let result1 = StringUtils::truncate(test_string, 100);
    assert_eq!(result1, test_string);
    
    // Test with length equal to string length
    let result2 = StringUtils::truncate(test_string, test_string.len());
    assert_eq!(result2, test_string);
    
    // Test with length less than string length
    let result3 = StringUtils::truncate(test_string, 10);
    assert_eq!(result3, "This is...");
    
    // Test with length less than 3 (minimum for ellipsis)
    let result4 = StringUtils::truncate(test_string, 2);
    assert_eq!(result4, "...");
    
    // Test with empty string
    let result5 = StringUtils::truncate("", 10);
    assert_eq!(result5, "");
}

#[test]
fn test_string_utils_sanitize_for_logging() {
    // Test StringUtils::sanitize_for_logging with different inputs
    let test_cases = [
        ("", ""),
        ("No special characters", "No special characters"),
        ("Line 1\nLine 2", "Line 1 Line 2"),
        ("Tab\tseparated", "Tab separated"),
        ("Mixed\nwith\ttabs", "Mixed with tabs"),
        ("Multiple\n\n\nnewlines", "Multiple   newlines"),
        ("Multiple\t\ttabs", "Multiple  tabs"),
    ];
    
    for (input, expected) in test_cases {
        let result = StringUtils::sanitize_for_logging(input);
        assert_eq!(result, expected);
    }
}

#[test]
fn test_string_utils_edge_cases() {
    // Test StringUtils with edge cases
    
    // Test truncate with very large max_len
    let long_string = "a".repeat(1000);
    let result = StringUtils::truncate(&long_string, 10000);
    assert_eq!(result, long_string);
    
    // Test truncate with max_len = 0
    let result = StringUtils::truncate("Test", 0);
    assert_eq!(result, "...");
    
    // Test truncate with max_len = 3
    let result = StringUtils::truncate("Test", 3);
    assert_eq!(result, "...");
    
    // Test sanitize_for_logging with empty string
    let result = StringUtils::sanitize_for_logging("");
    assert_eq!(result, "");
    
    // Test sanitize_for_logging with only special characters
    let result = StringUtils::sanitize_for_logging("\n\t\n\t");
    assert_eq!(result, "    ");
}

#[test]
fn test_error_utils_with_real_errors() {
    // Test ErrorUtils with real standard library errors
    use std::fs::File;
    
    // Test with file not found error
    let error = File::open("non_existent_file.txt").unwrap_err();
    let error_object = ErrorUtils::from_std_error(&error, "fs::file", "open");
    assert_eq!(error_object.code(), "ERR-INT-GEN-001_ERR_O");
    assert_eq!(error_object.module_path(), "fs::file");
    assert_eq!(error_object.operation(), "open");
    assert!(error_object.message().contains("fs::file"));
    
    // Test get_root_cause with real error
    let root_cause = ErrorUtils::get_root_cause(&error);
    assert!(!root_cause.to_string().is_empty());
    
    // Test format_error_chain with real error
    let chain = ErrorUtils::format_error_chain(&error);
    assert!(!chain.is_empty());
}

#[test]
fn test_string_utils_performance() {
    // Test StringUtils performance with large strings
    let large_string = "a".repeat(10000);
    
    // Test truncate
    let result = StringUtils::truncate(&large_string, 100);
    assert_eq!(result.len(), 100);
    assert!(result.ends_with("..."));
    
    // Test sanitize_for_logging
    let large_string_with_newlines = large_string.replace("a", "a\n");
    let result = StringUtils::sanitize_for_logging(&large_string_with_newlines);
    assert!(!result.contains('\n'));
}