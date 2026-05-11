//! Error conversion implementations
//!
//! Provides `From` implementations for converting external error types into ErrorObject.

#[cfg(feature = "jsonwebtoken")]
mod auth_conversions;
#[cfg(feature = "db")]
mod db_conversions;
#[cfg(feature = "serde-json")]
mod serde_conversions;
mod std_conversions;

#[cfg(test)]
mod tests {
    use crate::classification::{ErrorSource, ImpactScope, Recoverability, Severity};
    use crate::error_code::registry;
    use crate::error_object::ErrorObject;

    #[test]
    fn test_from_io_error_preserves_context() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "权限不足");
        let err: ErrorObject = io_err.into();
        assert_eq!(err.source(), ErrorSource::FS);
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.impact_scope(), ImpactScope::OPERATION);
        assert_eq!(err.recoverability(), Recoverability::SemiAuto);
        assert_eq!(err.code(), registry::IO_FAILED);
        assert!(err.message().contains("I/O 错误"));
        assert!(err.message().contains("权限不足"));
        assert_eq!(err.user_message(), "文件操作失败");
        assert_eq!(err.module_path(), "io");
        assert_eq!(err.operation(), "io_operation");
    }

    #[test]
    fn test_from_io_error_various_kinds() {
        let kinds = vec![
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::ConnectionRefused,
            std::io::ErrorKind::BrokenPipe,
            std::io::ErrorKind::AlreadyExists,
            std::io::ErrorKind::InvalidInput,
            std::io::ErrorKind::TimedOut,
        ];
        for kind in kinds {
            let io_err = std::io::Error::new(kind, "测试错误");
            let err: ErrorObject = io_err.into();
            assert_eq!(err.code(), registry::IO_FAILED);
        }
    }

    #[test]
    fn test_from_string_preserves_message() {
        let msg = "自定义业务错误";
        let err: ErrorObject = msg.to_string().into();
        assert_eq!(err.source(), ErrorSource::INT);
        assert_eq!(err.code(), registry::GENERAL_FALLBACK);
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
        assert_eq!(err.message(), msg);
        assert_eq!(err.user_message(), "操作失败，请稍后重试");
        assert_eq!(err.module_path(), "unknown");
        assert_eq!(err.operation(), "unknown");
    }

    #[test]
    fn test_from_str_delegates_to_string() {
        let err: ErrorObject = "短文本错误".into();
        assert_eq!(err.source(), ErrorSource::INT);
        assert_eq!(err.code(), registry::GENERAL_FALLBACK);
        assert!(err.message().contains("短文本错误"));
    }

    #[cfg(feature = "serde-json")]
    #[test]
    fn test_from_serde_json_error_preserves_context() {
        let serde_err = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
        let err: ErrorObject = serde_err.into();
        assert_eq!(err.source(), ErrorSource::INT);
        assert_eq!(err.code(), registry::SERDE_FAILED);
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.impact_scope(), ImpactScope::OPERATION);
        assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
        assert!(err.message().contains("序列化/反序列化失败"));
        assert_eq!(err.user_message(), "数据处理失败，请检查输入格式");
        assert_eq!(err.module_path(), "serde");
        assert_eq!(err.operation(), "serialize");
    }

    #[cfg(feature = "serde-json")]
    #[test]
    fn test_from_serde_json_error_serialization_failure() {
        let val = serde_json::Value::Number(42i64.into());
        let serde_err = serde_json::to_string_pretty(&val).unwrap();
        let _ = serde_err;
        let serde_err = serde_json::from_str::<Vec<i32>>("not an array").unwrap_err();
        let err: ErrorObject = serde_err.into();
        assert!(err.message().contains("序列化/反序列化失败"));
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_from_surrealdb_error_preserves_context() {
        let db_err = surrealdb::Error::Db(surrealdb::error::Db::Thrown("表不存在".to_string()));
        let err: ErrorObject = db_err.into();
        assert_eq!(err.source(), ErrorSource::FS);
        assert_eq!(err.code(), registry::DB_QUERY_FAILED);
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.impact_scope(), ImpactScope::OPERATION);
        assert_eq!(err.recoverability(), Recoverability::SemiAuto);
        assert!(err.message().contains("数据库查询失败"));
        assert!(err.message().contains("表不存在"));
        assert_eq!(err.user_message(), "数据库操作失败，请稍后重试");
        assert_eq!(err.module_path(), "db");
        assert_eq!(err.operation(), "query");
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_from_surrealdb_error_various_db_errors() {
        let errors = vec![
            surrealdb::Error::Db(surrealdb::error::Db::Thrown("语法错误".to_string())),
            surrealdb::Error::Db(surrealdb::error::Db::Thrown("权限不足".to_string())),
        ];
        for db_err in errors {
            let err: ErrorObject = db_err.into();
            assert_eq!(err.code(), registry::DB_QUERY_FAILED);
            assert_eq!(err.source(), ErrorSource::FS);
        }
    }

    #[cfg(feature = "jsonwebtoken")]
    #[test]
    fn test_from_jsonwebtoken_error_preserves_context() {
        let jwt_err =
            jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidToken);
        let err: ErrorObject = jwt_err.into();
        assert_eq!(err.source(), ErrorSource::SEC);
        assert_eq!(err.code(), registry::TOKEN_INVALID);
        assert_eq!(err.severity(), Severity::WARNING);
        assert_eq!(err.impact_scope(), ImpactScope::OPERATION);
        assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
        assert!(err.message().contains("Token 验证失败"));
        assert_eq!(err.user_message(), "认证令牌无效或已过期");
        assert_eq!(err.module_path(), "auth");
        assert_eq!(err.operation(), "validate_token");
    }

    #[cfg(feature = "jsonwebtoken")]
    #[test]
    fn test_from_jsonwebtoken_error_various_kinds() {
        let kinds = vec![
            jsonwebtoken::errors::ErrorKind::InvalidToken,
            jsonwebtoken::errors::ErrorKind::InvalidSignature,
            jsonwebtoken::errors::ErrorKind::ExpiredSignature,
        ];
        for kind in kinds {
            let jwt_err = jsonwebtoken::errors::Error::from(kind);
            let err: ErrorObject = jwt_err.into();
            assert_eq!(err.source(), ErrorSource::SEC);
            assert_eq!(err.code(), registry::TOKEN_INVALID);
        }
    }

    #[test]
    fn test_error_classification_correctness() {
        let io_err: ErrorObject = std::io::Error::new(std::io::ErrorKind::NotFound, "test").into();
        assert_eq!(io_err.source(), ErrorSource::FS, "I/O 错误应归类为 FS 来源");

        let str_err: ErrorObject = "test".into();
        assert_eq!(
            str_err.source(),
            ErrorSource::INT,
            "字符串错误应归类为 INT 来源"
        );

        #[cfg(feature = "serde-json")]
        {
            let serde_err: ErrorObject = serde_json::from_str::<i32>("x").unwrap_err().into();
            assert_eq!(
                serde_err.source(),
                ErrorSource::INT,
                "Serde 错误应归类为 INT 来源"
            );
        }

        #[cfg(feature = "db")]
        {
            let db_err: ErrorObject =
                surrealdb::Error::Db(surrealdb::error::Db::Thrown("test".to_string())).into();
            assert_eq!(
                db_err.source(),
                ErrorSource::FS,
                "数据库错误应归类为 FS 来源"
            );
        }

        #[cfg(feature = "jsonwebtoken")]
        {
            let jwt_err: ErrorObject =
                jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidToken)
                    .into();
            assert_eq!(
                jwt_err.source(),
                ErrorSource::SEC,
                "JWT 错误应归类为 SEC 来源"
            );
        }
    }
}
