use error_core::error_code::ErrorCode;

#[test]
fn test_error_code_parse_regex_error() {}

#[test]
fn test_error_code_parse_invalid_format() {
    let invalid_formats = [
        "",
        "ERR",
        "ERR-AIM",
        "ERR-AIM-LM-002_INVALID_S",
        "ERR-AIM-LM-002_ERR_X",
        "err-aim-lm-002_err_s",
    ];

    for format in &invalid_formats {
        let result = ErrorCode::parse(format);
        assert!(result.is_err());
    }

    #[cfg(feature = "regex")]
    {
        let regex_only_invalid = [
            "ERR-AIM-LM",
            "ERR-AIM-LM-00",
            "ERR-AIM-LM-002_",
            "ERR-AIM-LM-002_ERR",
        ];
        for format in &regex_only_invalid {
            let result = ErrorCode::parse(format);
            assert!(result.is_err());
        }
    }
}

#[test]
fn test_error_code_parse_invalid_source() {
    let result = ErrorCode::parse("ERR-XXX-LM-002_ERR_S");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.message().contains("Invalid error source")
            || err.message().contains("无效的错误来源")
            || err.message().contains("错误码格式无效")
    );
}

#[test]
fn test_error_code_parse_invalid_sequence() {
    let result = ErrorCode::parse("ERR-AIM-LM-ABC_ERR_S");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.message().contains("Invalid error code format")
            || err.message().contains("错误码格式无效")
            || err.message().contains("无效的序号")
    );
}

#[test]
fn test_error_code_parse_invalid_severity() {
    let result = ErrorCode::parse("ERR-AIM-LM-002_INVALID_S");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.message().contains("Invalid error code format")
            || err.message().contains("错误码格式无效")
            || err.message().contains("无效的严重级别")
    );
}

#[test]
fn test_error_code_parse_invalid_impact_scope() {
    let result = ErrorCode::parse("ERR-AIM-LM-002_ERR_X");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.message().contains("Invalid error code format")
            || err.message().contains("错误码格式无效")
            || err.message().contains("无效的影响范围")
    );
}

#[test]
fn test_error_code_new_invalid_module() {
    #[cfg(feature = "regex")]
    {
        let invalid_modules = ["", "A", "AAAAAA", "1234", "A1B2", "abc"];

        for module in &invalid_modules {
            let result = ErrorCode::new(
                error_core::classification::ErrorSource::AIM,
                module,
                1,
                error_core::classification::Severity::ERROR,
                error_core::classification::ImpactScope::SESSION,
            );
            assert!(result.is_err());
        }
    }
}
