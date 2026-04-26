//! Classification fuzz tests
//! 
//! This module contains fuzz tests for the classification module to ensure coverage of all possible cases.

use error_core::classification::*;
use std::str::FromStr;

#[test]
fn test_error_source_fuzzing() {
    // Test all ErrorSource variants
    let sources = vec![
        ("USR", Ok(ErrorSource::USR), "Valid USR"),
        ("AIM", Ok(ErrorSource::AIM), "Valid AIM"),
        ("FS", Ok(ErrorSource::FS), "Valid FS"),
        ("NET", Ok(ErrorSource::NET), "Valid NET"),
        ("CFG", Ok(ErrorSource::CFG), "Valid CFG"),
        ("SEC", Ok(ErrorSource::SEC), "Valid SEC"),
        ("TOOL", Ok(ErrorSource::TOOL), "Valid TOOL"),
        ("SESS", Ok(ErrorSource::SESS), "Valid SESS"),
        ("STATE", Ok(ErrorSource::STATE), "Valid STATE"),
        ("EXT", Ok(ErrorSource::EXT), "Valid EXT"),
        ("LSP", Ok(ErrorSource::LSP), "Valid LSP"),
        ("MCP", Ok(ErrorSource::MCP), "Valid MCP"),
        ("SYS", Ok(ErrorSource::SYS), "Valid SYS"),
        ("INT", Ok(ErrorSource::INT), "Valid INT"),
        ("UNK", Ok(ErrorSource::UNK), "Valid UNK"),
        ("", Err(()), "Empty string"),
        ("invalid", Err(()), "Invalid string"),
        ("usr", Err(()), "Lowercase"),
        ("US", Err(()), "Too short"),
        ("USRR", Err(()), "Too long"),
        ("123", Err(()), "Numeric"),
        ("US R", Err(()), "With space"),
        ("US@", Err(()), "With special character"),
    ];
    
    for (input, expected, description) in sources {
        let result = ErrorSource::from_str(input);
        match expected {
            Ok(expected_source) => {
                assert!(result.is_ok(), "Failed to parse '{}' ({})", input, description);
                let parsed = result.unwrap();
                assert_eq!(parsed, expected_source, "Parsed '{}' as {:?}, expected {:?} ({})", input, parsed, expected_source, description);
                assert_eq!(parsed.as_str(), input, "as_str() returned wrong value for '{}' ({})", input, description);
                assert_eq!(format!("{}", parsed), input, "Display returned wrong value for '{}' ({})", input, description);
            }
            Err(_) => {
                assert!(result.is_err(), "Expected error for '{}' ({})", input, description);
            }
        }
    }
}

#[test]
fn test_severity_fuzzing() {
    // Test all Severity variants
    let severities = vec![
        ("CRI", Ok(Severity::CRITICAL), "Valid CRI"),
        ("ERR", Ok(Severity::ERROR), "Valid ERR"),
        ("WRN", Ok(Severity::WARNING), "Valid WRN"),
        ("INF", Ok(Severity::INFO), "Valid INF"),
        ("", Err(()), "Empty string"),
        ("invalid", Err(()), "Invalid string"),
        ("cri", Err(()), "Lowercase"),
        ("CR", Err(()), "Too short"),
        ("CRIT", Err(()), "Too long"),
        ("123", Err(()), "Numeric"),
        ("CR I", Err(()), "With space"),
        ("CR@", Err(()), "With special character"),
    ];
    
    for (input, expected, description) in severities {
        let result = Severity::from_str(input);
        match expected {
            Ok(expected_severity) => {
                assert!(result.is_ok(), "Failed to parse '{}' ({})", input, description);
                let parsed = result.unwrap();
                assert_eq!(parsed, expected_severity, "Parsed '{}' as {:?}, expected {:?} ({})", input, parsed, expected_severity, description);
                assert_eq!(parsed.as_str(), input, "as_str() returned wrong value for '{}' ({})", input, description);
                assert_eq!(format!("{}", parsed), input, "Display returned wrong value for '{}' ({})", input, description);
            }
            Err(_) => {
                assert!(result.is_err(), "Expected error for '{}' ({})", input, description);
            }
        }
    }
}

