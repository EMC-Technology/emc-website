//! `Error` object core data model
//! 
//! This module defines the `ErrorObject` struct, which is the core data model for error handling.
//! It includes fields for error identification, classification, content, context, causality, and recovery.

use std::collections::HashMap;
use std::marker::PhantomData;
#[cfg(feature = "uuid")]
use uuid::Uuid;
#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};
use crate::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
use crate::propagation::{ContextFrame, RecoveryHint, RetryConfig};
use crate::error_code::registry;

#[allow(missing_docs)]
pub mod state {
    #[allow(missing_docs)]
    pub struct Missing;
    #[allow(missing_docs)]
    pub struct Present;
}

/// Error object core data model
/// 
/// Represents a comprehensive error with all necessary information for error handling, logging, and recovery.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ErrorObject {
    // 标识域
    code: String,
    #[cfg(feature = "uuid")]
    error_id: Uuid,
    
    // 分类域
    source: ErrorSource,
    severity: Severity,
    impact_scope: ImpactScope,
    recoverability: Recoverability,
    
    // 内容域
    message: String,
    user_message: String,
    details: HashMap<String, serde_json::Value>,
    
    // 上下文域
    #[cfg(feature = "chrono")]
    timestamp: DateTime<Utc>,
    session_id: Option<String>,
    request_id: Option<String>,
    module_path: String,
    operation: String,
    
    // 因果域
    cause: Option<Box<Self>>,
    context_chain: Vec<ContextFrame>,
    
    // 恢复域
    recovery_hints: Vec<RecoveryHint>,
    retry_config: Option<RetryConfig>,
}

impl std::fmt::Display for ErrorObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        if let Some(cause) = &self.cause {
            write!(f, "\nCaused by: {cause}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ErrorObject {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.as_ref().map(|e| e.as_ref() as &dyn std::error::Error)
    }
}

impl ErrorObject {
    /// Create a new `ErrorObject` builder
    #[must_use]
    pub fn builder() -> ErrorObjectBuilder {
        ErrorObjectBuilder::new()
    }
    
