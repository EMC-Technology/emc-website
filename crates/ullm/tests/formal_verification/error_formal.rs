#![cfg(test)]
//! Error 模块形式化验证测试
//!
//! ## 不变量 (Invariants)
//!
//! 1. `is_retryable()` 对所有错误类型返回一致结果
//! 2. `from_http_status` 遵循 HTTP 语义映射规则
//! 3. `safe_failure_class()` 不返回空字符串

use crate::error::LlmError;
use http::StatusCode;

#[test]
fn test_miri_is_retryable_exhaustive() {
    let retryable = [
        LlmError::Network("x".into()),
        LlmError::Timeout(100),
        LlmError::RateLimitExceeded { retry_after_ms: None },
        LlmError::ModelUnavailable("x".into()),
        LlmError::ServerOverloaded,
    ];

    let non_retryable = [
        LlmError::AuthenticationError { message: "x".into() },
        LlmError::PermissionError { message: "x".into() },
        LlmError::ContextWindowExceeded { estimated_tokens: 1, limit_tokens: 2 },
        LlmError::PromptTooLarge(1),
        LlmError::InvalidRequest { message: "x".into() },
        LlmError::StreamError("x".into()),
        LlmError::ToolCallError { message: "x".into(), tool_name: None },
        LlmError::MissingCredentials { provider: "x".into(), env_vars: vec![] },
        LlmError::ExpiredToken("x".into()),
        LlmError::JsonParse {
            provider: "x".into(),
            model: "y".into(),
            body_snippet: "z".into(),
            source: serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err(),
        },
        LlmError::RetriesExhausted { attempts: 3 },
        LlmError::Config("x".into()),
        LlmError::RequestBodySizeExceeded { estimated_bytes: 1, max_bytes: 1 },
        LlmError::Cancelled,
        LlmError::HttpClientInit("x".into()),
        LlmError::Other("x".into()),
    ];

    for err in retryable {
        assert!(
            err.is_retryable(),
            "MIRI: {:?} MUST be retryable",
            err
        );
    }

    for err in non_retryable {
        assert!(
            !err.is_retryable(),
            "MIRI: {:?} MUST NOT be retryable",
            err
        );
    }
}

#[test]
fn test_miri_safe_failure_class_non_empty() {
    let errors: Vec<LlmError> = vec![
        LlmError::Network("x".into()),
        LlmError::Timeout(0),
        LlmError::RateLimitExceeded { retry_after_ms: None },
        LlmError::AuthenticationError { message: "x".into() },
        LlmError::PermissionError { message: "x".into() },
        LlmError::ContextWindowExceeded { estimated_tokens: 0, limit_tokens: 0 },
        LlmError::PromptTooLarge(0),
        LlmError::ModelUnavailable("x".into()),
        LlmError::ServerOverloaded,
        LlmError::InvalidRequest { message: "x".into() },
        LlmError::StreamError("x".into()),
        LlmError::ToolCallError { message: "x".into(), tool_name: None },
        LlmError::MissingCredentials { provider: "x".into(), env_vars: vec![] },
        LlmError::ExpiredToken("x".into()),
        LlmError::JsonParse {
            provider: "x".into(),
            model: "y".into(),
            body_snippet: "z".into(),
            source: serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err(),
        },
        LlmError::RetriesExhausted { attempts: 0 },
        LlmError::Config("x".into()),
        LlmError::RequestBodySizeExceeded { estimated_bytes: 0, max_bytes: 0 },
        LlmError::Cancelled,
        LlmError::HttpClientInit("x".into()),
        LlmError::Other("x".into()),
    ];

    for err in errors {
        let class = err.safe_failure_class();
        assert!(
            !class.is_empty(),
            "THEOREM: safe_failure_class() MUST NEVER return empty string for {:?}",
            err
        );
    }
}