#[test]
fn test_impact_scope_fuzzing() {
    // Test all ImpactScope variants
    let scopes = vec![
        ("G", Ok(ImpactScope::GLOBAL), "Valid G"),
        ("S", Ok(ImpactScope::SESSION), "Valid S"),
        ("O", Ok(ImpactScope::OPERATION), "Valid O"),
        ("M", Ok(ImpactScope::MODULE), "Valid M"),
        ("", Err(()), "Empty string"),
        ("invalid", Err(()), "Invalid string"),
        ("g", Err(()), "Lowercase"),
        ("GG", Err(()), "Too long"),
        ("1", Err(()), "Numeric"),
        ("G ", Err(()), "With space"),
        ("G@", Err(()), "With special character"),
    ];
    
    for (input, expected, description) in scopes {
        let result = ImpactScope::from_str(input);
        match expected {
            Ok(expected_scope) => {
                assert!(result.is_ok(), "Failed to parse '{}' ({})", input, description);
                let parsed = result.unwrap();
                assert_eq!(parsed, expected_scope, "Parsed '{}' as {:?}, expected {:?} ({})", input, parsed, expected_scope, description);
                assert_eq!(parsed.as_str(), input, "as_str() returned wrong value for '{}' ({})", input, description);
                assert_eq!(format!("{}", parsed), input, "Display returned wrong value for '{}' ({})", input, description);
            }
            Err(_) => {
                assert!(result.is_err(), "Expected error for '{}' ({})", input, description);
            }
        }
    }
}

#[test]
fn test_recoverability_fuzzing() {
    // Test all Recoverability variants
    let recoverabilities = vec![
        ("AUTO", Ok(Recoverability::AutoRecoverable), "Valid AUTO"),
        ("SEMI", Ok(Recoverability::SemiAuto), "Valid SEMI"),
        ("MANUAL", Ok(Recoverability::ManualIntervention), "Valid MANUAL"),
        ("NON", Ok(Recoverability::NonRecoverable), "Valid NON"),
        ("", Err(()), "Empty string"),
        ("invalid", Err(()), "Invalid string"),
        ("auto", Err(()), "Lowercase"),
        ("AU", Err(()), "Too short"),
        ("AUTOO", Err(()), "Too long"),
        ("123", Err(()), "Numeric"),
        ("AUTO ", Err(()), "With space"),
        ("AUTO@", Err(()), "With special character"),
    ];
    
    for (input, expected, description) in recoverabilities {
        let result = Recoverability::from_str(input);
        match expected {
            Ok(expected_recoverability) => {
                assert!(result.is_ok(), "Failed to parse '{}' ({})", input, description);
                let parsed = result.unwrap();
                assert_eq!(parsed, expected_recoverability, "Parsed '{}' as {:?}, expected {:?} ({})", input, parsed, expected_recoverability, description);
                assert_eq!(parsed.as_str(), input, "as_str() returned wrong value for '{}' ({})", input, description);
                assert_eq!(format!("{}", parsed), input, "Display returned wrong value for '{}' ({})", input, description);
            }
            Err(_) => {
                assert!(result.is_err(), "Expected error for '{}' ({})", input, description);
            }
        }
    }
}

