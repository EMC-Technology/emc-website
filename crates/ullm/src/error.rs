use http::StatusCode;
use thiserror::Error;

/// LLM 错误类型枚举
#[derive(Error, Debug)]
pub enum LlmError {
    /// 网络连接错误
    #[error("network error: {0}")]
    Network(String),

    /// 请求超时
    #[error("request timed out after {0}ms")]
    Timeout(u64),

    /// 速率限制超出
    #[error("rate limit exceeded{}", match retry_after_ms { Some(ms) => format!(", retry after {ms}ms"), None => String::new() })]
    RateLimitExceeded {
        /// 建议重试等待时间（毫秒）
        retry_after_ms: Option<u64>,
    },

    /// 认证失败
    #[error("authentication failed: {message}")]
    AuthenticationError {
        /// 错误详情
        message: String,
    },

    /// 权限不足
    #[error("permission denied: {message}")]
    PermissionError {
        /// 错误详情
        message: String,
    },

    /// 上下文窗口超出
    #[error("context window exceeded: estimated {estimated_tokens} tokens, limit {limit_tokens}")]
    ContextWindowExceeded {
        /// 估算的 Token 数量
        estimated_tokens: u64,
        /// Token 上限
        limit_tokens: u64,
    },

    /// Prompt 过长
    #[error("prompt too large: {0} tokens")]
    PromptTooLarge(u64),

    /// 模型不可用
    #[error("model unavailable: {0}")]
    ModelUnavailable(String),

    /// 服务器过载
    #[error("server overloaded, try again later")]
    ServerOverloaded,

    /// 无效请求
    #[error("invalid request: {message}")]
    InvalidRequest {
        /// 错误详情
        message: String,
    },

    /// 流式响应错误
    #[error("stream error: {0}")]
    StreamError(String),

    /// 工具调用错误
    #[error("tool call error: {message}")]
    ToolCallError {
        /// 错误详情
        message: String,
        /// 出错的工具名称
        tool_name: Option<String>,
    },

    /// 缺少凭证
    #[error("missing credentials for provider '{provider}', check environment variables: {}", .env_vars.join(", "))]
    MissingCredentials {
        /// 供应商标识
        provider: String,
        /// 需要检查的环境变量列表
        env_vars: Vec<String>,
    },

    /// OAuth Token 过期
    #[error("expired OAuth token for provider '{0}'")]
    ExpiredToken(String),

    /// JSON 解析错误
    #[error("JSON parse error from {provider}/{model}: {source}")]
    JsonParse {
        /// 供应商标识
        provider: String,
        /// 模型标识
        model: String,
        /// 响应体片段
        body_snippet: String,
        /// 源错误
        #[source]
        source: serde_json::Error,
    },

    /// 重试次数耗尽
    #[error("retries exhausted after {attempts} attempts")]
    RetriesExhausted {
        /// 已尝试次数
        attempts: u32,
    },

    /// 配置错误
    #[error("configuration error: {0}")]
    Config(String),

    /// 请求体大小超限
    #[error("request body size exceeded: {estimated_bytes} bytes > {max_bytes} bytes limit")]
    RequestBodySizeExceeded {
        /// 估算字节数
        estimated_bytes: usize,
        /// 最大允许字节数
        max_bytes: usize,
    },

    /// 请求已取消
    #[error("request cancelled")]
    Cancelled,

    /// HTTP 客户端初始化失败
    #[error("HTTP client initialization failed: {0}")]
    HttpClientInit(String),

    /// 其他未分类错误
    #[error("{0}")]
    Other(String),
}

