//! 抽取引擎错误类型
//!
//! 所有错误通过 `error-core::helpers` 统一构造，遵循项目宪章的统一错误处理原则。
//! 详见文档: §7 | 用例: UC-006

pub use error_core::prelude::Result;

/// 构造 LLM API 调用失败错误
///
/// 当与大语言模型的通信失败时使用此函数构造统一错误对象。
///
/// # Errors
///
/// 本函数始终返回包含 LLM 错误码的 `ErrorObject`，不会返回 `Ok`。
#[must_use]
pub fn llm_error(message: impl Into<String>) -> error_core::ErrorObject {
    error_core::helpers::llm_api_error(&message.into(), "call")
}

/// 构造配置无效错误
///
/// 当系统配置不满足约束条件时使用此函数构造统一错误对象。
///
/// # Errors
///
/// 本函数始终返回包含配置错误码的 `ErrorObject`，不会返回 `Ok`。
#[must_use]
pub fn invalid_config(message: impl Into<String>) -> error_core::ErrorObject {
    error_core::helpers::config_error(&message.into())
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

/// 构造数据库操作错误
///
/// 当数据库查询或事务执行失败时使用此函数构造统一错误对象。
///
/// # Errors
///
/// 本函数始终返回包含数据库错误码的 `ErrorObject`，不会返回 `Ok`。
#[must_use]
pub fn database_error(message: impl Into<String>) -> error_core::ErrorObject {
    error_core::helpers::db_error(&message.into())
}

/// 构造超时错误
///
/// 当抽取操作超过允许的时间限制时使用此函数构造统一错误对象。
///
/// # Errors
///
/// 本函数始终返回包含超时错误码的 `ErrorObject`，不会返回 `Ok`。
#[must_use]
pub fn timeout_error() -> error_core::ErrorObject {
    error_core::helpers::eval_timeout_error("抽取操作超时")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_error_code_matches_registry() {
        let err = llm_error("connection refused");
        assert_eq!(err.code(), error_core::error_code::registry::LLM_API_FAILED);
    }

    #[test]
    fn test_invalid_config_code_matches_registry() {
        let err = invalid_config("bad config");
        assert_eq!(err.code(), error_core::error_code::registry::CONFIG_FAILED);
    }

    #[test]
    fn test_serialization_error_code_matches_registry() {
        let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let err = serialization_error(&json_err);
        assert_eq!(err.code(), error_core::error_code::registry::SERDE_FAILED);
    }

    #[test]
    fn test_database_error_code_matches_registry() {
        let err = database_error("connection lost");
        assert_eq!(err.code(), error_core::error_code::registry::DB_QUERY_FAILED);
    }

    #[test]
    fn test_timeout_error_code_matches_registry() {
        let err = timeout_error();
        assert_eq!(err.code(), error_core::error_code::registry::EVAL_TIMEOUT);
    }
}
