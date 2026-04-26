//! JSON Web Token error conversions

use crate::error_object::ErrorObject;
use crate::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
use crate::error_code::registry;

impl From<jsonwebtoken::errors::Error> for ErrorObject {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        Self::builder()
            .code(registry::TOKEN_INVALID)
            .source(ErrorSource::SEC)
            .severity(Severity::WARNING)
            .impact_scope(ImpactScope::OPERATION)
            .recoverability(Recoverability::NonRecoverable)
            .message(&format!("Token 验证失败: {e}"))
            .user_message("认证令牌无效或已过期")
            .module_path("auth")
            .operation("validate_token")
            .build()
    }
}
