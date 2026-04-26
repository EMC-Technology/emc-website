//! Axum Router 构建器
//!
//! 路由注册与中间件链配置。
//! 中间件执行顺序（从外到内）：
//!
//! ```text
//! 请求 → CORS → Tracing → PII Redact → Logging → Error Handler → Auth → Authorization → Permission → Route Handler
//! ```

use axum::{
    Extension, Router, middleware,
    response::IntoResponse,
    routing::{delete, get, post},
};
use tower_http::cors::CorsLayer;

use crate::auth::{Role, auth_middleware, permission_middleware};
use crate::authz::middleware::authorization_middleware;
use crate::error_handler::error_handler_middleware;
use crate::handler::{
    AppState, delete_document, full_text_search, get_block, get_block_tokens, get_document,
    get_document_blocks, health_check, list_documents, trace_references, upload_document,
    vector_search,
};
use crate::logging_middleware::logging_middleware;
use crate::middleware::{
    pii_redact::pii_redact_middleware, tracing_middleware::tracing_middleware,
};
use crate::observability_endpoints::{metrics_endpoint, tracing_debug_endpoint};

const TRACEPARENT_HEADER: &str = "traceparent";
const TRACESTATE_HEADER: &str = "tracestate";

/// 构建 Axum 路由器
///
/// 注册所有 API 路由、WebSocket 端点、中间件层。
/// 认证中间件应用于受保护的 API 路由，健康检查端点无需认证。
/// 权限中间件应用于写操作和删除操作的路由组。
///
/// # 可观测性端点
///
/// - `GET /metrics` - Prometheus 指标导出
/// - `GET /api/v1/debug/tracing` - Tracing 调试信息（仅非生产环境）
///
/// # Panics
///
/// 当 CORS header 名称解析失败时 panic（仅限硬编码的内部值，外部输入不会触发）。
#[allow(clippy::too_many_lines)]
pub fn build_router(state: AppState) -> Router {
    let ws_manager = state.ws_manager.clone();

    // ========== 业务路由 ==========

    let ws_route = axum::routing::get({
        let manager = ws_manager;
        move |ws: axum::extract::WebSocketUpgrade, Extension(role): Extension<Role>| {
            let manager = manager.clone();
            async move {
                if role == Role::Anonymous {
                    return (axum::http::StatusCode::UNAUTHORIZED, "WebSocket requires authentication").into_response();
                }
                manager.handle_ws_upgrade(ws).into_response()
            }
        }
    });

    let read_routes = Router::new()
        .route("/documents", get(list_documents))
        .route("/documents/{id}", get(get_document))
        .route("/documents/{id}/blocks", get(get_document_blocks))
        .route("/blocks/{id}", get(get_block))
        .route("/blocks/{id}/tokens", get(get_block_tokens))
        .route("/tokens/{id}/references", get(trace_references))
        .route("/search/vector", post(vector_search))
        .route("/search/fulltext", get(full_text_search));

    let write_routes = Router::new()
        .route("/documents", post(upload_document))
        .layer(middleware::from_fn(permission_middleware));

    let delete_routes = Router::new()
        .route("/documents/{id}", delete(delete_document))
        .layer(middleware::from_fn(permission_middleware));

    let api_routes = Router::new()
        .merge(read_routes)
        .merge(write_routes)
        .merge(delete_routes)
        .route("/ws", ws_route)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            authorization_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // ========== 健康检查与可观测性端点 ==========

    let health_route = Router::new().route("/healthz", get(health_check));

    let observability_routes = Router::new()
        .route("/metrics", get(metrics_endpoint))
        .route("/debug/tracing", get(tracing_debug_endpoint));

    // ========== CORS 配置 ==========

    let allowed_origins = std::env::var("KNOWLEDGE_CORS_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:3000,http://localhost:5173".to_string());
    let origins: Vec<_> = allowed_origins
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let allow_methods = [
        axum::http::Method::GET,
        axum::http::Method::POST,
        axum::http::Method::PUT,
        axum::http::Method::PATCH,
        axum::http::Method::DELETE,
    ];
    let allow_headers = [
        axum::http::header::CONTENT_TYPE,
        axum::http::header::AUTHORIZATION,
        // SAFETY: W3C trace header 名称为硬编码常量字符串，解析为 HeaderName 不可能失败
        TRACEPARENT_HEADER.parse().expect("hardcoded W3C trace header name is valid"),
        TRACESTATE_HEADER.parse().expect("hardcoded W3C trace header name is valid"),
    ];

    let cors = if origins.is_empty() {
        tracing::warn!(
            "KNOWLEDGE_CORS_ORIGINS 未配置或解析结果为空，CORS 仅允许 localhost 访问。\
             生产环境请显式配置允许的来源"
        );
        // SAFETY: localhost URL 为硬编码字面量，解析为 Origin 不可能失败
        let localhost_origins: Vec<_> = [
            "http://localhost:3000".parse().expect("localhost URL is valid"),
            "http://localhost:5173".parse().expect("localhost URL is valid"),
            "http://127.0.0.1:3000".parse().expect("localhost URL is valid"),
        ]
        .into_iter()
        .collect();
        CorsLayer::new()
            .allow_origin(localhost_origins)
            .allow_methods(allow_methods)
            .allow_headers(allow_headers)
    } else {
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(allow_methods)
            .allow_headers(allow_headers)
    };

    // ========== 组装最终路由器 ==========
    //
    // 中间件链（从外到内，请求先经过最外层的中间件）：
    // 1. CORS - 跨域处理
    // 2. Tracing Middleware - 创建 HTTP Span
    // 3. PII Redaction - 敏感信息检测与脱敏
    // 4. Logging Middleware - 结构化日志记录
    // 5. Error Handler - 统一错误响应

    Router::new()
        .nest("/api/v1", api_routes.merge(observability_routes))
        .merge(health_route)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            error_handler_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            logging_middleware,
        ))
        .layer(middleware::from_fn(pii_redact_middleware))
        .layer(middleware::from_fn(tracing_middleware))
        .layer(cors)
        .with_state(state)
}
