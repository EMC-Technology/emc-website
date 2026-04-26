/// Schema 版本管理器（DDL 执行 + 版本追踪 + 迁移编排）
///
/// 负责数据库 Schema 的初始化、版本记录和迁移执行。
/// 所有 DDL 语句按顺序执行，慢操作（>5s）会记录审计日志。

use std::sync::Arc;
use std::time::Instant;

use crate::database::DatabaseClient;
use crate::Result;
use crate::error::helpers;

/// 当前数据库 Schema 版本
pub const SCHEMA_VERSION: &str = "1.0.0";

const SLOW_OPERATION_THRESHOLD_MS: u64 = 5000;

/// Schema 版本管理器，负责数据库 Schema 的初始化、版本记录和迁移执行
pub struct SchemaManager<D: DatabaseClient> {
    client: Arc<D>,
}

impl<D: DatabaseClient> SchemaManager<D> {
    /// 创建 SchemaManager 实例
    ///
    /// # 参数
    ///
    /// * `client` - 数据库客户端的 `Arc` 引用
    pub const fn new(client: Arc<D>) -> Self { Self { client } }

    /// 执行数据库迁移
    pub async fn migrate(&self) -> Result<()> { self.initialize().await }

    /// 初始化数据库 Schema
    pub async fn initialize(&self) -> Result<()> {
        let ddl = schema_ddl();
        let statements = parse_statements(&ddl);
        for stmt in &statements {
            let start = Instant::now();
            self.client.query(stmt, serde_json::json!({})).await.map_err(|e| helpers::db_error(&e.to_string()))?;
            #[allow(clippy::cast_possible_truncation)]
            log_slow_op(stmt, start.elapsed().as_millis() as u64);
        }
        self.record_version().await?;
        Ok(())
    }

    /// 获取当前数据库 Schema 版本
    pub async fn get_version(&self) -> Result<String> {
        let results = self.client.query("SELECT version FROM schema_version ORDER BY applied_at DESC LIMIT 1", serde_json::json!({})).await.map_err(|e| helpers::db_error(&e.to_string()))?;
        if results.is_empty() { return Err(helpers::not_found("schema_version", "latest")); }
        if let Some(surrealdb::sql::Value::Object(obj)) = results.first() {
            if let Some(v) = obj.get("version") { return Ok(v.to_string().trim_matches(|c| c == '"' || c == '\'').to_string()); }
        }
        Err(helpers::internal_error("无法解析 schema_version 记录"))
    }

    async fn record_version(&self) -> Result<()> {
        self.client.create("schema_version", serde_json::json!({"version": SCHEMA_VERSION})).await.map_err(|e| helpers::db_error(&e.to_string()))?;
        Ok(())
    }
}

