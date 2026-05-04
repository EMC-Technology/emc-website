//! 统一错误处理中间件

use crate::handler::AppState;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use error_core::prelude::*;

#[derive(Debug, serde::Serialize)]
/// API 统一错误类型
///
/// 将内部 [`ErrorObject`] 转换为 HTTP 响应友好的格式。
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
    /// HTTP 状态码
    pub status: u16,
    /// 错误严重程度
    pub severity: String,
    /// 可恢复性
    pub recoverability: String,
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
            status: err.http_status(),
            severity: err.severity().as_str().to_string(),
            recoverability: err.recoverability().as_str().to_string(),
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
        let status = if let Ok(s) = StatusCode::from_u16(self.status) {
            s
        } else {
            tracing::warn!(
                invalid_status = self.status,
                error_id = %self.error_id,
                code = %self.code,
                "无效的HTTP状态码，降级为500; 违反'0黑盒推断'原则的状态码来源需排查"
            );
            StatusCode::INTERNAL_SERVER_ERROR
        };

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
            "服务端错误已由 error_handler_middleware 捕获"
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
        assert_eq!(api_err.status, 400);
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
            status: 400,
            severity: "ERR".to_string(),
            recoverability: "NonRecoverable".to_string(),
            details: None,
        };

        let json = serde_json::to_string(&api_err).unwrap();
        assert!(json.contains("\"code\":\"ERR-USR-VAL-001_ERR_O\""));
        assert!(!json.contains("\"message\""), "内部消息不应序列化到客户端");
        assert!(json.contains("\"user_message\":\"测试\""));
        assert!(json.contains("\"status\":400"));
    }
}
