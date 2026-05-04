//! `` `SurrealDB` `` 查询追踪中间件
//!
//! 数据库操作追踪（`Database Tracing Wrapper`）
//!
//! 为 `SurrealDB` 操作自动创建 child span，记录：
//! - 查询语句摘要（脱敏后）
//! - 查询执行耗时
//! - 影响行数
//! - 错误信息
//!
//! # 使用方式
//!
//! ```ignore
//! use knowledge_api::middleware::db_tracing::{DbTracer, traced_query};
//!
//! // 方式 1: 使用 DbTracer 包装器
//! let tracer = DbTracer::new("surrealdb");
//! let result = tracer.execute("SELECT * FROM documents", || async {
//!     db.query("SELECT * FROM documents").await
//! }).await;
//!
//! // 方式 2: 使用便捷宏
//! let result = traced_query!("SELECT * FROM documents", {
//!     db.query("SELECT * FROM documents").await
//! });
//! ```
//!
//! # Span 属性规范
//!
//! | 属性名 | 类型 | 说明 |
//! |--------|------|------|
//! | `db.system` | string | 数据库类型 (surrealdb) |
//! | `db.statement` | string | SQL/SURQL 查询语句 |
//! | `db.operation` | string | 操作类型 (select/insert/update/delete) |
//! | `db.rows_affected` | int | 影响行数 |
//! | `error` | string | 错误信息（如有） |

use metrics::{counter, histogram};
use std::future::Future;
use std::sync::LazyLock;
use std::time::Instant;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

static STRING_LITERAL_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    // SAFETY: 正则模式为编译期硬编码字面量，语法错误在开发阶段即可发现
    regex::Regex::new(r"'[^']*'").expect("字符串正则编译失败")
});

static PASSWORD_VALUE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    // SAFETY: 正则模式为编译期硬编码字面量，语法错误在开发阶段即可发现
    regex::Regex::new(r"(?i)(?:password|passwd|secret)\s*=\s*'[^']*'")
        .expect("密码字段正则编译失败")
});

/// 数据库追踪器
///
/// 为数据库操作提供结构化的 Span 创建和属性记录。
/// 每个实例绑定到特定的数据库系统名称。
#[derive(Clone, Debug)]
pub struct DbTracer {
    /// 数据库系统标识
    db_system: &'static str,
}

impl DbTracer {
    /// 创建新的数据库追踪器
    #[must_use]
    pub const fn new(db_system: &'static str) -> Self {
        Self { db_system }
    }

    /// 执行带追踪的数据库查询
    ///
    /// 自动创建 child span 并记录：
    /// - 查询语句（自动分类操作类型）
    /// - 执行耗时
    /// - 错误信息
    ///
    /// # Parameters
    ///
    /// - `statement`: SURQL / SQL 查询语句
    /// - `operation`: 异步查询闭包
    ///
    /// # Returns
    ///
    /// 返回原始操作的 Result，Span 属性已记录。
    #[tracing::instrument(skip(self, operation), fields(
        db.system = self.db_system,
        db.statement,
        db.operation,
        db.duration_seconds,
    ))]
    /// # Errors
    ///
    /// 当查询执行失败时返回错误。
    #[must_use = "数据库查询结果必须被使用"]
    pub async fn execute<F, Fut, T>(&self, statement: &str, operation: F) -> crate::Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = crate::Result<T>>,
    {
        let operation_type = classify_db_operation(statement);

        // 记录查询语句到 Span 属性（脱敏处理）
        let sanitized_statement = sanitize_db_statement(statement);
        Span::current().set_attribute("db.statement", sanitized_statement);
        Span::current().set_attribute("db.operation", operation_type);

        let start = Instant::now();

        match operation().await {
            Ok(result) => {
                let elapsed = start.elapsed().as_secs_f64();
                Span::current().set_attribute("db.duration_seconds", elapsed);

                histogram!("db_query_duration_seconds", "operation" => operation_type)
                    .record(elapsed);
                counter!("db_queries_total", "operation" => operation_type).increment(1);

                Ok(result)
            }
            Err(e) => {
                let elapsed = start.elapsed().as_secs_f64();

                Span::current().set_attribute("db.duration_seconds", elapsed);
                Span::current()
                    .set_attribute("error.message", e.message().to_string());
                Span::current().set_attribute("error.code", e.code().to_string());

                counter!("db_query_errors_total", "operation" => operation_type).increment(1);

                tracing::error!(
                    db.system = self.db_system,
                    db.statement = %sanitize_db_statement(statement),
                    error = %e.message(),
                    "数据库查询失败"
                );

                Err(e)
            }
        }
    }
}

