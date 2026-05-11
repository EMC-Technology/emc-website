/// 合规性报告与异常检测
pub mod compliance;
/// 防篡改审计日志
pub mod tamper_proof;
/// 审计日志系统
///
/// # 审计事件编码体系
///
/// 本模块使用 `AUDIT-XXX` 编码体系（业务层审计事件），
/// 与 `tamper_proof::AuditEventType` 的 `SYS-XXX`/`USR-XXX`/`DAT-XXX` 编码体系
/// （基础设施层审计事件）互为补充：
///
/// - `AuditEvent`（本模块）：面向领域模型，关注业务语义
/// - `AuditEventType`（tamper_proof）：面向合规审计，关注防篡改和细粒度追踪
///
/// 两者通过 `AuditEvent::to_tamper_proof_type()` 方法建立映射关系。
///
/// # 事件码分配
///
/// | 事件码 | 语义 | 范围 |
/// |--------|------|------|
/// | AUDIT-001 | 用户登录 | 用户认证 |
/// | AUDIT-002 | 用户登出 | 用户认证 |
/// | AUDIT-010 | 文件上传 | 文件操作 |
/// | AUDIT-011 | 文件删除 | 文件操作 |
/// | AUDIT-020 | 解析开始 | 解析流水线 |
/// | AUDIT-021 | 解析完成 | 解析流水线 |
/// | AUDIT-022 | 解析失败 | 解析流水线 |
/// | AUDIT-030 | 查询执行 | 查询操作 |
/// | AUDIT-040 | 数据导出 | 数据管理 |
/// | AUDIT-050 | 配置修改 | 系统管理 |
/// | AUDIT-060 | 数据删除 | 数据管理 |
/// | AUDIT-070 | 系统启动 | 系统生命周期 |
/// | AUDIT-071 | 系统关闭 | 系统生命周期 |
/// | AUDIT-080 | 文档创建 | 文档操作 |
/// | AUDIT-081 | 文档更新 | 文档操作 |
/// | AUDIT-082 | 文档删除 | 文档操作 |
/// | AUDIT-090 | 权限变更 | 权限管理 |
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;

use crate::Result;
use crate::error::helpers;

/// 审计事件触发源
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum AuditTriggeredBy {
    /// 系统自动触发
    System,
    /// 用户操作触发
    User(String),
    /// 服务账户触发
    ServiceAccount(String),
    /// API 密钥触发
    ApiKey(String),
    /// 定时任务触发
    ScheduledJob(String),
}

/// 查询类型枚举
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum QueryType {
    /// 全文搜索
    FullTextSearch,
    /// 向量相似度搜索
    VectorSearch,
    /// 图遍历查询
    GraphTraversal,
    /// 聚合统计
    Aggregation,
    /// 自定义查询
    Custom(String),
}

/// 导出类型枚举
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum ExportType {
    /// CSV 格式导出
    Csv,
    /// JSON 格式导出
    Json,
    /// PDF 报告导出
    Pdf,
    /// 自定义格式导出
    Custom(String),
}

/// 审计角色枚举
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum AuditRole {
    /// 系统管理员
    Admin,
    /// 编辑者
    Editor,
    /// 查看者
    Viewer,
    /// 审计员
    Auditor,
    /// 自定义角色
    Custom(String),
}

impl std::fmt::Display for AuditRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => write!(f, "Admin"),
            Self::Editor => write!(f, "Editor"),
            Self::Viewer => write!(f, "Viewer"),
            Self::Auditor => write!(f, "Auditor"),
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

/// 审计影响范围枚举
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum AuditScope {
    /// 本地范围
    Local,
    /// 租户范围
    Tenant(String),
    /// 全局范围
    Global,
}

impl std::fmt::Display for AuditScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Tenant(t) => write!(f, "tenant:{t}"),
            Self::Global => write!(f, "global"),
        }
    }
}

