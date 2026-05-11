//! 统一错误处理中间件

use crate::handler::AppState;
use axum::{
    Json,
    extract::State,
    middleware::Next,
    response::{IntoResponse, Response},
};
use error_core::prelude::*;
use serde::{Deserialize, Serialize};

/// HTTP 状态码枚举（Axiom-3: 可枚举的封闭集合）
///
/// 替代 `u16` 数值状态表示，确保状态码在编译期可穷举验证。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HttpStatusCode {
    /// 400 Bad Request
    BadRequest,
    /// 401 Unauthorized
    Unauthorized,
    /// 403 Forbidden
    Forbidden,
    /// 404 Not Found
    NotFound,
    /// 409 Conflict
    Conflict,
    /// 422 Unprocessable Entity
    UnprocessableEntity,
    /// 429 Too Many Requests
    TooManyRequests,
    /// 500 Internal Server Error
    #[default]
    InternalServerError,
    /// 502 Bad Gateway
    BadGateway,
    /// 503 Service Unavailable
    ServiceUnavailable,
}

impl HttpStatusCode {
    /// 转换为数值状态码
    #[must_use]
    pub const fn as_u16(&self) -> u16 {
        match self {
            Self::BadRequest => 400,
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::Conflict => 409,
            Self::UnprocessableEntity => 422,
            Self::TooManyRequests => 429,
            Self::InternalServerError => 500,
            Self::BadGateway => 502,
            Self::ServiceUnavailable => 503,
        }
    }

    /// 从数值状态码构造（仅支持已知值）
    #[must_use]
    pub fn from_u16(code: u16) -> Option<Self> {
        match code {
            400 => Some(Self::BadRequest),
            401 => Some(Self::Unauthorized),
            403 => Some(Self::Forbidden),
            404 => Some(Self::NotFound),
            409 => Some(Self::Conflict),
            422 => Some(Self::UnprocessableEntity),
            429 => Some(Self::TooManyRequests),
            500 => Some(Self::InternalServerError),
            502 => Some(Self::BadGateway),
            503 => Some(Self::ServiceUnavailable),
            _ => None,
        }
    }
}

/// API 统一错误类型
///
/// 将内部 [`ErrorObject`] 转换为 HTTP 响应友好的格式。
#[derive(Debug, Serialize)]
pub struct ApiError {
    /// 错误代码（如 ERR-USR-VAL-001）
    pub code: String,
    /// 错误唯一标识符
    pub error_id: String,
    /// 内部错误消息
    #[serde(skip_serializing)]
    pub message: String,
    /// 面向用户的错误消息
    pub user_message: String,
    /// HTTP 状态码（Axiom-3: 类型化枚举）
    pub status: HttpStatusCode,
    /// 错误严重程度
    pub severity: Severity,
    /// 可恢复性
    pub recoverability: Recoverability,
    /// 附加详情（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    #[must_use]
    /// 从 `ErrorObject` 构建 API 错误实例
    ///
    /// 将底层错误对象转换为标准化的 API 错误响应格式，
    /// 提取错误代码、消息、严重性等字段。
    ///
    /// # Arguments
    ///
    /// * `err` - 底层 `ErrorObject` 引用
    pub fn from_error_object(err: &ErrorObject) -> Self {
        Self {
            code: err.code().to_string(),
            error_id: err.error_id().to_string(),
            message: err.message().to_string(),
            user_message: err.user_message().to_string(),
            status: HttpStatusCode::from_u16(err.http_status())
                .unwrap_or(HttpStatusCode::InternalServerError),
            severity: err.severity(),
            recoverability: err.recoverability(),
            details: None,
        }
    }

    #[must_use]
    /// 附加详细错误信息
    ///
    /// 向当前错误实例追加结构化的详细信息（如堆栈跟踪、上下文数据等）。
    /// 采用链式调用风格，返回 `self` 以支持流畅 API。
    ///
    /// # Arguments
    ///
    /// * `details` - 任意 JSON 值，通常包含调试所需的额外上下文
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status_code = self.status.as_u16();
        let status = axum::http::StatusCode::from_u16(status_code)
            .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);

        let body = Json(self);

        (status, body).into_response()
    }
}

impl From<ErrorObject> for ApiError {
    fn from(err: ErrorObject) -> Self {
        Self::from_error_object(&err)
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for ApiError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        error_core::helpers::general_fallback_error(&err.to_string()).into()
    }
}

/// 错误处理中间件
///
/// 捕获请求处理中的错误，记录日志并返回标准化错误响应。
pub async fn error_handler_middleware(
    _state: State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let response = next.run(request).await;

    if response.status().is_server_error() {
        tracing::error!(
            status = response.status().as_u16(),
            "服务端错误已由 error_handler_middleware 捕获 — 内部错误详情仅记录于服务端日志，不返回客户端"
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_object_to_api_error() {
        let err = error_core::helpers::validation_error("test error", "test_op");
        let api_err = ApiError::from(err);

        assert!(api_err.code.contains("USR"));
        assert_eq!(api_err.status, HttpStatusCode::BadRequest);
    }

    #[test]
    fn test_not_found_error_mapping() {
        let err = error_core::helpers::not_found("resource", "test");
        let api_err = ApiError::from(err);

        assert!(api_err.code.contains("USR"));
    }

    #[test]
    fn test_api_error_serialization() {
        let api_err = ApiError {
            code: "ERR-USR-VAL-001_ERR_O".to_string(),
            error_id: "test-uuid".to_string(),
            message: "测试错误".to_string(),
            user_message: "测试".to_string(),
            status: HttpStatusCode::BadRequest,
            severity: Severity::ERROR,
            recoverability: Recoverability::NonRecoverable,
            details: None,
        };

        let json = serde_json::to_string(&api_err).unwrap();
        assert!(json.contains("\"code\":\"ERR-USR-VAL-001_ERR_O\""));
        assert!(!json.contains("\"message\""), "内部消息不应序列化到客户端");
        assert!(json.contains("\"user_message\":\"测试\""));
        assert!(json.contains("\"status\":\"BadRequest\""));
    }
}
