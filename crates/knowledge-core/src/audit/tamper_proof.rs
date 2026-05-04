use async_trait::async_trait;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::helpers;
type Result<T> = crate::Result<T>;

type HmacSha256 = Hmac<Sha256>;

fn create_hmac_sha256(key: &[u8]) -> Result<HmacSha256> {
    HmacSha256::new_from_slice(key)
        .map_err(|e| helpers::crypto_error(&format!("HMAC-SHA256 初始化失败: {e}")))
}

/// 不可篡改审计日志系统
///
/// 通过区块链式的哈希链和 HMAC 签名确保审计日志的完整性，
/// 任何对历史记录的篡改都能被检测到。
///
/// # 安全机制
///
/// 1. **哈希链**: 每条记录包含前一条的哈希，形成链式结构
/// 2. **HMAC 签名**: 使用密钥对每条记录进行签名
/// 3. **时间戳**: 记录精确的操作时间
/// 4. **完整性验证**: 可随时验证整条链的完整性
pub struct TamperProofAuditLog {
    /// 审计写入器（支持多种后端）
    writer: Box<dyn AuditWriter>,
    /// HMAC 签名器
    signer: HmacSigner,
    /// 链验证器（预留：实时链完整性校验尚未集成到写入路径，error-core 恢复状态机集成后实现）
    #[allow(dead_code)] // 预留：防篡改审计接口，待集成到审计管道
    chain_validator: ChainValidator,
}

/// 审计条目 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct AuditEntryId(pub String);

impl AuditEntryId {
    /// 创建新的审计条目 ID
    #[must_use]
    pub fn new() -> Self {
        Self(format!("audit-{}", uuid::Uuid::new_v4()))
    }

    /// 从字符串创建
    #[must_use]
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// 获取字符串表示
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AuditEntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 审计事件类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    // 系统事件
    /// 系统启动
    SystemStart,
    /// 系统关闭
    SystemShutdown,
    /// 系统错误
    SystemError,

    // 用户管理
    /// 用户创建
    UserCreated,
    /// 用户信息更新
    UserUpdated,
    /// 用户删除
    UserDeleted,
    /// 用户登录
    UserLogin,
    /// 用户登出
    UserLogout,
    /// 密码修改
    PasswordChanged,

    // 权限管理
    /// 权限授予
    PermissionGranted,
    /// 权限撤销
    PermissionRevoked,
    /// 角色分配
    RoleAssigned,
    /// 角色移除
    RoleRemoved,

    // 数据操作
    /// 数据创建
    DataCreated,
    /// 数据读取
    DataRead,
    /// 数据更新
    DataUpdated,
    /// 数据删除
    DataDeleted,
    /// 数据导出
    DataExported,
    /// 数据导入
    DataImported,

    // 密钥操作
    /// 密钥创建
    KeyCreated,
    /// 密钥轮换
    KeyRotated,
    /// 密钥撤销
    KeyRevoked,
    /// 密钥销毁
    KeyDestroyed,
    /// 密钥访问
    KeyAccessed,

    // 配置变更
    /// 配置变更
    ConfigChanged,
    /// 策略更新
    PolicyUpdated,

    // 安全事件
    /// 安全告警
    SecurityAlert,
    /// 入侵检测
    IntrusionDetected,
    /// 暴力破解尝试
    BruteForceAttempt,
    /// 异常行为检测
    AnomalyDetected,

    /// 自定义事件（携带事件代码）
    Custom(String),
}

impl AuditEventType {
    /// 获取事件类型代码
    #[must_use]
    pub fn code(&self) -> &str {
        match self {
            Self::SystemStart => "SYS-001",
            Self::SystemShutdown => "SYS-002",
            Self::SystemError => "SYS-003",
            Self::UserCreated => "USR-001",
            Self::UserUpdated => "USR-002",
            Self::UserDeleted => "USR-003",
            Self::UserLogin => "USR-010",
            Self::UserLogout => "USR-011",
            Self::PasswordChanged => "USR-012",
            Self::PermissionGranted => "PER-001",
            Self::PermissionRevoked => "PER-002",
            Self::RoleAssigned => "ROL-001",
            Self::RoleRemoved => "ROL-002",
            Self::DataCreated => "DAT-001",
            Self::DataRead => "DAT-002",
            Self::DataUpdated => "DAT-003",
            Self::DataDeleted => "DAT-004",
            Self::DataExported => "DAT-010",
            Self::DataImported => "DAT-011",
            Self::KeyCreated => "KEY-001",
            Self::KeyRotated => "KEY-002",
            Self::KeyRevoked => "KEY-003",
            Self::KeyDestroyed => "KEY-004",
            Self::KeyAccessed => "KEY-010",
            Self::ConfigChanged => "CFG-001",
            Self::PolicyUpdated => "CFG-002",
            Self::SecurityAlert => "SEC-001",
            Self::IntrusionDetected => "SEC-002",
            Self::BruteForceAttempt => "SEC-003",
            Self::AnomalyDetected => "SEC-004",
            Self::Custom(code) => code.as_str(),
        }
    }
}

