//! 评估器错误类型
//!
//! 所有错误通过 `error-core::helpers` 统一构造，遵循项目宪章的统一错误处理原则。
//! 详见文档: §9

pub use error_core::prelude::Result;

/// 构造 LLM 判决失败错误
///
/// 当 LLM-as-Judge 的判决过程失败时使用此函数构造统一错误对象。
///
/// # Errors
///
/// 本函数始终返回包含 LLM 判决错误码的 `ErrorObject`，不会返回 `Ok`。
#[must_use]
pub fn llm_judgment_failed(message: impl Into<String>) -> error_core::ErrorObject {
    error_core::helpers::llm_judgment_error(&message.into(), "judge")
}

/// 构造超时错误
///
/// 当评估操作超过允许的时间限制时使用此函数构造统一错误对象。
///
/// # Errors
///
/// 本函数始终返回包含超时错误码的 `ErrorObject`，不会返回 `Ok`。
#[must_use]
pub fn timeout(message: impl Into<String>) -> error_core::ErrorObject {
    error_core::helpers::eval_timeout_error(&message.into())
}

/// 构造指标计算失败错误
///
/// 当 RAGAS 指标计算过程中发生错误时使用此函数构造统一错误对象。
///
/// # Errors
///
/// 本函数始终返回包含指标计算错误码的 `ErrorObject`，不会返回 `Ok`。
#[must_use]
pub fn metric_calculation_failed(message: impl Into<String>) -> error_core::ErrorObject {
    error_core::helpers::metric_calc_error(&message.into())
}

/// 构造数据集加载错误
///
/// 当黄金数据集文件读取或解析失败时使用此函数构造统一错误对象。
///
/// # Errors
///
/// 本函数始终返回包含数据集加载错误码的 `ErrorObject`，不会返回 `Ok`。
#[must_use]
pub fn dataset_load_error(source: &std::io::Error) -> error_core::ErrorObject {
    error_core::helpers::dataset_load_error(&source.to_string())
}

/// 构造序列化/反序列化错误
///
/// 当 JSON 或其他格式的序列化操作失败时使用此函数构造统一错误对象。
///
/// # Errors
///
/// 本函数始终返回包含序列化错误码的 `ErrorObject`，不会返回 `Ok`。
#[must_use]
pub fn serialization_error(source: &serde_json::Error) -> error_core::ErrorObject {
    error_core::helpers::serde_error(&source.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_judgment_failed_code_matches_registry() {
        let err = llm_judgment_failed("connection refused");
        assert_eq!(
            err.code(),
            error_core::error_code::registry::LLM_JUDGMENT_FAILED
        );
    }

    #[test]
    fn test_timeout_code_matches_registry() {
        let err = timeout("30s elapsed");
        assert_eq!(err.code(), error_core::error_code::registry::EVAL_TIMEOUT);
    }

    #[test]
    fn test_metric_calculation_failed_code_matches_registry() {
        let err = metric_calculation_failed("division by zero");
        assert_eq!(
            err.code(),
            error_core::error_code::registry::METRIC_CALC_FAILED
        );
    }

    #[test]
    fn test_dataset_load_error_code_matches_registry() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = dataset_load_error(&io_err);
        assert_eq!(
            err.code(),
            error_core::error_code::registry::DATASET_LOAD_FAILED
        );
    }

    #[test]
    fn test_serialization_error_code_matches_registry() {
        let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let err = serialization_error(&json_err);
        assert_eq!(err.code(), error_core::error_code::registry::SERDE_FAILED);
    }
}
