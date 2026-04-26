/// 审计日志系统
///
/// # 审计事件类型
/// - AUDIT-001: 系统启动
/// - AUDIT-002: 系统关闭
/// - AUDIT-010: 文件上传
/// - AUDIT-011: 文件删除
/// - AUDIT-020: 文档创建
/// - AUDIT-021: 文档更新
/// - AUDIT-022: 文档删除
/// - AUDIT-030: 查询执行
/// - AUDIT-040: 权限变更
/// - AUDIT-050: 配置修改
/// - AUDIT-060: 数据导出
///

/// 防篡改审计日志
pub mod tamper_proof;
/// 合规性报告与异常检测
pub mod compliance;

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{Write, BufWriter};
use std::path::Path;
use std::sync::Mutex;

use crate::Result;
use crate::error::helpers;

/// 审计事件类型
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum AuditEvent {
    /// 系统启动
    SystemStart,
    /// 系统关闭
    SystemShutdown,
    /// 文件上传
    FileUpload {
        /// 上传文件的路径
        file_path: String,
        /// 文件大小（字节）
        file_size: u64,
    },
    /// 文件删除
    FileDelete {
        /// 被删除文件的路径
        file_path: String,
    },
    /// 文档创建
    DocumentCreate {
        /// 文档唯一标识符
        doc_id: String,
        /// 文档标题
        title: String,
    },
    /// 文档更新
    DocumentUpdate {
        /// 文档唯一标识符
        doc_id: String,
        /// 文档标题
        title: String,
    },
    /// 文档删除
    DocumentDelete {
        /// 文档唯一标识符
        doc_id: String,
    },
    /// 查询执行
    QueryExecute {
        /// 查询类型
        query_type: String,
        /// 查询参数
        query_params: String,
    },
    /// 权限变更
    PermissionChange {
        /// 用户标识符
        user_id: String,
        /// 变更后的角色
        role: String,
    },
    /// 配置修改
    ConfigChange {
        /// 配置键名
        key: String,
        /// 配置值
        value: String,
    },
    /// 数据导出
    DataExport {
        /// 导出类型
        export_type: String,
        /// 导出记录数量
        record_count: usize,
    },
}

impl AuditEvent {
    /// 获取事件类型代码
    #[must_use]
    pub const fn code(&self) -> &str {
        match self {
            Self::SystemStart => "AUDIT-001",
            Self::SystemShutdown => "AUDIT-002",
            Self::FileUpload { .. } => "AUDIT-010",
            Self::FileDelete { .. } => "AUDIT-011",
            Self::DocumentCreate { .. } => "AUDIT-020",
            Self::DocumentUpdate { .. } => "AUDIT-021",
            Self::DocumentDelete { .. } => "AUDIT-022",
            Self::QueryExecute { .. } => "AUDIT-030",
            Self::PermissionChange { .. } => "AUDIT-040",
            Self::ConfigChange { .. } => "AUDIT-050",
            Self::DataExport { .. } => "AUDIT-060",
        }
    }

    /// 获取事件描述
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::SystemStart => "系统启动".to_string(),
            Self::SystemShutdown => "系统关闭".to_string(),
            Self::FileUpload { file_path, .. } => format!("文件上传: {file_path}"),
            Self::FileDelete { file_path } => format!("文件删除: {file_path}"),
            Self::DocumentCreate { title, .. } => format!("文档创建: {title}"),
            Self::DocumentUpdate { title, .. } => format!("文档更新: {title}"),
            Self::DocumentDelete { doc_id } => format!("文档删除: {doc_id}"),
            Self::QueryExecute { query_type, .. } => format!("查询执行: {query_type}"),
            Self::PermissionChange { user_id, role } => format!("权限变更: {user_id} -> {role}"),
            Self::ConfigChange { key, .. } => format!("配置修改: {key}"),
            Self::DataExport { export_type, .. } => format!("数据导出: {export_type}"),
        }
    }
}

/// 审计日志条目
#[derive(Debug, Serialize)]
pub struct AuditLogEntry {
    /// 事件时间戳
    timestamp: DateTime<Utc>,
    /// 事件类型代码
    event_code: String,
    /// 事件描述
    description: String,
    /// 操作者
    actor: String,
    /// 详细信息（JSON 格式）
    details: serde_json::Value,
    /// 影响范围
    scope: String,
}

/// 审计日志记录器
/// 
/// 负责记录审计事件，支持日志脱敏和持久化。
pub struct AuditLogger {
    writer: Mutex<BufWriter<File>>,
}

