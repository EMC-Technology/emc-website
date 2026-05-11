//! Standard library error conversions

use crate::classification::{ErrorSource, ImpactScope, Recoverability, Severity};
use crate::error_code::registry;
use crate::error_object::ErrorObject;

impl From<std::io::Error> for ErrorObject {
    fn from(e: std::io::Error) -> Self {
        let cause_msg = e.to_string();

        let mut obj = Self::builder()
            .code(registry::IO_FAILED)
            .source(ErrorSource::FS)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::OPERATION)
            .recoverability(Recoverability::SemiAuto)
            .message(&format!("I/O 错误: {cause_msg}"))
            .user_message("文件操作失败")
            .module_path("io")
            .operation("io_operation")
            .build();

        let mut source = std::error::Error::source(&e);
        while let Some(err) = source {
            let inner_msg = err.to_string();
            let cause_obj = Self::builder()
                .code(registry::IO_FAILED)
                .source(ErrorSource::FS)
                .severity(Severity::ERROR)
                .impact_scope(ImpactScope::OPERATION)
                .recoverability(Recoverability::SemiAuto)
                .message(&inner_msg)
                .user_message("文件操作失败")
                .module_path("io")
                .operation("io_operation")
                .build();
            obj = obj.with_cause(cause_obj);
            source = err.source();
        }

        obj
    }
}

impl From<String> for ErrorObject {
    fn from(s: String) -> Self {
        Self::builder()
            .code(registry::GENERAL_FALLBACK)
            .source(ErrorSource::INT)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::OPERATION)
            .recoverability(Recoverability::NonRecoverable)
            .message(&s)
            .user_message("操作失败，请稍后重试")
            .module_path("unknown")
            .operation("unknown")
            .build()
    }
}

impl From<&str> for ErrorObject {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}
