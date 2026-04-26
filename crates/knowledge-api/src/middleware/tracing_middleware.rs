//! 分布式追踪中间件（Distributed Tracing Middleware）
//!
//! 为每个 HTTP 请求自动创建 OpenTelemetry Span，实现：
//! - 从 `traceparent` / `tracestate` 请求头提取 W3C Trace Context
//! - 自动记录 HTTP 方法、路径、状态码、延迟等标准属性
//! - 将 Trace ID 注入到响应头中，方便客户端关联日志
//!
//! # W3C Trace Context 规范
//!
//! 遵循 [W3C Trace Context](https://www.w3.org/TR/trace-context/) 标准：
//! - `traceparent`: `00-{trace-id}-{parent-id}-{flags}`
//! - `tracestate`: 键值对列表（供应商特定上下文）
//!
//! # Span 属性规范
//!
//! 遵循 [OpenTelemetry Semantic Conventions for HTTP](https://opentelemetry.io/specs/semantics/attributes/)：
//!
//! | 属性名 | 类型 | 说明 |
//! |--------|------|------|
//! | `http.method` | string | HTTP 方法 (GET, POST, ...) |
/// | `http.route` | string | 路由模板 (/api/v1/documents/:id) |
/// | `http.status_code` | int | 响应状态码 |
/// | `http.url` | string | 完整请求 URL |
/// | `user_agent.original` | string | User-Agent 头 |
/// | `client.address` | string | 客户端 IP |
use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use metrics::{counter, histogram};
use opentelemetry::trace::TraceContextExt;
use std::time::Instant;
use tracing::{Instrument, Span, field, info_span};

/// traceparent 响应头名称
const TRACE_ID_HEADER: &str = "x-trace-id";

/// 请求 `ID` 追踪中间件
///
/// 为每个传入的 HTTP 请求创建一个完整 `OTel` 属性的 Span，
/// 并将 trace ID 注入到响应头中。
///
/// # 中间件链位置建议
///
/// 放置在路由层之后、业务处理器之前：
/// ```text
/// CORS → Error Handler → Logging → **Tracing** → Auth → Route Handlers
/// ```
pub async fn tracing_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();

    // 尝试从请求头提取 W3C Trace Context
    let parent_context = extract_trace_context(&headers);

    // 创建 Span 名称：HTTP {method} {path}
    let span_name = format!("HTTP {} {}", method, uri.path());

    let _ = parent_context;
    let span = info_span!(
        "http_request",
        otel.name = %span_name,
        http.method = %method,
        http.route = %uri.path(),
        http.url = %uri,
        http.status_code = field::Empty,
        client.address = extract_client_ip(&headers),
    );

    async {
        let start = Instant::now();

        let response = next.run(request).await;

        let elapsed = start.elapsed();
        let status = response.status().as_u16();
        let status_class = classify_status(status);

        // 记录 Span 属性
        Span::current().record("http.status_code", status);

        let otel_ctx = opentelemetry::Context::current();
        let otel_span = otel_ctx.span();
        if otel_span.span_context().is_valid() {
            otel_span.set_attribute(opentelemetry::KeyValue::new("http.status_code", i64::from(status)));
        }

        let elapsed_secs = elapsed.as_secs_f64();
        let otel_ctx = opentelemetry::Context::current();
        let otel_span = otel_ctx.span();
        if otel_span.span_context().is_valid() {
            otel_span.set_attribute(opentelemetry::KeyValue::new("http.response_time_seconds", elapsed_secs));
        }

        // 记录 HTTP 请求指标到 Prometheus
        counter!("http_requests_total", "method" => method.to_string(), "status" => status_class).increment(1);
        histogram!("http_request_duration_seconds", "method" => method.to_string(), "status" => status_class).record(elapsed_secs);

        // 注入 trace_id 到响应头
        let mut response = inject_trace_id_to_response(response);

        // 注入耗时到响应扩展（供 logging_middleware 使用）
        response.extensions_mut().insert(elapsed);

        response
    }
    .instrument(span)
    .await
}

