//! 调试与可观测性端点（Debug & Observability Endpoints）
//!
//! 提供以下端点：
//! - `GET /metrics` - Prometheus 格式指标导出
//! - `GET /api/v1/debug/tracing` - 当前活跃 Span 信息（仅开发环境）
//!
//! # 安全说明
//!
//! `/debug/tracing` 端点仅在非生产环境可用，
//! 通过环境变量 `ENVIRONMENT=production` 自动禁用。

use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use serde::Serialize;

use crate::middleware::tracing_middleware::current_trace_info;
use crate::observability::is_production;

/// Prometheus 指标响应
///
/// 返回 Prometheus exposition format 兼容的纯文本指标数据。
/// # Errors
    ///
    /// 当 Prometheus Recorder 安装失败或环境为生产环境时返回错误响应。
    #[must_use]
    pub async fn metrics_endpoint() -> impl IntoResponse {
    use metrics_exporter_prometheus::PrometheusBuilder;

    let handle = match PrometheusBuilder::new().install_recorder() {
        Ok(handle) => handle,
        Err(e) => {
            tracing::error!("Prometheus Recorder 安装失败: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "text/plain")],
                "Prometheus metrics unavailable",
            )
                .into_response();
        }
    };

    let metrics = handle.render();
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        metrics,
    )
        .into_response()
}

/// Tracing 调试信息响应
#[derive(Debug, Serialize)]
pub struct TracingDebugResponse {
    /// 当前请求的 trace 信息
    pub current_trace: super::middleware::tracing_middleware::TraceInfo,
    /// 是否为生产环境
    pub environment: String,
    /// 服务版本
    pub service_version: &'static str,
    /// 可用端点列表
    pub available_endpoints: Vec<&'static str>,
}

/// 获取当前追踪调试信息
///
/// # Returns
///
/// 返回当前活跃 Span 的 trace ID、span ID、采样状态等信息。
/// 在生产环境中返回 403 Forbidden。
#[tracing::instrument(fields(endpoint = "/debug/tracing"))]
/// # Errors
    ///
    /// 当在生产环境中调用或追踪信息获取失败时返回错误。
    #[must_use = "调试端点结果必须被使用"]
    pub async fn tracing_debug_endpoint(
    headers: HeaderMap,
) -> Result<Json<TracingDebugResponse>, ApiError> {
    if is_production() {
        return Err(ApiError::forbidden(
            "调试端点在生产环境中不可用",
            "DEBUG_ENDPOINT_DISABLED_IN_PRODUCTION",
        ));
    }

    // 记录调试访问
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    tracing::info!(
        user_agent = %user_agent,
        "Tracing 调试端点被访问"
    );

    Ok(Json(TracingDebugResponse {
        current_trace: current_trace_info(),
        environment: std::env::var("ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_string()),
        service_version: crate::API_VERSION,
        available_endpoints: vec![
            "/metrics",
            "/api/v1/debug/tracing",
            "/healthz",
        ],
    }))
}

/// API 错误类型（用于调试端点的错误响应）
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 禁止访问错误 —— 用于调试端点的权限控制响应
    #[error("{message}")]
    Forbidden {
        /// 错误消息内容
        message: String,
        /// 错误代码标识
        code: String,
    },
}

impl ApiError {
    #[must_use]
    /// 创建禁止访问错误实例
    ///
    /// # Arguments
    ///
    /// * `message` - 错误描述信息
    /// * `code` - 错误分类代码
    pub fn forbidden(message: &str, code: &str) -> Self {
        Self::Forbidden {
            message: message.to_string(),
            code: code.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            Self::Forbidden { message, code } => (
                StatusCode::FORBIDDEN,
                serde_json::json!({
                    "error": "forbidden",
                    "code": code,
                    "message": message,
                }),
            ),
        };

        (status, Json(body)).into_response()
    }
}