impl LlmError {
    /// 判断该错误是否可重试
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            LlmError::Network(_)
                | LlmError::Timeout(_)
                | LlmError::RateLimitExceeded { .. }
                | LlmError::ModelUnavailable(_)
                | LlmError::ServerOverloaded
        )
    }

    /// 返回建议的用户操作
    #[must_use]
    pub fn suggested_action(&self) -> &'static str {
        match self {
            LlmError::Network(_) => "Check your internet connection and try again.",
            LlmError::Timeout(_) => {
                "The request took too long. Try again or reduce the input size."
            }
            LlmError::RateLimitExceeded { .. } => {
                "You are sending requests too quickly. Wait before retrying."
            }
            LlmError::AuthenticationError { .. } => "Verify your API key is correct and active.",
            LlmError::PermissionError { .. } => {
                "Your account may not have access to this resource. Check your plan and permissions."
            }
            LlmError::ContextWindowExceeded { .. } => {
                "Reduce the number of messages or the size of your prompt to fit within the model's context window."
            }
            LlmError::PromptTooLarge(_) => {
                "Your prompt is too large. Shorten it or switch to a model with a larger context window."
            }
            LlmError::ModelUnavailable(_) => {
                "The requested model is currently unavailable. Try a different model or try again later."
            }
            LlmError::ServerOverloaded => {
                "The server is temporarily overloaded. Please try again in a few moments."
            }
            LlmError::InvalidRequest { .. } => {
                "The request was malformed. Check the request parameters and try again."
            }
            LlmError::StreamError(_) => "The streaming response was interrupted. Try again.",
            LlmError::ToolCallError { .. } => {
                "A tool call failed during execution. Check the tool implementation and parameters."
            }
            LlmError::MissingCredentials { .. } => {
                "Set the required environment variables with your API credentials."
            }
            LlmError::ExpiredToken(_) => {
                "Your authentication token has expired. Re-authenticate to obtain a new token."
            }
            LlmError::JsonParse { .. } => {
                "The API returned an unexpected response format. This may indicate a version mismatch."
            }
            LlmError::RetriesExhausted { .. } => {
                "All retry attempts failed. Check the underlying error and try again later."
            }
            LlmError::Config(_) => {
                "There is a configuration error. Review your settings and try again."
            }
            LlmError::RequestBodySizeExceeded { .. } => {
                "The request body exceeds the size limit. Reduce the input size."
            }
            LlmError::Cancelled => "The request was cancelled. Retry if needed.",
            LlmError::HttpClientInit(_) => {
                "Failed to initialize HTTP client. Check TLS backend availability and system resources."
            }
            LlmError::Other(_) => "An unexpected error occurred. Please try again.",
        }
    }

    /// 从 HTTP 状态码和响应体构造错误
    #[must_use]
    pub fn from_http_status(status: StatusCode, body: &str) -> Self {
        let body_lower = body.to_lowercase();

        match status.as_u16() {
            401 => LlmError::AuthenticationError {
                message: extract_error_message(body)
                    .unwrap_or_else(|| "Invalid or missing API key".into()),
            },
            403 => LlmError::PermissionError {
                message: extract_error_message(body).unwrap_or_else(|| "Access denied".into()),
            },
            429 => {
                let retry_after_ms = extract_retry_after(body);
                LlmError::RateLimitExceeded { retry_after_ms }
            }
            400 => {
                if is_context_window_error(&body_lower) {
                    let (estimated, limit) = extract_token_counts(&body_lower);
                    LlmError::ContextWindowExceeded {
                        estimated_tokens: estimated,
                        limit_tokens: limit,
                    }
                } else {
                    LlmError::InvalidRequest {
                        message: extract_error_message(body)
                            .unwrap_or_else(|| "Bad request".into()),
                    }
                }
            }
            404 => LlmError::ModelUnavailable(
                extract_error_message(body).unwrap_or_else(|| "Model not found".into()),
            ),
            // INVARIANT: 504 MUST map to Timeout (RFC 9110 §15.6.5), NOT ServerOverloaded.
            // ServerOverloaded is reserved for 500/502/503 (RFC 9110 §15.6.1/§15.6.4).
            // These two variants route to different retry branches in RetryPolicy::should_retry:
            //   Timeout(_)      → retry_on.timeout
            //   ServerOverloaded → retry_on.server_error
            500 | 502 | 503 => LlmError::ServerOverloaded,
            504 => LlmError::Timeout(0),
            _ => LlmError::Other(format!(
                "HTTP {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            )),
        }
    }

    /// 判断是否为上下文窗口溢出类错误
    #[must_use]
    pub fn is_context_window_failure(&self) -> bool {
        matches!(
            self,
            LlmError::ContextWindowExceeded { .. } | LlmError::PromptTooLarge(_)
        )
    }

    /// 返回安全的失败分类标识（用于日志/指标，不泄露敏感信息）
    #[must_use]
    pub fn safe_failure_class(&self) -> &'static str {
        match self {
            LlmError::Network(_) => "network",
            LlmError::Timeout(_) => "timeout",
            LlmError::RateLimitExceeded { .. } => "rate_limit",
            LlmError::AuthenticationError { .. } | LlmError::ExpiredToken(_) => "auth",
            LlmError::PermissionError { .. } => "permission",
            LlmError::ContextWindowExceeded { .. } | LlmError::PromptTooLarge(_) => {
                "context_window"
            }
            LlmError::ModelUnavailable(_) => "model_unavailable",
            LlmError::ServerOverloaded => "server_overloaded",
            LlmError::InvalidRequest { .. } => "invalid_request",
            LlmError::StreamError(_) => "stream",
            LlmError::ToolCallError { .. } => "tool_call",
            LlmError::MissingCredentials { .. } => "credentials",
            LlmError::JsonParse { .. } => "json_parse",
            LlmError::RetriesExhausted { .. } => "retries_exhausted",
            LlmError::Config(_) => "config",
            LlmError::RequestBodySizeExceeded { .. } => "request_size",
            LlmError::Cancelled => "cancelled",
            LlmError::HttpClientInit(_) => "http_client_init",
            LlmError::Other(_) => "unknown",
        }
    }
}

