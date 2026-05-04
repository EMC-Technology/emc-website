//! Error code lifecycle tests
//!
//! This module tests the full lifecycle of error codes, including edge cases and boundary values.
#![allow(clippy::uninlined_format_args)]

use error_core::error_code::ErrorCode;
use error_core::classification::{ErrorSource, Severity, ImpactScope};

#[test]
fn test_error_code_lifecycle_full() {
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

    let sequences = [0, 1, 999];

    let modules = ["LM", "ABCD", "XY"];

    for source in &sources {
        for severity in &severities {
            for impact_scope in &impact_scopes {
                for sequence in &sequences {
                    for module in &modules {
                        let result = ErrorCode::new(*source, module, *sequence, *severity, *impact_scope);
                        assert!(result.is_ok(), "Failed to create error code for source={:?}, module={}, sequence={}, severity={:?}, impact_scope={:?}",
                                source, module, sequence, severity, impact_scope);

                        let error_code = result.unwrap();

                        assert!(!error_code.code().is_empty());
                        assert_eq!(error_code.source(), *source);
                        assert_eq!(error_code.module(), *module);
                        assert_eq!(error_code.sequence(), *sequence);
                        assert_eq!(error_code.severity(), *severity);
                        assert_eq!(error_code.impact_scope(), *impact_scope);

                        let parse_result = ErrorCode::parse(error_code.code());
                        assert!(parse_result.is_ok(), "Failed to parse generated error code: {}", error_code.code());

                        let parsed_code = parse_result.unwrap();
                        assert_eq!(parsed_code, error_code);
                    }
                }
            }
        }
    }
}

#[test]
fn test_error_code_edge_cases() {
    let max_sequence = 999;
    let result = ErrorCode::new(ErrorSource::AIM, "LM", max_sequence, Severity::ERROR, ImpactScope::SESSION);
    assert!(result.is_ok());

    let error_code = result.unwrap();
    assert_eq!(error_code.sequence(), max_sequence);
    assert!(error_code.code().contains("999"));

    let result = ErrorCode::new(ErrorSource::AIM, "XY", 1, Severity::ERROR, ImpactScope::SESSION);
    assert!(result.is_ok());

    let result = ErrorCode::new(ErrorSource::AIM, "ABCDE", 1, Severity::ERROR, ImpactScope::SESSION);
    assert!(result.is_ok());

    #[cfg(feature = "regex")]
    {
        let result = ErrorCode::new(ErrorSource::AIM, "X", 1, Severity::ERROR, ImpactScope::SESSION);
        assert!(result.is_err());

        let result = ErrorCode::new(ErrorSource::AIM, "ABCDEF", 1, Severity::ERROR, ImpactScope::SESSION);
        assert!(result.is_err());
    }
}

#[test]
fn test_error_code_parse_edge_cases() {
    let valid_codes = [
        "ERR-AIM-LM-000_CRI_G",
        "ERR-AIM-LM-999_INF_O",
        "ERR-EXT-ABCDE-500_ERR_S",
        "ERR-INT-XY-250_WRN_M",
    ];

    for code in &valid_codes {
        let result = ErrorCode::parse(code);
        assert!(result.is_ok(), "Failed to parse valid code: {}", code);
    }

    let common_invalid_codes = [
        "ERR-AIM-LM-001_XXX_S",
        "ERR-AIM-LM-001_ERR_X",
        "err-aim-lm-001_err_s",
        "ERR AIM LM 001 ERR S",
    ];

    for code in &common_invalid_codes {
        let result = ErrorCode::parse(code);
        assert!(result.is_err(), "Should have failed to parse invalid code: {}", code);
    }

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
            assert!(result.is_err(), "Should have failed to parse invalid code: {}", code);
        }
    }
}

#[test]
fn test_error_code_equality() {
    let code1 = ErrorCode::new(ErrorSource::AIM, "LM", 1, Severity::ERROR, ImpactScope::SESSION).unwrap();
    let code2 = ErrorCode::new(ErrorSource::AIM, "LM", 1, Severity::ERROR, ImpactScope::SESSION).unwrap();
    let code3 = ErrorCode::new(ErrorSource::EXT, "LM", 1, Severity::ERROR, ImpactScope::SESSION).unwrap();

    assert_eq!(code1, code2);
    assert_ne!(code1, code3);

    let code4 = code1.clone();
    assert_eq!(code1, code4);
}

#[test]
fn test_error_code_hash() {
    use std::collections::HashMap;

    let code1 = ErrorCode::new(ErrorSource::AIM, "LM", 1, Severity::ERROR, ImpactScope::SESSION).unwrap();
    let code2 = ErrorCode::new(ErrorSource::EXT, "LM", 2, Severity::WARNING, ImpactScope::MODULE).unwrap();

    let mut map = HashMap::new();
    map.insert(code1.clone(), "value1");
    map.insert(code2.clone(), "value2");

    assert_eq!(map.get(&code1), Some(&"value1"));
    assert_eq!(map.get(&code2), Some(&"value2"));
}

#[test]
fn test_error_code_debug() {
    let code = ErrorCode::new(ErrorSource::AIM, "LM", 1, Severity::ERROR, ImpactScope::SESSION).unwrap();
    let debug_str = format!("{:?}", code);
    assert!(debug_str.contains("ErrorCode"));
    assert!(debug_str.contains("ERR-AIM-LM-001_ERR_S"));
}