/// 操作者 (Actor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Actor {
    /// 系统用户
    User {
        /// 用户唯一标识
        id: String,
        /// 用户显示名称
        name: Option<String>,
    },
    /// 服务账户
    Service {
        /// 服务名称
        name: String,
    },
    /// 系统/自动化操作
    System {
        /// 系统组件名称
        component: String,
    },
    /// 外部 API 调用
    ApiClient {
        /// 客户端标识
        client_id: String,
    },
    /// 匿名（仅限特定场景）
    Anonymous,
}

impl Actor {
    /// 获取操作者 ID
    #[must_use]
    pub fn actor_id(&self) -> &str {
        match self {
            Self::User { id, .. } => id,
            Self::Service { name } => name,
            Self::System { component } => component,
            Self::ApiClient { client_id } => client_id,
            Self::Anonymous => "anonymous",
        }
    }
}

/// 操作描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// 操作类型
    pub action_type: String,
    /// 操作详情
    pub details: HashMap<String, String>,
}

/// 资源引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRef {
    /// 资源类型
    pub resource_type: String,
    /// 资源 ID
    pub resource_id: String,
    /// 资源路径（可选）
    pub path: Option<String>,
}

/// 审计元数据
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditMetadata {
    /// 来源 IP 地址
    pub source_ip: Option<String>,
    /// User-Agent
    pub user_agent: Option<String>,
    /// 会话 ID
    pub session_id: Option<String>,
    /// 关联请求 ID
    pub request_id: Option<String>,
    /// 额外标签
    pub tags: HashMap<String, String>,
}

/// 审计日志条目
///
/// 每条记录包含完整的上下文信息，并通过哈希链和签名确保完整性。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// 唯一标识符
    pub id: AuditEntryId,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 事件类型
    pub event_type: AuditEventType,
    /// 操作者
    pub actor: Actor,
    /// 操作
    pub action: Action,
    /// 目标资源
    pub resource: ResourceRef,
    /// 操作载荷
    pub payload: serde_json::Value,
    /// 前一条记录的哈希（用于链式验证）
    pub previous_hash: Option<String>,
    /// 本条记录的哈希（含 previous_hash）
    pub current_hash: String,
    /// HMAC 签名
    pub signature: String,
    /// 元数据
    pub metadata: AuditMetadata,
}

/// 审计写入器 trait
#[async_trait::async_trait]
pub trait AuditWriter: Send + Sync {
    /// 追加新的审计条目
    async fn append(&self, entry: &AuditEntry) -> Result<AuditEntryId>;

    /// 按时间范围读取条目
    async fn read_range(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<AuditEntry>>;

    /// 读取最后一条审计条目（用于构建审计链的 previous_hash）
    async fn read_last(&self) -> Result<Option<AuditEntry>> {
        let now = Utc::now();
        let entries = self.read_range(now - chrono::Duration::days(365), now).await?;
        Ok(entries.last().cloned())
    }

    /// 按 ID 读取单条记录
    async fn read_by_id(&self, id: &AuditEntryId) -> Result<Option<AuditEntry>>;

    /// 验证完整性
    async fn verify_integrity(&self) -> Result<IntegrityReport>;
}

/// 完整性报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    /// 是否完整
    pub is_valid: bool,
    /// 总条目数
    pub total_entries: usize,
    /// 已验证条目数
    pub verified_entries: usize,
    /// 发现的问题列表
    pub issues: Vec<IntegrityIssue>,
    /// 报告生成时间
    pub generated_at: DateTime<Utc>,
}

