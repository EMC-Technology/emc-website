use crate::error::LlmError;
use error_core::error_code::registry;
use error_core::prelude::*;

impl LlmError {
    /// 将 LLM 错误按四维分类（来源/严重度/影响范围/可恢复性/错误码）进行归类
    #[must_use]
    pub fn classify_4d(
        &self,
    ) -> (
        ErrorSource,
        Severity,
        ImpactScope,
        Recoverability,
        &'static str,
    ) {
        (
            self.error_source(),
            self.severity(),
            self.impact_scope(),
            self.recoverability(),
            self.error_code(),
        )
    }

    #[must_use]
    fn error_source(&self) -> ErrorSource {
        match self {
            LlmError::Network(_) | LlmError::Timeout(_) | LlmError::StreamError(_) => {
                ErrorSource::NET
            }
            LlmError::RateLimitExceeded { .. }
            | LlmError::ModelUnavailable(_)
            | LlmError::ServerOverloaded
            | LlmError::JsonParse { .. } => ErrorSource::EXT,
            LlmError::AuthenticationError { .. }
            | LlmError::PermissionError { .. }
            | LlmError::ExpiredToken(_) => ErrorSource::SEC,
            LlmError::ContextWindowExceeded { .. }
            | LlmError::PromptTooLarge(_)
            | LlmError::RequestBodySizeExceeded { .. } => ErrorSource::AIM,
            LlmError::InvalidRequest { .. } => ErrorSource::USR,
            LlmError::ToolCallError { .. } => ErrorSource::TOOL,
            LlmError::MissingCredentials { .. }
            | LlmError::Config(_)
            | LlmError::HttpClientInit(_) => ErrorSource::CFG,
            LlmError::RetriesExhausted { .. } | LlmError::Cancelled => ErrorSource::SYS,
            LlmError::Other(_) => ErrorSource::UNK,
        }
    }

    #[must_use]
    fn severity(&self) -> Severity {
        match self {
            LlmError::AuthenticationError { .. }
            | LlmError::PermissionError { .. }
            | LlmError::MissingCredentials { .. } => Severity::CRITICAL,
            LlmError::Network(_)
            | LlmError::ContextWindowExceeded { .. }
            | LlmError::ModelUnavailable(_)
            | LlmError::ToolCallError { .. }
            | LlmError::RetriesExhausted { .. }
            | LlmError::Config(_)
            | LlmError::HttpClientInit(_) => Severity::ERROR,
            LlmError::Timeout(_)
            | LlmError::RateLimitExceeded { .. }
            | LlmError::PromptTooLarge(_)
            | LlmError::ServerOverloaded
            | LlmError::StreamError(_)
            | LlmError::ExpiredToken(_)
            | LlmError::RequestBodySizeExceeded { .. }
            | LlmError::Other(_) => Severity::WARNING,
            LlmError::InvalidRequest { .. } | LlmError::JsonParse { .. } | LlmError::Cancelled => {
                Severity::INFO
            }
        }
    }

    #[must_use]
    fn impact_scope(&self) -> ImpactScope {
        match self {
            LlmError::AuthenticationError { .. }
            | LlmError::PermissionError { .. }
            | LlmError::MissingCredentials { .. }
            | LlmError::ExpiredToken(_)
            | LlmError::Config(_)
            | LlmError::HttpClientInit(_) => ImpactScope::SESSION,
            _ => ImpactScope::OPERATION,
        }
    }

    #[must_use]
    fn recoverability(&self) -> Recoverability {
        match self {
            LlmError::Network(_)
            | LlmError::Timeout(_)
            | LlmError::RateLimitExceeded { .. }
            | LlmError::ModelUnavailable(_)
            | LlmError::ServerOverloaded
            | LlmError::StreamError(_)
            | LlmError::Cancelled => Recoverability::AutoRecoverable,
            LlmError::ContextWindowExceeded { .. }
            | LlmError::PromptTooLarge(_)
            | LlmError::ToolCallError { .. }
            | LlmError::ExpiredToken(_)
            | LlmError::RequestBodySizeExceeded { .. }
            | LlmError::Other(_) => Recoverability::SemiAuto,
            LlmError::RetriesExhausted { .. } => Recoverability::ManualIntervention,
            LlmError::AuthenticationError { .. }
            | LlmError::PermissionError { .. }
            | LlmError::InvalidRequest { .. }
            | LlmError::MissingCredentials { .. }
            | LlmError::JsonParse { .. }
            | LlmError::Config(_)
            | LlmError::HttpClientInit(_) => Recoverability::NonRecoverable,
        }
    }