#[test]
fn test_miri_suggested_action_non_empty() {
    let errors: Vec<LlmError> = vec![
        LlmError::Network("x".into()),
        LlmError::Timeout(0),
        LlmError::RateLimitExceeded { retry_after_ms: None },
        LlmError::AuthenticationError { message: "x".into() },
        LlmError::PermissionError { message: "x".into() },
        LlmError::ContextWindowExceeded { estimated_tokens: 0, limit_tokens: 0 },
        LlmError::PromptTooLarge(0),
        LlmError::ModelUnavailable("x".into()),
        LlmError::ServerOverloaded,
        LlmError::InvalidRequest { message: "x".into() },
        LlmError::StreamError("x".into()),
        LlmError::ToolCallError { message: "x".into(), tool_name: None },
        LlmError::MissingCredentials { provider: "x".into(), env_vars: vec![] },
        LlmError::ExpiredToken("x".into()),
        LlmError::JsonParse {
            provider: "x".into(),
            model: "y".into(),
            body_snippet: "z".into(),
            source: serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err(),
        },
        LlmError::RetriesExhausted { attempts: 0 },
        LlmError::Config("x".into()),
        LlmError::RequestBodySizeExceeded { estimated_bytes: 0, max_bytes: 0 },
        LlmError::Cancelled,
        LlmError::HttpClientInit("x".into()),
        LlmError::Other("x".into()),
    ];

    for err in errors {
        let action = err.suggested_action();
        assert!(
            !action.is_empty(),
            "THEOREM: suggested_action() MUST NEVER return empty string for {:?}",
            err
        );
    }
}

#[test]
fn test_kani_from_http_status_rfc9110_compliance() {
    assert!(
        matches!(
            LlmError::from_http_status(StatusCode::GATEWAY_TIMEOUT, ""),
            LlmError::Timeout(_)
        ),
        "THEOREM: HTTP 504 MUST map to Timeout per RFC 9110 §15.6.5"
    );

    for status in [StatusCode::INTERNAL_SERVER_ERROR, StatusCode::BAD_GATEWAY, StatusCode::SERVICE_UNAVAILABLE] {
        assert!(
            matches!(
                LlmError::from_http_status(status, ""),
                LlmError::ServerOverloaded
            ),
            "THEOREM: HTTP 5xx ({}) MUST map to ServerOverloaded",
            status
        );
    }

    assert!(
        matches!(
            LlmError::from_http_status(StatusCode::UNAUTHORIZED, ""),
            LlmError::AuthenticationError { .. }
        ),
        "THEOREM: HTTP 401 MUST map to AuthenticationError"
    );

    assert!(
        matches!(
            LlmError::from_http_status(StatusCode::FORBIDDEN, ""),
            LlmError::PermissionError { .. }
        ),
        "THEOREM: HTTP 403 MUST map to PermissionError"
    );

    assert!(
        matches!(
            LlmError::from_http_status(StatusCode::TOO_MANY_REQUESTS, ""),
            LlmError::RateLimitExceeded { .. }
        ),
        "THEOREM: HTTP 429 MUST map to RateLimitExceeded"
    );
}

#[test]
fn test_kani_timeout_and_server_overloaded_partition() {
    let timeout_err = LlmError::from_http_status(StatusCode::GATEWAY_TIMEOUT, "");
    let server_err = LlmError::from_http_status(StatusCode::INTERNAL_SERVER_ERROR, "");

    assert_ne!(
        timeout_err.safe_failure_class(),
        server_err.safe_failure_class(),
        "THEOREM: 504 (Timeout) and 500 (ServerOverloaded) MUST route to different failure classes"
    );

    assert_eq!(
        timeout_err.safe_failure_class(), "timeout",
        "THEOREM: HTTP 504 MUST have failure_class 'timeout'"
    );

    assert_eq!(
        server_err.safe_failure_class(), "server_overloaded",
        "THEOREM: HTTP 5xx MUST have failure_class 'server_overloaded'"
    );
}

#[test]
fn test_kani_context_window_detection() {
    let context_window_bodies = [
        "context_length_exceeded",
        "context window exceeded",
        "maximum context length",
        "too many tokens",
        "token limit exceeded",
        "reduce the length",
    ];

    for body in context_window_bodies {
        let err = LlmError::from_http_status(StatusCode::BAD_REQUEST, body);
        assert!(
            err.is_context_window_failure(),
            "THEOREM: body containing '{}' MUST be classified as context window error",
            body
        );
    }
}