    /// Convenience method to create an `ErrorObject` with minimal required fields
    ///
    /// Generates a fully compliant error code following the format:
    /// `ERR-{SOURCE}-{MODULE}-{SEQ}_{SEVERITY}_{IMPACT}`
    ///
    /// # Deprecated
    ///
    /// 此方法绕过 `helpers` 模块和注册表约束，生成的错误码不受全局管控。
    /// 请使用 `error_core::helpers` 中的统一构造函数。
    #[deprecated(
        since = "0.2.0",
        note = "请使用 error_core::helpers 中的统一构造函数，确保错误码受注册表管控"
    )]
    #[must_use]
    pub fn from_source(
        source: ErrorSource,
        module: &str,
        operation: &str,
        message: &str,
    ) -> Self {
        let severity = match source {
            ErrorSource::SEC => Severity::CRITICAL,
            _ => Severity::ERROR,
        };
        let impact_scope = ImpactScope::OPERATION;
        let recoverability = match source {
            ErrorSource::NET => Recoverability::AutoRecoverable,
            ErrorSource::FS => Recoverability::SemiAuto,
            _ => Recoverability::NonRecoverable,
        };
        let module_short: String = module.chars().take(5).collect();
        let code = format!(
            "ERR-{}-{}-001_{}_{}",
            source.as_str(),
            module_short.to_uppercase(),
            severity.as_str(),
            impact_scope.as_str()
        );

        let user_message = match source {
            ErrorSource::NET => "网络服务暂时不可用，请稍后重试",
            ErrorSource::FS => "文件操作失败，请稍后重试",
            ErrorSource::SEC => "安全操作失败",
            _ => "操作失败，请稍后重试",
        };

        Self {
            code,
            #[cfg(feature = "uuid")]
            error_id: Uuid::new_v4(),
            source,
            severity,
            impact_scope,
            recoverability,
            message: message.to_string(),
            user_message: user_message.to_string(),
            details: HashMap::new(),
            #[cfg(feature = "chrono")]
            timestamp: Utc::now(),
            session_id: None,
            request_id: None,
            module_path: module.to_string(),
            operation: operation.to_string(),
            cause: None,
            context_chain: Vec::new(),
            recovery_hints: Vec::new(),
            retry_config: None,
        }
    }
    
    /// Wrap a `std::error::Error` as the cause of this `ErrorObject`
    #[must_use]
    pub fn with_cause_std(mut self, error: impl std::error::Error + 'static) -> Self {
        let cause_obj = crate::helpers::internal_error(&error.to_string());
        self.cause = Some(Box::new(cause_obj));
        self
    }
    
    /// 获取 HTTP 状态码映射
    ///
    /// 基于错误码和严重级别进行映射，覆盖设计文档要求的所有状态码：
    /// - 400 Bad Request — 用户输入错误
    /// - 401 Unauthorized — 认证失败
    /// - 404 Not Found — 资源未找到
    /// - 415 Unsupported Media Type — 不支持的格式
    /// - 422 Unprocessable Entity — 解析失败
    /// - 500 Internal Server Error — 内部错误
    /// - 503 Service Unavailable — 数据库/网络不可用
    #[must_use]
    pub fn http_status(&self) -> u16 {
        if self.code == registry::NOT_FOUND
            || self.code == registry::INVALID_RECORD_ID
        {
            return 404;
        }
        if self.code == registry::UNSUPPORTED_FORMAT {
            return 415;
        }
        if self.code == registry::PARSE_FAILED {
            return 422;
        }
        if self.code == registry::VALIDATION_FAILED {
            return 400;
        }
        if self.code == registry::AUTH_FAILED
            || self.code == registry::TOKEN_INVALID
        {
            return 401;
        }
        if self.code == registry::DB_QUERY_FAILED
            || self.code == registry::CONNECTION_LOST
            || self.code == registry::IO_FAILED
        {
            return 503;
        }
        if self.code == registry::CRYPTO_FAILED {
            return 500;
        }

        match self.severity {
            Severity::CRITICAL => 500,
            Severity::ERROR => match self.source {
                ErrorSource::USR => 400,
                ErrorSource::SEC => 401,
                _ => 500,
            },
            Severity::WARNING => 400,
            Severity::INFO => 200,
        }
    }
    
    /// Get the error code
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }
    
    /// Get the error ID
    #[cfg(feature = "uuid")]
    #[must_use]
    pub const fn error_id(&self) -> &Uuid {
        &self.error_id
    }
    
    /// Get the error source
    #[must_use]
    pub const fn source(&self) -> ErrorSource {
        self.source
    }
    
    /// Get the severity level
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }
    
    /// Get the impact scope
    #[must_use]
    pub const fn impact_scope(&self) -> ImpactScope {
        self.impact_scope
    }
    
    /// Get the recoverability
    #[must_use]
    pub const fn recoverability(&self) -> Recoverability {
        self.recoverability
    }
    
    /// Get the error message
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
    
    /// Get the user-friendly message
    #[must_use]
    pub fn user_message(&self) -> &str {
        &self.user_message
    }
    
    /// Get the details
    #[must_use]
    pub const fn details(&self) -> &HashMap<String, serde_json::Value> {
        &self.details
    }
    
    /// Get the timestamp
    #[cfg(feature = "chrono")]
    #[must_use]
    pub const fn timestamp(&self) -> &DateTime<Utc> {
        &self.timestamp
    }
    
    /// Get the session ID
    #[must_use]
    pub const fn session_id(&self) -> Option<&String> {
        self.session_id.as_ref()
    }
    
    /// Get the request ID
    #[must_use]
    pub const fn request_id(&self) -> Option<&String> {
        self.request_id.as_ref()
    }
    
    /// Get the module path
    #[must_use]
    pub fn module_path(&self) -> &str {
        &self.module_path
    }
    
    /// Get the operation
    #[must_use]
    pub fn operation(&self) -> &str {
        &self.operation
    }
    
    /// Get the cause
    #[must_use]
    pub fn cause(&self) -> Option<&Self> {
        self.cause.as_deref()
    }
    
    /// Get the context chain
    #[must_use]
    pub const fn context_chain(&self) -> &Vec<ContextFrame> {
        &self.context_chain
    }
    
    /// Get the recovery hints
    #[must_use]
    pub const fn recovery_hints(&self) -> &Vec<RecoveryHint> {
        &self.recovery_hints
    }
    
    /// Get the retry config
    #[must_use]
    pub const fn retry_config(&self) -> Option<&RetryConfig> {
        self.retry_config.as_ref()
    }
    
    /// Add a context frame to the error object
    pub fn add_context_frame(&mut self, frame: ContextFrame) {
        self.context_chain.push(frame);
    }
    
    /// Set the cause of the error
    pub fn set_cause(&mut self, cause: Self) {
        self.cause = Some(Box::new(cause));
    }

    /// Clear all details (for stripping internal information before external exposure)
    pub fn clear_details(&mut self) {
        self.details.clear();
    }

    /// Clear the context chain (for stripping internal information before external exposure)
    pub fn clear_context_chain(&mut self) {
        self.context_chain.clear();
    }
}