    #[must_use]
    fn error_code(&self) -> &'static str {
        match self {
            LlmError::Network(_) => registry::ULLM_NETWORK,
            LlmError::Timeout(_) => registry::ULLM_TIMEOUT,
            LlmError::RateLimitExceeded { .. } => registry::ULLM_RATE_LIMIT,
            LlmError::AuthenticationError { .. } => registry::ULLM_AUTH,
            LlmError::PermissionError { .. } => registry::ULLM_PERMISSION,
            LlmError::ContextWindowExceeded { .. } => registry::ULLM_CONTEXT_WINDOW,
            LlmError::PromptTooLarge(_) => registry::ULLM_PROMPT_TOO_LARGE,
            LlmError::ModelUnavailable(_) => registry::ULLM_MODEL_UNAVAILABLE,
            LlmError::ServerOverloaded => registry::ULLM_SERVER_OVERLOADED,
            LlmError::InvalidRequest { .. } => registry::ULLM_INVALID_REQUEST,
            LlmError::StreamError(_) => registry::ULLM_STREAM,
            LlmError::ToolCallError { .. } => registry::ULLM_TOOL_CALL,
            LlmError::MissingCredentials { .. } => registry::ULLM_MISSING_CREDENTIALS,
            LlmError::ExpiredToken(_) => registry::ULLM_EXPIRED_TOKEN,
            LlmError::JsonParse { .. } => registry::ULLM_JSON_PARSE,
            LlmError::RetriesExhausted { .. } => registry::ULLM_RETRIES_EXHAUSTED,
            LlmError::Config(_) => registry::ULLM_CONFIG,
            LlmError::RequestBodySizeExceeded { .. } => registry::ULLM_REQUEST_BODY_SIZE,
            LlmError::Cancelled => registry::ULLM_CANCELLED,
            LlmError::Other(_) => registry::ULLM_OTHER,
            LlmError::HttpClientInit(_) => registry::ULLM_HTTP_CLIENT_INIT,
        }
    }

    /// 将 `LlmError` 转换为结构化 `ErrorObject`
    ///
    /// # Panics
    ///
    /// 若 `ErrorObject::build_checked()` 缺少必填字段则 panic（当前实现保证所有字段已设置）。
    #[must_use]
    pub fn to_error_object(&self) -> ErrorObject {
        let (source, severity, scope, recoverability, code) = self.classify_4d();
        let message = self.to_string();
        let operation = self.operation_name();

        ErrorObject::builder()
            .code(code)
            .source(source)
            .severity(severity)
            .impact_scope(scope)
            .recoverability(recoverability)
            .message(&message)
            .user_message(&message)
            .module_path("ullm")
            .operation(operation)
            .detail(
                "llm_failure_class",
                serde_json::Value::String(self.operation_name().to_string()),
            )
            .detail("is_retryable", serde_json::Value::Bool(self.is_retryable()))
            .build_checked()
            .expect("all required fields are set")
    }

    fn operation_name(&self) -> &'static str {
        match self {
            LlmError::Network(_) => "network",
            LlmError::Timeout(_) => "timeout",
            LlmError::RateLimitExceeded { .. } => "rate_limit",
            LlmError::AuthenticationError { .. } => "authentication",
            LlmError::PermissionError { .. } => "permission",
            LlmError::ContextWindowExceeded { .. } => "context_window",
            LlmError::PromptTooLarge(_) => "prompt_size",
            LlmError::ModelUnavailable(_) => "model_access",
            LlmError::ServerOverloaded => "server_load",
            LlmError::InvalidRequest { .. } => "request_validation",
            LlmError::StreamError(_) => "stream",
            LlmError::ToolCallError { .. } => "tool_call",
            LlmError::MissingCredentials { .. } => "credential_check",
            LlmError::ExpiredToken(_) => "token_refresh",
            LlmError::JsonParse { .. } => "response_parse",
            LlmError::RetriesExhausted { .. } => "retry_policy",
            LlmError::Config(_) => "config_load",
            LlmError::RequestBodySizeExceeded { .. } => "request_size",
            LlmError::Cancelled => "cancellation",
            LlmError::Other(_) => "unknown",
            LlmError::HttpClientInit(_) => "http_client_init",
        }
    }
}

