//! 中间件集合
//!
//! 统一导出所有 Axum 中间件，包括：
//! - PII 脱敏中间件（防止敏感信息泄漏）
//! - 分布式追踪中间件（HTTP Span 自动创建）
//! - 数据库追踪包装器（DB 操作 Span）
//! - mTLS 认证中间件（服务间零信任认证）
//! - 安全头中间件（防止常见 Web 攻击）

pub mod db_tracing;
pub mod mtls;
pub mod pii_redact;
pub mod security_headers;
pub mod tracing_middleware;

pub use db_tracing::{DbTracer, default_db_tracer};
pub use mtls::{
    CertificateVerifier, ClientIdentity, Environment, MtlsConfig, ServiceTokenClaims,
    StsTokenGenerator, TrustLevel,
};
pub use pii_redact::{PiiRedactionResult, pii_redact_middleware, redact_pii, sanitize_log_message};
pub use security_headers::security_headers_middleware;
pub use tracing_middleware::{TraceInfo, current_trace_info, tracing_middleware};