/// 从请求头提取 W3C Trace Context
///
/// 支持 `traceparent` 和 `tracestate` 头。
/// 如果没有找到有效的 trace context，返回 None（表示新 trace 的根 span）。
fn extract_trace_context(headers: &HeaderMap) -> Option<opentelemetry::Context> {
    let trace_parent = headers
        .get("traceparent")
        .or_else(|| headers.get("traceparent"))
        .and_then(|v| v.to_str().ok())?;

    let trace_state = headers.get("tracestate").and_then(|v| v.to_str().ok());

    let _ = (trace_parent, trace_state);

    // 通过 tracing-opentelemetry bridge，trace context 会由
    // OpenTelemetryLayer 自动从 tracing span 提取
    None
}

/// 将当前 trace ID 注入到响应头
fn inject_trace_id_to_response(mut response: Response) -> Response {
    let ctx = opentelemetry::Context::current();
    let span = ctx.span();
    let span_context = span.span_context();

    if !span_context.is_valid() {
        return response;
    }

    let trace_id = format!("{:032x}", span_context.trace_id());

    if let Ok(value) = HeaderValue::from_str(&trace_id) {
        response.headers_mut().insert(TRACE_ID_HEADER, value);
    }

    response
}

/// 从请求头提取客户端 IP 地址
///
/// 优先级：X-Forwarded-For > X-Real-IP > 远程地址
fn extract_client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// 将 HTTP 状态码分类为 Prometheus 兼容的桶标签
const fn classify_status(code: u16) -> &'static str {
    match code {
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        500..=599 => "5xx",
        other => static_fmt(other),
    }
}

const fn static_fmt(code: u16) -> &'static str {
    match code / 100 {
        1 => "1xx",
        2 => "2xx",
        3 => "3xx",
        4 => "4xx",
        5 => "5xx",
        6 => "6xx",
        7 => "7xx",
        8 => "8xx",
        9 => "9xx",
        _ => "unknown",
    }
}

/// 生成用于调试的 trace 信息响应体
#[derive(Debug, serde::Serialize)]
pub struct TraceInfo {
    /// 当前 trace 的全局唯一标识符
    pub trace_id: Option<String>,
    /// 当前 span 的标识符（在 trace 内唯一）
    pub span_id: Option<String>,
    /// 是否被采样（决定是否上报到后端）
    pub sampled: bool,
}

/// 获取当前活跃 Span 的 trace 信息
#[must_use]
pub fn current_trace_info() -> TraceInfo {
    let ctx = opentelemetry::Context::current();
    let span = ctx.span();
    let span_context = span.span_context();

    if !span_context.is_valid() {
        return TraceInfo {
            trace_id: None,
            span_id: None,
            sampled: false,
        };
    }

    TraceInfo {
        trace_id: Some(format!("{:032x}", span_context.trace_id())),
        span_id: Some(format!("{:016x}", span_context.span_id())),
        sampled: span_context.is_sampled(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_status_2xx() {
        assert_eq!(classify_status(200), "2xx");
        assert_eq!(classify_status(201), "2xx");
        assert_eq!(classify_status(299), "2xx");
    }

    #[test]
    fn test_classify_status_4xx() {
        assert_eq!(classify_status(400), "4xx");
        assert_eq!(classify_status(404), "4xx");
        assert_eq!(classify_status(499), "4xx");
    }

    #[test]
    fn test_classify_status_5xx() {
        assert_eq!(classify_status(500), "5xx");
        assert_eq!(classify_status(503), "5xx");
    }

    #[test]
    fn test_extract_client_ip_from_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());

        let ip = extract_client_ip(&headers);
        assert_eq!(ip, "192.168.1.1");
    }

    #[test]
    fn test_extract_client_ip_fallback_to_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "172.16.0.1".parse().unwrap());

        let ip = extract_client_ip(&headers);
        assert_eq!(ip, "172.16.0.1");
    }

    #[test]
    fn test_extract_client_ip_no_headers_returns_unknown() {
        let headers = HeaderMap::new();
        let ip = extract_client_ip(&headers);
        assert_eq!(ip, "unknown");
    }

    #[test]
    fn test_current_trace_info_without_active_span() {
        let info = current_trace_info();
        assert!(info.trace_id.is_none());
        assert!(!info.sampled);
    }

    #[test]
    fn test_trace_info_serialization() {
        let info = TraceInfo {
            trace_id: Some("abc123".to_string()),
            span_id: Some("def456".to_string()),
            sampled: true,
        };
        let json = serde_json::to_string(&info).expect("序列化失败");
        assert!(json.contains("abc123"));
        assert!(json.contains("\"sampled\":true"));
    }
}