/// 获取当前 Schema 的 DDL 语句集合
///
/// 包含所有表定义、字段定义和索引定义。
/// DDL 语句按依赖顺序排列，确保先创建被引用的表。
#[must_use]
pub fn schema_ddl() -> String {
    vec![
        "DEFINE TABLE document SCHEMAFULL;",
        "DEFINE FIELD path ON document TYPE string;",
        "DEFINE FIELD hash ON document TYPE string;",
        "DEFINE FIELD source_type ON document TYPE string;",
        "DEFINE FIELD file_size ON document TYPE number;",
        "DEFINE FIELD created_at ON document TYPE datetime DEFAULT time::now();",
        "DEFINE TABLE block SCHEMAFULL;",
        "DEFINE FIELD document_id ON block TYPE record;",
        "DEFINE FIELD block_type ON block TYPE string;",
        "DEFINE FIELD start_line ON block TYPE number;",
        "DEFINE FIELD end_line ON block TYPE number;",
        "DEFINE FIELD content_hash ON block TYPE string;",
        "DEFINE FIELD embedding ON block TYPE array;",
        "DEFINE FIELD created_at ON block TYPE datetime DEFAULT time::now();",
        "DEFINE TABLE token SCHEMAFULL;",
        "DEFINE FIELD block_id ON token TYPE record;",
        "DEFINE FIELD content ON token TYPE string;",
        "DEFINE FIELD token_type ON token TYPE string;",
        "DEFINE FIELD start_char ON token TYPE number;",
        "DEFINE FIELD global_offset ON token TYPE number;",
        "DEFINE TABLE reference SCHEMAFULL;",
        "DEFINE FIELD source_id ON reference TYPE record<document,block,token>;",
        "DEFINE FIELD target_id ON reference TYPE record<document,block,token>;",
        "DEFINE FIELD ref_type ON reference TYPE string;",
        "DEFINE FIELD created_at ON reference TYPE datetime DEFAULT time::now();",
        "DEFINE TABLE idempotency SCHEMAFULL;",
        "DEFINE FIELD key ON idempotency TYPE string;",
        "DEFINE FIELD record_id ON idempotency TYPE string;",
        "DEFINE FIELD created_at ON idempotency TYPE datetime DEFAULT time::now();",
        "DEFINE TABLE schema_version SCHEMAFULL;",
        "DEFINE FIELD version ON schema_version TYPE string;",
        "DEFINE FIELD applied_at ON schema_version TYPE datetime DEFAULT time::now();",
        "DEFINE INDEX idx_document_hash ON document COLUMNS hash UNIQUE;",
        "DEFINE INDEX idx_block_document ON block COLUMNS document_id;",
        "DEFINE INDEX idx_token_block ON token COLUMNS block_id;",
        "DEFINE INDEX idx_token_global_offset ON token COLUMNS global_offset UNIQUE;",
        "DEFINE INDEX idx_reference_source ON reference COLUMNS source_id;",
        "DEFINE INDEX idx_reference_target ON reference COLUMNS target_id;",
        "DEFINE INDEX idx_idempotency_key ON idempotency COLUMNS key UNIQUE;",
    ].join("\n")
}

/// 将 DDL 文本解析为独立的 SQL 语句列表
///
/// 过滤空行和注释，按分号分割为可独立执行的语句。
#[must_use]
pub fn parse_statements(ddl: &str) -> Vec<String> {
    ddl.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("--")).collect::<Vec<_>>().join("\n").split(';').map(str::trim).filter(|s| !s.is_empty()).map(|s| format!("{s};")).collect()
}

fn log_slow_op(statement: &str, elapsed_ms: u64) {
    if elapsed_ms > SLOW_OPERATION_THRESHOLD_MS {
        tracing::warn!(elapsed_ms, threshold_ms = SLOW_OPERATION_THRESHOLD_MS, stmt = &statement[..statement.len().min(80)], "Slow Schema operation [AUDIT-030]");
    } else {
        tracing::debug!(elapsed_ms, stmt = &statement[..statement.len().min(80)], "Schema op done");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::MockDbClient;
    use serde_json::json;

    #[test] fn test_new_creates_instance() { let _m = SchemaManager::new(Arc::new(MockDbClient::new())); }
    #[test] fn test_schema_ddl_not_empty() { let d = schema_ddl(); assert!(!d.is_empty()); assert!(d.contains("DEFINE TABLE")); }
    #[test] fn test_parse_splits() { assert_eq!(parse_statements("A; B; C;").len(), 3); }

    #[tokio::test]
    async fn test_get_version_ok() {
        let m = MockDbClient::new();
        m.register_response("SELECT version FROM schema_version ORDER BY applied_at DESC LIMIT 1", json!([{"version": SCHEMA_VERSION}]));
        assert_eq!(SchemaManager::new(Arc::new(m)).get_version().await.unwrap(), SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn test_get_version_empty_err() {
        let m = MockDbClient::new();
        m.register_response("SELECT version FROM schema_version ORDER BY applied_at DESC LIMIT 1", json!([]));
        assert!(SchemaManager::new(Arc::new(m)).get_version().await.is_err());
    }
}
