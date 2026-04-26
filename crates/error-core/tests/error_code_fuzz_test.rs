//! Error code fuzz tests
//! 
//! This module contains fuzz tests for the error_code module to ensure coverage of all possible cases.

use error_core::error_code::ErrorCode;
use error_core::classification::{ErrorSource, Severity, ImpactScope};

#[test]
fn test_error_code_valid_cases() {
    // Test valid error code cases
    let valid_cases = vec![
        (ErrorSource::USR, "UI", 1, Severity::ERROR, ImpactScope::OPERATION, "ERR-USR-UI-001_ERR_O"),
        (ErrorSource::AIM, "LM", 2, Severity::ERROR, ImpactScope::SESSION, "ERR-AIM-LM-002_ERR_S"),
        (ErrorSource::FS, "IO", 3, Severity::WARNING, ImpactScope::MODULE, "ERR-FS-IO-003_WRN_M"),
        (ErrorSource::NET, "API", 4, Severity::CRITICAL, ImpactScope::GLOBAL, "ERR-NET-API-004_CRI_G"),
        (ErrorSource::CFG, "CONF", 5, Severity::INFO, ImpactScope::SESSION, "ERR-CFG-CONF-005_INF_S"),
        (ErrorSource::SEC, "AUTH", 6, Severity::ERROR, ImpactScope::GLOBAL, "ERR-SEC-AUTH-006_ERR_G"),
        (ErrorSource::TOOL, "CLI", 7, Severity::WARNING, ImpactScope::OPERATION, "ERR-TOOL-CLI-007_WRN_O"),
        (ErrorSource::SESS, "SESS", 8, Severity::ERROR, ImpactScope::SESSION, "ERR-SESS-SESS-008_ERR_S"),
        (ErrorSource::STATE, "STATE", 9, Severity::CRITICAL, ImpactScope::MODULE, "ERR-STATE-STATE-009_CRI_M"),
        (ErrorSource::EXT, "EXT", 10, Severity::INFO, ImpactScope::OPERATION, "ERR-EXT-EXT-010_INF_O"),
        (ErrorSource::LSP, "LSP", 11, Severity::ERROR, ImpactScope::GLOBAL, "ERR-LSP-LSP-011_ERR_G"),
        (ErrorSource::MCP, "MCP", 12, Severity::WARNING, ImpactScope::SESSION, "ERR-MCP-MCP-012_WRN_S"),
        (ErrorSource::SYS, "SYS", 13, Severity::CRITICAL, ImpactScope::GLOBAL, "ERR-SYS-SYS-013_CRI_G"),
        (ErrorSource::INT, "INT", 14, Severity::ERROR, ImpactScope::MODULE, "ERR-INT-INT-014_ERR_M"),
        (ErrorSource::UNK, "UNK", 15, Severity::INFO, ImpactScope::OPERATION, "ERR-UNK-UNK-015_INF_O"),
    ];
    
    for (source, module, sequence, severity, impact_scope, expected_code) in valid_cases {
        let result = ErrorCode::new(source, module, sequence, severity, impact_scope);
        assert!(result.is_ok(), "Failed to create error code for source={:?}, module={}, sequence={}, severity={:?}, impact_scope={:?}", source, module, sequence, severity, impact_scope);
        let error_code = result.unwrap();
        assert_eq!(error_code.code(), expected_code, "Expected code '{}', got '{}'", expected_code, error_code.code());
        assert_eq!(error_code.source(), source);
        assert_eq!(error_code.module(), module);
        assert_eq!(error_code.sequence(), sequence);
        assert_eq!(error_code.severity(), severity);
        assert_eq!(error_code.impact_scope(), impact_scope);
        
        // Test parsing the generated code
        let parse_result = ErrorCode::parse(expected_code);
        assert!(parse_result.is_ok(), "Failed to parse code '{}'", expected_code);
        let parsed_code = parse_result.unwrap();
        assert_eq!(parsed_code, error_code);
    }
}