/// 审计事件类型
///
/// 事件码分配遵循 Spec 第13章 13.6 节定义：
///
/// | 事件码 | 语义 | 范围 |
/// |--------|------|------|
/// | AUDIT-001 | 用户登录 | 用户认证 |
/// | AUDIT-002 | 用户登出 | 用户认证 |
/// | AUDIT-010 | 文件上传 | 文件操作 |
/// | AUDIT-011 | 文件删除 | 文件操作 |
/// | AUDIT-020 | 解析开始 | 解析流水线 |
/// | AUDIT-021 | 解析完成 | 解析流水线 |
/// | AUDIT-022 | 解析失败 | 解析流水线 |
/// | AUDIT-030 | 查询执行 | 查询操作 |
/// | AUDIT-040 | 数据导出 | 数据管理 |
/// | AUDIT-050 | 配置修改 | 系统管理 |
/// | AUDIT-060 | 数据删除 | 数据管理 |
/// | AUDIT-070 | 系统启动 | 系统生命周期 |
/// | AUDIT-071 | 系统关闭 | 系统生命周期 |
/// | AUDIT-080 | 文档创建 | 文档操作 |
/// | AUDIT-081 | 文档更新 | 文档操作 |
/// | AUDIT-082 | 文档删除 | 文档操作 |
/// | AUDIT-090 | 权限变更 | 权限管理 |
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum AuditEvent {
    /// 用户登录
    UserLogin {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 用户标识符
        user_id: String,
    },
    /// 用户登出
    UserLogout {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 用户标识符
        user_id: String,
    },
    /// 文件上传
    FileUpload {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 上传文件的路径
        file_path: String,
        /// 文件大小（字节）
        file_size: u64,
    },
    /// 文件删除
    FileDelete {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 被删除文件的路径
        file_path: String,
    },
    /// 解析开始
    ParseStart {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 文档唯一标识符
        doc_id: String,
        /// 文件路径
        file_path: String,
    },
    /// 解析完成
    ParseComplete {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 文档唯一标识符
        doc_id: String,
        /// 解析耗时（毫秒）
        duration_ms: u64,
    },
    /// 解析失败
    ParseFailed {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 文档唯一标识符
        doc_id: String,
        /// 失败原因
        reason: String,
    },
    /// 查询执行
    QueryExecute {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 查询类型
        query_type: QueryType,
        /// 查询参数
        query_params: String,
    },
    /// 数据导出
    DataExport {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 导出类型
        export_type: ExportType,
        /// 导出记录数量
        record_count: usize,
    },
    /// 配置修改
    ConfigChange {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 配置键名
        key: String,
        /// 配置值
        value: String,
    },
    /// 数据删除
    DataDelete {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 被删除数据的标识符
        data_id: String,
        /// 数据类型
        data_type: String,
    },
    /// 系统启动
    SystemStart {
        /// 触发源
        triggered_by: AuditTriggeredBy,
    },
    /// 系统关闭
    SystemShutdown {
        /// 触发源
        triggered_by: AuditTriggeredBy,
    },
    /// 文档创建
    DocumentCreate {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 文档唯一标识符
        doc_id: String,
        /// 文档标题
        title: String,
    },
    /// 文档更新
    DocumentUpdate {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 文档唯一标识符
        doc_id: String,
        /// 文档标题
        title: String,
    },
    /// 文档删除
    DocumentDelete {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 文档唯一标识符
        doc_id: String,
    },
    /// 权限变更
    PermissionChange {
        /// 触发源
        triggered_by: AuditTriggeredBy,
        /// 用户标识符
        user_id: String,
        /// 变更后的角色
        role: AuditRole,
    },
}

impl AuditEvent {
    /// 获取事件类型代码
    #[must_use]
    pub const fn code(&self) -> &str {
        match self {
            Self::UserLogin { .. } => "AUDIT-001",
            Self::UserLogout { .. } => "AUDIT-002",
            Self::FileUpload { .. } => "AUDIT-010",
            Self::FileDelete { .. } => "AUDIT-011",
            Self::ParseStart { .. } => "AUDIT-020",
            Self::ParseComplete { .. } => "AUDIT-021",
            Self::ParseFailed { .. } => "AUDIT-022",
            Self::QueryExecute { .. } => "AUDIT-030",
            Self::DataExport { .. } => "AUDIT-040",
            Self::ConfigChange { .. } => "AUDIT-050",
            Self::DataDelete { .. } => "AUDIT-060",
            Self::SystemStart { .. } => "AUDIT-070",
            Self::SystemShutdown { .. } => "AUDIT-071",
            Self::DocumentCreate { .. } => "AUDIT-080",
            Self::DocumentUpdate { .. } => "AUDIT-081",
            Self::DocumentDelete { .. } => "AUDIT-082",
            Self::PermissionChange { .. } => "AUDIT-090",
        }
    }