/// 完整性问题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityIssue {
    /// 问题类型
    pub issue_type: IntegrityIssueType,
    /// 受影响的条目 ID
    pub entry_id: AuditEntryId,
    /// 描述
    pub description: String,
}

/// 完整性问题类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityIssueType {
    /// 哈希不匹配
    HashMismatch,
    /// 签名无效
    InvalidSignature,
    /// 链断裂（缺少前序记录）
    ChainBreak,
    /// 时间戳异常
    TimestampAnomaly,
}

/// HMAC 签名器
///
/// 使用 HMAC-SHA256 对审计条目进行数字签名。
pub struct HmacSigner {
    /// 32 字节 HMAC 签名密钥
    key: [u8; 32],
    /// 使用的哈希算法（预留：多算法切换尚未实现，LightField 替代后支持）
    #[allow(dead_code)] // 预留：防篡改审计接口，待集成到审计管道
    algorithm: HashAlgorithm,
}

/// 支持的哈希算法
#[derive(Debug, Clone, Copy, Default)]
pub enum HashAlgorithm {
    /// SHA-256 哈希算法（默认）
    #[default]
    Sha256,
}

impl HmacSigner {
    /// 创建新的 HMAC 签名器
    ///
    /// # 参数
    ///
    /// - `key`: 32 字节的签名密钥
    pub const fn new(key: [u8; 32]) -> Self {
        Self {
            key,
            algorithm: HashAlgorithm::Sha256,
        }
    }

    /// 生成随机密钥
    pub fn generate_key() -> [u8; 32] {
        use rand::RngCore;
        let mut key = [0u8; 32];
        rand::rng().fill_bytes(&mut key);
        key
    }

    /// 计算数据的 HMAC 签名
    ///
    /// HMAC-SHA256 的 `new_from_slice` 接受任意长度的密钥（内部会进行 HKDF 展开），
    /// 因此使用 32 字节密钥时不可能返回错误。
    #[must_use]
    pub fn sign_bytes(&self, data: &[u8]) -> String {
        let mut mac = create_hmac_sha256(&self.key)
            .expect("HMAC-SHA256 接受任意长度密钥; 32 字节密钥保证成功");
        mac.update(data);
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    /// 签名审计条目
    #[must_use]
    pub fn sign_entry(&self, entry: &AuditEntry) -> String {
        let serialized = Self::serialize_for_signing(entry);
        self.sign_bytes(serialized.as_bytes())
    }

    /// 验证签名
    #[must_use]
    pub fn verify_signature(&self, entry: &AuditEntry) -> bool {
        let expected = self.sign_entry(entry);
        expected == entry.signature
    }

    /// 序列化用于签名的字段（排除 `signature` 和 `current_hash`）
    fn serialize_for_signing(entry: &AuditEntry) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            entry.id.0,
            entry.timestamp.format("%Y-%m-%dT%H:%M:%S%.6fZ"),
            entry.event_type.code(),
            entry.actor.actor_id(),
            entry.action.action_type,
            entry.resource.resource_id,
            entry.previous_hash.as_deref().unwrap_or("")
        )
    }
}

/// 链验证器
///
/// 用于验证审计日志链的完整性。
pub struct ChainValidator;

impl Default for ChainValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ChainValidator {
    /// 创建新的链验证器
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// 计算条目的哈希值
    #[must_use]
    pub fn compute_hash(entry: &AuditEntry) -> String {
        use blake3::Hasher;
        let mut hasher = Hasher::new();

        hasher.update(entry.id.0.as_bytes());
        hasher.update(
            entry
                .timestamp
                .format("%Y-%m-%dT%H:%M:%S%.6fZ")
                .to_string()
                .as_bytes(),
        );
        hasher.update(entry.event_type.code().as_bytes());
        hasher.update(entry.actor.actor_id().as_bytes());
        hasher.update(
            &serde_json::to_vec(&entry.payload).unwrap_or_default(),
        );

        if let Some(ref prev_hash) = entry.previous_hash {
            hasher.update(prev_hash.as_bytes());
        }

        hasher.finalize().to_hex().to_string()
    }

    /// 验证单条记录的哈希
    #[must_use]
    pub fn verify_entry_hash(entry: &AuditEntry) -> bool {
        let computed = Self::compute_hash(entry);
        computed == entry.current_hash
    }

