/// 知识图谱系统统一错误类型
///
/// 所有错误构造均委托至 `error_core::helpers` 模块。
/// 本模块仅提供向后兼容的重导出。
pub use error_core::helpers;

use error_core::ErrorObject;

#[cfg(feature = "db")]
use crate::cqrs::AggregateError;
#[cfg(feature = "db")]
use crate::cache::l2_cache::L2CacheError;
#[cfg(feature = "db")]
use crate::cache::cache_manager::CacheManagerError;
#[cfg(feature = "event-driven")]
use crate::event::event_handler::HandleError;
#[cfg(feature = "event-driven")]
use crate::event::event_bus::EventError;

#[cfg(feature = "db")]
impl From<AggregateError> for ErrorObject {
    fn from(err: AggregateError) -> Self {
        match err {
            AggregateError::InvalidState(msg) => helpers::aggregate_invalid_state(&msg),
            AggregateError::BusinessRuleViolation(msg) => helpers::aggregate_business_rule(&msg),
            AggregateError::NotFound(msg) => helpers::aggregate_not_found(&msg),
            AggregateError::VersionConflict { expected, actual } => helpers::aggregate_version_conflict(expected, actual),
            AggregateError::SerializationError(msg) => helpers::aggregate_serialization_error(&msg),
        }
    }
}

#[cfg(feature = "db")]
impl From<L2CacheError> for ErrorObject {
    fn from(err: L2CacheError) -> Self {
        match err {
            L2CacheError::Connection(msg) => helpers::cache_l2_connection_error(&msg),
            L2CacheError::Serialization(msg) => helpers::cache_l2_serialization_error(&msg),
            L2CacheError::Deserialization(msg) => helpers::cache_l2_deserialization_error(&msg),
            L2CacheError::Operation(msg) => helpers::cache_l2_operation_error(&msg),
        }
    }
}

#[cfg(feature = "db")]
impl From<CacheManagerError> for ErrorObject {
    fn from(err: CacheManagerError) -> Self {
        match err {
            CacheManagerError::L2Error(msg) => helpers::cache_manager_l2_error(&msg),
            CacheManagerError::SerializationError(msg) => helpers::cache_manager_serialization_error(&msg),
        }
    }
}

#[cfg(feature = "event-driven")]
impl From<HandleError> for ErrorObject {
    fn from(err: HandleError) -> Self {
        match err {
            HandleError::TypeMismatch => helpers::event_type_mismatch(),
            HandleError::ProcessingFailed(msg) => helpers::event_processing_failed(&msg),
            HandleError::Other(msg) => helpers::event_processing_failed(&msg),
        }
    }
}

#[cfg(feature = "event-driven")]
impl From<EventError> for ErrorObject {
    fn from(err: EventError) -> Self {
        match err {
            EventError::NoSubscribers { event_id } => helpers::event_no_subscribers(&event_id),
            EventError::Shutdown => helpers::event_bus_shutdown(),
            EventError::PublishTimeout { elapsed_ms } => helpers::event_publish_timeout(elapsed_ms),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::helpers;
    use error_core::classification::{ErrorSource, ImpactScope, Severity};

    #[test]
    fn test_helpers_not_found_constructible() {
        let err = helpers::not_found("document", "doc:123");
        assert_eq!(err.source(), ErrorSource::USR);
        assert!(err.message().contains("资源不存在"));
        assert!(err.message().contains("document"));
        assert!(err.message().contains("doc:123"));
    }

    #[test]
    fn test_helpers_db_error_constructible() {
        let err = helpers::db_error("连接超时");
        assert_eq!(err.source(), ErrorSource::FS);
        assert!(err.message().contains("连接超时"));
    }

    #[test]
    fn test_helpers_parse_error_constructible() {
        let err = helpers::parse_error("JSON 格式错误");
        assert_eq!(err.source(), ErrorSource::FS);
        assert!(err.message().contains("JSON 格式错误"));
    }

    #[test]
    fn test_helpers_validation_error_constructible() {
        let err = helpers::validation_error("字段不能为空", "submit");
        assert_eq!(err.source(), ErrorSource::USR);
        assert!(err.message().contains("字段不能为空"));
        assert_eq!(err.operation(), "submit");
    }

    #[test]
    fn test_helpers_internal_error_constructible() {
        let err = helpers::internal_error("空指针引用");
        assert_eq!(err.source(), ErrorSource::INT);
        assert_eq!(err.severity(), Severity::CRITICAL);
        assert_eq!(err.impact_scope(), ImpactScope::GLOBAL);
        assert!(err.message().contains("空指针引用"));
    }

    #[test]
    fn test_helpers_auth_error_constructible() {
        let err = helpers::auth_error("令牌过期", "login");
        assert_eq!(err.source(), ErrorSource::SEC);
        assert!(err.message().contains("令牌过期"));
    }

    #[test]
    fn test_helpers_config_error_constructible() {
        let err = helpers::config_error("端口缺失");
        assert_eq!(err.source(), ErrorSource::CFG);
        assert!(err.message().contains("端口缺失"));
    }

    #[test]
    fn test_helpers_crypto_error_constructible() {
        let err = helpers::crypto_error("AES 解密失败");
        assert_eq!(err.source(), ErrorSource::SEC);
        assert_eq!(err.severity(), Severity::CRITICAL);
        assert!(err.message().contains("AES 解密失败"));
    }

    #[test]
    fn test_helpers_unsupported_format_constructible() {
        let err = helpers::unsupported_format("xlsx");
        assert_eq!(err.source(), ErrorSource::USR);
        assert!(err.message().contains("xlsx"));
    }

    #[test]
    fn test_helpers_connection_lost_constructible() {
        let err = helpers::connection_lost();
        assert_eq!(err.source(), ErrorSource::NET);
        assert!(err.message().contains("网络连接中断"));
    }

    #[test]
    fn test_error_display_meaningful() {
        let err = helpers::not_found("block", "block:abc");
        let display = format!("{err}");
        assert!(display.contains("ERR-"), "Display 应包含错误码，实际: {display}");
        assert!(display.contains("资源不存在"), "Display 应包含错误消息，实际: {display}");
    }

    #[test]
    fn test_from_io_error_works() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
        let err: error_core::ErrorObject = io_err.into();
        assert_eq!(err.source(), ErrorSource::FS);
        assert!(err.message().contains("I/O 错误"));
    }

    #[test]
    fn test_from_serde_json_error_works() {
        let serde_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let err: error_core::ErrorObject = serde_err.into();
        assert_eq!(err.source(), ErrorSource::INT);
        assert!(err.message().contains("序列化/反序列化失败"));
    }

    #[test]
    fn test_from_string_works() {
        let err: error_core::ErrorObject = "自定义错误".to_string().into();
        assert_eq!(err.source(), ErrorSource::INT);
        assert!(err.message().contains("自定义错误"));
    }

    #[test]
    fn test_from_str_works() {
        let err: error_core::ErrorObject = "字符串错误".into();
        assert_eq!(err.source(), ErrorSource::INT);
        assert!(err.message().contains("字符串错误"));
    }
}
