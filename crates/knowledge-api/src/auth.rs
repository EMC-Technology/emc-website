//! 用户角色与权限控制

#![allow(clippy::significant_drop_tightening)]

use axum::{
    Extension, Json,
    extract::State,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::handler::AppState;
use error_core::helpers;
use knowledge_core::{AuditEvent, AuditLogger};

/// 用户角色枚举
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum Role {
    /// 管理员：拥有所有权限
    Admin,
    /// 编辑者：可读写，不可删除或管理
    Editor,
    /// 查看者：仅可读取
    Viewer,
    /// 服务账户：可读写，不可删除或管理
    ServiceAccount,
    /// 匿名用户：无任何权限
    Anonymous,
}

impl std::str::FromStr for Role {
    type Err = error_core::ErrorObject;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(Self::Admin),
            "editor" => Ok(Self::Editor),
            "viewer" => Ok(Self::Viewer),
            "serviceaccount" => Ok(Self::ServiceAccount),
            "anonymous" => Ok(Self::Anonymous),
            other => Err(helpers::validation_error(
                &format!("未知角色: {other}"),
                "role_from_str",
            )),
        }
    }
}

impl Role {
    /// 检查当前角色是否拥有指定权限
    #[must_use]
    #[allow(clippy::match_same_arms)]
    pub const fn has_permission(&self, permission: Permission) -> bool {
        match (self, permission) {
            (Self::Admin, _) => true,
            (Self::Editor | Self::ServiceAccount, Permission::Read | Permission::Write) => true,
            (Self::Viewer, Permission::Read) => true,
            (Self::Editor | Self::ServiceAccount | Self::Viewer | Self::Anonymous, _) => false,
        }
    }
}

/// 操作权限枚举
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum Permission {
    /// 读取权限
    Read,
    /// 写入权限
    Write,
    /// 删除权限
    Delete,
    /// 管理权限
    Admin,
}

/// JWT Claims 结构
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// 用户名
    pub sub: String,
    /// 角色标识
    pub role: String,
    /// 过期时间（Unix 时间戳）
    pub exp: u64,
    /// 签发时间（Unix 时间戳）
    pub iat: u64,
    /// 签发者
    pub iss: String,
    /// 受众
    pub aud: String,
    /// JWT 唯一标识（用于 token 撤销/黑名单）
    pub jti: String,
}

/// 认证服务
///
/// 提供 JWT Token 的生成与验证功能。
pub struct AuthService {
    jwt_secret: String,
    jwt_expiry_hours: u8,
    audit_logger: Option<Box<AuditLogger>>,
}

impl AuthService {
    /// 创建认证服务实例
    ///
    /// # Errors
    ///
    /// 当 JWT 密钥为空或过短时返回错误。
    pub fn new(jwt_secret: &str, jwt_expiry_hours: u8) -> crate::Result<Self> {
        if jwt_secret.is_empty() {
            return Err(helpers::auth_error("JWT 密钥不能为空，请设置环境变量 KNOWLEDGE_JWT_SECRET", "auth_new"));
        }
        if jwt_secret.len() < 32 {
            return Err(helpers::auth_error("JWT 密钥长度不能少于 32 个字符（HMAC-SHA256 要求 256 位密钥）", "auth_new"));
        }
        Ok(Self {
            jwt_secret: jwt_secret.to_string(),
            jwt_expiry_hours,
            audit_logger: None,
        })
    }

    /// 为认证服务附加审计日志记录器（Builder 模式）
    #[must_use]
    pub fn with_audit_logger(mut self, logger: AuditLogger) -> Self {
        self.audit_logger = Some(Box::new(logger));
        self
    }

    /// # Errors
    ///
    /// 系统时间获取失败或 JWT 编码失败时返回错误。
    pub fn generate_token(&self, username: &str, role: &Role) -> crate::Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| helpers::internal_error(&format!("系统时间获取失败: {e}")))?
            .as_secs();

        let exp = now + u64::from(self.jwt_expiry_hours) * 3600;

        let jti = hex::encode(rand::random::<[u8; 16]>());

        let claims = Claims {
            sub: username.to_string(),
            role: serde_json::to_string(role)?,
            exp,
            iat: now,
            iss: "knowledge-api".to_string(),
            aud: "knowledge-api".to_string(),
            jti,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;

        if let Some(logger) = &self.audit_logger {
            let event = AuditEvent::PermissionChange {
                user_id: username.to_string(),
                role: format!("{role:?}"),
            };
            let _ = logger.log(username, &event, "auth");
        }

        Ok(token)
    }

    /// # Errors
    ///
    /// JWT 解码失败或签名验证不通过时返回错误。
    pub fn validate_token(&self, token: &str) -> crate::Result<Claims> {
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &{
                let mut validation = Validation::new(Algorithm::HS256);
                validation.set_issuer(&["knowledge-api"]);
                validation.set_audience(&["knowledge-api"]);
                validation
            },
        )?;

        Ok(claims.claims)
    }

    /// # Errors
    ///
    /// 角色字段反序列化失败时返回错误。
    pub fn get_role_from_claims(&self, claims: &Claims) -> crate::Result<Role> {
        let role: Role = serde_json::from_str(&claims.role)?;
        Ok(role)
    }
}