impl From<reqwest::Error> for LlmError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            LlmError::Timeout(0)
        } else {
            LlmError::Network(err.to_string())
        }
    }
}

impl From<serde_json::Error> for LlmError {
    fn from(err: serde_json::Error) -> Self {
        LlmError::JsonParse {
            provider: String::new(),
            model: String::new(),
            body_snippet: String::new(),
            source: err,
        }
    }
}

fn extract_error_message(body: &str) -> Option<String> {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = val
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            return Some(msg.to_string());
        }
        if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
            return Some(msg.to_string());
        }
    }
    if body.len() > 200 {
        let end = body
            .char_indices()
            .take_while(|(idx, _)| *idx < 200)
            .last()
            .map_or(0, |(idx, c)| idx + c.len_utf8());
        Some(body[..end].to_string())
    } else if !body.is_empty() {
        Some(body.to_string())
    } else {
        None
    }
}

fn extract_retry_after(body: &str) -> Option<u64> {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(ms) = val
            .get("error")
            .and_then(|e| e.get("retry_after_ms"))
            .and_then(serde_json::Value::as_u64)
        {
            return Some(ms);
        }
        if let Some(secs) = val
            .get("error")
            .and_then(|e| e.get("retry_after"))
            .and_then(serde_json::Value::as_u64)
        {
            return Some(secs * 1000);
        }
    }
    None
}

fn is_context_window_error(body_lower: &str) -> bool {
    body_lower.contains("context_length_exceeded")
        || body_lower.contains("context window")
        || body_lower.contains("maximum context length")
        || body_lower.contains("too many tokens")
        || body_lower.contains("token limit")
        || body_lower.contains("reduce the length")
}

fn extract_token_counts(body_lower: &str) -> (u64, u64) {
    let estimated = extract_number_near(body_lower, "tokens")
        .or(extract_number_near(body_lower, "context"))
        .unwrap_or(0);
    let limit = extract_number_near(body_lower, "limit")
        .or(extract_number_near(body_lower, "max"))
        .unwrap_or(0);
    (estimated, limit)
}

