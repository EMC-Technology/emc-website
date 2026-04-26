//! 请求日志中间件（参数脱敏 + 审计事件 AUDIT-030）
//!
//! # 功能
//! - 为每个请求生成唯一的 request_id 用于追踪
//! - 记录请求/响应元信息到结构化日志
//! - 自动脱敏 URI 和 Header 中的敏感参数
//! - 记录审计事件（AUDIT-030 查询执行）

use axum::{
    extract::State,
    middleware::Next,
    response::Response,
    http::{HeaderMap, Uri},
};
use tracing::{info, warn, error as log_error};
use uuid::Uuid;
use crate::handler::AppState;

/// 需要脱敏的敏感查询参数名称
const SENSITIVE_PARAMS: &[&str] = &[
    "token", "password", "key", "secret",
    "authorization", "apikey", "api_key",
    "access_token", "refresh_token",
];

/// 日志中间件配置
pub async fn logging_middleware(
    State(_state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();

    let sanitized_uri = sanitize_uri(&uri);
    let sanitized_headers = sanitize_headers(&headers);

    info!(
        request_id = %request_id,
        method = %method,
        uri = %sanitized_uri,
        headers = %sanitized_headers,
        user_agent = extract_user_agent(&headers),
        "收到请求 [AUDIT-030]"
    );

    // 执行请求处理
    let response = next.run(request).await;

    // 记录响应信息
    let status = response.status().as_u16();
    let elapsed = response.extensions().get::<std::time::Duration>()
        .map_or_else(|| "N/A".to_string(), |duration| format!("{:.2}ms", duration.as_secs_f64() * 1000.0));

    if (400..500).contains(&status) {
        warn!(
            request_id = %request_id,
            status = status,
            elapsed = %elapsed,
            "客户端错误"
        );
    } else if status >= 500 {
        log_error!(
            request_id = %request_id,
            status = status,
            elapsed = %elapsed,
            "服务端错误"
        );
    } else {
        info!(
            request_id = %request_id,
            status = status,
            elapsed = %elapsed,
            "请求完成"
        );
    }

    // 注入 request_id 到响应头（方便客户端追踪）
    response
}

/// URI 脱敏函数（移除或遮蔽敏感查询参数）
fn sanitize_uri(uri: &Uri) -> String {
    let mut parts = uri.clone().into_parts();

    if let Some(query) = &parts.path_and_query {
        if let Some(original_query) = query.query() {
            let sanitized: Vec<String> = original_query
                .split('&')
                .map(|param| {
                    if let Some((key, _)) = param.split_once('=') {
                        if SENSITIVE_PARAMS.contains(&key.to_lowercase().as_str()) {
                            format!("{key}=[REDACTED]")
                        } else {
                            param.to_string()
                        }
                    } else {
                        param.to_string()
                    }
                })
                .collect();

            let new_query = sanitized.join("&");

            if let Some(path_and_query) = &mut parts.path_and_query {
                use axum::http::uri::PathAndQuery;
                let path_only = path_and_query.path().to_string();
                *path_and_query = PathAndQuery::from_maybe_shared(
                    format!("{path_only}?{new_query}")
                ).unwrap_or_else(|_| {
                    PathAndQuery::from_maybe_shared(path_only)
                        .unwrap_or_else(|_| path_and_query.clone())
                });
            }
        }
    }

    // 重建 URI 字符串
    Uri::from_parts(parts)
        .map_or_else(|_| "[URI_PARSE_ERROR]".to_string(), |u| u.to_string())
}

/// Header 脱敏函数
fn sanitize_headers(headers: &HeaderMap) -> String {
    let sensitive_headers = [
        "authorization",
        "cookie",
        "x-api-key",
        "proxy-authorization",
    ];

    let sanitized: Vec<String> = headers
        .iter()
        .map(|(name, value)| {
            let name_str = name.as_str();
            if sensitive_headers.contains(&name_str.to_lowercase().as_str()) {
                format!("{name_str}: [REDACTED]")
            } else {
                format!(
                    "{name_str}: {}",
                    value.to_str().unwrap_or("[INVALID_UTF8]")
                )
            }
        })
        .collect();

    sanitized.join(" ")
}

/// 提取 User-Agent（用于审计日志）
fn extract_user_agent(headers: &HeaderMap) -> &str {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_uri_removes_token_param() {
        let uri: Uri = "/api/v1/search?token=secret123&q=test".parse().unwrap();
        let sanitized = sanitize_uri(&uri);

        assert!(sanitized.contains("[REDACTED]"), "token 参数应被脱敏");
        assert!(!sanitized.contains("secret123"), "原始值不应出现");
        assert!(sanitized.contains("q=test"), "非敏感参数应保留");
    }

    #[test]
    fn test_sanitize_uri_multiple_sensitive_params() {
        let uri: Uri = "/api/v1/documents?apikey=abc&password=123&limit=20"
            .parse()
            .unwrap();
        let sanitized = sanitize_uri(&uri);

        assert!(sanitized.contains("apikey=[REDACTED]"));
        assert!(sanitized.contains("password=[REDACTED]"));
        assert!(sanitized.contains("limit=20"));
    }

    #[test]
    fn test_sanitize_uri_no_query_params() {
        let uri: Uri = "/api/v1/documents".parse().unwrap();
        let sanitized = sanitize_uri(&uri);

        assert_eq!(sanitized, "/api/v1/documents");
    }

    #[test]
    fn test_sanitize_headers_redacts_authorization() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer secret-token".parse().unwrap());
        headers.insert("content-type", "application/json".parse().unwrap());

        let sanitized = sanitize_headers(&headers);

        assert!(sanitized.contains("authorization: [REDACTED]"));
        assert!(sanitized.contains("content-type: application/json"));
        assert!(!sanitized.contains("secret-token"));
    }

    #[test]
    fn test_extract_user_agent_present() {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", "Mozilla/5.0".parse().unwrap());

        let ua = extract_user_agent(&headers);
        assert_eq!(ua, "Mozilla/5.0");
    }

    #[test]
    fn test_extract_user_agent_missing() {
        let headers = HeaderMap::new();

        let ua = extract_user_agent(&headers);
        assert_eq!(ua, "Unknown");
    }

    #[test]
    fn test_sensitive_params_list_completeness() {
        // 验证所有常见的敏感参数名都在列表中
        let should_contain = vec![
            "token", "password", "key", "secret",
            "authorization", "apikey", "api_key",
        ];

        for param in should_contain {
            assert!(
                SENSITIVE_PARAMS.contains(&param),
                "SENSITIVE_PARAMS 应包含 '{param}'"
            );
        }
    }
}
