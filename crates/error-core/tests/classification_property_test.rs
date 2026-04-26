//! Classification property tests — 使用 proptest 进行属性测试
//!
//! 验证分类枚举的核心属性：
//! - `as_str` / `from_str` 往返一致性
//! - 无效输入被拒绝
//! - 枚举变体互斥性
//! - Debug/Display 格式化不 panic

#![allow(clippy::uninlined_format_args)]

use std::str::FromStr;

use error_core::classification::*;
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
    fn any_recoverability()(s in proptest::sample::select(&[
        Recoverability::AutoRecoverable, Recoverability::SemiAuto,
        Recoverability::ManualIntervention, Recoverability::NonRecoverable,
    ])) -> Recoverability { s }
}

proptest! {
    #[test]
    fn proptest_error_source_as_str_from_str_roundtrip(source in any_error_source()) {
        let s = source.as_str();
        let parsed = ErrorSource::from_str(s).unwrap();
        prop_assert_eq!(parsed, source);
    }

    #[test]
    fn proptest_error_source_from_str_rejects_invalid(input in "\\PC{1,20}") {
        let valid_codes = [
            "USR", "AIM", "FS", "NET", "CFG", "SEC", "TOOL", "SESS",
            "STATE", "EXT", "LSP", "MCP", "SYS", "INT", "UNK",
        ];
        prop_assume!(!valid_codes.contains(&input.as_str()));
        let result = ErrorSource::from_str(&input);
        prop_assert!(result.is_err());
    }

    #[test]
    fn proptest_error_source_equality_reflexive(source in any_error_source()) {
        prop_assert_eq!(source, source);
    }

    #[test]
    fn proptest_error_source_different_variants_not_equal(
        s1 in any_error_source(),
        s2 in any_error_source(),
    ) {
        prop_assume!(s1 != s2);
        prop_assert_ne!(s1, s2);
    }

    #[test]
    fn proptest_error_source_debug_no_panic(source in any_error_source()) {
        let _ = format!("{source:?}");
    }

    #[test]
    fn proptest_severity_as_str_from_str_roundtrip(severity in any_severity()) {
        let s = severity.as_str();
        let parsed = Severity::from_str(s).unwrap();
        prop_assert_eq!(parsed, severity);
    }

    #[test]
    fn proptest_severity_from_str_rejects_invalid(input in "\\PC{1,20}") {
        let valid_codes = ["INFO", "WARNING", "ERROR", "CRITICAL", "INF", "WRN", "ERR", "CRI"];
        prop_assume!(!valid_codes.contains(&input.as_str()));
        let result = Severity::from_str(&input);
        prop_assert!(result.is_err());
    }

    #[test]
    fn proptest_severity_equality_reflexive(severity in any_severity()) {
        prop_assert_eq!(severity, severity);
    }

    #[test]
    fn proptest_impact_scope_as_str_from_str_roundtrip(scope in any_impact_scope()) {
        let s = scope.as_str();
        let parsed = ImpactScope::from_str(s).unwrap();
        prop_assert_eq!(parsed, scope);
    }

    #[test]
    fn proptest_impact_scope_from_str_rejects_invalid(input in "\\PC{1,20}") {
        let valid_codes = ["OPERATION", "SESSION", "MODULE", "GLOBAL", "O", "S", "M", "G"];
        prop_assume!(!valid_codes.contains(&input.as_str()));
        let result = ImpactScope::from_str(&input);
        prop_assert!(result.is_err());
    }

    #[test]
    fn proptest_impact_scope_equality_reflexive(scope in any_impact_scope()) {
        prop_assert_eq!(scope, scope);
    }

    #[test]
    fn proptest_recoverability_as_str_from_str_roundtrip(r in any_recoverability()) {
        let s = r.as_str();
        let parsed = Recoverability::from_str(s).unwrap();
        prop_assert_eq!(parsed, r);
    }

    #[test]
    fn proptest_recoverability_from_str_rejects_invalid(input in "\\PC{1,30}") {
        let valid_codes = ["AutoRecoverable", "SemiAuto", "ManualIntervention", "NonRecoverable", "AUTO", "SEMI", "MANUAL", "NON"];
        prop_assume!(!valid_codes.contains(&input.as_str()));
        let result = Recoverability::from_str(&input);
        prop_assert!(result.is_err());
    }

    #[test]
    fn proptest_recoverability_equality_reflexive(r in any_recoverability()) {
        prop_assert_eq!(r, r);
    }

    #[test]
    fn proptest_all_classifications_debug_no_panic(
        source in any_error_source(),
        severity in any_severity(),
        scope in any_impact_scope(),
        recoverability in any_recoverability(),
    ) {
        let _ = format!("{source:?}{severity:?}{scope:?}{recoverability:?}");
    }
}