/// 认证中间件
///
/// 从请求头中提取 Bearer Token，验证 JWT 并注入用户角色到请求扩展。
/// 未携带 Token 时角色设为 Anonymous；Token 有效但角色字段无法解析时返回 401。
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    let auth_service = &state.auth_service;

    let role = if let Some(token) = token {
        match auth_service.validate_token(token) {
            Ok(claims) => match auth_service.get_role_from_claims(&claims) {
                Ok(role) => role,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "JWT Token 角色字段解析失败，拒绝请求"
                    );
                    return (axum::http::StatusCode::UNAUTHORIZED, "Token 角色无效")
                        .into_response();
                }
            },
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "JWT Token 验证失败，拒绝请求 [AUDIT-SEC-002]"
                );
                return (axum::http::StatusCode::UNAUTHORIZED, "Token 无效或已过期")
                    .into_response();
            }
        }
    } else {
        Role::Anonymous
    };

    req.extensions_mut().insert(role);

    next.run(req).await
}

/// 权限校验中间件
///
/// 从请求扩展中提取用户角色，根据 HTTP 方法映射所需权限，
/// 拒绝权限不足的请求。
pub async fn permission_middleware(
    Extension(role): Extension<Role>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let permission = match req.method().as_str() {
        "GET" | "HEAD" | "OPTIONS" => Permission::Read,
        "POST" | "PUT" | "PATCH" => Permission::Write,
        "DELETE" => Permission::Delete,
        _ => Permission::Admin,
    };

    if !role.has_permission(permission) {
        return (axum::http::StatusCode::FORBIDDEN, "权限不足").into_response();
    }

    next.run(req).await
}

struct LoginAttempt {
    failure_count: u32,
    first_failure: Instant,
    locked_until: Option<Instant>,
}

/// 登录速率限制器
///
/// 基于滑动窗口的登录尝试频率控制，防止暴力破解攻击。
/// 当用户在时间窗口内失败次数超过阈值时，触发临时锁定。
pub struct LoginRateLimiter {
    /// 每个用户的登录尝试记录（username -> attempt）
    attempts: Mutex<HashMap<String, LoginAttempt>>,
    /// 触发锁定的最大失败次数
    max_attempts: u32,
    /// 失败计数的时间窗口长度
    window: Duration,
    /// 锁定持续时长
    lockout: Duration,
}

impl LoginRateLimiter {
    #[must_use]
    /// 使用默认参数创建登录限流器
    ///
    /// 默认配置：
    /// - 最大尝试次数：5 次
    /// - 时间窗口：300 秒（5 分钟）
    /// - 锁定时长：900 秒（15 分钟）
    pub fn new() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
            max_attempts: 5,
            window: Duration::from_secs(300),
            lockout: Duration::from_secs(900),
        }
    }

    /// 检查并记录登录尝试，返回是否允许
    pub fn check_and_record(&self, username: &str) -> bool {
        let mut attempts = self.attempts.lock().unwrap_or_else(|e| {
            tracing::warn!("登录限流 Mutex 中毒，恢复访问: {e}");
            e.into_inner()
        });
        let now = Instant::now();

        if let Some(attempt) = attempts.get(username) {
            if let Some(locked_until) = attempt.locked_until {
                if now < locked_until {
                    return false;
                }
            }
        }

        let entry = attempts
            .entry(username.to_string())
            .or_insert(LoginAttempt {
                failure_count: 0,
                first_failure: now,
                locked_until: None,
            });

        if let Some(locked_until) = entry.locked_until {
            if now >= locked_until {
                entry.failure_count = 0;
                entry.first_failure = now;
                entry.locked_until = None;
            }
        }

        if now.duration_since(entry.first_failure) > self.window {
            entry.failure_count = 0;
            entry.first_failure = now;
        }

        entry.failure_count += 1;
        if entry.failure_count >= self.max_attempts {
            entry.locked_until = Some(now + self.lockout);
            tracing::warn!(
                username = username,
                "登录速率限制触发，锁定 15 分钟 [AUDIT-SEC-001]"
            );
        }

        true
    }

    /// 记录登录成功并清除该用户的失败计数
    ///
    /// # Arguments
    ///
    /// * `username` - 成功认证的用户名
    pub fn record_success(&self, username: &str) {
        let mut attempts = self.attempts.lock().unwrap_or_else(|e| {
            tracing::warn!("登录限流 Mutex 中毒，恢复访问: {e}");
            e.into_inner()
        });
        attempts.remove(username);
    }
}