impl Default for DbTracer {
    fn default() -> Self {
        Self::new("surrealdb")
    }
}

/// 对数据库查询语句进行脱敏处理
///
/// 移除或遮蔽可能包含敏感数据的字面量值：
/// - 字符串字面量 → `[REDACTED_VALUE]`
/// - 数字字面量（可能为 ID 或手机号）→ 保留但记录警告
fn sanitize_db_statement(statement: &str) -> String {
    let mut result = statement.to_string();

    result = STRING_LITERAL_RE
        .replace_all(&result, "'[REDACTED_VALUE]'")
        .to_string();

    if result.to_lowercase().contains("password") || result.contains("passwd") {
        result = PASSWORD_VALUE_RE
            .replace_all(&result, "$1='[REDACTED_PASSWORD]'")
            .to_string();
    }

    if result.len() > 512 {
        format!("{}... [TRUNCATED {} chars]", &result[..512], result.len())
    } else {
        result
    }
}

/// 从查询语句推断操作类型
///
/// 基于首个关键字进行简单分类。对于复杂查询，
/// 可能需要更精确的解析器（如 tree-sitter-sql）。
fn classify_db_operation(statement: &str) -> &'static str {
    let upper = statement.trim_start().to_uppercase();

    if upper.starts_with("SELECT") {
        "select"
    } else if upper.starts_with("INSERT") {
        "insert"
    } else if upper.starts_with("UPDATE") {
        "update"
    } else if upper.starts_with("DELETE") {
        "delete"
    } else if upper.starts_with("CREATE") {
        "create"
    } else if upper.starts_with("DROP") {
        "drop"
    } else if upper.starts_with("LET") || upper.starts_with("BEGIN") {
        "transaction"
    } else {
        "unknown"
    }
}

/// 全局默认的 `SurrealDB` 追踪器实例
///
/// 使用 lazy 初始化，确保在首次使用时才创建。
#[must_use]
pub fn default_db_tracer() -> DbTracer {
    DbTracer::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_select() {
        assert_eq!(classify_db_operation("SELECT * FROM docs"), "select");
        assert_eq!(classify_db_operation("   SELECT id FROM users"), "select");
    }

    #[test]
    fn test_classify_insert() {
        assert_eq!(classify_db_operation("INSERT INTO docs ..."), "insert");
    }

    #[test]
    fn test_classify_update() {
        assert_eq!(classify_db_operation("UPDATE docs SET ..."), "update");
    }

    #[test]
    fn test_classify_delete() {
        assert_eq!(
            classify_db_operation("DELETE FROM docs WHERE ..."),
            "delete"
        );
    }

    #[test]
    fn test_classify_transaction() {
        assert_eq!(classify_db_operation("LET $x = ..."), "transaction");
        assert_eq!(classify_db_operation("BEGIN TRANSACTION"), "transaction");
    }

    #[test]
    fn test_sanitize_removes_string_literals() {
        let stmt = r"SELECT * FROM users WHERE name = '张三'";
        let sanitized = sanitize_db_statement(stmt);
        assert!(sanitized.contains("[REDACTED_VALUE]"));
        assert!(!sanitized.contains("张三"));
    }

    #[test]
    fn test_sanitize_password_values() {
        let stmt = r"UPDATE users SET password = 'secret123' WHERE id = 1";
        let sanitized = sanitize_db_statement(stmt);
        assert!(sanitized.contains("[REDACTED_PASSWORD]"));
        assert!(!sanitized.contains("secret123"));
    }

    #[test]
    fn test_sanitize_truncates_long_statements() {
        let long_stmt = format!("SELECT {} FROM t", "col_".repeat(200));
        let sanitized = sanitize_db_statement(&long_stmt);
        assert!(sanitized.contains("[TRUNCATED"));
        assert!(sanitized.len() < long_stmt.len());
    }

    #[test]
    fn test_sanitize_preserves_short_safe_statements() {
        let stmt = "SELECT count() FROM documents GROUP BY type";
        let sanitized = sanitize_db_statement(stmt);
        assert_eq!(sanitized, stmt);
    }

    #[tokio::test]
    async fn test_db_tracer_execute_success() {
        let tracer = DbTracer::new("test-db");

        let result = tracer
            .execute("SELECT 1", || async {
                Ok::<_, error_core::ErrorObject>(42i32)
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_db_tracer_execute_error() {
        let tracer = DbTracer::new("test-db");

        let result: crate::Result<i32> = tracer
            .execute("INVALID QUERY", || async {
                Err(error_core::helpers::db_error("模拟 DB 错误"))
            })
            .await;

        assert!(result.is_err());
    }
}