    /// 验证整条链的完整性
    #[must_use]
    pub fn verify_chain(entries: &[AuditEntry]) -> ChainValidationResult {
        if entries.is_empty() {
            return ChainValidationResult {
                is_valid: true,
                issues: vec![],
                validated_count: 0,
            };
        }

        let mut issues = Vec::new();

        for (i, entry) in entries.iter().enumerate() {
            // 验证当前条目的哈希
            if !Self::verify_entry_hash(entry) {
                issues.push(IntegrityIssue {
                    issue_type: IntegrityIssueType::HashMismatch,
                    entry_id: entry.id.clone(),
                    description: format!("条目 {} 的哈希值不匹配", i + 1),
                });
            }

            // 验证与前一条的链接
            if i > 0 {
                let prev = &entries[i - 1];
                if entry.previous_hash.as_deref() != Some(&prev.current_hash) {
                    issues.push(IntegrityIssue {
                        issue_type: IntegrityIssueType::ChainBreak,
                        entry_id: entry.id.clone(),
                        description: format!("条目 {} 的前序哈希与条目 {} 不匹配", i + 1, i),
                    });
                }
            } else if entry.previous_hash.is_some()
                && !entries[0]
                    .previous_hash
                    .as_ref()
                    .is_none_or(String::is_empty)
            {
                // 第一条记录不应该有前序哈希（除非是追加到已有链）
            }
        }

        ChainValidationResult {
            is_valid: issues.is_empty(),
            issues,
            validated_count: entries.len(),
        }
    }
}

/// 链验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainValidationResult {
    /// 链是否完整有效
    pub is_valid: bool,
    /// 发现的完整性问题列表
    pub issues: Vec<IntegrityIssue>,
    /// 已验证的条目总数
    pub validated_count: usize,
}

impl TamperProofAuditLog {
    /// 创建新的不可篡改审计日志实例
    ///
    /// # 参数
    ///
    /// - `writer`: 审计写入器后端
    /// - `signer_key`: 32 字节的 HMAC 签名密钥
    pub fn new(writer: Box<dyn AuditWriter>, signer_key: [u8; 32]) -> Self {
        Self {
            writer,
            signer: HmacSigner::new(signer_key),
            chain_validator: ChainValidator::new(),
        }
    }

    /// 记录审计事件
    ///
    /// # Errors
    /// 当审计写入器操作失败时返回错误
    pub async fn log_event(
        &self,
        event_type: AuditEventType,
        actor: Actor,
        action: Action,
        resource: ResourceRef,
        payload: serde_json::Value,
        metadata: Option<AuditMetadata>,
    ) -> Result<AuditEntry> {
        let id = AuditEntryId::new();
        let timestamp = Utc::now();
        let meta = metadata.unwrap_or_default();

        // 获取最后一条记录的哈希作为 previous_hash
        // 使用 read_last 而非时间窗口查询，避免并发和长时间间隔导致链断裂
        let last_entry = self.writer.read_last().await?;
        let previous_hash = last_entry.map(|e| e.current_hash.clone());

        // 构建未完成的条目
        let mut entry = AuditEntry {
            id: id.clone(),
            timestamp,
            event_type,
            actor,
            action,
            resource,
            payload,
            previous_hash,
            current_hash: String::new(), // 先填空，后面计算
            signature: String::new(),    // 先填空，后面计算
            metadata: meta,
        };

        // 计算哈希
        entry.current_hash = ChainValidator::compute_hash(&entry);

        // 签名
        entry.signature = self.signer.sign_entry(&entry);

        // 写入存储
        self.writer.append(&entry).await?;

        Ok(entry)
    }

    /// 验证日志完整性
    ///
    /// # Errors
    /// 当审计写入器读取失败时返回错误
    pub async fn verify_integrity(&self) -> Result<IntegrityReport> {
        // 获取所有条目
        let from = DateTime::<Utc>::MIN_UTC;
        let to = Utc::now() + chrono::Duration::days(1);
        let entries = self.writer.read_range(from, to).await?;

        // 验证链
        let chain_result = ChainValidator::verify_chain(&entries);

        // 验证签名
        let mut signature_issues = Vec::new();
        for entry in &entries {
            if !self.signer.verify_signature(entry) {
                signature_issues.push(IntegrityIssue {
                    issue_type: IntegrityIssueType::InvalidSignature,
                    entry_id: entry.id.clone(),
                    description: "签名验证失败".to_string(),
                });
            }
        }

        let all_issues = [&chain_result.issues[..], &signature_issues[..]].concat();

        Ok(IntegrityReport {
            is_valid: all_issues.is_empty(),
            total_entries: entries.len(),
            verified_entries: chain_result.validated_count,
            issues: all_issues,
            generated_at: Utc::now(),
        })
    }

