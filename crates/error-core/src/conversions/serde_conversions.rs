//! Serde JSON error conversions

use crate::classification::{ErrorSource, ImpactScope, Recoverability, Severity};
use crate::error_code::registry;
use crate::error_object::ErrorObject;

impl From<serde_json::Error> for ErrorObject {
    fn from(e: serde_json::Error) -> Self {
        Self::builder()
            .code(registry::SERDE_FAILED)
            .source(ErrorSource::INT)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::OPERATION)
            .recoverability(Recoverability::NonRecoverable)
            .message(&format!("序列化/反序列化失败: {e}"))
            .user_message("数据处理失败，请检查输入格式")
            .module_path("serde")
            .operation("serialize")
            .build()
    }
}