impl Default for LoginRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// 登录请求
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
}

/// 登录响应
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    /// JWT Token
    pub token: String,
    /// 用户角色
    pub role: String,
}

/// 登录处理器
///
/// 验证用户名和密码，成功时返回 JWT Token。
///
/// # Errors
///
/// - 用户名或密码为空
/// - 登录频率超限
/// - 用户名或密码错误
/// - Token 生成失败
pub fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> crate::Result<Json<LoginResponse>> {
    if req.username.is_empty() || req.password.is_empty() {
        return Err(helpers::auth_error("用户名和密码不能为空", "login"));
    }

    let rate_limiter = &state.rate_limiter;
    if !rate_limiter.check_and_record(&req.username) {
        return Err(helpers::auth_error("登录尝试过于频繁，请稍后再试", "login"));
    }

    let auth_service = &state.auth_service;

    let users = configured_users();
    let (role, username) = users
        .iter()
        .find(|(u, _)| u == &req.username)
        .and_then(|(u, (role, password))| {
            if verify_password(&req.password, password) {
                Some((role.clone(), u.as_str()))
            } else {
                None
            }
        })
        .ok_or_else(|| {
            helpers::auth_error("用户名或密码错误", "login")
        })?;

    rate_limiter.record_success(&req.username);

    let token = auth_service.generate_token(username, &role)?;

    Ok(Json(LoginResponse {
        token,
        role: format!("{role:?}"),
    }))
}

fn configured_users() -> Vec<(String, (Role, String))> {
    let admin_pass = std::env::var("KNOWLEDGE_ADMIN_PASSWORD").unwrap_or_else(|_| {
        tracing::error!("KNOWLEDGE_ADMIN_PASSWORD 未设置，admin 用户不可用");
        String::new()
    });
    let editor_pass = std::env::var("KNOWLEDGE_EDITOR_PASSWORD").unwrap_or_else(|_| {
        tracing::error!("KNOWLEDGE_EDITOR_PASSWORD 未设置，editor 用户不可用");
        String::new()
    });
    let viewer_pass = std::env::var("KNOWLEDGE_VIEWER_PASSWORD").unwrap_or_else(|_| {
        tracing::error!("KNOWLEDGE_VIEWER_PASSWORD 未设置，viewer 用户不可用");
        String::new()
    });

    let mut users = Vec::new();
    if !admin_pass.is_empty() {
        users.push(("admin".to_string(), (Role::Admin, admin_pass)));
    }
    if !editor_pass.is_empty() {
        users.push(("editor".to_string(), (Role::Editor, editor_pass)));
    }
    if !viewer_pass.is_empty() {
        users.push(("viewer".to_string(), (Role::Viewer, viewer_pass)));
    }

    if users.is_empty() {
        tracing::error!(
            "未配置任何用户凭据，认证服务不可用。\
             请设置 KNOWLEDGE_ADMIN_PASSWORD 等环境变量。\
             详见 CONTRIBUTING.md 中的安全配置章节"
        );
    }

    users
}

fn verify_password(input: &str, expected: &str) -> bool {
    if expected.starts_with("$argon2") {
        verify_argon2_password(input, expected)
    } else {
        tracing::error!("检测到明文密码存储，拒绝认证");
        false
    }
}

fn hash_password_argon2(password: &str) -> crate::Result<String> {
    use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};
    use password_hash::SaltString;

    if password.len() > 128 {
        return Err(helpers::auth_error("密码长度超过限制", "hash_password"));
    }

    let salt = SaltString::generate(&mut password_hash::rand_core::OsRng);
    let params = Params::new(19456, 2, 1, Some(32)).unwrap_or_else(|_| Params::default());
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| helpers::auth_error(&format!("密码哈希计算失败: {e}"), "hash_password"))?;
    Ok(hash.to_string())
}