struct BuilderData {
    code: Option<String>,
    #[cfg(feature = "uuid")]
    error_id: Option<Uuid>,
    source: Option<ErrorSource>,
    severity: Option<Severity>,
    impact_scope: Option<ImpactScope>,
    recoverability: Option<Recoverability>,
    message: Option<String>,
    user_message: Option<String>,
    details: HashMap<String, serde_json::Value>,
    #[cfg(feature = "chrono")]
    timestamp: Option<DateTime<Utc>>,
    session_id: Option<String>,
    request_id: Option<String>,
    module_path: Option<String>,
    operation: Option<String>,
    cause: Option<Box<ErrorObject>>,
    context_chain: Vec<ContextFrame>,
    recovery_hints: Vec<RecoveryHint>,
    retry_config: Option<RetryConfig>,
}

#[allow(missing_docs, clippy::type_complexity)]
pub struct ErrorObjectBuilder<
    Code = state::Missing,
    Src = state::Missing,
    Sev = state::Missing,
    Imp = state::Missing,
    Rec = state::Missing,
    Msg = state::Missing,
    Usr = state::Missing,
    Mod = state::Missing,
    Opr = state::Missing,
> {
    data: BuilderData,
    _marker: PhantomData<(Code, Src, Sev, Imp, Rec, Msg, Usr, Mod, Opr)>,
}

impl Default for ErrorObjectBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorObjectBuilder {
    #[allow(missing_docs)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: BuilderData {
                code: None,
                #[cfg(feature = "uuid")]
                error_id: Some(Uuid::new_v4()),
                source: None,
                severity: None,
                impact_scope: None,
                recoverability: None,
                message: None,
                user_message: None,
                details: HashMap::new(),
                #[cfg(feature = "chrono")]
                timestamp: Some(Utc::now()),
                session_id: None,
                request_id: None,
                module_path: None,
                operation: None,
                cause: None,
                context_chain: Vec::new(),
                recovery_hints: Vec::new(),
                retry_config: None,
            },
            _marker: PhantomData,
        }
    }
}

#[allow(missing_docs)]
impl<C, S, Se, I, R, M, U, Mo, O> ErrorObjectBuilder<C, S, Se, I, R, M, U, Mo, O> {
    #[must_use]
    pub fn code(self, code: &str) -> ErrorObjectBuilder<state::Present, S, Se, I, R, M, U, Mo, O> {
        ErrorObjectBuilder {
            data: BuilderData { code: Some(code.to_string()), ..self.data },
            _marker: PhantomData,
        }
    }

    #[cfg(feature = "uuid")]
    #[must_use]
    pub fn error_id(mut self, error_id: Uuid) -> Self {
        self.data.error_id = Some(error_id);
        self
    }

