//! Classification module formal verification tests using Kani

use error_core::classification::*;

// Test ErrorSource as_str method
#[kani::proof]
fn test_error_source_as_str() {
    let sources = vec![
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

    for source in sources {
        let s = source.as_str();
        // Ensure the result is not empty
        assert!(!s.is_empty());
    }
}

// Test ErrorSource from_str method with valid inputs
#[kani::proof]
fn test_error_source_from_str_valid() {
    let valid_inputs = vec![
        ("USR", ErrorSource::USR),
        ("AIM", ErrorSource::AIM),
        ("FS", ErrorSource::FS),
        ("NET", ErrorSource::NET),
        ("CFG", ErrorSource::CFG),
        ("SEC", ErrorSource::SEC),
        ("TOOL", ErrorSource::TOOL),
        ("SESS", ErrorSource::SESS),
        ("STATE", ErrorSource::STATE),
        ("EXT", ErrorSource::EXT),
        ("LSP", ErrorSource::LSP),
        ("MCP", ErrorSource::MCP),
        ("SYS", ErrorSource::SYS),
        ("INT", ErrorSource::INT),
        ("UNK", ErrorSource::UNK),
    ];

    for (input, expected) in valid_inputs {
        let result = ErrorSource::from_str(input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }
}

// Test ErrorSource from_str method with invalid input
#[kani::proof]
fn test_error_source_from_str_invalid() {
    let invalid_inputs = vec!["INVALID", "", "123", "usr", "aim"];

    for input in invalid_inputs {
        let result = ErrorSource::from_str(input);
        assert!(result.is_err());
    }
}

// Test Severity as_str method
#[kani::proof]
fn test_severity_as_str() {
    let severities = vec![
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];

    for severity in severities {
        let s = severity.as_str();
        assert!(!s.is_empty());
    }
}

// Test Severity from_str method with valid inputs
#[kani::proof]
fn test_severity_from_str_valid() {
    let valid_inputs = vec![
        ("CRI", Severity::CRITICAL),
        ("ERR", Severity::ERROR),
        ("WRN", Severity::WARNING),
        ("INF", Severity::INFO),
    ];

    for (input, expected) in valid_inputs {
        let result = Severity::from_str(input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }
}

// Test Severity from_str method with invalid input
#[kani::proof]
fn test_severity_from_str_invalid() {
    let invalid_inputs = vec!["INVALID", "", "123", "cri", "err"];

    for input in invalid_inputs {
        let result = Severity::from_str(input);
        assert!(result.is_err());
    }
}

// Test ImpactScope as_str method
#[kani::proof]
fn test_impact_scope_as_str() {
    let scopes = vec![
        ImpactScope::GLOBAL,
        ImpactScope::SESSION,
        ImpactScope::OPERATION,
        ImpactScope::MODULE,
    ];

    for scope in scopes {
        let s = scope.as_str();
        assert!(!s.is_empty());
    }
}

// Test ImpactScope from_str method with valid inputs
#[kani::proof]
fn test_impact_scope_from_str_valid() {
    let valid_inputs = vec![
        ("G", ImpactScope::GLOBAL),
        ("S", ImpactScope::SESSION),
        ("O", ImpactScope::OPERATION),
        ("M", ImpactScope::MODULE),
    ];

    for (input, expected) in valid_inputs {
        let result = ImpactScope::from_str(input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }
}

// Test ImpactScope from_str method with invalid input
#[kani::proof]
fn test_impact_scope_from_str_invalid() {
    let invalid_inputs = vec!["INVALID", "", "123", "g", "s"];

    for input in invalid_inputs {
        let result = ImpactScope::from_str(input);
        assert!(result.is_err());
    }
}

// Test Recoverability as_str method
#[kani::proof]
fn test_recoverability_as_str() {
    let recoverabilities = vec![
        Recoverability::AutoRecoverable,
        Recoverability::SemiAuto,
        Recoverability::ManualIntervention,
        Recoverability::NonRecoverable,
    ];

    for recoverability in recoverabilities {
        let s = recoverability.as_str();
        assert!(!s.is_empty());
    }
}

// Test Recoverability from_str method with valid inputs
#[kani::proof]
fn test_recoverability_from_str_valid() {
    let valid_inputs = vec![
        ("AUTO", Recoverability::AutoRecoverable),
        ("SEMI", Recoverability::SemiAuto),
        ("MANUAL", Recoverability::ManualIntervention),
        ("NON", Recoverability::NonRecoverable),
    ];

    for (input, expected) in valid_inputs {
        let result = Recoverability::from_str(input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }
}

// Test Recoverability from_str method with invalid input
#[kani::proof]
fn test_recoverability_from_str_invalid() {
    let invalid_inputs = vec!["INVALID", "", "123", "auto", "semi"];

    for input in invalid_inputs {
        let result = Recoverability::from_str(input);
        assert!(result.is_err());
    }
}

// Test Display trait implementations
#[kani::proof]
fn test_display_traits() {
    // Test ErrorSource Display
    assert_eq!(format!("{}", ErrorSource::USR), "USR");
    assert_eq!(format!("{}", ErrorSource::AIM), "AIM");
    
    // Test Severity Display
    assert_eq!(format!("{}", Severity::CRITICAL), "CRI");
    assert_eq!(format!("{}", Severity::ERROR), "ERR");
    
    // Test ImpactScope Display
    assert_eq!(format!("{}", ImpactScope::GLOBAL), "G");
    assert_eq!(format!("{}", ImpactScope::SESSION), "S");
    
    // Test Recoverability Display
    assert_eq!(format!("{}", Recoverability::AutoRecoverable), "AUTO");
    assert_eq!(format!("{}", Recoverability::SemiAuto), "SEMI");
}
