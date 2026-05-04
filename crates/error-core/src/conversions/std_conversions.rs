//! Standard library error conversions

use crate::error_object::ErrorObject;
use crate::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
use crate::error_code::registry;

impl From<std::io::Error> for ErrorObject {
    fn from(e: std::io::Error) -> Self {
        let cause_msg = e.to_string();
        let mut source_chain: Vec<String> = Vec::new();
        let mut source = std::error::Error::source(&e);
        while let Some(err) = source {
            source_chain.push(err.to_string());
            source = err.source();
        }

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

        // NOTE: cause chain 被扁平化为单个 ErrorObject（用 → 连接），
        // 而非嵌套的 cause.cause.cause 结构。这是为了简化错误展示，
        // 但丢失了因果链的层级关系。未来可改为递归嵌套。
        if !source_chain.is_empty() {
            let cause_obj = Self::builder()
                .code(registry::IO_FAILED)
                .source(ErrorSource::FS)
                .severity(Severity::ERROR)
                .impact_scope(ImpactScope::OPERATION)
                .recoverability(Recoverability::SemiAuto)
                .message(&source_chain.join(" → "))
                .user_message("文件操作失败")
                .module_path("io")
                .operation("io_operation")
                .build();
            obj.set_cause(cause_obj);
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