    /// 获取事件描述
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::UserLogin { user_id, .. } => format!("用户登录: {user_id}"),
            Self::UserLogout { user_id, .. } => format!("用户登出: {user_id}"),
            Self::FileUpload { file_path, .. } => format!("文件上传: {file_path}"),
            Self::FileDelete { file_path, .. } => format!("文件删除: {file_path}"),
            Self::ParseStart { doc_id, .. } => format!("解析开始: {doc_id}"),
            Self::ParseComplete {
                doc_id,
                duration_ms,
                ..
            } => format!("解析完成: {doc_id} ({duration_ms}ms)"),
            Self::ParseFailed { doc_id, reason, .. } => {
                format!("解析失败: {doc_id} — {reason}")
            }
            Self::QueryExecute { query_type, .. } => format!("查询执行: {query_type:?}"),
            Self::DataExport {
                export_type,
                record_count,
                ..
            } => format!("数据导出: {export_type:?} ({record_count} 条)"),
            Self::ConfigChange { key, .. } => format!("配置修改: {key}"),
            Self::DataDelete {
                data_id, data_type, ..
            } => format!("数据删除: {data_type}/{data_id}"),
            Self::SystemStart { .. } => "系统启动".to_string(),
            Self::SystemShutdown { .. } => "系统关闭".to_string(),
            Self::DocumentCreate { title, .. } => format!("文档创建: {title}"),
            Self::DocumentUpdate { title, .. } => format!("文档更新: {title}"),
            Self::DocumentDelete { doc_id, .. } => format!("文档删除: {doc_id}"),
            Self::PermissionChange { user_id, role, .. } => {
                format!("权限变更: {user_id} -> {role}")
            }
        }
    }

    /// 将业务层审计事件映射为基础设施层防篡改审计事件类型
    ///
    /// 建立两套审计编码体系的桥接关系：
    /// - `AuditEvent`（`AUDIT-XXX`）→ `AuditEventType`（`SYS-XXX`/`USR-XXX`/`DAT-XXX`）
    #[must_use]
    pub fn to_tamper_proof_type(&self) -> tamper_proof::AuditEventType {
        use tamper_proof::AuditEventType;
        match self {
            Self::UserLogin { .. } => AuditEventType::UserLogin,
            Self::UserLogout { .. } => AuditEventType::UserLogout,
            Self::FileUpload { .. } | Self::ParseStart { .. } | Self::DocumentCreate { .. } => {
                AuditEventType::DataCreated
            }
            Self::FileDelete { .. } | Self::DataDelete { .. } | Self::DocumentDelete { .. } => {
                AuditEventType::DataDeleted
            }
            Self::ParseComplete { .. } | Self::DocumentUpdate { .. } => AuditEventType::DataUpdated,
            Self::ParseFailed { .. } => AuditEventType::SystemError,
            Self::QueryExecute { .. } => AuditEventType::DataRead,
            Self::DataExport { .. } => AuditEventType::DataExported,
            Self::ConfigChange { .. } => AuditEventType::ConfigChanged,
            Self::SystemStart { .. } => AuditEventType::SystemStart,
            Self::SystemShutdown { .. } => AuditEventType::SystemShutdown,
            Self::PermissionChange { .. } => AuditEventType::PermissionGranted,
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
    scope: AuditScope,
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

        Ok(Self { writer })
    }

    /// 记录审计事件
    ///
    /// # Errors
    /// 当日志写入失败或 Mutex 锁获取失败时返回错误
    pub fn log(&self, actor: &str, event: &AuditEvent, scope: &AuditScope) -> Result<()> {
        let details = serialize_event(event)?;
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            event_code: event.code().to_string(),
            description: event.description(),
            actor: actor.to_string(),
            details,
            scope: scope.clone(),
        };

        let json = serde_json::to_vec(&entry)?;

        self.writer
            .lock()
            .map_err(|_| helpers::io_error("Mutex poisoned: 审计日志写入器锁获取失败"))?
            .write_all(&json)?;

        Ok(())
    }

    /// 关闭日志记录器
    ///
    /// # Errors
    /// 当缓冲区刷新失败或 Mutex 锁获取失败时返回错误
    pub fn close(&self) -> Result<()> {
        self.writer
            .lock()
            .map_err(|_| helpers::io_error("Mutex poisoned: 审计日志写入器锁获取失败"))?
            .flush()?;
        Ok(())
    }
}