#[test]
fn test_deductive_is_retryable_partition_completeness() {
    let all_errors: Vec<LlmError> = vec![
        LlmError::Network("x".into()),
        LlmError::Timeout(0),
        LlmError::RateLimitExceeded { retry_after_ms: None },
        LlmError::AuthenticationError { message: "x".into() },
        LlmError::PermissionError { message: "x".into() },
        LlmError::ContextWindowExceeded { estimated_tokens: 0, limit_tokens: 0 },
        LlmError::PromptTooLarge(0),
        LlmError::ModelUnavailable("x".into()),
        LlmError::ServerOverloaded,
        LlmError::InvalidRequest { message: "x".into() },
        LlmError::StreamError("x".into()),
        LlmError::ToolCallError { message: "x".into(), tool_name: None },
        LlmError::MissingCredentials { provider: "x".into(), env_vars: vec![] },
        LlmError::ExpiredToken("x".into()),
        LlmError::JsonParse {
            provider: "x".into(),
            model: "y".into(),
            body_snippet: "z".into(),
            source: serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err(),
        },
        LlmError::RetriesExhausted { attempts: 0 },
        LlmError::Config("x".into()),
        LlmError::RequestBodySizeExceeded { estimated_bytes: 0, max_bytes: 0 },
        LlmError::Cancelled,
        LlmError::HttpClientInit("x".into()),
        LlmError::Other("x".into()),
    ];

    for err in all_errors {
        let is_retryable = err.is_retryable();
        let is_context = err.is_context_window_failure();
        let safe_class = err.safe_failure_class();

        assert!(
            !safe_class.is_empty(),
            "INVARIANT: safe_failure_class never empty"
        );
        assert!(
            is_retryable || !is_retryable,
            "THEOREM: is_retryable MUST return deterministically for {:?}",
            err
        );
    }
}

#[test]
fn test_deductive_from_http_status_504_not_server_overloaded() {
    let err_504 = LlmError::from_http_status(StatusCode::GATEWAY_TIMEOUT, "timeout");

    assert!(
        !matches!(err_504, LlmError::ServerOverloaded),
        "THEOREM: HTTP 504 MUST NOT map to ServerOverloaded (RFC 9110 §15.6.5)"
    );

    assert!(
        matches!(err_504, LlmError::Timeout(_)),
        "THEOREM: HTTP 504 MUST map to Timeout"
    );
}

#[test]
fn test_deductive_safe_failure_class_stability() {
    let err = LlmError::Network("test".into());

    let class1 = err.safe_failure_class();
    let class2 = err.safe_failure_class();
    let class3 = err.safe_failure_class();

    assert_eq!(
        class1, class2,
        "THEOREM: safe_failure_class MUST be deterministic"
    );
    assert_eq!(
        class2, class3,
        "THEOREM: safe_failure_class MUST return same value on repeated calls"
    );
}

#[test]
fn test_deductive_is_context_window_failure_partition() {
    let context_errors = [
        LlmError::ContextWindowExceeded {
            estimated_tokens: 100,
            limit_tokens: 50,
        },
        LlmError::PromptTooLarge(9999),
    ];

    let non_context_errors = [
        LlmError::Network("x".into()),
        LlmError::Timeout(0),
        LlmError::ServerOverloaded,
        LlmError::InvalidRequest { message: "x".into() },
    ];

    for err in context_errors {
        assert!(
            err.is_context_window_failure(),
            "THEOREM: {:?} MUST be context window failure",
            err
        );
    }

    for err in non_context_errors {
        assert!(
            !err.is_context_window_failure(),
            "THEOREM: {:?} MUST NOT be context window failure",
            err
        );
    }
}

#[test]
fn test_deductive_rate_limit_error_retryable() {
    let err_with_retry = LlmError::RateLimitExceeded {
        retry_after_ms: Some(5000),
    };
    let err_without_retry = LlmError::RateLimitExceeded { retry_after_ms: None };

    assert!(
        err_with_retry.is_retryable(),
        "THEOREM: RateLimitExceeded with retry_after_ms is retryable"
    );
    assert!(
        err_without_retry.is_retryable(),
        "THEOREM: RateLimitExceeded without retry_after_ms is also retryable"
    );
}