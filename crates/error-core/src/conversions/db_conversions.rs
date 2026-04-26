//! Database error conversions (surrealdb)

use crate::error_object::ErrorObject;
use crate::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
use crate::error_code::registry;

impl From<surrealdb::Error> for ErrorObject {
    fn from(e: surrealdb::Error) -> Self {
        Self::builder()
            .code(registry::DB_QUERY_FAILED)
            .source(ErrorSource::FS)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::OPERATION)
            .recoverability(Recoverability::SemiAuto)
            .message(&format!("数据库查询失败: {e}"))
            .user_message("数据库操作失败，请稍后重试")
            .module_path("db")
            .operation("query")
            .build()
    }
}