fn serialize_event(event: &AuditEvent) -> Result<serde_json::Value> {
    let value = serde_json::to_value(event)?;
    Ok(mask_sensitive_data(value))
}

fn mask_sensitive_data(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(mut obj) => {
            for (key, val) in &mut obj {
                match key.as_str() {
                    "doc_id" | "user_id" => {
                        if let serde_json::Value::String(s) = val
                            && s.len() > 8
                        {
                            *val = serde_json::Value::String(format!("{}***", &s[..8]));
                        }
                    }
                    "query_params" | "value" => {
                        *val = serde_json::Value::String("[REDACTED]".to_string());
                    }
                    _ => {
                        *val = mask_sensitive_data(val.clone());
                    }
                }
            }
            serde_json::Value::Object(obj)
        }
        serde_json::Value::Array(arr) => {
            let masked_arr = arr.into_iter().map(mask_sensitive_data).collect();
            serde_json::Value::Array(masked_arr)
        }
        _ => value,
    }
}

// 导出新模块的公共 API
pub use tamper_proof::{
    Action, Actor, AuditActionType, AuditEntry, AuditEntryId, AuditEventType, AuditMetadata,
    AuditResourceType, AuditWriter, ChainValidationResult, ChainValidator, FileAuditWriter,
    HmacSigner, IntegrityReport, ResourceRef, TamperProofAuditLog,
};

pub use compliance::{
    AccessPatternReport, AnomalyAlert, AnomalyRule, ComplianceReporter, DateRange, GDPRReport,
    SOC2Report, ScopeConfig,
};

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_audit_event_code() {
        assert_eq!(
            AuditEvent::UserLogin {
                triggered_by: AuditTriggeredBy::User("admin".to_string()),
                user_id: "admin".to_string(),
            }
            .code(),
            "AUDIT-001"
        );
        assert_eq!(
            AuditEvent::SystemStart {
                triggered_by: AuditTriggeredBy::System
            }
            .code(),
            "AUDIT-070"
        );
        assert_eq!(
            AuditEvent::FileUpload {
                triggered_by: AuditTriggeredBy::User("admin".to_string()),
                file_path: "test.txt".to_string(),
                file_size: 1024
            }
            .code(),
            "AUDIT-010"
        );
        assert_eq!(
            AuditEvent::ParseStart {
                triggered_by: AuditTriggeredBy::System,
                doc_id: "doc_001".to_string(),
                file_path: "test.txt".to_string(),
            }
            .code(),
            "AUDIT-020"
        );
        assert_eq!(
            AuditEvent::QueryExecute {
                triggered_by: AuditTriggeredBy::User("analyst".to_string()),
                query_type: QueryType::FullTextSearch,
                query_params: "test".to_string()
            }
            .code(),
            "AUDIT-030"
        );
    }

    #[test]
    fn test_audit_event_description() {
        assert_eq!(
            AuditEvent::SystemStart {
                triggered_by: AuditTriggeredBy::System
            }
            .description(),
            "系统启动"
        );
        assert_eq!(
            AuditEvent::FileUpload {
                triggered_by: AuditTriggeredBy::User("admin".to_string()),
                file_path: "test.txt".to_string(),
                file_size: 1024
            }
            .description(),
            "文件上传: test.txt"
        );
    }

    #[test]
    fn test_mask_sensitive_data() {
        let test_value = serde_json::json! {
            {
                "doc_id": "1234567890abcdef",
                "user_id": "user1234567890",
                "query_params": "sensitive data",
                "file_path": "/path/to/file.txt",
                "file_size": 1024
            }
        };

        let masked = mask_sensitive_data(test_value);

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
            triggered_by: AuditTriggeredBy::User("admin".to_string()),
            file_path: "/test/file.txt".to_string(),
            file_size: 1024,
        };

        logger.log("admin", &event, &AuditScope::Local).unwrap();
        logger.close().unwrap();

        assert!(Path::new(&log_path).exists());
    }
}
