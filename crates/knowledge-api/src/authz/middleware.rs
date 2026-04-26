//! 授权中间件（Axum 集成层）
//!
//! 将 `Cedar` 授权引擎无缝集成到 Axum 请求处理链中：
//! - 从请求扩展中提取已认证的主体身份
//! - 构建完整的 `AuthorizationRequest`
//! - 调用 `AuthorizationEngine` 执行决策
//! - 将决策结果注入到请求扩展中供下游 Handler 使用

use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use tracing::{debug, error, instrument};

use crate::authz::policy::{
    Action, ActionCategory, AuthorizationDecision, AuthorizationRequest, Context, DecisionType,
    DenialReason, LiteralValue, Principal, Resource,
};
use crate::handler::AppState;

/// 授权中间件状态扩展
///
/// 注入到 Axum Request Extensions 中，下游 Handler 可通过 `Extension<AuthzResult>` 提取。
#[derive(Debug, Clone)]
pub struct AuthzResult {
    /// 授权决策结果
    pub decision: AuthorizationDecision,
}

/// 已认证主体信息（由上游 `auth_middleware` 注入）
///
/// 从 JWT Claims 或 `mTLS` 证书中提取的身份信息。
#[derive(Debug, Clone)]
pub struct AuthenticatedPrincipal {
    /// 用户/服务 ID
    pub id: String,
    /// 实体类型
    pub entity_type: String,
    /// 角色列表
    pub roles: Vec<String>,
    /// 额外属性
    pub attrs: HashMap<String, LiteralValue>,
}

/// 授权中间件
///
/// 在每个需要权限控制的路由上使用，执行以下流程：
/// 1. 从 `Request Extensions` 提取 `AuthenticatedPrincipal`
/// 2. 根据路由信息构建 `Resource` 和 `Action`
/// 3. 调用 `AuthorizationEngine.authorize()`
/// 4. Allow → 放行；Deny → 返回 403 Forbidden
///
/// # Example
///
/// ```ignore
/// let protected_routes = Router::new()
///     .route("/api/v1/documents", get(list_documents))
///     .layer(middleware::from_fn_with_state(
///     app_state.clone(),
///     authorization_middleware,
/// ));
/// ```
#[instrument(skip(state, req, next), fields(path = %req.uri().path(), method = %req.method()))]
pub async fn authorization_middleware(
    State(state): State<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let Some(authz_engine) = &state.authz_engine else {
        error!("AuthorizationEngine 未初始化，按零信任原则拒绝请求");
        return InternalErrorResponse {
            message: "授权服务暂时不可用".to_string(),
            code: "AUTHZ_ENGINE_UNINITIALIZED",
        }
        .into_response();
    };

    let Some(principal) = req.extensions_mut().remove::<AuthenticatedPrincipal>() else {
        debug!("未找到已认证主体信息，返回 401");
        return UnauthorizedResponse {
            message: "未提供有效的身份凭证".to_string(),
            code: "AUTHZ_UNAUTHENTICATED",
        }
        .into_response();
    };

    let (action, resource) = build_action_and_resource_from_request(&req);

    let source_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .or_else(|| req.headers().get("X-Real-IP").and_then(|v| v.to_str().ok()))
        .map(String::from);

    let user_agent = req
        .headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let authz_req = AuthorizationRequest {
        principal: Principal {
            id: principal.id.clone(),
            entity_type: principal.entity_type.clone(),
            roles: principal.roles.clone(),
            attrs: principal.attrs.clone(),
        },
        action,
        resource,
        context: Context {
            request_time: chrono::Utc::now(),
            source_ip,
            user_agent,
            device_fingerprint: None,
            extra: HashMap::new(),
        },
    };

    match authz_engine.authorize(&authz_req).await {
        Ok(decision) => {
            debug!(
                decision = ?decision.decision,
                principal_id = %authz_req.principal.id,
                action = %authz_req.action.id,
                "授权决策完成"
            );

            match decision.decision {
                DecisionType::Allowed => {
                    req.extensions_mut().insert(AuthzResult { decision });
                    next.run(req).await
                }
                DecisionType::Denied { reason } => {
                    ForbiddenResponse::from_denial_reason(reason).into_response()
                }
            }
        }
        Err(e) => {
            error!(error = %e, "授权引擎内部错误");
            InternalErrorResponse {
                message: "授权服务暂时不可用".to_string(),
                code: "AUTHZ_INTERNAL_ERROR",
            }
            .into_response()
        }
    }
}

/// 从 HTTP 请求中提取 Action 和 Resource 信息
///
/// 基于请求方法、路径和查询参数推断动作类别和目标资源。
fn build_action_and_resource_from_request(
    req: &Request<axum::body::Body>,
) -> (Action, Resource) {
    let method = req.method();
    let path = req.uri().path();

    let action_category = match method.as_str() {
        "GET" | "HEAD" | "OPTIONS" => ActionCategory::Read,
        "POST" | "PUT" | "PATCH" => ActionCategory::Write,
        "DELETE" => ActionCategory::Delete,
        _ => ActionCategory::Admin,
    };

    // 从路径推断资源类型和 ID
    let (resource_type, resource_id) = parse_resource_from_path(path);
    let action_id = format!(
        "{}::{}",
        to_lowercase_first(&resource_type),
        action_category.as_str()
    );

    let action = Action {
        id: action_id,
        resource_type: resource_type.clone(),
        category: action_category,
    };

    let resource = Resource {
        id: resource_id.unwrap_or_else(|| "*".to_string()),
        resource_type,
        owner_id: None,
        scope_id: None,
        attrs: HashMap::new(),
    };

    (action, resource)
}