#[test]
fn test_error_code_invalid_cases() {
    // Test invalid error code cases
    let invalid_cases = vec![
        // Invalid module names
        (ErrorSource::AIM, "", 1, Severity::ERROR, ImpactScope::SESSION, "Empty module name"),
        (ErrorSource::AIM, "A", 1, Severity::ERROR, ImpactScope::SESSION, "Module name too short"),
        (ErrorSource::AIM, "ABCDEF", 1, Severity::ERROR, ImpactScope::SESSION, "Module name too long"),
        (ErrorSource::AIM, "abc", 1, Severity::ERROR, ImpactScope::SESSION, "Module name lowercase"),
        (ErrorSource::AIM, "123", 1, Severity::ERROR, ImpactScope::SESSION, "Module name numeric"),
        (ErrorSource::AIM, "A B", 1, Severity::ERROR, ImpactScope::SESSION, "Module name with space"),
        (ErrorSource::AIM, "A@B", 1, Severity::ERROR, ImpactScope::SESSION, "Module name with special character"),
        
        // Invalid sequence numbers
        (ErrorSource::AIM, "LM", 1000, Severity::ERROR, ImpactScope::SESSION, "Sequence number too large"),
    ];
    
    for (source, module, sequence, severity, impact_scope, description) in invalid_cases {
        let result = ErrorCode::new(source, module, sequence, severity, impact_scope);
        assert!(result.is_err(), "Expected error for {}: source={:?}, module={}, sequence={}, severity={:?}, impact_scope={:?}", description, source, module, sequence, severity, impact_scope);
    }
}

#[test]
fn test_error_code_parse_invalid_cases() {
    // Test invalid error code strings
    let invalid_codes = vec![
        ("", "Empty string"),
        ("INVALID", "Invalid format"),
        ("ERR", "Too short"),
        ("ERR-AIM", "Missing module, sequence, severity, and impact scope"),
        ("ERR-AIM-LM", "Missing sequence, severity, and impact scope"),
        ("ERR-AIM-LM-001", "Missing severity and impact scope"),
        ("ERR-AIM-LM-001_", "Missing severity and impact scope"),
        ("ERR-AIM-LM-001_ERR", "Missing impact scope"),
        ("ERR-AIM-LM-00_ERR_S", "Invalid sequence format"),
        ("ERR-AIM-LM-002_INVALID_S", "Invalid severity"),
        ("ERR-AIM-LM-002_ERR_X", "Invalid impact scope"),
        ("ERR-XXX-LM-002_ERR_S", "Invalid error source"),
        ("ERR-AIM-A-002_ERR_S", "Invalid module format"),
        ("ERR-AIM-LM-ABC_ERR_S", "Non-numeric sequence"),
        ("ERR-AIM-LM-002_ERR", "Missing impact scope"),
        ("ERR-AIM-LM_ERR_S", "Missing sequence"),
        ("ERR-AIM-002_ERR_S", "Missing module"),
        ("ERR-002_ERR_S", "Missing source"),
        ("ERR-AIM-LM-002_", "Missing severity and impact scope"),
        ("ERR-AIM-LM-002_ERR_S_", "Extra character at end"),
        ("_ERR-AIM-LM-002_ERR_S", "Extra character at beginning"),
    ];
    
    for (code, description) in invalid_codes {
        let result = ErrorCode::parse(code);
        assert!(result.is_err(), "Expected error for {}: '{}'", description, code);
    }
}

#[test]
fn test_error_code_sequence_boundaries() {
    // Test sequence number boundaries
    let boundary_sequences = [0, 999]; // Minimum and maximum valid sequences
    
    for sequence in boundary_sequences {
        let result = ErrorCode::new(
            ErrorSource::AIM,
            "LM",
            sequence,
            Severity::ERROR,
            ImpactScope::SESSION,
        );
        assert!(result.is_ok(), "Failed for sequence={}", sequence);
        let error_code = result.unwrap();
        assert_eq!(error_code.sequence(), sequence);
        
        // Test parsing
        let parse_result = ErrorCode::parse(error_code.code());
        assert!(parse_result.is_ok(), "Failed to parse code for sequence={}", sequence);
        let parsed_code = parse_result.unwrap();
        assert_eq!(parsed_code.sequence(), sequence);
    }
}