    #[must_use]
    pub fn source(self, source: ErrorSource) -> ErrorObjectBuilder<C, state::Present, Se, I, R, M, U, Mo, O> {
        ErrorObjectBuilder {
            data: BuilderData { source: Some(source), ..self.data },
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn severity(self, severity: Severity) -> ErrorObjectBuilder<C, S, state::Present, I, R, M, U, Mo, O> {
        ErrorObjectBuilder {
            data: BuilderData { severity: Some(severity), ..self.data },
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn impact_scope(self, impact_scope: ImpactScope) -> ErrorObjectBuilder<C, S, Se, state::Present, R, M, U, Mo, O> {
        ErrorObjectBuilder {
            data: BuilderData { impact_scope: Some(impact_scope), ..self.data },
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn recoverability(self, recoverability: Recoverability) -> ErrorObjectBuilder<C, S, Se, I, state::Present, M, U, Mo, O> {
        ErrorObjectBuilder {
            data: BuilderData { recoverability: Some(recoverability), ..self.data },
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn message(self, message: &str) -> ErrorObjectBuilder<C, S, Se, I, R, state::Present, U, Mo, O> {
        ErrorObjectBuilder {
            data: BuilderData { message: Some(message.to_string()), ..self.data },
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn user_message(self, user_message: &str) -> ErrorObjectBuilder<C, S, Se, I, R, M, state::Present, Mo, O> {
        ErrorObjectBuilder {
            data: BuilderData { user_message: Some(user_message.to_string()), ..self.data },
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn detail(mut self, key: &str, value: serde_json::Value) -> Self {
        self.data.details.insert(key.to_string(), value);
        self
    }

    #[cfg(feature = "chrono")]
    #[must_use]
    pub fn timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.data.timestamp = Some(timestamp);
        self
    }

    #[must_use]
    pub fn session_id(mut self, session_id: &str) -> Self {
        self.data.session_id = Some(session_id.to_string());
        self
    }

    #[must_use]
    pub fn request_id(mut self, request_id: &str) -> Self {
        self.data.request_id = Some(request_id.to_string());
        self
    }

    #[must_use]
    pub fn module_path(self, module_path: &str) -> ErrorObjectBuilder<C, S, Se, I, R, M, U, state::Present, O> {
        ErrorObjectBuilder {
            data: BuilderData { module_path: Some(module_path.to_string()), ..self.data },
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn operation(self, operation: &str) -> ErrorObjectBuilder<C, S, Se, I, R, M, U, Mo, state::Present> {
        ErrorObjectBuilder {
            data: BuilderData { operation: Some(operation.to_string()), ..self.data },
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn cause(mut self, cause: ErrorObject) -> Self {
        self.data.cause = Some(Box::new(cause));
        self
    }

    #[must_use]
    pub fn context_frame(mut self, frame: ContextFrame) -> Self {
        self.data.context_chain.push(frame);
        self
    }

    #[must_use]
    pub fn recovery_hint(mut self, hint: RecoveryHint) -> Self {
        self.data.recovery_hints.push(hint);
        self
    }

    #[must_use]
    pub fn retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.data.retry_config = Some(retry_config);
        self
    }

    /// Build the `ErrorObject`, returning a `Result` that validates all required fields.
    /// 
    /// # Errors
    /// 
    /// Returns an error if any required field is not set.
    pub fn build_checked(self) -> crate::Result<ErrorObject> {
        let code = self.data.code.ok_or_else(|| {
            crate::error_code::ec_error("Error code is required")
        })?;
        #[cfg(feature = "uuid")]
        let error_id = self.data.error_id.ok_or_else(|| {
            crate::error_code::ec_error("Error ID is required")
        })?;
        let source = self.data.source.ok_or_else(|| {
            crate::error_code::ec_error("Error source is required")
        })?;
        let severity = self.data.severity.ok_or_else(|| {
            crate::error_code::ec_error("Severity is required")
        })?;
        let impact_scope = self.data.impact_scope.ok_or_else(|| {
            crate::error_code::ec_error("Impact scope is required")
        })?;
        let recoverability = self.data.recoverability.ok_or_else(|| {
            crate::error_code::ec_error("Recoverability is required")
        })?;
        let message = self.data.message.ok_or_else(|| {
            crate::error_code::ec_error("Error message is required")
        })?;
        let user_message = self.data.user_message.ok_or_else(|| {
            crate::error_code::ec_error("User message is required")
        })?;
        #[cfg(feature = "chrono")]
        let timestamp = self.data.timestamp.ok_or_else(|| {
            crate::error_code::ec_error("Timestamp is required")
        })?;
        let module_path = self.data.module_path.ok_or_else(|| {
            crate::error_code::ec_error("Module path is required")
        })?;
        let operation = self.data.operation.ok_or_else(|| {
            crate::error_code::ec_error("Operation is required")
        })?;

        Ok(ErrorObject {
            code,
            #[cfg(feature = "uuid")]
            error_id,
            source,
            severity,
            impact_scope,
            recoverability,
            message,
            user_message,
            details: self.data.details,
            #[cfg(feature = "chrono")]
            timestamp,
            session_id: self.data.session_id,
            request_id: self.data.request_id,
            module_path,
            operation,
            cause: self.data.cause,
            context_chain: self.data.context_chain,
            recovery_hints: self.data.recovery_hints,
            retry_config: self.data.retry_config,
        })
    }
}

#[allow(missing_docs)]
impl ErrorObjectBuilder<
    state::Present, state::Present, state::Present, state::Present,
    state::Present, state::Present, state::Present, state::Present,
    state::Present,
> {
    /// Build the `ErrorObject`. All type-state guaranteed fields must be set.
    /// 
    /// # Panics
    /// 
    /// Panics if any type-state guaranteed field is not set (should never happen when using the builder correctly).
    #[must_use]
    pub fn build(self) -> ErrorObject {
        ErrorObject {
            code: self.data.code.expect("type-state guarantee: code is set"),
            #[cfg(feature = "uuid")]
            error_id: self.data.error_id.expect("type-state guarantee: error_id is set"),
            source: self.data.source.expect("type-state guarantee: source is set"),
            severity: self.data.severity.expect("type-state guarantee: severity is set"),
            impact_scope: self.data.impact_scope.expect("type-state guarantee: impact_scope is set"),
            recoverability: self.data.recoverability.expect("type-state guarantee: recoverability is set"),
            message: self.data.message.expect("type-state guarantee: message is set"),
            user_message: self.data.user_message.expect("type-state guarantee: user_message is set"),
            #[cfg(feature = "chrono")]
            timestamp: self.data.timestamp.expect("type-state guarantee: timestamp is set"),
            module_path: self.data.module_path.expect("type-state guarantee: module_path is set"),
            operation: self.data.operation.expect("type-state guarantee: operation is set"),
            details: self.data.details,
            session_id: self.data.session_id,
            request_id: self.data.request_id,
            cause: self.data.cause,
            context_chain: self.data.context_chain,
            recovery_hints: self.data.recovery_hints,
            retry_config: self.data.retry_config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_object_build() {
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();

        assert_eq!(error.code(), "ERR-AIM-LM-002_ERR_S");
        assert_eq!(error.source(), ErrorSource::AIM);
        assert_eq!(error.severity(), Severity::ERROR);
        assert_eq!(error.impact_scope(), ImpactScope::SESSION);
        assert_eq!(error.recoverability(), Recoverability::AutoRecoverable);
        assert_eq!(error.message(), "AI model call timed out");
        assert_eq!(error.user_message(), "AI model service is temporarily unavailable");
        assert_eq!(error.module_path(), "ai_model::lm_manager");
        assert_eq!(error.operation(), "generate_code_completion");
    }
    
    #[test]
    fn test_error_object_missing_required_fields() {
        // Test missing code
        let error = ErrorObject::builder()
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build_checked();

        assert!(error.is_err());

        // Test missing source
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build_checked();

        assert!(error.is_err());

        // Test missing severity
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build_checked();

        assert!(error.is_err());

        // Test missing impact_scope
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build_checked();

        assert!(error.is_err());

        // Test missing recoverability
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build_checked();

        assert!(error.is_err());

        // Test missing message
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build_checked();

        assert!(error.is_err());

        // Test missing user_message
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build_checked();

        assert!(error.is_err());

        // Test missing module_path
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .operation("generate_code_completion")
            .build_checked();

        assert!(error.is_err());

        // Test missing operation
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .build_checked();

        assert!(error.is_err());
    }

    #[test]
    fn test_error_object_getters() {
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();
        
        assert_eq!(error.code(), "ERR-AIM-LM-002_ERR_S");
        assert_eq!(error.source(), ErrorSource::AIM);
        assert_eq!(error.severity(), Severity::ERROR);
        assert_eq!(error.impact_scope(), ImpactScope::SESSION);
        assert_eq!(error.recoverability(), Recoverability::AutoRecoverable);
        assert_eq!(error.message(), "AI model call timed out");
        assert_eq!(error.user_message(), "AI model service is temporarily unavailable");
        assert_eq!(error.module_path(), "ai_model::lm_manager");
        assert_eq!(error.operation(), "generate_code_completion");
        assert!(error.session_id().is_none());
        assert!(error.request_id().is_none());
        assert!(error.cause().is_none());
        assert!(error.retry_config().is_none());
        assert!(error.recovery_hints().is_empty());
        assert!(error.context_chain().is_empty());
    }

    #[test]
    fn test_error_object_add_context_frame() {
        let mut error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();
        
        let context_frame = ContextFrame::new("api_gateway", HashMap::new());
        error.add_context_frame(context_frame);
        
        assert_eq!(error.context_chain().len(), 1);
        assert_eq!(error.context_chain()[0].source(), "api_gateway");
    }

    #[test]
    fn test_error_object_set_cause() {
        let cause = ErrorObject::builder()
            .code("ERR-NET-API-001_ERR_S")
            .source(ErrorSource::NET)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Network timeout")
            .user_message("Network service is temporarily unavailable")
            .module_path("network::api_client")
            .operation("send_request")
            .build();

        let mut error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();
        
        error.set_cause(cause);
        assert!(error.cause().is_some());
        assert_eq!(error.cause().unwrap().code(), "ERR-NET-API-001_ERR_S");
    }

    #[test]
    fn test_error_object_builder_with_optional_fields() {
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .session_id("session_123")
            .request_id("request_456")
            .detail("model_name", serde_json::json!("gpt-4"))
            .build();

        assert_eq!(error.session_id().unwrap(), "session_123");
        assert_eq!(error.request_id().unwrap(), "request_456");
        assert_eq!(error.details().get("model_name").unwrap(), &serde_json::json!("gpt-4"));
    }

    #[test]
    fn test_error_object_builder_with_recovery_hints() {
        let recovery_hint = RecoveryHint::new("Retry", "Retry the operation", HashMap::new());
        let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);

        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .recovery_hint(recovery_hint)
            .retry_config(retry_config)
            .build();

        assert_eq!(error.recovery_hints().len(), 1);
        assert!(error.retry_config().is_some());
    }

    #[test]
    fn test_error_object_builder_with_context_frame() {
        let context_frame = ContextFrame::new("api_gateway", HashMap::new());

        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .context_frame(context_frame)
            .build();

        assert_eq!(error.context_chain().len(), 1);
    }

    #[test]
    fn test_error_object_builder_with_cause() {
        let cause = ErrorObject::builder()
            .code("ERR-NET-API-001_ERR_S")
            .source(ErrorSource::NET)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Network timeout")
            .user_message("Network service is temporarily unavailable")
            .module_path("network::api_client")
            .operation("send_request")
            .build();

        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .cause(cause)
            .build();

        assert!(error.cause().is_some());
    }

    #[test]
    fn test_error_object_builder_default_impl() {
        let builder = ErrorObjectBuilder::default();
        let error = builder
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();

        assert_eq!(error.code(), "ERR-AIM-LM-002_ERR_S");
    }
}