/// 简单的路径解析器 —— 从 URL 路径推断资源类型和资源 ID
///
/// 支持格式：`/api/v1/{resource_type}/{resource_id}`
fn parse_resource_from_path(path: &str) -> (String, Option<String>) {
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    if segments.len() >= 4 && segments[0] == "api" {
        let resource_type = segments[2].to_string();
        let resource_id = segments.get(3).map(ToString::to_string);
        (resource_type, resource_id)
    } else {
        ("Unknown".to_string(), None)
    }
}

fn to_lowercase_first(s: &str) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    if let Some(first) = chars.first_mut() {
        *first = first.to_ascii_lowercase();
    }
    chars.into_iter().collect()
}

impl ActionCategory {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Delete => "delete",
            Self::Admin => "admin",
            Self::Execute => "execute",
        }
    }
}

// ========== 错误响应类型 ==========

struct UnauthorizedResponse {
    message: String,
    code: &'static str,
}

impl IntoResponse for UnauthorizedResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            [("Content-Type", "application/json")],
            serde_json::json!({
                "success": false,
                "error": {
                    "code": self.code,
                    "message": self.message,
                }
            })
            .to_string(),
        )
            .into_response()
    }
}

struct ForbiddenResponse {
    status_code: StatusCode,
    message: String,
    code: String,
    details: Option<String>,
}

impl ForbiddenResponse {
    fn from_denial_reason(reason: DenialReason) -> Self {
        match reason {
            DenialReason::NoMatchingPolicy => Self {
                status_code: StatusCode::FORBIDDEN,
                message: "没有足够的权限执行此操作".to_string(),
                code: "AUTHZ_FORBIDDEN_NO_POLICY".to_string(),
                details: None,
            },
            DenialReason::ExplicitDeny => Self {
                status_code: StatusCode::FORBIDDEN,
                message: "此操作被安全策略显式禁止".to_string(),
                code: "AUTHZ_FORBIDDEN_EXPLICIT_DENY".to_string(),
                details: None,
            },
            DenialReason::MissingPermission { required, held } => Self {
                status_code: StatusCode::FORBIDDEN,
                message: format!("缺少必要权限: {required} (当前持有: {held:?})"),
                code: "AUTHZ_FORBIDDEN_MISSING_PERMISSION".to_string(),
                details: None,
            },
            DenialReason::ConditionNotMet { condition, detail } => Self {
                status_code: StatusCode::FORBIDDEN,
                message: format!("访问条件不满足: {condition}"),
                code: "AUTHZ_FORBIDDEN_CONDITION_NOT_MET".to_string(),
                details: Some(detail),
            },
            DenialReason::ResourceNotFound => Self {
                status_code: StatusCode::NOT_FOUND,
                message: "目标资源不存在".to_string(),
                code: "AUTHZ_RESOURCE_NOT_FOUND".to_string(),
                details: None,
            },
            DenialReason::Unauthenticated => Self {
                status_code: StatusCode::UNAUTHORIZED,
                message: "用户未登录或会话已过期".to_string(),
                code: "AUTHZ_UNAUTHENTICATED".to_string(),
                details: None,
            },
            DenialReason::AccountDisabled => Self {
                status_code: StatusCode::FORBIDDEN,
                message: "账户已被禁用".to_string(),
                code: "AUTHZ_ACCOUNT_DISABLED".to_string(),
                details: None,
            },
            DenialReason::EvaluationError(err) => Self {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                message: "授权评估出错".to_string(),
                code: "AUTHZ_EVALUATION_ERROR".to_string(),
                details: Some(err),
            },
        }
    }
}

impl IntoResponse for ForbiddenResponse {
    fn into_response(self) -> Response {
        let mut body = serde_json::json!({
            "success": false,
            "error": {
                "code": self.code,
                "message": self.message,
            }
        });
        if let Some(details) = self.details {
            body["error"]["details"] = serde_json::Value::String(details);
        }

        (
            self.status_code,
            [("Content-Type", "application/json")],
            body.to_string(),
        )
            .into_response()
    }
}



struct InternalErrorResponse {
    message: String,
    code: &'static str,
}

impl IntoResponse for InternalErrorResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("Content-Type", "application/json")],
            serde_json::json!({
                "success": false,
                "error": {
                    "code": self.code,
                    "message": self.message,
                }
            })
            .to_string(),
        )
            .into_response()
    }
}

/// 授权结果提取器 —— 用于下游 Handler 获取授权决策详情
pub struct AuthorizationExtractor;

impl AuthorizationExtractor {
    /// 从 Request Extensions 中提取授权决策结果
    ///
    /// # Panics
    ///
    /// 若授权中间件未正确注入 `AuthzResult` 则 panic。
    /// 此 panic 仅在路由配置错误时触发，属于不可恢复的编程错误。
    #[must_use]
    pub fn extract(ext: &axum::http::Extensions) -> &AuthorizationDecision {
        &ext.get::<AuthzResult>()
            .expect("[AUTHZ-INVARIANT] authorization_middleware 必须在此路由之前执行")
            .decision
    }
}