#[test]
fn test_all_enum_variants() {
    // Test all variants of all enums
    let all_error_sources = vec![
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
    
    let all_severities = vec![
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];
    
    let all_impact_scopes = vec![
        ImpactScope::GLOBAL,
        ImpactScope::SESSION,
        ImpactScope::OPERATION,
        ImpactScope::MODULE,
    ];
    
    let all_recoverabilities = vec![
        Recoverability::AutoRecoverable,
        Recoverability::SemiAuto,
        Recoverability::ManualIntervention,
        Recoverability::NonRecoverable,
    ];
    
    // Test that all variants can be created, converted to string, and formatted
    for source in all_error_sources {
        let _ = source.as_str();
        let _ = format!("{:?}", source);
        let _ = format!("{}", source);
    }
    
    for severity in all_severities {
        let _ = severity.as_str();
        let _ = format!("{:?}", severity);
        let _ = format!("{}", severity);
    }
    
    for scope in all_impact_scopes {
        let _ = scope.as_str();
        let _ = format!("{:?}", scope);
        let _ = format!("{}", scope);
    }
    
    for recoverability in all_recoverabilities {
        let _ = recoverability.as_str();
        let _ = format!("{:?}", recoverability);
        let _ = format!("{}", recoverability);
    }
}

#[test]
fn test_enum_equality() {
    // Test equality for all enums
    assert_eq!(ErrorSource::USR, ErrorSource::USR);
    assert_ne!(ErrorSource::USR, ErrorSource::AIM);
    
    assert_eq!(Severity::ERROR, Severity::ERROR);
    assert_ne!(Severity::ERROR, Severity::WARNING);
    
    assert_eq!(ImpactScope::SESSION, ImpactScope::SESSION);
    assert_ne!(ImpactScope::SESSION, ImpactScope::MODULE);
    
    assert_eq!(Recoverability::AutoRecoverable, Recoverability::AutoRecoverable);
    assert_ne!(Recoverability::AutoRecoverable, Recoverability::SemiAuto);
}

#[test]
fn test_enum_hash() {
    // Test that all enums can be used as keys in a hash map
    use std::collections::HashMap;
    
    let mut source_map = HashMap::new();
    source_map.insert(ErrorSource::USR, "User error");
    source_map.insert(ErrorSource::AIM, "AI model error");
    assert_eq!(source_map.get(&ErrorSource::USR), Some(&"User error"));
    
    let mut severity_map = HashMap::new();
    severity_map.insert(Severity::ERROR, "Error level");
    severity_map.insert(Severity::WARNING, "Warning level");
    assert_eq!(severity_map.get(&Severity::ERROR), Some(&"Error level"));
    
    let mut scope_map = HashMap::new();
    scope_map.insert(ImpactScope::SESSION, "Session impact");
    scope_map.insert(ImpactScope::MODULE, "Module impact");
    assert_eq!(scope_map.get(&ImpactScope::SESSION), Some(&"Session impact"));
    
    let mut recoverability_map = HashMap::new();
    recoverability_map.insert(Recoverability::AutoRecoverable, "Auto recoverable");
    recoverability_map.insert(Recoverability::SemiAuto, "Semi-auto recoverable");
    assert_eq!(recoverability_map.get(&Recoverability::AutoRecoverable), Some(&"Auto recoverable"));
}

#[test]
fn test_from_str_edge_cases() {
    // Test edge cases for FromStr implementations
    
    // Test valid inputs
    assert!(ErrorSource::from_str("USR").is_ok(), "Failed to parse 'USR'");
    assert!(ErrorSource::from_str("AIM").is_ok(), "Failed to parse 'AIM'");
    assert!(Severity::from_str("CRI").is_ok(), "Failed to parse 'CRI'");
    assert!(Severity::from_str("ERR").is_ok(), "Failed to parse 'ERR'");
    assert!(ImpactScope::from_str("G").is_ok(), "Failed to parse 'G'");
    assert!(ImpactScope::from_str("S").is_ok(), "Failed to parse 'S'");
    assert!(Recoverability::from_str("AUTO").is_ok(), "Failed to parse 'AUTO'");
    assert!(Recoverability::from_str("SEMI").is_ok(), "Failed to parse 'SEMI'");
    
    // Test invalid inputs
    assert!(ErrorSource::from_str("").is_err(), "Expected error for empty string");
    assert!(ErrorSource::from_str("invalid").is_err(), "Expected error for 'invalid'");
    assert!(Severity::from_str("").is_err(), "Expected error for empty string");
    assert!(Severity::from_str("invalid").is_err(), "Expected error for 'invalid'");
    assert!(ImpactScope::from_str("").is_err(), "Expected error for empty string");
    assert!(ImpactScope::from_str("invalid").is_err(), "Expected error for 'invalid'");
    assert!(Recoverability::from_str("").is_err(), "Expected error for empty string");
    assert!(Recoverability::from_str("invalid").is_err(), "Expected error for 'invalid'");
}