impl From<LlmError> for ErrorObject {
    fn from(err: LlmError) -> Self {
        err.to_error_object()
    }
}

impl From<ErrorObject> for LlmError {
    fn from(obj: ErrorObject) -> Self {
        match obj.code() {
            registry::ULLM_NETWORK => LlmError::Network(obj.message().to_string()),
            registry::ULLM_TIMEOUT => LlmError::Timeout(0),
            registry::ULLM_RATE_LIMIT => LlmError::RateLimitExceeded {
                retry_after_ms: None,
            },
            registry::ULLM_AUTH => LlmError::AuthenticationError {
                message: obj.message().to_string(),
            },
            registry::ULLM_PERMISSION => LlmError::PermissionError {
                message: obj.message().to_string(),
            },
            registry::ULLM_CONTEXT_WINDOW => LlmError::ContextWindowExceeded {
                estimated_tokens: 0,
                limit_tokens: 0,
            },
            registry::ULLM_PROMPT_TOO_LARGE => LlmError::PromptTooLarge(0),
            registry::ULLM_MODEL_UNAVAILABLE => {
                LlmError::ModelUnavailable(obj.message().to_string())
            }
            registry::ULLM_SERVER_OVERLOADED => LlmError::ServerOverloaded,
            registry::ULLM_INVALID_REQUEST => LlmError::InvalidRequest {
                message: obj.message().to_string(),
            },
            registry::ULLM_STREAM => LlmError::StreamError(obj.message().to_string()),
            registry::ULLM_TOOL_CALL => LlmError::ToolCallError {
                message: obj.message().to_string(),
                tool_name: None,
            },
            registry::ULLM_MISSING_CREDENTIALS => LlmError::MissingCredentials {
                provider: String::new(),
                env_vars: vec![],
            },
            registry::ULLM_EXPIRED_TOKEN => LlmError::ExpiredToken(obj.message().to_string()),
            registry::ULLM_JSON_PARSE => LlmError::JsonParse {
                provider: String::new(),
                model: String::new(),
                body_snippet: String::new(),
                source: serde_json::from_str::<serde_json::Value>("{}").unwrap_err(),
            },
            registry::ULLM_RETRIES_EXHAUSTED => LlmError::RetriesExhausted { attempts: 0 },
            registry::ULLM_CONFIG => LlmError::Config(obj.message().to_string()),
            registry::ULLM_REQUEST_BODY_SIZE => LlmError::RequestBodySizeExceeded {
                estimated_bytes: 0,
                max_bytes: 0,
            },
            registry::ULLM_CANCELLED => LlmError::Cancelled,
            registry::ULLM_HTTP_CLIENT_INIT => LlmError::HttpClientInit(obj.message().to_string()),
            _ => LlmError::Other(obj.message().to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_4d_network() {
        let err = LlmError::Network("conn refused".into());
        let (source, severity, scope, recoverability, code) = err.classify_4d();

        assert_eq!(source, ErrorSource::NET);
        assert_eq!(severity, Severity::ERROR);
        assert_eq!(scope, ImpactScope::OPERATION);
        assert_eq!(recoverability, Recoverability::AutoRecoverable);
        assert_eq!(code, "ERR-NET-LLM-001_ERR_O");
    }

    #[allow(clippy::too_many_lines)]
    #[test]
    fn test_classify_4d_all_variants() {
        let cases: Vec<(
            LlmError,
            ErrorSource,
            Severity,
            ImpactScope,
            Recoverability,
            &str,
        )> = vec![
            (
                LlmError::Network("x".into()),
                ErrorSource::NET,
                Severity::ERROR,
                ImpactScope::OPERATION,
                Recoverability::AutoRecoverable,
                "ERR-NET-LLM-001_ERR_O",
            ),
            (
                LlmError::Timeout(100),
                ErrorSource::NET,
                Severity::WARNING,
                ImpactScope::OPERATION,
                Recoverability::AutoRecoverable,
                "ERR-NET-LLM-002_WRN_O",
            ),
            (
                LlmError::RateLimitExceeded {
                    retry_after_ms: None,
                },
                ErrorSource::EXT,
                Severity::WARNING,
                ImpactScope::OPERATION,
                Recoverability::AutoRecoverable,
                "ERR-EXT-LLM-003_WRN_O",
            ),
            (
                LlmError::AuthenticationError {
                    message: "x".into(),
                },
                ErrorSource::SEC,
                Severity::CRITICAL,
                ImpactScope::SESSION,
                Recoverability::NonRecoverable,
                "ERR-SEC-LLM-004_CRI_S",
            ),
            (
                LlmError::PermissionError {
                    message: "x".into(),
                },
                ErrorSource::SEC,
                Severity::CRITICAL,
                ImpactScope::SESSION,
                Recoverability::NonRecoverable,
                "ERR-SEC-LLM-005_CRI_S",
            ),
            (
                LlmError::ContextWindowExceeded {
                    estimated_tokens: 1,
                    limit_tokens: 2,
                },
                ErrorSource::AIM,
                Severity::ERROR,
                ImpactScope::OPERATION,
                Recoverability::SemiAuto,
                "ERR-AIM-LLM-006_ERR_O",
            ),
            (
                LlmError::PromptTooLarge(1),
                ErrorSource::AIM,
                Severity::WARNING,
                ImpactScope::OPERATION,
                Recoverability::SemiAuto,
                "ERR-AIM-LLM-007_WRN_O",
            ),
            (
                LlmError::ModelUnavailable("x".into()),
                ErrorSource::EXT,
                Severity::ERROR,
                ImpactScope::OPERATION,
                Recoverability::AutoRecoverable,
                "ERR-EXT-LLM-008_ERR_O",
            ),
            (
                LlmError::ServerOverloaded,
                ErrorSource::EXT,
                Severity::WARNING,
                ImpactScope::OPERATION,
                Recoverability::AutoRecoverable,
                "ERR-EXT-LLM-009_WRN_O",
            ),
            (
                LlmError::InvalidRequest {
                    message: "x".into(),
                },
                ErrorSource::USR,
                Severity::INFO,
                ImpactScope::OPERATION,
                Recoverability::NonRecoverable,
                "ERR-USR-LLM-010_INF_O",
            ),
            (
                LlmError::StreamError("x".into()),
                ErrorSource::NET,
                Severity::WARNING,
                ImpactScope::OPERATION,
                Recoverability::AutoRecoverable,
                "ERR-NET-LLM-011_WRN_O",
            ),
            (
                LlmError::ToolCallError {
                    message: "x".into(),
                    tool_name: None,
                },
                ErrorSource::TOOL,
                Severity::ERROR,
                ImpactScope::OPERATION,
                Recoverability::SemiAuto,
                "ERR-TOOL-LLM-012_ERR_O",
            ),
            (
                LlmError::MissingCredentials {
                    provider: "x".into(),
                    env_vars: vec![],
                },
                ErrorSource::CFG,
                Severity::CRITICAL,
                ImpactScope::SESSION,
                Recoverability::NonRecoverable,
                "ERR-CFG-LLM-013_CRI_S",
            ),
            (
                LlmError::ExpiredToken("x".into()),
                ErrorSource::SEC,
                Severity::WARNING,
                ImpactScope::SESSION,
                Recoverability::SemiAuto,
                "ERR-SEC-LLM-014_WRN_S",
            ),
            (
                LlmError::JsonParse {
                    provider: "x".into(),
                    model: "y".into(),
                    body_snippet: "z".into(),
                    source: serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err(),
                },
                ErrorSource::EXT,
                Severity::INFO,
                ImpactScope::OPERATION,
                Recoverability::NonRecoverable,
                "ERR-EXT-LLM-015_INF_O",
            ),
            (
                LlmError::RetriesExhausted { attempts: 3 },
                ErrorSource::SYS,
                Severity::ERROR,
                ImpactScope::OPERATION,
                Recoverability::ManualIntervention,
                "ERR-SYS-LLM-016_ERR_O",
            ),
            (
                LlmError::Config("x".into()),
                ErrorSource::CFG,
                Severity::ERROR,
                ImpactScope::SESSION,
                Recoverability::NonRecoverable,
                "ERR-CFG-LLM-017_ERR_S",
            ),
            (
                LlmError::RequestBodySizeExceeded {
                    estimated_bytes: 100,
                    max_bytes: 50,
                },
                ErrorSource::AIM,
                Severity::WARNING,
                ImpactScope::OPERATION,
                Recoverability::SemiAuto,
                "ERR-AIM-LLM-018_WRN_O",
            ),
            (
                LlmError::Cancelled,
                ErrorSource::SYS,
                Severity::INFO,
                ImpactScope::OPERATION,
                Recoverability::AutoRecoverable,
                "ERR-SYS-LLM-019_INF_O",
            ),
            (
                LlmError::Other("x".into()),
                ErrorSource::UNK,
                Severity::WARNING,
                ImpactScope::OPERATION,
                Recoverability::SemiAuto,
                "ERR-UNK-LLM-020_WRN_O",
            ),
        ];

        for (err, exp_source, exp_sev, exp_scope, exp_recov, exp_code) in cases {
            let (source, severity, scope, recoverability, code) = err.classify_4d();
            assert_eq!(source, exp_source, "source mismatch for {err:?}");
            assert_eq!(severity, exp_sev, "severity mismatch for {err:?}");
            assert_eq!(scope, exp_scope, "scope mismatch for {err:?}");
            assert_eq!(
                recoverability, exp_recov,
                "recoverability mismatch for {err:?}"
            );
            assert_eq!(code, exp_code, "code mismatch for {err:?}");
        }
    }

    #[test]
    fn test_classify_4d_error_code_format() {
        let err = LlmError::Network("x".into());
        let (_, _, _, _, code) = err.classify_4d();
        assert!(code.starts_with("ERR-"), "code must start with ERR-");
        assert!(code.contains("-LLM-"), "code must contain -LLM-");
        let parts: Vec<&str> = code.split('_').collect();
        assert!(
            parts.len() >= 3,
            "code must have at least 3 underscore-separated parts"
        );
    }

    #[test]
    fn test_to_error_object_network() {
        let err = LlmError::Network("connection refused".into());
        let obj = err.to_error_object();

        assert_eq!(obj.code(), "ERR-NET-LLM-001_ERR_O");
        assert_eq!(obj.source(), ErrorSource::NET);
        assert_eq!(obj.severity(), Severity::ERROR);
        assert_eq!(obj.impact_scope(), ImpactScope::OPERATION);
        assert_eq!(obj.recoverability(), Recoverability::AutoRecoverable);
        assert_eq!(obj.message(), "network error: connection refused");
        assert_eq!(obj.module_path(), "ullm");
        assert_eq!(obj.operation(), "network");
    }

    #[test]
    fn test_to_error_object_auth() {
        let err = LlmError::AuthenticationError {
            message: "bad key".into(),
        };
        let obj = err.to_error_object();

        assert_eq!(obj.code(), "ERR-SEC-LLM-004_CRI_S");
        assert_eq!(obj.source(), ErrorSource::SEC);
        assert_eq!(obj.severity(), Severity::CRITICAL);
        assert!(obj.message().contains("bad key"));
    }

    #[test]
    fn test_from_llm_error_for_error_object() {
        let err = LlmError::RateLimitExceeded {
            retry_after_ms: Some(5000),
        };
        let obj: ErrorObject = err.into();

        assert_eq!(obj.code(), "ERR-EXT-LLM-003_WRN_O");
        assert_eq!(obj.source(), ErrorSource::EXT);
        assert_eq!(obj.module_path(), "ullm");
    }

    #[test]
    fn test_registry_constants_match_classify_4d() {
        assert_eq!(registry::ULLM_NETWORK, "ERR-NET-LLM-001_ERR_O");
        assert_eq!(registry::ULLM_TIMEOUT, "ERR-NET-LLM-002_WRN_O");
        assert_eq!(registry::ULLM_RATE_LIMIT, "ERR-EXT-LLM-003_WRN_O");
        assert_eq!(registry::ULLM_AUTH, "ERR-SEC-LLM-004_CRI_S");
        assert_eq!(registry::ULLM_CANCELLED, "ERR-SYS-LLM-019_INF_O");
        assert_eq!(registry::ULLM_OTHER, "ERR-UNK-LLM-020_WRN_O");
    }

    #[test]
    fn test_from_error_object_to_llm_error_network() {
        let obj = LlmError::Network("timeout".into()).to_error_object();
        let llm_err: LlmError = obj.into();
        assert!(matches!(llm_err, LlmError::Network(_)));
    }

    #[test]
    fn test_from_error_object_to_llm_error_auth() {
        let obj = LlmError::AuthenticationError {
            message: "bad".into(),
        }
        .to_error_object();
        let llm_err: LlmError = obj.into();
        assert!(matches!(llm_err, LlmError::AuthenticationError { .. }));
    }

    #[test]
    fn test_from_error_object_to_llm_error_unknown_code() {
        let obj = ErrorObject::builder()
            .code("ERR-UNK-XXXX-999_WRN_O")
            .source(ErrorSource::UNK)
            .severity(Severity::WARNING)
            .impact_scope(ImpactScope::OPERATION)
            .recoverability(Recoverability::SemiAuto)
            .message("unknown error")
            .user_message("unknown error")
            .module_path("test")
            .operation("test")
            .build_checked()
            .expect("all required fields are set");
        let llm_err: LlmError = obj.into();
        assert!(matches!(llm_err, LlmError::Other(_)));
    }

    #[test]
    fn test_roundtrip_network() {
        let original = LlmError::Network("conn refused".into());
        let obj: ErrorObject = original.into();
        let roundtrip: LlmError = obj.into();
        assert!(matches!(roundtrip, LlmError::Network(_)));
        if let LlmError::Network(msg) = roundtrip {
            assert!(msg.contains("conn refused"));
        }
    }

    #[test]
    fn test_roundtrip_auth() {
        let original = LlmError::AuthenticationError {
            message: "bad key".into(),
        };
        let obj: ErrorObject = original.into();
        let roundtrip: LlmError = obj.into();
        assert!(matches!(roundtrip, LlmError::AuthenticationError { .. }));
        if let LlmError::AuthenticationError { message } = roundtrip {
            assert!(message.contains("bad key"));
        }
    }

    #[test]
    fn test_roundtrip_rate_limit() {
        let original = LlmError::RateLimitExceeded {
            retry_after_ms: Some(5000),
        };
        let obj: ErrorObject = original.into();
        let roundtrip: LlmError = obj.into();
        assert!(matches!(roundtrip, LlmError::RateLimitExceeded { .. }));
    }

    #[test]
    fn test_roundtrip_cancelled() {
        let original = LlmError::Cancelled;
        let obj: ErrorObject = original.into();
        let roundtrip: LlmError = obj.into();
        assert!(matches!(roundtrip, LlmError::Cancelled));
    }

    #[test]
    fn test_roundtrip_preserves_error_code() {
        let original = LlmError::Network("x".into());
        let obj: ErrorObject = original.into();
        assert_eq!(obj.code(), registry::ULLM_NETWORK);
        let roundtrip: LlmError = obj.into();
        let (_, _, _, _, code) = roundtrip.classify_4d();
        assert_eq!(code, registry::ULLM_NETWORK);
    }
}
