//! Error code property tests — 使用 proptest 进行属性测试
//!
//! 验证 `ErrorCode` 的核心属性：
//! - new/parse 往返一致性
//! - 序列号边界验证
//! - 模块名边界验证
//! - 随机无效输入不 panic

#![allow(clippy::uninlined_format_args)]

use error_core::error_code::ErrorCode;
use error_core::classification::{ErrorSource, Severity, ImpactScope};
use proptest::prelude::*;

prop_compose! {
    fn any_error_source()(s in proptest::sample::select(&[
        ErrorSource::USR, ErrorSource::AIM, ErrorSource::FS, ErrorSource::NET,
        ErrorSource::CFG, ErrorSource::SEC, ErrorSource::TOOL, ErrorSource::SESS,
        ErrorSource::STATE, ErrorSource::EXT, ErrorSource::LSP, ErrorSource::MCP,
        ErrorSource::SYS, ErrorSource::INT, ErrorSource::UNK,
    ])) -> ErrorSource { s }
}

prop_compose! {
    fn any_severity()(s in proptest::sample::select(&[
        Severity::INFO, Severity::WARNING, Severity::ERROR, Severity::CRITICAL,
    ])) -> Severity { s }
}

prop_compose! {
    fn any_impact_scope()(s in proptest::sample::select(&[
        ImpactScope::OPERATION, ImpactScope::SESSION, ImpactScope::MODULE, ImpactScope::GLOBAL,
    ])) -> ImpactScope { s }
}

prop_compose! {
    fn valid_module()(m in proptest::sample::select(&["LM", "API", "DB", "FS", "CFG", "SEC", "NET", "IO", "ABCD"])) -> String { m.to_string() }
}

proptest! {
    #[test]
    fn proptest_error_code_new_parse_roundtrip(
        source in any_error_source(),
        module in valid_module(),
        sequence in 0u32..=999u32,
        severity in any_severity(),
        impact_scope in any_impact_scope(),
    ) {
        let result = ErrorCode::new(source, &module, sequence, severity, impact_scope);
        prop_assert!(result.is_ok());
        let error_code = result.unwrap();
        prop_assert_eq!(error_code.source(), source);
        prop_assert_eq!(error_code.module(), module);
        prop_assert_eq!(error_code.sequence(), sequence);
        prop_assert_eq!(error_code.severity(), severity);
        prop_assert_eq!(error_code.impact_scope(), impact_scope);

        let parse_result = ErrorCode::parse(error_code.code());
        prop_assert!(parse_result.is_ok());
        prop_assert_eq!(parse_result.unwrap(), error_code);
    }

    #[cfg(feature = "regex")]
    #[test]
    fn proptest_error_code_invalid_sequence(
        source in any_error_source(),
        module in valid_module(),
        sequence in 1000u32..=99999u32,
        severity in any_severity(),
        impact_scope in any_impact_scope(),
    ) {
        let result = ErrorCode::new(source, &module, sequence, severity, impact_scope);
        prop_assert!(result.is_err());
    }

    #[cfg(feature = "regex")]
    #[test]
    fn proptest_error_code_invalid_module_too_short(
        source in any_error_source(),
        module in "[A-Z]",
        severity in any_severity(),
        impact_scope in any_impact_scope(),
    ) {
        let result = ErrorCode::new(source, &module, 1, severity, impact_scope);
        prop_assert!(result.is_err());
    }

    #[cfg(feature = "regex")]
    #[test]
    fn proptest_error_code_invalid_module_too_long(
        source in any_error_source(),
        module in "[A-Z]{6,10}",
        severity in any_severity(),
        impact_scope in any_impact_scope(),
    ) {
        let result = ErrorCode::new(source, &module, 1, severity, impact_scope);
        prop_assert!(result.is_err());
    }

    #[cfg(feature = "regex")]
    #[test]
    fn proptest_error_code_invalid_module_lowercase(
        source in any_error_source(),
        module in "[a-z]{2,4}",
        severity in any_severity(),
        impact_scope in any_impact_scope(),
    ) {
        let result = ErrorCode::new(source, &module, 1, severity, impact_scope);
        prop_assert!(result.is_err());
    }

    #[test]
    fn proptest_error_code_parse_random_input_no_panic(input in "\\PC{0,50}") {
        let _ = ErrorCode::parse(&input);
    }

    #[test]
    fn proptest_error_code_code_format_consistency(
        source in any_error_source(),
        module in valid_module(),
        sequence in 0u32..=999u32,
        severity in any_severity(),
        impact_scope in any_impact_scope(),
    ) {
        let error_code = ErrorCode::new(source, &module, sequence, severity, impact_scope).unwrap();
        let code = error_code.code();
        let module_str = format!("-{module}-");
        let seq_str = format!("{sequence:03}");
        prop_assert!(code.starts_with("ERR-"));
        prop_assert!(code.contains(&module_str));
        prop_assert!(code.contains(&seq_str));
    }
}
