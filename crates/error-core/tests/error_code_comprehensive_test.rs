//! Error code comprehensive lifecycle tests
//!
//! This module tests the complete lifecycle of error codes, covering all edge cases and boundary values.
#![allow(clippy::uninlined_format_args)]

use error_core::classification::{ErrorSource, ImpactScope, Severity};
use error_core::error_code::ErrorCode;

#[test]
fn test_error_code_complete_lifecycle() {
    // Test all possible combinations of error sources, severities, and impact scopes
    let sources = [
        ErrorSource::AIM,
        ErrorSource::EXT,
        ErrorSource::INT,
        ErrorSource::NET,
        ErrorSource::SYS,
        ErrorSource::USR,
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

    // Test sequence number boundaries
    let sequences = [0, 1, 999];

    // Test module name boundaries
    let modules = ["LM", "ABCD", "XY"];

    // Test all combinations
    for source in &sources {
        for severity in &severities {
            for impact_scope in &impact_scopes {
                for sequence in &sequences {
                    for module in &modules {
                        // Create error code
                        let result =
                            ErrorCode::new(*source, module, *sequence, *severity, *impact_scope);
                        assert!(
                            result.is_ok(),
                            "Failed to create error code for source={:?}, module={}, sequence={}, severity={:?}, impact_scope={:?}",
                            source,
                            module,
                            sequence,
                            severity,
                            impact_scope
                        );

                        let error_code = result.unwrap();

                        // Test all getters
                        assert!(!error_code.code().is_empty());
                        assert_eq!(error_code.source(), *source);
                        assert_eq!(error_code.module(), *module);
                        assert_eq!(error_code.sequence(), *sequence);
                        assert_eq!(error_code.severity(), *severity);
                        assert_eq!(error_code.impact_scope(), *impact_scope);

                        // Test parsing the generated code
                        let parse_result = ErrorCode::parse(error_code.code());
                        assert!(
                            parse_result.is_ok(),
                            "Failed to parse generated error code: {}",
                            error_code.code()
                        );

                        let parsed_code = parse_result.unwrap();
                        assert_eq!(parsed_code, error_code);

                        // Test equality
                        assert_eq!(error_code, error_code);

                        // Test cloning
                        let cloned_code = error_code.clone();
                        assert_eq!(cloned_code, error_code);

                        // Test debug formatting
                        let debug_str = format!("{:?}", error_code);
                        assert!(debug_str.contains("ErrorCode"));
                        assert!(debug_str.contains(error_code.code()));
                    }
                }
            }
        }
    }
}

#[test]
fn test_error_code_edge_cases() {
    // Test sequence number boundaries
    let max_sequence = 999;
    let result = ErrorCode::new(
        ErrorSource::AIM,
        "LM",
        max_sequence,
        Severity::ERROR,
        ImpactScope::SESSION,
    );
    assert!(result.is_ok());

    let error_code = result.unwrap();
    assert_eq!(error_code.sequence(), max_sequence);
    assert!(error_code.code().contains("999"));

    // Test module name edge cases
    // Minimum length (2 characters)
    let result = ErrorCode::new(
        ErrorSource::AIM,
        "XY",
        1,
        Severity::ERROR,
        ImpactScope::SESSION,
    );
    assert!(result.is_ok());

    // Maximum length (4 characters)
    let result = ErrorCode::new(
        ErrorSource::AIM,
        "ABCD",
        1,
        Severity::ERROR,
        ImpactScope::SESSION,
    );
    assert!(result.is_ok());

    // Test invalid module names
    #[cfg(feature = "regex")]
    {
        let result = ErrorCode::new(
            ErrorSource::AIM,
            "X",
            1,
            Severity::ERROR,
            ImpactScope::SESSION,
        );
        assert!(result.is_err());

        let result = ErrorCode::new(
            ErrorSource::AIM,
            "ABCDEF",
            1,
            Severity::ERROR,
            ImpactScope::SESSION,
        );
        assert!(result.is_err());
    }
}

#[test]
fn test_error_code_parse_edge_cases() {
    // Test all valid formats
    let valid_codes = [
        "ERR-AIM-LM-000_CRI_G",   // Minimum sequence, critical severity, global scope
        "ERR-AIM-LM-999_INF_O",   // Maximum sequence, info severity, operation scope
        "ERR-EXT-ABCD-500_ERR_S", // 4-letter module
        "ERR-INT-XY-250_WRN_M",   // 2-letter module
    ];

    for code in &valid_codes {
        let result = ErrorCode::parse(code);
        assert!(result.is_ok(), "Failed to parse valid code: {}", code);
    }

    // Test invalid formats (common to both regex and non-regex parsers)
    let invalid_codes = [
        "ERR-AIM-LM-001_XXX_S",
        "ERR-AIM-LM-001_ERR_X",
        "err-aim-lm-001_err_s",
        "ERR AIM LM 001 ERR S",
    ];

    for code in &invalid_codes {
        let result = ErrorCode::parse(code);
        assert!(
            result.is_err(),
            "Should have failed to parse invalid code: {}",
            code
        );
    }

    // Test invalid formats only caught by regex parser
    #[cfg(feature = "regex")]
    {
        let regex_only_invalid = [
            "ERR-AIM-LM-1000_ERR_S",
            "ERR-AIM-LM-0_ERR_S",
            "ERR-AIM-L-001_ERR_S",
            "ERR-AIM-ABCDEF-001_ERR_S",
            "ERR-A-001_ERR_S",
            "ERR-ABCDE-001_ERR_S",
        ];
        for code in &regex_only_invalid {
            let result = ErrorCode::parse(code);
            assert!(
                result.is_err(),
                "Should have failed to parse invalid code: {}",
                code
            );
        }
    }
}

#[test]
fn test_error_code_hash() {
    // Test that error codes can be used as keys in a hash map
    use std::collections::HashMap;

    let code1 = ErrorCode::new(
        ErrorSource::AIM,
        "LM",
        1,
        Severity::ERROR,
        ImpactScope::SESSION,
    )
    .unwrap();
    let code2 = ErrorCode::new(
        ErrorSource::EXT,
        "LM",
        2,
        Severity::WARNING,
        ImpactScope::MODULE,
    )
    .unwrap();

    let mut map = HashMap::new();
    map.insert(code1.clone(), "value1");
    map.insert(code2.clone(), "value2");

    assert_eq!(map.get(&code1), Some(&"value1"));
    assert_eq!(map.get(&code2), Some(&"value2"));
}

#[test]
fn test_error_code_equality() {
    // Test equality of error codes
    let code1 = ErrorCode::new(
        ErrorSource::AIM,
        "LM",
        1,
        Severity::ERROR,
        ImpactScope::SESSION,
    )
    .unwrap();
    let code2 = ErrorCode::new(
        ErrorSource::AIM,
        "LM",
        1,
        Severity::ERROR,
        ImpactScope::SESSION,
    )
    .unwrap();
    let code3 = ErrorCode::new(
        ErrorSource::EXT,
        "LM",
        1,
        Severity::ERROR,
        ImpactScope::SESSION,
    )
    .unwrap();

    assert_eq!(code1, code2);
    assert_ne!(code1, code3);

    // Test cloning
    let code4 = code1.clone();
    assert_eq!(code1, code4);
}