impl AuditLogger {
    /// 创建审计日志记录器
    pub fn new(log_path: &str) -> Result<Self> {
        let path = Path::new(log_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        let writer = Mutex::new(BufWriter::new(file));

        Ok(Self {
            writer,
        })
    }

    /// 记录审计事件
    ///
    /// # Errors
    /// 当日志写入失败或 Mutex 锁获取失败时返回错误
    pub fn log(&self, actor: &str, event: &AuditEvent, scope: &str) -> Result<()> {
        let details = self.serialize_event(event)?;
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            event_code: event.code().to_string(),
            description: event.description(),
            actor: actor.to_string(),
            details,
            scope: scope.to_string(),
        };

        let json = serde_json::to_string(&entry)?;

        writeln!(
            self.writer.lock().map_err(|_| {
                helpers::internal_error("Mutex poisoned: 审计日志写入器锁获取失败")
            })?,
            "{json}"
        )?;

        Ok(())
    }

    /// 序列化事件为 JSON
    fn serialize_event(&self, event: &AuditEvent) -> Result<serde_json::Value> {
        let value = serde_json::to_value(event)?;
        let masked = self.mask_sensitive_data(value);
        Ok(masked)
    }

    /// 脱敏敏感数据
    #[allow(clippy::match_same_arms, clippy::self_only_used_in_recursion)]
    fn mask_sensitive_data(&self, value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(mut obj) => {
                for (key, val) in &mut obj {
                    match key.as_str() {
                        "doc_id" | "user_id" => {
                            if let serde_json::Value::String(s) = val {
                                if s.len() > 8 {
                                    *val = serde_json::Value::String(format!("{}***", &s[..8]));
                                }
                            }
                        }
                        "query_params" | "value" => {
                            *val = serde_json::Value::String("[REDACTED]".to_string());
                        }
                        _ => {
                            *val = self.mask_sensitive_data(val.clone());
                        }
                    }
                }
                serde_json::Value::Object(obj)
            }
            serde_json::Value::Array(arr) => {
                let masked_arr = arr
                    .into_iter()
                    .map(|v| self.mask_sensitive_data(v))
                    .collect();
                serde_json::Value::Array(masked_arr)
            }
            _ => value,
        }
    }

    /// 关闭日志记录器
    ///
    /// # Errors
    /// 当缓冲区刷新失败或 Mutex 锁获取失败时返回错误
    pub fn close(&self) -> Result<()> {
        self.writer.lock().map_err(|_| {
            helpers::internal_error("Mutex poisoned: 审计日志写入器锁获取失败")
        })?.flush()?;
        Ok(())
    }
}

// 导出新模块的公共 API
pub use tamper_proof::{
    TamperProofAuditLog, HmacSigner, ChainValidator, AuditEntry, AuditEntryId,
    AuditEventType, Actor, Action, ResourceRef, AuditMetadata, IntegrityReport,
    FileAuditWriter, AuditWriter, ChainValidationResult,
};

pub use compliance::{
    ComplianceReporter, SOC2Report, GDPRReport, AccessPatternReport,
    DateRange, ScopeConfig, AnomalyRule, AnomalyAlert,
};

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_audit_event_code() {
        assert_eq!(AuditEvent::SystemStart.code(), "AUDIT-001");
        assert_eq!(AuditEvent::FileUpload { file_path: "test.txt".to_string(), file_size: 1024 }.code(), "AUDIT-010");
        assert_eq!(AuditEvent::QueryExecute { query_type: "search".to_string(), query_params: "test".to_string() }.code(), "AUDIT-030");
    }

    #[test]
    fn test_audit_event_description() {
        assert_eq!(AuditEvent::SystemStart.description(), "系统启动");
        assert_eq!(AuditEvent::FileUpload { file_path: "test.txt".to_string(), file_size: 1024 }.description(), "文件上传: test.txt");
    }

    #[test]
    fn test_mask_sensitive_data() {
        let logger = AuditLogger::new("./test-audit.log").unwrap();
        
        let test_value = serde_json::json! {
            {
                "doc_id": "1234567890abcdef",
                "user_id": "user1234567890",
                "query_params": "sensitive data",
                "file_path": "/path/to/file.txt",
                "file_size": 1024
            }
        };
        
        let masked = logger.mask_sensitive_data(test_value);
        
        assert_eq!(masked["doc_id"], "12345678***");
        assert_eq!(masked["user_id"], "user1234***");
        assert_eq!(masked["query_params"], "[REDACTED]");
        assert_eq!(masked["file_path"], "/path/to/file.txt");
        assert_eq!(masked["file_size"], 1024);
    }

    #[test]
    fn test_audit_logger() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("audit.log").to_str().unwrap().to_string();
        
        let logger = AuditLogger::new(&log_path).unwrap();
        
        let event = AuditEvent::FileUpload {
            file_path: "/test/file.txt".to_string(),
            file_size: 1024,
        };
        
        logger.log("admin", &event, "local").unwrap();
        logger.close().unwrap();
        
        assert!(Path::new(&log_path).exists());
    }
}
