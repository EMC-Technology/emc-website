//! Error code lifecycle tests
//!
//! This module tests the full lifecycle of error codes, including edge cases and boundary values.
#![allow(clippy::uninlined_format_args)]

use error_core::error_code::ErrorCode;
use error_core::classification::{ErrorSource, Severity, ImpactScope};

#[test]
fn test_error_code_lifecycle_full() {
    // Test with all combinations of error sources, severities, and impact scopes
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
                        let result = ErrorCode::new(*source, module, *sequence, *severity, *impact_scope);
                        assert!(result.is_ok(), "Failed to create error code for source={:?}, module={}, sequence={}, severity={:?}, impact_scope={:?}", 
                                source, module, sequence, severity, impact_scope);
                        
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
    // Test sequence number boundaries
    let max_sequence = 999;
    let result = ErrorCode::new(ErrorSource::AIM, "LM", max_sequence, Severity::ERROR, ImpactScope::SESSION);
    assert!(result.is_ok());
    
    let error_code = result.unwrap();
    assert_eq!(error_code.sequence(), max_sequence);
    assert!(error_code.code().contains("999"));
    
    // Test module name edge cases
    // Minimum length (2 characters)
    let result = ErrorCode::new(ErrorSource::AIM, "XY", 1, Severity::ERROR, ImpactScope::SESSION);
    assert!(result.is_ok());
    
    // Maximum length (5 characters)
    let result = ErrorCode::new(ErrorSource::AIM, "ABCDE", 1, Severity::ERROR, ImpactScope::SESSION);
    assert!(result.is_ok());
    
    // Test invalid module names
    // Too short (1 character)
    let result = ErrorCode::new(ErrorSource::AIM, "X", 1, Severity::ERROR, ImpactScope::SESSION);
    assert!(result.is_err());
    
    // Too long (6 characters)
    let result = ErrorCode::new(ErrorSource::AIM, "ABCDEF", 1, Severity::ERROR, ImpactScope::SESSION);
    assert!(result.is_err());
}

#[test]
fn test_error_code_parse_edge_cases() {
    // Test all valid formats
    let valid_codes = [
        "ERR-AIM-LM-000_CRI_G", // Minimum sequence, critical severity, global scope
        "ERR-AIM-LM-999_INF_O", // Maximum sequence, info severity, operation scope
        "ERR-EXT-ABCDE-500_ERR_S", // 5-letter module
        "ERR-INT-XY-250_WRN_M", // 2-letter module
    ];
    
    for code in &valid_codes {
        let result = ErrorCode::parse(code);
        assert!(result.is_ok(), "Failed to parse valid code: {}", code);
    }
    
    // Test invalid formats
    let invalid_codes = [
        "ERR-AIM-LM-1000_ERR_S", // Sequence too long
        "ERR-AIM-LM-0_ERR_S", // Sequence too short (not 3 digits)
        "ERR-AIM-L-001_ERR_S", // Module too short
        "ERR-AIM-ABCDEF-001_ERR_S", // Module too long (6 chars)
        "ERR-A-001_ERR_S", // Source too short
        "ERR-ABCDE-001_ERR_S", // Source too long
        "ERR-AIM-LM-001_XXX_S", // Invalid severity
        "ERR-AIM-LM-001_ERR_X", // Invalid impact scope
        "err-aim-lm-001_err_s", // Lowercase
        "ERR AIM LM 001 ERR S", // Spaces instead of hyphens/underscores
    ];
    
    for code in &invalid_codes {
        let result = ErrorCode::parse(code);
        assert!(result.is_err(), "Should have failed to parse invalid code: {}", code);
    }
}

#[test]
fn test_error_code_equality() {
    // Test equality of error codes
    let code1 = ErrorCode::new(ErrorSource::AIM, "LM", 1, Severity::ERROR, ImpactScope::SESSION).unwrap();
    let code2 = ErrorCode::new(ErrorSource::AIM, "LM", 1, Severity::ERROR, ImpactScope::SESSION).unwrap();
    let code3 = ErrorCode::new(ErrorSource::EXT, "LM", 1, Severity::ERROR, ImpactScope::SESSION).unwrap();
    
    assert_eq!(code1, code2);
    assert_ne!(code1, code3);
    
    // Test cloning
    let code4 = code1.clone();
    assert_eq!(code1, code4);
}

#[test]
fn test_error_code_hash() {
    // Test that error codes can be used as keys in a hash map
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
    // Test debug formatting
    let code = ErrorCode::new(ErrorSource::AIM, "LM", 1, Severity::ERROR, ImpactScope::SESSION).unwrap();
    let debug_str = format!("{:?}", code);
    assert!(debug_str.contains("ErrorCode"));
    assert!(debug_str.contains("ERR-AIM-LM-001_ERR_S"));
}