fn verify_argon2_password(password: &str, hash: &str) -> bool {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};

    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(error = %e, "argon2 哈希格式无效");
            return false;
        }
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// 生成 argon2 哈希密码（用于管理员工具）
///
/// # Errors
///
/// 密码哈希计算失败时返回错误。
pub fn generate_password_hash(password: &str) -> crate::Result<String> {
    hash_password_argon2(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_from_str() {
        assert!("admin".parse::<Role>().is_ok());
        assert!("editor".parse::<Role>().is_ok());
        assert!("viewer".parse::<Role>().is_ok());
        assert!("serviceaccount".parse::<Role>().is_ok());
        assert!("anonymous".parse::<Role>().is_ok());
        let result = "invalid".parse::<Role>();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("未知角色"),
            "错误消息应包含'未知角色'，实际: {err}"
        );
    }

    #[test]
    fn test_role_has_permission() {
        assert!(Role::Admin.has_permission(Permission::Read));
        assert!(Role::Admin.has_permission(Permission::Write));
        assert!(Role::Admin.has_permission(Permission::Delete));
        assert!(Role::Admin.has_permission(Permission::Admin));

        assert!(Role::Editor.has_permission(Permission::Read));
        assert!(Role::Editor.has_permission(Permission::Write));
        assert!(!Role::Editor.has_permission(Permission::Delete));
        assert!(!Role::Editor.has_permission(Permission::Admin));

        assert!(Role::Viewer.has_permission(Permission::Read));
        assert!(!Role::Viewer.has_permission(Permission::Write));
        assert!(!Role::Viewer.has_permission(Permission::Delete));
        assert!(!Role::Viewer.has_permission(Permission::Admin));

        assert!(Role::ServiceAccount.has_permission(Permission::Read));
        assert!(Role::ServiceAccount.has_permission(Permission::Write));
        assert!(!Role::ServiceAccount.has_permission(Permission::Delete));
        assert!(!Role::ServiceAccount.has_permission(Permission::Admin));

        assert!(!Role::Anonymous.has_permission(Permission::Read));
        assert!(!Role::Anonymous.has_permission(Permission::Write));
        assert!(!Role::Anonymous.has_permission(Permission::Delete));
        assert!(!Role::Anonymous.has_permission(Permission::Admin));
    }

    #[test]
    fn test_jwt_token() {
        let auth_service = AuthService::new("test-secret-that-is-at-least-32-chars", 1).expect("测试密钥应合法");
        let token = auth_service
            .generate_token("testuser", &Role::Editor)
            .unwrap();
        let claims = auth_service.validate_token(&token).unwrap();
        assert_eq!(claims.sub, "testuser");
        let role = auth_service.get_role_from_claims(&claims).unwrap();
        assert_eq!(role, Role::Editor);
    }

    #[test]
    fn test_verify_password_plaintext_storage_rejected() {
        assert!(!verify_password("same_password", "same_password"));
        assert!(!verify_password("admin", "admin"));
    }

    #[test]
    fn test_verify_password_wrong() {
        assert!(!verify_password("wrong", "admin"));
    }

    #[test]
    fn test_argon2_hash_and_verify() {
        let hash = generate_password_hash("test_password").unwrap();
        assert!(hash.starts_with("$argon2"));
        assert!(verify_argon2_password("test_password", &hash));
        assert!(!verify_argon2_password("wrong_password", &hash));
    }

    #[test]
    fn test_verify_argon2_invalid_hash() {
        assert!(!verify_argon2_password("password", "not-a-valid-hash"));
    }

    #[test]
    fn test_rate_limiter_allows_initial_attempts() {
        let limiter = LoginRateLimiter::new();
        assert!(limiter.check_and_record("testuser"));
    }

    #[test]
    fn test_rate_limiter_blocks_after_max_failures() {
        let limiter = LoginRateLimiter::new();
        for _ in 0..5 {
            limiter.check_and_record("testuser");
        }
        assert!(!limiter.check_and_record("testuser"));
    }

    #[test]
    fn test_rate_limiter_clears_on_success() {
        let limiter = LoginRateLimiter::new();
        for _ in 0..4 {
            limiter.check_and_record("testuser");
        }
        limiter.record_success("testuser");
        assert!(limiter.check_and_record("testuser"));
    }
}