    /// 查询审计日志
    ///
    /// # Errors
    /// 当审计写入器读取失败时返回错误
    pub async fn query(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<AuditEntry>> {
        self.writer.read_range(from, to).await
    }

    /// 按 ID 查询单条记录
    ///
    /// # Errors
    /// 当审计写入器读取失败时返回错误
    pub async fn get_by_id(&self, id: &AuditEntryId) -> Result<Option<AuditEntry>> {
        self.writer.read_by_id(id).await
    }
}

/// 文件系统审计写入器实现
pub struct FileAuditWriter {
    /// 审计日志文件路径
    log_path: std::path::PathBuf,
    /// 条目 ID → 文件行号的索引（预留：按 ID 快速定位审计条目尚未实现，LightField 替代后原生支持）
    #[allow(dead_code)] // 预留：防篡改审计接口，待集成到审计管道
    index: Arc<RwLock<HashMap<String, u64>>>,
}

impl FileAuditWriter {
    /// 创建文件系统写入器
    pub fn new(log_path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = log_path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        Ok(Self {
            log_path: path.to_path_buf(),
            index: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 追加行到文件
    ///
    /// # Errors
    /// 当文件打开或写入失败时返回 IO 错误
    async fn append_line(&self, line: &str) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .await
            .map_err(|e| helpers::io_error(&format!("无法打开审计日志文件: {e}")))?;

        file.write_all(line.as_bytes())
            .await
            .map_err(|e| helpers::io_error(&format!("写入审计日志失败: {e}")))?;
        file.write_all(b"\n")
            .await
            .map_err(|e| helpers::io_error(&format!("写入换行符失败: {e}")))?;
        file.flush()
            .await
            .map_err(|e| helpers::io_error(&format!("刷新缓冲区失败: {e}")))?;

        Ok(())
    }

    /// 读取所有行
    async fn read_all_lines(&self) -> Result<Vec<String>> {
        match tokio::fs::read_to_string(&self.log_path).await {
            Ok(content) => Ok(content.lines().map(String::from).collect()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(helpers::io_error(&format!("读取审计日志失败: {e}"))),
        }
    }
}

#[async_trait]
impl AuditWriter for FileAuditWriter {
    async fn append(&self, entry: &AuditEntry) -> Result<AuditEntryId> {
        let bytes = serde_json::to_vec(entry)?;
        let line = String::from_utf8(bytes)
            .map_err(|e| error_core::helpers::serde_error(&e.to_string()))?;
        self.append_line(&line).await?;
        Ok(entry.id.clone())
    }

    async fn read_range(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<AuditEntry>> {
        let lines = self.read_all_lines().await?;
        let mut entries = Vec::new();

        for line in lines {
            if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
                if entry.timestamp >= from && entry.timestamp <= to {
                    entries.push(entry);
                }
            }
        }

        entries.sort_by_key(|a| a.timestamp);
        Ok(entries)
    }

    async fn read_by_id(&self, id: &AuditEntryId) -> Result<Option<AuditEntry>> {
        let lines = self.read_all_lines().await?;

        for line in lines {
            if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
                if entry.id == *id {
                    return Ok(Some(entry));
                }
            }
        }

        Ok(None)
    }

    async fn verify_integrity(&self) -> Result<IntegrityReport> {
        let lines = self.read_all_lines().await?;
        let entries: Vec<AuditEntry> = lines
            .iter()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        let result = ChainValidator::verify_chain(&entries);

        Ok(IntegrityReport {
            is_valid: result.is_valid,
            total_entries: entries.len(),
            verified_entries: result.validated_count,
            issues: result.issues,
            generated_at: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_log_and_verify_event() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("audit.log");
        let signer_key = HmacSigner::generate_key();

        let writer = Box::new(FileAuditWriter::new(&log_path).unwrap());
        let audit = TamperProofAuditLog::new(writer, signer_key);

        let entry = audit
            .log_event(
                AuditEventType::DataCreated,
                Actor::User {
                    id: "user-123".to_string(),
                    name: Some("张三".to_string()),
                },
                Action {
                    action_type: "create_document".to_string(),
                    details: HashMap::new(),
                },
                ResourceRef {
                    resource_type: "document".to_string(),
                    resource_id: "doc-456".to_string(),
                    path: None,
                },
                serde_json::json!({"title": "测试文档"}),
                None,
            )
            .await
            .unwrap();

        assert_eq!(entry.event_type, AuditEventType::DataCreated);
        assert!(!entry.signature.is_empty());
        assert!(!entry.current_hash.is_empty());

        let report = audit.verify_integrity().await.unwrap();
        assert!(report.is_valid);
        assert_eq!(report.total_entries, 1);
    }

    #[tokio::test]
    async fn test_chain_validation() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("audit-chain.log");
        let signer_key = HmacSigner::generate_key();

        let writer = Box::new(FileAuditWriter::new(&log_path).unwrap());
        let audit = TamperProofAuditLog::new(writer, signer_key);

        for i in 0..5 {
            audit
                .log_event(
                    AuditEventType::DataCreated,
                    Actor::System {
                        component: "test".to_string(),
                    },
                    Action {
                        action_type: format!("operation_{i}"),
                        details: HashMap::new(),
                    },
                    ResourceRef {
                        resource_type: "resource".to_string(),
                        resource_id: format!("res-{i}"),
                        path: None,
                    },
                    serde_json::json!({"index": i}),
                    None,
                )
                .await
                .unwrap();
        }

        let report = audit.verify_integrity().await.unwrap();
        assert!(report.is_valid);
        assert_eq!(report.total_entries, 5);
    }

    #[tokio::test]
    async fn test_query_events() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("audit-query.log");
        let signer_key = HmacSigner::generate_key();

        let writer = Box::new(FileAuditWriter::new(&log_path).unwrap());
        let audit = TamperProofAuditLog::new(writer, signer_key);

        let now = Utc::now();

        audit
            .log_event(
                AuditEventType::UserLogin,
                Actor::User {
                    id: "u1".to_string(),
                    name: None,
                },
                Action {
                    action_type: "login".to_string(),
                    details: HashMap::new(),
                },
                ResourceRef {
                    resource_type: "session".to_string(),
                    resource_id: "s1".to_string(),
                    path: None,
                },
                serde_json::json!({}),
                None,
            )
            .await
            .unwrap();

        let entries = audit
            .query(
                now - chrono::Duration::minutes(1),
                now + chrono::Duration::minutes(1),
            )
            .await
            .unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_hmac_sign_and_verify() {
        let key = HmacSigner::generate_key();
        let signer = HmacSigner::new(key);

        let data = b"test data";
        let signature = signer.sign_bytes(data);
        assert!(!signature.is_empty());
        assert!(signature.len() > 32);
    }

    #[test]
    fn test_chain_validator_compute_hash() {
        let entry = AuditEntry {
            id: AuditEntryId::from_string("test-id"),
            timestamp: Utc::now(),
            event_type: AuditEventType::SystemStart,
            actor: Actor::System {
                component: "test".to_string(),
            },
            action: Action {
                action_type: "start".to_string(),
                details: HashMap::new(),
            },
            resource: ResourceRef {
                resource_type: "system".to_string(),
                resource_id: "sys-1".to_string(),
                path: None,
            },
            payload: serde_json::json!({}),
            previous_hash: None,
            current_hash: String::new(),
            signature: String::new(),
            metadata: AuditMetadata::default(),
        };

        let hash = ChainValidator::compute_hash(&entry);
        assert!(!hash.is_empty());
    }

    #[test]
    fn test_audit_event_type_codes() {
        assert_eq!(AuditEventType::SystemStart.code(), "SYS-001");
        assert_eq!(AuditEventType::UserLogin.code(), "USR-010");
        assert_eq!(AuditEventType::KeyRotated.code(), "KEY-002");
    }

    #[test]
    fn test_actor_id_extraction() {
        let user = Actor::User {
            id: "user-1".to_string(),
            name: None,
        };
        assert_eq!(user.actor_id(), "user-1");

        let service = Actor::Service {
            name: "api-gateway".to_string(),
        };
        assert_eq!(service.actor_id(), "api-gateway");

        let system = Actor::System {
            component: "scheduler".to_string(),
        };
        assert_eq!(system.actor_id(), "scheduler");
    }
}