#[test]
fn test_error_code_module_boundaries() {
    // Test module name boundaries
    let valid_modules = ["LM", "API", "DB", "FS", "ABCD"]; // Valid module names (2-4 uppercase letters)
    
    for module in valid_modules {
        let result = ErrorCode::new(
            ErrorSource::AIM,
            module,
            1,
            Severity::ERROR,
            ImpactScope::SESSION,
        );
        assert!(result.is_ok(), "Failed for module={}", module);
        let error_code = result.unwrap();
        assert_eq!(error_code.module(), module);
        
        // Test parsing
        let parse_result = ErrorCode::parse(error_code.code());
        assert!(parse_result.is_ok(), "Failed to parse code for module={}", module);
        let parsed_code = parse_result.unwrap();
        assert_eq!(parsed_code.module(), module);
    }
}

#[test]
fn test_error_code_all_combinations() {
    // Test all combinations of ErrorSource, Severity, and ImpactScope
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
    
    let modules = ["LM", "API", "DB", "FS"];
    
    for source in &sources {
        for module in &modules {
            for severity in &severities {
                for impact_scope in &impact_scopes {
                    let result = ErrorCode::new(*source, module, 1, *severity, *impact_scope);
                    assert!(result.is_ok(), "Failed for source={:?}, module={}, severity={:?}, impact_scope={:?}", source, module, severity, impact_scope);
                    let error_code = result.unwrap();
                    
                    // Test parsing
                    let parse_result = ErrorCode::parse(error_code.code());
                    assert!(parse_result.is_ok(), "Failed to parse code: {}", error_code.code());
                    let parsed_code = parse_result.unwrap();
                    assert_eq!(parsed_code, error_code);
                }
            }
        }
    }
}

#[test]
fn test_error_code_getters() {
    // Test all getter methods
    let result = ErrorCode::new(
        ErrorSource::AIM,
        "LM",
        123,
        Severity::ERROR,
        ImpactScope::SESSION,
    );
    assert!(result.is_ok());
    let error_code = result.unwrap();
    
    // Test all getter methods
    assert_eq!(error_code.code(), "ERR-AIM-LM-123_ERR_S");
    assert_eq!(error_code.source(), ErrorSource::AIM);
    assert_eq!(error_code.module(), "LM");
    assert_eq!(error_code.sequence(), 123);
    assert_eq!(error_code.severity(), Severity::ERROR);
    assert_eq!(error_code.impact_scope(), ImpactScope::SESSION);
}

#[test]
fn test_error_code_equality() {
    // Test equality
    let code1 = ErrorCode::new(
        ErrorSource::AIM,
        "LM",
        123,
        Severity::ERROR,
        ImpactScope::SESSION,
    ).unwrap();
    
    let code2 = ErrorCode::new(
        ErrorSource::AIM,
        "LM",
        123,
        Severity::ERROR,
        ImpactScope::SESSION,
    ).unwrap();
    
    let code3 = ErrorCode::new(
        ErrorSource::USR,
        "UI",
        456,
        Severity::WARNING,
        ImpactScope::OPERATION,
    ).unwrap();
    
    assert_eq!(code1, code2);
    assert_ne!(code1, code3);
}

#[test]
fn test_error_code_hash() {
    // Test that ErrorCode can be used as a key in a hash map
    use std::collections::HashMap;
    
    let code1 = ErrorCode::new(
        ErrorSource::AIM,
        "LM",
        123,
        Severity::ERROR,
        ImpactScope::SESSION,
    ).unwrap();
    
    let code2 = ErrorCode::new(
        ErrorSource::USR,
        "UI",
        456,
        Severity::WARNING,
        ImpactScope::OPERATION,
    ).unwrap();
    
    let mut map = HashMap::new();
    map.insert(code1.clone(), "value1");
    map.insert(code2.clone(), "value2");
    
    assert_eq!(map.get(&code1), Some(&"value1"));
    assert_eq!(map.get(&code2), Some(&"value2"));
}

#[test]
fn test_error_code_debug_format() {
    // Test debug formatting
    let code = ErrorCode::new(
        ErrorSource::AIM,
        "LM",
        123,
        Severity::ERROR,
        ImpactScope::SESSION,
    ).unwrap();
    
    let debug_str = format!("{:?}", code);
    assert!(debug_str.contains("ERR-AIM-LM-123_ERR_S"));
    assert!(debug_str.contains("AIM"));
    assert!(debug_str.contains("LM"));
    assert!(debug_str.contains("123"));
    assert!(debug_str.contains("ERROR"));
    assert!(debug_str.contains("SESSION"));
}