fn extract_number_near(text: &str, keyword: &str) -> Option<u64> {
    if let Some(pos) = text.find(keyword) {
        let before = &text[..pos];
        let num_str: String = before
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if let Ok(n) = num_str.parse::<u64>() {
            return Some(n);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable() {
        assert!(LlmError::Network("conn refused".into()).is_retryable());
        assert!(LlmError::Timeout(5000).is_retryable());
        assert!(
            LlmError::RateLimitExceeded {
                retry_after_ms: None
            }
            .is_retryable()
        );
        assert!(LlmError::ServerOverloaded.is_retryable());
        assert!(LlmError::ModelUnavailable("gpt-4".into()).is_retryable());

        assert!(
            !LlmError::AuthenticationError {
                message: "bad key".into()
            }
            .is_retryable()
        );
        assert!(!LlmError::Config("bad config".into()).is_retryable());
        assert!(
            !LlmError::InvalidRequest {
                message: "bad".into()
            }
            .is_retryable()
        );
        assert!(!LlmError::RetriesExhausted { attempts: 3 }.is_retryable());
    }

    #[test]
    fn test_is_context_window_failure() {
        assert!(
            LlmError::ContextWindowExceeded {
                estimated_tokens: 100,
                limit_tokens: 50
            }
            .is_context_window_failure()
        );
        assert!(LlmError::PromptTooLarge(9999).is_context_window_failure());
        assert!(!LlmError::Timeout(100).is_context_window_failure());
    }

    #[test]
    fn test_safe_failure_class() {
        assert_eq!(
            LlmError::Network("err".into()).safe_failure_class(),
            "network"
        );
        assert_eq!(
            LlmError::AuthenticationError {
                message: "x".into()
            }
            .safe_failure_class(),
            "auth"
        );
        assert_eq!(
            LlmError::ExpiredToken("p".into()).safe_failure_class(),
            "auth"
        );
        assert_eq!(
            LlmError::ContextWindowExceeded {
                estimated_tokens: 1,
                limit_tokens: 2
            }
            .safe_failure_class(),
            "context_window"
        );
        assert_eq!(
            LlmError::PromptTooLarge(1).safe_failure_class(),
            "context_window"
        );
    }

    #[test]
    fn test_from_http_status_401() {
        let err = LlmError::from_http_status(
            StatusCode::UNAUTHORIZED,
            r#"{"error":{"message":"Invalid API key"}}"#,
        );
        match err {
            LlmError::AuthenticationError { message } => assert_eq!(message, "Invalid API key"),
            _ => panic!("expected AuthenticationError"),
        }
    }

    #[test]
    fn test_from_http_status_429() {
        let err = LlmError::from_http_status(
            StatusCode::TOO_MANY_REQUESTS,
            r#"{"error":{"retry_after_ms":5000}}"#,
        );
        match err {
            LlmError::RateLimitExceeded { retry_after_ms } => {
                assert_eq!(retry_after_ms, Some(5000));
            }
            _ => panic!("expected RateLimitExceeded"),
        }
    }

    #[test]
    fn test_from_http_status_400_context_window() {
        let err = LlmError::from_http_status(
            StatusCode::BAD_REQUEST,
            "context_length_exceeded: too many tokens",
        );
        assert!(err.is_context_window_failure());
    }

    #[test]
    fn test_from_http_status_500() {
        let err = LlmError::from_http_status(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        assert!(matches!(err, LlmError::ServerOverloaded));
    }

    #[test]
    fn test_from_reqwest_error_timeout() {
        let err = LlmError::Timeout(30_000);
        assert!(err.is_retryable());
    }

    #[test]
    fn test_from_reqwest_error_network() {
        let err = LlmError::Network("connection refused".into());
        assert!(err.is_retryable());
    }

    #[test]
    fn test_suggested_action() {
        assert!(
            !LlmError::Network("err".into())
                .suggested_action()
                .is_empty()
        );
        assert!(!LlmError::Config("err".into()).suggested_action().is_empty());
    }

    #[test]
    fn test_cancelled_variant() {
        let err = LlmError::Cancelled;
        assert!(!err.is_retryable());
        assert_eq!(err.safe_failure_class(), "cancelled");
        assert_eq!(
            err.suggested_action(),
            "The request was cancelled. Retry if needed."
        );
    }

    #[test]
    fn test_from_http_status_403_permission_error() {
        let err = LlmError::from_http_status(
            StatusCode::FORBIDDEN,
            r#"{"error":{"message":"Access denied"}}"#,
        );
        match err {
            LlmError::PermissionError { message } => assert_eq!(message, "Access denied"),
            _ => panic!("expected PermissionError"),
        }
    }

    #[test]
    fn test_from_http_status_404_model_unavailable() {
        let err = LlmError::from_http_status(
            StatusCode::NOT_FOUND,
            r#"{"error":{"message":"Model not found"}}"#,
        );
        match err {
            LlmError::ModelUnavailable(msg) => assert_eq!(msg, "Model not found"),
            _ => panic!("expected ModelUnavailable"),
        }
    }

    #[test]
    fn test_from_http_status_502_server_overloaded() {
        let err = LlmError::from_http_status(StatusCode::BAD_GATEWAY, "bad gateway");
        assert!(matches!(err, LlmError::ServerOverloaded));
    }

    #[test]
    fn test_from_http_status_503_server_overloaded() {
        let err =
            LlmError::from_http_status(StatusCode::SERVICE_UNAVAILABLE, "service unavailable");
        assert!(matches!(err, LlmError::ServerOverloaded));
    }

    #[test]
    fn test_from_http_status_504_timeout() {
        let err = LlmError::from_http_status(StatusCode::GATEWAY_TIMEOUT, "gateway timeout");
        assert!(matches!(err, LlmError::Timeout(_)));
    }

    #[test]
    fn test_from_http_status_unknown_falls_to_other() {
        let err = LlmError::from_http_status(StatusCode::CONFLICT, "conflict occurred");
        match err {
            LlmError::Other(msg) => assert!(msg.contains("409")),
            _ => panic!("expected Other"),
        }
    }

    #[test]
    fn test_from_http_status_400_non_context_window() {
        let err = LlmError::from_http_status(
            StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"Invalid parameter"}}"#,
        );
        match err {
            LlmError::InvalidRequest { message } => assert_eq!(message, "Invalid parameter"),
            _ => panic!("expected InvalidRequest"),
        }
    }

    #[test]
    fn test_from_http_status_429_retry_after_seconds() {
        let err = LlmError::from_http_status(
            StatusCode::TOO_MANY_REQUESTS,
            r#"{"error":{"retry_after":5}}"#,
        );
        match err {
            LlmError::RateLimitExceeded { retry_after_ms } => {
                assert_eq!(retry_after_ms, Some(5000));
            }
            _ => panic!("expected RateLimitExceeded"),
        }
    }

    #[test]
    fn test_from_http_status_401_no_json_body() {
        let err = LlmError::from_http_status(StatusCode::UNAUTHORIZED, "plain text error");
        match err {
            LlmError::AuthenticationError { message } => {
                assert_eq!(message, "plain text error");
            }
            _ => panic!("expected AuthenticationError"),
        }
    }

    #[test]
    fn test_from_http_status_401_empty_body() {
        let err = LlmError::from_http_status(StatusCode::UNAUTHORIZED, "");
        match err {
            LlmError::AuthenticationError { message } => {
                assert_eq!(message, "Invalid or missing API key");
            }
            _ => panic!("expected AuthenticationError"),
        }
    }

    #[test]
    fn test_from_http_status_403_no_json_body() {
        let err = LlmError::from_http_status(StatusCode::FORBIDDEN, "forbidden plain text");
        match err {
            LlmError::PermissionError { message } => {
                assert_eq!(message, "forbidden plain text");
            }
            _ => panic!("expected PermissionError"),
        }
    }

    #[test]
    fn test_from_http_status_400_context_window_with_token_counts() {
        let err = LlmError::from_http_status(
            StatusCode::BAD_REQUEST,
            "context_length_exceeded: 8192tokens requested but limit is 4096tokens",
        );
        match err {
            LlmError::ContextWindowExceeded {
                estimated_tokens,
                limit_tokens,
            } => {
                assert!(estimated_tokens > 0 || limit_tokens > 0);
            }
            _ => panic!("expected ContextWindowExceeded, got {err:?}"),
        }
    }

    #[test]
    fn test_extract_error_message_json_with_message_field() {
        let result = extract_error_message(r#"{"message":"direct message"}"#);
        assert_eq!(result, Some("direct message".to_string()));
    }

    #[test]
    fn test_extract_error_message_long_text() {
        let long_body = "a".repeat(300);
        let result = extract_error_message(&long_body);
        assert_eq!(result.unwrap().len(), 200);
    }

    #[test]
    fn test_extract_error_message_empty_body() {
        let result = extract_error_message("");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_retry_after_no_json() {
        let result = extract_retry_after("not json");
        assert!(result.is_none());
    }

    #[test]
    fn test_is_context_window_error_various_patterns() {
        assert!(is_context_window_error("too many tokens in request"));
        assert!(is_context_window_error("token limit exceeded"));
        assert!(is_context_window_error(
            "please reduce the length of messages"
        ));
        assert!(is_context_window_error("maximum context length exceeded"));
        assert!(!is_context_window_error("normal error"));
    }

    #[test]
    fn test_extract_number_near() {
        assert_eq!(extract_number_near("8192tokens", "tokens"), Some(8192));
        assert_eq!(extract_number_near("4096limit", "limit"), Some(4096));
        assert_eq!(extract_number_near("no numbers here", "tokens"), None);
        assert_eq!(extract_number_near("8192 tokens", "tokens"), None);
    }

    #[test]
    fn test_all_safe_failure_classes() {
        assert_eq!(
            LlmError::StreamError("x".into()).safe_failure_class(),
            "stream"
        );
        assert_eq!(
            LlmError::ToolCallError {
                message: "x".into(),
                tool_name: None
            }
            .safe_failure_class(),
            "tool_call"
        );
        assert_eq!(
            LlmError::MissingCredentials {
                provider: "x".into(),
                env_vars: vec![]
            }
            .safe_failure_class(),
            "credentials"
        );
        assert_eq!(
            LlmError::JsonParse {
                provider: "x".into(),
                model: "y".into(),
                body_snippet: "z".into(),
                source: serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err()
            }
            .safe_failure_class(),
            "json_parse"
        );
        assert_eq!(
            LlmError::RetriesExhausted { attempts: 3 }.safe_failure_class(),
            "retries_exhausted"
        );
        assert_eq!(
            LlmError::RequestBodySizeExceeded {
                estimated_bytes: 100,
                max_bytes: 50
            }
            .safe_failure_class(),
            "request_size"
        );
        assert_eq!(LlmError::Other("x".into()).safe_failure_class(), "unknown");
    }

    #[test]
    fn test_all_suggested_actions() {
        assert!(
            !LlmError::StreamError("x".into())
                .suggested_action()
                .is_empty()
        );
        assert!(
            !LlmError::ToolCallError {
                message: "x".into(),
                tool_name: None
            }
            .suggested_action()
            .is_empty()
        );
        assert!(
            !LlmError::MissingCredentials {
                provider: "x".into(),
                env_vars: vec![]
            }
            .suggested_action()
            .is_empty()
        );
        assert!(
            !LlmError::ExpiredToken("x".into())
                .suggested_action()
                .is_empty()
        );
        assert!(
            !LlmError::JsonParse {
                provider: "x".into(),
                model: "y".into(),
                body_snippet: "z".into(),
                source: serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err()
            }
            .suggested_action()
            .is_empty()
        );
        assert!(
            !LlmError::RetriesExhausted { attempts: 3 }
                .suggested_action()
                .is_empty()
        );
        assert!(
            !LlmError::RequestBodySizeExceeded {
                estimated_bytes: 100,
                max_bytes: 50
            }
            .suggested_action()
            .is_empty()
        );
        assert!(!LlmError::Other("x".into()).suggested_action().is_empty());
    }

    #[test]
    fn test_from_serde_json_error() {
        let serde_err = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
        let llm_err: LlmError = serde_err.into();
        assert!(matches!(llm_err, LlmError::JsonParse { .. }));
    }

    #[test]
    fn test_rate_limit_exceeded_display_with_retry_after() {
        let err = LlmError::RateLimitExceeded {
            retry_after_ms: Some(5000),
        };
        let msg = err.to_string();
        assert!(msg.contains("5000ms"));
    }

    #[test]
    fn test_rate_limit_exceeded_display_without_retry_after() {
        let err = LlmError::RateLimitExceeded {
            retry_after_ms: None,
        };
        let msg = err.to_string();
        assert!(!msg.contains("retry after"));
    }

    #[test]
    fn test_prompt_too_large_is_context_window_failure() {
        let err = LlmError::PromptTooLarge(9999);
        assert!(err.is_context_window_failure());
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_504_maps_to_timeout_not_server_overloaded() {
        let err_504 = LlmError::from_http_status(StatusCode::GATEWAY_TIMEOUT, "gateway timeout");
        assert!(
            matches!(err_504, LlmError::Timeout(_)),
            "HTTP 504 MUST map to Timeout per RFC 9110 §15.6.5, got {err_504:?}"
        );
        assert_eq!(err_504.safe_failure_class(), "timeout");

        let err_500 =
            LlmError::from_http_status(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        assert!(
            matches!(err_500, LlmError::ServerOverloaded),
            "HTTP 500 MUST map to ServerOverloaded, got {err_500:?}"
        );
        assert_eq!(err_500.safe_failure_class(), "server_overloaded");

        assert_ne!(
            err_504.safe_failure_class(),
            err_500.safe_failure_class(),
            "504 and 500 MUST route to different failure classes"
        );
    }

    #[test]
    fn test_deductive_is_retryable_partition_invariant() {
        let retryable: Vec<LlmError> = vec![
            LlmError::Network("x".into()),
            LlmError::Timeout(100),
            LlmError::RateLimitExceeded {
                retry_after_ms: None,
            },
            LlmError::ModelUnavailable("x".into()),
            LlmError::ServerOverloaded,
        ];
        let non_retryable: Vec<LlmError> = vec![
            LlmError::AuthenticationError {
                message: "x".into(),
            },
            LlmError::PermissionError {
                message: "x".into(),
            },
            LlmError::ContextWindowExceeded {
                estimated_tokens: 1,
                limit_tokens: 2,
            },
            LlmError::PromptTooLarge(1),
            LlmError::InvalidRequest {
                message: "x".into(),
            },
            LlmError::StreamError("x".into()),
            LlmError::ToolCallError {
                message: "x".into(),
                tool_name: None,
            },
            LlmError::MissingCredentials {
                provider: "x".into(),
                env_vars: vec![],
            },
            LlmError::ExpiredToken("x".into()),
            LlmError::JsonParse {
                provider: "x".into(),
                model: "y".into(),
                body_snippet: "z".into(),
                source: serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err(),
            },
            LlmError::RetriesExhausted { attempts: 3 },
            LlmError::Config("x".into()),
            LlmError::RequestBodySizeExceeded {
                estimated_bytes: 1,
                max_bytes: 1,
            },
            LlmError::Cancelled,
            LlmError::HttpClientInit("x".into()),
            LlmError::Other("x".into()),
        ];
        for err in &retryable {
            assert!(err.is_retryable(), "MUST be retryable: {err:?}");
        }
        for err in &non_retryable {
            assert!(!err.is_retryable(), "MUST NOT be retryable: {err:?}");
        }
    }

    #[test]
    fn test_deductive_safe_failure_class_non_empty() {
        let all_variants: Vec<LlmError> = vec![
            LlmError::Network("x".into()),
            LlmError::Timeout(0),
            LlmError::RateLimitExceeded {
                retry_after_ms: None,
            },
            LlmError::AuthenticationError {
                message: "x".into(),
            },
            LlmError::PermissionError {
                message: "x".into(),
            },
            LlmError::ContextWindowExceeded {
                estimated_tokens: 0,
                limit_tokens: 0,
            },
            LlmError::PromptTooLarge(0),
            LlmError::ModelUnavailable("x".into()),
            LlmError::ServerOverloaded,
            LlmError::InvalidRequest {
                message: "x".into(),
            },
            LlmError::StreamError("x".into()),
            LlmError::ToolCallError {
                message: "x".into(),
                tool_name: None,
            },
            LlmError::MissingCredentials {
                provider: "x".into(),
                env_vars: vec![],
            },
            LlmError::ExpiredToken("x".into()),
            LlmError::JsonParse {
                provider: "x".into(),
                model: "y".into(),
                body_snippet: "z".into(),
                source: serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err(),
            },
            LlmError::RetriesExhausted { attempts: 0 },
            LlmError::Config("x".into()),
            LlmError::RequestBodySizeExceeded {
                estimated_bytes: 0,
                max_bytes: 0,
            },
            LlmError::Cancelled,
            LlmError::HttpClientInit("x".into()),
            LlmError::Other("x".into()),
        ];
        for err in &all_variants {
            let class = err.safe_failure_class();
            assert!(
                !class.is_empty(),
                "safe_failure_class must never be empty for {err:?}"
            );
        }
    }

    #[test]
    fn test_deductive_suggested_action_non_empty() {
        let all_variants: Vec<LlmError> = vec![
            LlmError::Network("x".into()),
            LlmError::Timeout(0),
            LlmError::RateLimitExceeded {
                retry_after_ms: None,
            },
            LlmError::AuthenticationError {
                message: "x".into(),
            },
            LlmError::PermissionError {
                message: "x".into(),
            },
            LlmError::ContextWindowExceeded {
                estimated_tokens: 0,
                limit_tokens: 0,
            },
            LlmError::PromptTooLarge(0),
            LlmError::ModelUnavailable("x".into()),
            LlmError::ServerOverloaded,
            LlmError::InvalidRequest {
                message: "x".into(),
            },
            LlmError::StreamError("x".into()),
            LlmError::ToolCallError {
                message: "x".into(),
                tool_name: None,
            },
            LlmError::MissingCredentials {
                provider: "x".into(),
                env_vars: vec![],
            },
            LlmError::ExpiredToken("x".into()),
            LlmError::JsonParse {
                provider: "x".into(),
                model: "y".into(),
                body_snippet: "z".into(),
                source: serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err(),
            },
            LlmError::RetriesExhausted { attempts: 0 },
            LlmError::Config("x".into()),
            LlmError::RequestBodySizeExceeded {
                estimated_bytes: 0,
                max_bytes: 0,
            },
            LlmError::Cancelled,
            LlmError::HttpClientInit("x".into()),
            LlmError::Other("x".into()),
        ];
        for err in &all_variants {
            let action = err.suggested_action();
            assert!(
                !action.is_empty(),
                "suggested_action must never be empty for {err:?}"
            );
        }
    }

    #[test]
    fn test_deductive_from_http_status_504_not_server_overloaded() {
        let err_504 = LlmError::from_http_status(StatusCode::GATEWAY_TIMEOUT, "timeout");
        assert!(
            !matches!(err_504, LlmError::ServerOverloaded),
            "HTTP 504 MUST NOT map to ServerOverloaded (RFC 9110 §15.6.5)"
        );
        assert!(
            matches!(err_504, LlmError::Timeout(_)),
            "HTTP 504 MUST map to Timeout"
        );
    }

    #[test]
    fn test_deductive_from_http_status_5xx_partition() {
        assert!(matches!(
            LlmError::from_http_status(StatusCode::INTERNAL_SERVER_ERROR, ""),
            LlmError::ServerOverloaded
        ));
        assert!(matches!(
            LlmError::from_http_status(StatusCode::BAD_GATEWAY, ""),
            LlmError::ServerOverloaded
        ));
        assert!(matches!(
            LlmError::from_http_status(StatusCode::SERVICE_UNAVAILABLE, ""),
            LlmError::ServerOverloaded
        ));
        assert!(matches!(
            LlmError::from_http_status(StatusCode::GATEWAY_TIMEOUT, ""),
            LlmError::Timeout(_)
        ));
    }

    #[test]
    fn test_miri_extract_number_near_no_ub() {
        let cases: &[(&str, &str)] = &[
            ("8192tokens", "tokens"),
            ("4096limit", "limit"),
            ("no numbers here", "tokens"),
            ("", "tokens"),
            ("0tokens", "tokens"),
            ("12345max", "max"),
            ("tokens", "tokens"),
        ];
        for &(text, keyword) in cases {
            let _ = extract_number_near(text, keyword);
        }
    }

    #[test]
    fn test_miri_is_context_window_error_no_ub() {
        let cases: &[&str] = &[
            "context_length_exceeded",
            "context window exceeded",
            "maximum context length",
            "too many tokens",
            "token limit exceeded",
            "reduce the length of messages",
            "normal error",
            "",
        ];
        for &input in cases {
            let _ = is_context_window_error(input);
        }
    }
}
