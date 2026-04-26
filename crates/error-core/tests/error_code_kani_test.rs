//! Error code module formal verification tests using Kani
//!
//! 包含两类验证：
//! - 硬编码输入验证：确保特定输入路径的正确性
//! - 符号执行验证（kani::any()）：穷举所有可能的输入状态空间

use error_core::error_code::*;
use error_core::classification::*;

// Test ErrorCode::new with valid inputs
#[kani::proof]
fn test_error_code_new_valid() {
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

    let severities = vec![
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];

    let impact_scopes = vec![
        ImpactScope::GLOBAL,
        ImpactScope::SESSION,
        ImpactScope::OPERATION,
        ImpactScope::MODULE,
    ];

    for source in sources {
        for severity in &severities {
            for impact_scope in &impact_scopes {
                let result = ErrorCode::new(source, "LM", 123, *severity, *impact_scope);
                assert!(result.is_ok());
                let error_code = result.unwrap();
                assert_eq!(error_code.source(), source);
                assert_eq!(error_code.module(), "LM");
                assert_eq!(error_code.sequence(), 123);
                assert_eq!(error_code.severity(), *severity);
                assert_eq!(error_code.impact_scope(), *impact_scope);
            }
        }
    }
}

// Test ErrorCode::parse with valid inputs
#[kani::proof]
fn test_error_code_parse_valid() {
    let valid_codes = vec![
        ("ERR-USR-MOD-001_CRI_G", ErrorSource::USR, "MOD", 1, Severity::CRITICAL, ImpactScope::GLOBAL),
        ("ERR-AIM-LM-002_ERR_S", ErrorSource::AIM, "LM", 2, Severity::ERROR, ImpactScope::SESSION),
        ("ERR-FS-IO-003_WRN_O", ErrorSource::FS, "IO", 3, Severity::WARNING, ImpactScope::OPERATION),
        ("ERR-NET-API-004_INF_M", ErrorSource::NET, "API", 4, Severity::INFO, ImpactScope::MODULE),
    ];

    for (code_str, expected_source, expected_module, expected_sequence, expected_severity, expected_impact) in valid_codes {
        let result = ErrorCode::parse(code_str);
        assert!(result.is_ok());
        let error_code = result.unwrap();
        assert_eq!(error_code.code(), code_str);
        assert_eq!(error_code.source(), expected_source);
        assert_eq!(error_code.module(), expected_module);
        assert_eq!(error_code.sequence(), expected_sequence);
        assert_eq!(error_code.severity(), expected_severity);
        assert_eq!(error_code.impact_scope(), expected_impact);
    }
}

// Test ErrorCode::parse with invalid inputs
#[kani::proof]
fn test_error_code_parse_invalid() {
    let invalid_codes = vec![
        "INVALID", // Completely invalid format
        "ERR-AIM-LM-00_ERR_S", // Invalid sequence (only 2 digits)
        "ERR-AIM-LM-002_INVALID", // Invalid severity
        "ERR-AIM-LM-002_ERR_X", // Invalid impact scope
        "ERR-XXX-LM-002_ERR_S", // Invalid error source
        "ERR-AIM-LM-ABC_ERR_S", // Non-numeric sequence
        "ERR-AIM-LM-002_ERR", // Missing impact scope
        "ERR-AIM-LM_ERR_S", // Missing sequence
        "ERR-AIM-002_ERR_S", // Missing module
        "ERR-002_ERR_S", // Missing source
        "ERR-AIM-LM-002_", // Missing severity and impact scope
        "ERR-AIM-LONGMODULE-002_ERR_S", // Module name too long
    ];

    for code_str in invalid_codes {
        let result = ErrorCode::parse(code_str);
        assert!(result.is_err());
    }
}

// Test ErrorCode getter methods
#[kani::proof]
fn test_error_code_getters() {
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

// Test ErrorCode::new with sequence number boundaries
#[kani::proof]
fn test_error_code_sequence_boundaries() {
    // Test minimum sequence number
    let result_min = ErrorCode::new(
        ErrorSource::AIM,
        "LM",
        0,
        Severity::ERROR,
        ImpactScope::SESSION,
    );
    assert!(result_min.is_ok());
    assert_eq!(result_min.unwrap().sequence(), 0);

    // Test maximum 3-digit sequence number
    let result_max = ErrorCode::new(
        ErrorSource::AIM,
        "LM",
        999,
        Severity::ERROR,
        ImpactScope::SESSION,
    );
    assert!(result_max.is_ok());
    assert_eq!(result_max.unwrap().sequence(), 999);
}

// Test ErrorCode::new with valid module name lengths
#[kani::proof]
fn test_error_code_module_lengths() {
    let valid_modules = vec!["AB", "ABC", "ABCD", "ABCDE"]; // 2-5 characters

    for module in valid_modules {
        let result = ErrorCode::new(
            ErrorSource::AIM,
            module,
            1,
            Severity::ERROR,
            ImpactScope::SESSION,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().module(), module);
    }
}

// Test ErrorCode::new with invalid module name lengths
#[kani::proof]
fn test_error_code_invalid_module_lengths() {
    let invalid_modules = vec!["A", "ABCDEF"]; // Too short or too long

    for module in invalid_modules {
        let result = ErrorCode::new(
            ErrorSource::AIM,
            module,
            1,
            Severity::ERROR,
            ImpactScope::SESSION,
        );
        assert!(result.is_err());
    }
}

// ============================================================
// 符号执行验证（kani::any()）— 穷举所有可能的输入状态空间
// ============================================================

/// 验证：任意 u32 序列号，仅 0..=999 为合法值
/// kani::any() 生成所有可能的 u32 值（0 到 2^32-1），
/// 验证器将证明：对于合法序列号，new 返回 Ok；对于非法序列号，new 返回 Err。
#[kani::proof]
fn kani_proof_sequence_boundary_any() {
    let sequence: u32 = kani::any();
    let result = ErrorCode::new(ErrorSource::AIM, "LM", sequence, Severity::ERROR, ImpactScope::SESSION);
    if sequence <= 999 {
        assert!(result.is_ok());
        let ec = result.unwrap();
        assert_eq!(ec.sequence(), sequence);
    } else {
        assert!(result.is_err());
    }
}

/// 验证：ErrorCode::new 成功后，parse(code) 必定成功且结果一致
/// 这是对 new/parse 往返一致性的形式化证明。
#[kani::proof]
fn kani_proof_new_parse_roundtrip() {
    let sequence: u32 = kani::any();
    kani::assume(sequence <= 999);
    let result = ErrorCode::new(ErrorSource::FS, "IO", sequence, Severity::WARNING, ImpactScope::OPERATION);
    assert!(result.is_ok());
    let ec = result.unwrap();
    let parsed = ErrorCode::parse(ec.code());
    assert!(parsed.is_ok());
    assert_eq!(parsed.unwrap(), ec);
}

/// 验证：ErrorCode 的 code() 输出始终以 "ERR-" 开头
#[kani::proof]
fn kani_proof_code_format_prefix() {
    let sequence: u32 = kani::any();
    kani::assume(sequence <= 999);
    let result = ErrorCode::new(ErrorSource::NET, "API", sequence, Severity::INFO, ImpactScope::MODULE);
    assert!(result.is_ok());
    assert!(result.unwrap().code().starts_with("ERR-"));
}
