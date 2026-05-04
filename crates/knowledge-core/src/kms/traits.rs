use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::Result;

/// 密钥管理服务 trait
///
/// 定义了所有 KMS 后端必须实现的统一接口，支持：
/// - 数据加密密钥 (DEK) 生命周期管理
/// - 信封加密 (Envelope Encryption) 模式
/// - 数字签名与验证
/// - 密钥轮换与元数据查询
///
/// # 实现说明
///
/// 所有实现必须保证线程安全 (`Send + Sync`)，以支持异步并发调用。
/// 使用 `#[async_trait]` 确保返回的 `Future` 满足 `Send` 约束，
/// 以便在 `tokio::spawn` 等跨线程场景中使用。
#[async_trait]
pub trait KeyManagementService: Send + Sync {
    /// 生成数据加密密钥 (DEK)
    ///
    /// 生成一个新的 DEK，并使用指定的 KEK (Key Encryption Key) 加密后返回。
    /// 这是信封加密模式的核心操作。
    ///
    /// # 参数
    ///
    /// - `key_id`: KEK 的标识符，用于加密生成的 DEK
    ///
    /// # 返回
    ///
    /// 返回加密后的 DEK，包含密文、初始化向量等元数据。
    ///
    /// # 错误
    ///
    /// - KEK 不存在或无权限访问
    /// - KMS 服务不可用
    async fn generate_dek(&self, key_id: &str) -> Result<EncryptedKey>;

    /// 使用 DEK 加密数据
    ///
    /// 执行对称加密操作，使用提供的 DEK 对明文进行加密。
    ///
    /// # 参数
    ///
    /// - `dek`: 已加密的 DEK（内部会先解密）
    /// - `plaintext`: 待加密的明文数据
    ///
    /// # 返回
    ///
    /// 返回密文及关联的元数据。
    async fn encrypt(&self, dek: &EncryptedKey, plaintext: &[u8]) -> Result<Ciphertext>;

    /// 解密数据
    ///
    /// 使用 DEK 解密密文，返回原始明文。
    ///
    /// # 参数
    ///
    /// - `dek`: 用于解密的 DEK
    /// - `ciphertext`: 待解密的密文
    ///
    /// # 返回
    ///
    /// 解密后的明文数据。
    ///
    /// # 错误
    ///
    /// - DEK 无效或已过期
    /// - 密文格式错误或被篡改
    async fn decrypt(&self, dek: &EncryptedKey, ciphertext: &Ciphertext) -> Result<Vec<u8>>;

    /// 签名数据
    ///
    /// 使用指定的非对称密钥对数据进行数字签名。
    ///
    /// # 参数
    ///
    /// - `key_id`: 签名密钥的标识符（必须是 RSA 或 ECDSA 密钥）
    /// - `data`: 待签名的数据
    ///
    /// # 返回
    ///
    /// 数字签名结果。
    async fn sign(&self, key_id: &str, data: &[u8]) -> Result<Signature>;

    /// 验证签名
    ///
    /// 验证数据的数字签名是否有效。
    ///
    /// # 参数
    ///
    /// - `key_id`: 验证公钥的标识符
    /// - `data`: 原始数据
    /// - `signature`: 待验证的签名
    ///
    /// # 返回
    ///
    /// 签名是否有效。
    async fn verify(&self, key_id: &str, data: &[u8], signature: &Signature) -> Result<bool>;

    /// 密钥轮换
    ///
    /// 创建指定密钥的新版本，旧版本仍可用于解密但不能再用于加密。
    ///
    /// # 参数
    ///
    /// - `key_id`: 需要轮换的密钥标识符
    ///
    /// # 返回
    ///
    /// 轮换结果，包含新版本信息。
    async fn rotate_key(&self, key_id: &str) -> Result<RotationResult>;

    /// 列出所有密钥
    ///
    /// 返回当前 KMS 中所有可用密钥的元数据列表。
    async fn list_keys(&self) -> Result<Vec<KeyMetadata>>;

    /// 获取密钥元数据
    ///
    /// 查询指定密钥的详细元信息。
    ///
    /// # 参数
    ///
    /// - `key_id`: 密钥标识符
    ///
    /// # 错误
    ///
    /// 密钥不存在时返回错误。
    async fn describe_key(&self, key_id: &str) -> Result<KeyMetadata>;

    /// 销毁密钥 (不可逆)
    ///
    /// **危险操作**：永久删除指定密钥及其所有历史版本。
    /// 此操作不可撤销，执行前应进行二次确认。
    ///
    /// # 安全警告
    ///
    /// 销毁密钥后将无法解密任何使用该密钥加密的数据！
    /// 生产环境必须实施严格的审批流程。
    async fn destroy_key(&self, key_id: &str) -> Result<()>;

    /// 健康检查
    ///
    /// 检查 KMS 服务的可用性和状态。
    async fn health_check(&self) -> Result<KmsHealthStatus>;
}

/// 加密后的数据加密密钥 (DEK)
///
/// 在信封加密模式中，DEK 由 KMS 生成并用 KEK 加密存储。
/// 客户端收到 EncryptedKey 后，可请求 KMS 解密获取明文 DEK，
/// 或直接传递给 encrypt/decrypt 方法由 KMS 服务端处理。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedKey {
    /// 密钥唯一标识符
    pub key_id: String,
    /// 加密后的密钥材料 (ciphertext blob)
    pub ciphertext_blob: Vec<u8>,
    /// 初始化向量 (IV/Nonce)
    pub iv: Vec<u8>,
    /// 使用的加密算法
    pub algorithm: EncryptionAlgorithm,
    /// 密钥创建时间
    pub created_at: DateTime<Utc>,
    /// 用于加密此 DEK 的 KEK ID
    pub encrypted_with: String,
}

impl EncryptedKey {
    /// 创建新的 EncryptedKey
    pub fn new(
        key_id: String,
        ciphertext_blob: Vec<u8>,
        iv: Vec<u8>,
        algorithm: EncryptionAlgorithm,
        encrypted_with: String,
    ) -> Self {
        Self {
            key_id,
            ciphertext_blob,
            iv,
            algorithm,
            created_at: Utc::now(),
            encrypted_with,
        }
    }
}

/// 支持的加密算法枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EncryptionAlgorithm {
    /// AES-256-GCM (推荐，认证加密)
    #[default]
    Aes256Gcm,
    /// AES-256-CBC with HMAC-SHA256
    Aes256CbcHmacSha256,
    /// ChaCha20-Poly1305 (适合无 AES-NI 硬件的环境)
    ChaCha20Poly1305,
}

impl EncryptionAlgorithm {
    /// 获取算法的密钥长度（字节）
    pub const fn key_length(&self) -> usize {
        match self {
            Self::Aes256Gcm | Self::Aes256CbcHmacSha256 | Self::ChaCha20Poly1305 => 32,
        }
    }

    /// 获取算法的 IV/Nonce 长度（字节）
    pub const fn iv_length(&self) -> usize {
        match self {
            Self::Aes256Gcm | Self::ChaCha20Poly1305 => 12,
            Self::Aes256CbcHmacSha256 => 16,
        }
    }

    /// 获取算法的标签长度（字节，用于认证加密）
    pub const fn tag_length(&self) -> usize {
        match self {
            Self::Aes256Gcm | Self::ChaCha20Poly1305 => 16,
            Self::Aes256CbcHmacSha256 => 32,
        }
    }

    /// 获取算法显示名称
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Aes256Gcm => "AES-256-GCM",
            Self::Aes256CbcHmacSha256 => "AES-256-CBC-HMAC-SHA256",
            Self::ChaCha20Poly1305 => "ChaCha20-Poly1305",
        }
    }
}

impl std::fmt::Display for EncryptionAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// 加密后的密文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ciphertext {
    /// 密文数据（AES-GCM 模式下包含 ciphertext || tag）
    pub data: Vec<u8>,
    /// 加密使用的 Nonce/IV（必须在加密时存储，解密时使用相同的 Nonce）
    pub nonce: Vec<u8>,
    /// 认证标签（AES-GCM 模式下已嵌入 data 尾部，此字段保留用于显式存储场景）
    pub auth_tag: Option<Vec<u8>>,
    /// 关联的 DEK 引用
    pub dek_id: String,
    /// 加密时间戳
    pub encrypted_at: DateTime<Utc>,
    /// 使用的算法
    pub algorithm: EncryptionAlgorithm,
    /// 加密上下文 (可选的附加认证数据)
    pub context: Option<HashMap<String, String>>,
}

impl Ciphertext {
    /// 创建新的 Ciphertext
    pub fn new(
        data: Vec<u8>,
        dek_id: String,
        algorithm: EncryptionAlgorithm,
    ) -> Self {
        Self {
            data,
            nonce: Vec::new(),
            auth_tag: None,
            dek_id,
            encrypted_at: Utc::now(),
            algorithm,
            context: None,
        }
    }

    /// 设置加密使用的 Nonce
    ///
    /// AES-GCM 要求每次加密使用唯一的 Nonce，此方法用于存储加密时生成的 Nonce，
    /// 以便解密时使用相同的 Nonce。
    #[must_use]
    pub fn with_nonce(mut self, nonce: Vec<u8>) -> Self {
        self.nonce = nonce;
        self
    }

    /// 设置认证标签
    #[must_use]
    pub fn with_auth_tag(mut self, tag: Vec<u8>) -> Self {
        self.auth_tag = Some(tag);
        self
    }

    /// 设置加密上下文
    #[must_use]
    pub fn with_context(mut self, context: HashMap<String, String>) -> Self {
        self.context = Some(context);
        self
    }
}

/// 数字签名
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    /// 签名值
    pub value: Vec<u8>,
    /// 签名算法
    pub algorithm: SignatureAlgorithm,
    /// 签名时间戳
    pub signed_at: DateTime<Utc>,
    /// 签名使用的密钥 ID
    pub key_id: String,
}

impl Signature {
    /// 创建新签名
    pub fn new(value: Vec<u8>, algorithm: SignatureAlgorithm, key_id: String) -> Self {
        Self {
            value,
            algorithm,
            signed_at: Utc::now(),
            key_id,
        }
    }
}

/// 签名算法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureAlgorithm {
    /// RSA PKCS#1 v1.5 with SHA-256
    RsaPkcs1v15Sha256,
    /// RSA PSS with SHA-256
    RsaPssSha256,
    /// ECDSA with P-256 and SHA-256
    EcdsaP256Sha256,
    /// ECDSA with P-384 and SHA-384
    EcdsaP384Sha384,
    /// Ed25519 (推荐，性能好且安全)
    Ed25519,
    /// HMAC-SHA256 (对称签名)
    HmacSha256,
}

impl SignatureAlgorithm {
    /// 获取算法显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::RsaPkcs1v15Sha256 => "RSA-PKCS1v15-SHA256",
            Self::RsaPssSha256 => "RSA-PSS-SHA256",
            Self::EcdsaP256Sha256 => "ECDSA-P-256-SHA256",
            Self::EcdsaP384Sha384 => "ECDSA-P-384-SHA384",
            Self::Ed25519 => "Ed25519",
            Self::HmacSha256 => "HMAC-SHA256",
        }
    }
}

impl std::fmt::Display for SignatureAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// 密钥轮换结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationResult {
    /// 被轮换的密钥 ID
    pub key_id: String,
    /// 新版本号
    pub new_version: u64,
    /// 轮换时间
    pub rotated_at: DateTime<Utc>,
    /// 旧版本的待删除日期 (Pending Window)
    pub pending_deletion_date: Option<DateTime<Utc>>,
    /// 轮换状态
    pub status: RotationStatus,
}

/// 密钥轮换状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationStatus {
    /// 轮换完成
    Completed,
    /// 轮换中
    InProgress,
    /// 等待确认
    PendingConfirmation,
    /// 失败
    Failed,
}

/// 密钥元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    /// 密钥 ID
    pub key_id: String,
    /// 密钥用途 (ENCRYPT_DECRYPT | SIGN_VERIFY)
    pub key_usage: KeyUsage,
    /// 密钥类型
    pub key_type: KeyType,
    /// 密钥规格
    pub key_spec: KeySpec,
    /// 创建时间
    pub creation_date: DateTime<Utc>,
    /// 启用状态
    pub enabled: bool,
    /// 当前版本号
    pub version: u64,
    /// 描述信息
    pub description: Option<String>,
    /// 标签
    pub tags: HashMap<String, String>,
    /// 轮换配置
    pub rotation_config: Option<RotationConfig>,
    /// 过期时间 (None 表示永不过期)
    pub expiration_date: Option<DateTime<Utc>>,
}

/// 密钥用途
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyUsage {
    /// 加密和解密
    EncryptDecrypt,
    /// 签名和验证
    SignVerify,
    /// 同时支持两者
    Both,
}

/// 密钥类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyType {
    /// 对称加密密钥
    Symmetric,
    /// RSA 非对称密钥
    Rsa,
    /// ECC 非对称密钥
    Ecc,
    /// HMAC 密钥
    Hmac,
    /// EdDSA 密钥 (Ed25519 等)
    Eddsa,
}

/// 密钥规格
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeySpec {
    /// AES-128
    Aes128,
    /// AES-256
    Aes256,
    /// RSA-2048
    Rsa2048,
    /// RSA-3072
    Rsa3072,
    /// RSA-4096
    Rsa4096,
    /// ECC P-256
    EccP256,
    /// ECC P-384
    EccP384,
    /// HMAC-256
    Hmac256,
    /// Ed25519
    Ed25519,
}

impl KeySpec {
    /// 获取密钥材料的长度（字节）
    pub fn key_material_length(&self) -> usize {
        match self {
            Self::Aes128 => 16,
            Self::Aes256 | Self::EccP256 | Self::Hmac256 | Self::Ed25519 => 32,
            Self::Rsa2048 => 256,
            Self::Rsa3072 => 384,
            Self::Rsa4096 => 512,
            Self::EccP384 => 48,
        }
    }

    /// 获取显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Aes128 => "AES_128",
            Self::Aes256 => "AES_256",
            Self::Rsa2048 => "RSA_2048",
            Self::Rsa3072 => "RSA_3072",
            Self::Rsa4096 => "RSA_4096",
            Self::EccP256 => "ECC_NIST_P256",
            Self::EccP384 => "ECC_NIST_P384",
            Self::Hmac256 => "HMAC_256",
            Self::Ed25519 => "ED25519",
        }
    }
}

impl std::fmt::Display for KeySpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// 密钥轮换配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationConfig {
    /// 是否启用自动轮换
    pub enabled: bool,
    /// 轮换周期（天）
    pub rotation_period_days: u32,
    /// 上次轮换时间
    pub last_rotation_date: DateTime<Utc>,
    /// 下次计划轮换时间
    pub next_rotation_date: DateTime<Utc>,
}

impl RotationConfig {
    /// 创建新的轮换配置
    pub fn new(rotation_period_days: u32) -> Self {
        let now = Utc::now();
        Self {
            enabled: true,
            rotation_period_days,
            last_rotation_date: now,
            next_rotation_date: now + chrono::Duration::days(i64::from(rotation_period_days)),
        }
    }

    /// 更新轮换时间（执行轮换后调用）
    pub fn mark_rotated(&mut self) {
        let now = Utc::now();
        self.last_rotation_date = now;
        self.next_rotation_date = now + chrono::Duration::days(i64::from(self.rotation_period_days));
    }
}

/// KMS 健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KmsHealthStatus {
    /// 是否健康
    pub healthy: bool,
    /// 状态消息
    pub message: String,
    /// 响应延迟（毫秒）
    pub latency_ms: u64,
    /// 检查时间
    pub checked_at: DateTime<Utc>,
    /// 额外详情
    pub details: Option<HashMap<String, String>>,
}

impl KmsHealthStatus {
    /// 创建健康状态
    pub fn healthy(latency_ms: u64) -> Self {
        Self {
            healthy: true,
            message: "KMS service is healthy".to_string(),
            latency_ms,
            checked_at: Utc::now(),
            details: None,
        }
    }

    /// 创建不健康状态
    pub fn unhealthy(message: &str) -> Self {
        Self {
            healthy: false,
            message: message.to_string(),
            latency_ms: 0,
            checked_at: Utc::now(),
            details: None,
        }
    }
}

/// 加密上下文 (Encryption Context)
///
/// 用于在 AWS KMS 等服务中绑定密钥与特定上下文，
/// 提供额外的安全层。上下文中的键值对会被包含在加密操作中，
/// 解密时必须提供相同的上下文才能成功。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionContext {
    /// 上下文键值对
    pub context: HashMap<String, String>,
}

impl EncryptionContext {
    /// 创建空的加密上下文
    pub fn empty() -> Self {
        Self {
            context: HashMap::new(),
        }
    }

    /// 从 HashMap 创建
    pub fn from_map(context: HashMap<String, String>) -> Self {
        Self { context }
    }

    /// 添加键值对
    #[must_use]
    pub fn add(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// 转为 AWS SDK 格式
    #[cfg(feature = "kms-aws")]
    pub fn to_aws_format(&self) -> std::collections::HashMap<String, String> {
        self.context.clone()
    }
}

impl Default for EncryptionContext {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<HashMap<String, String>> for EncryptionContext {
    fn from(context: HashMap<String, String>) -> Self {
        Self { context }
    }
}

/// 解密后的 DEK (缓存用)
#[derive(Debug, Clone)]
pub struct DecryptedDek {
    /// 明文 DEK
    pub plaintext_key: Vec<u8>,
    /// 算法
    pub algorithm: EncryptionAlgorithm,
    /// 解密时间
    pub decrypted_at: DateTime<Utc>,
    /// 原始 EncryptedKey ID
    pub source_key_id: String,
    /// TTL 到期时间
    pub expires_at: DateTime<Utc>,
}

impl DecryptedDek {
    /// 检查是否已过期
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// 信封加密后的数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopedData {
    /// 加密后的数据
    pub ciphertext: Ciphertext,
    /// 加密后的 DEK
    pub encrypted_dek: EncryptedKey,
    /// 加密上下文
    pub context: Option<EncryptionContext>,
    /// 数据版本号 (用于密钥轮换追踪)
    pub version: u64,
}

/// 密钥轮换调度器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationSchedule {
    /// 密钥 ID
    pub key_id: String,
    /// 轮换周期（秒）
    pub interval_seconds: u64,
    /// 上次轮换时间
    pub last_rotation: DateTime<Utc>,
    /// 下次轮换时间
    pub next_rotation: DateTime<Utc>,
    /// 是否启用自动轮换
    pub auto_rotate: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_algorithm_properties() {
        assert_eq!(EncryptionAlgorithm::Aes256Gcm.key_length(), 32);
        assert_eq!(EncryptionAlgorithm::Aes256Gcm.iv_length(), 12);
        assert_eq!(EncryptionAlgorithm::Aes256Gcm.tag_length(), 16);

        assert_eq!(EncryptionAlgorithm::ChaCha20Poly1305.key_length(), 32);
        assert_eq!(EncryptionAlgorithm::ChaCha20Poly1305.iv_length(), 12);

        assert_eq!(
            EncryptionAlgorithm::Aes256CbcHmacSha256.display_name(),
            "AES-256-CBC-HMAC-SHA256"
        );
    }

    #[test]
    fn test_encrypted_key_creation() {
        let key = EncryptedKey::new(
            "test-key".to_string(),
            vec![1, 2, 3, 4],
            vec![0; 12],
            EncryptionAlgorithm::default(),
            "kek-id".to_string(),
        );

        assert_eq!(key.key_id, "test-key");
        assert_eq!(key.encrypted_with, "kek-id");
        assert_eq!(key.algorithm, EncryptionAlgorithm::Aes256Gcm);
    }

    #[test]
    fn test_ciphertext_builder() {
        let ct = Ciphertext::new(
            vec![1, 2, 3],
            "dek-123".to_string(),
            EncryptionAlgorithm::Aes256Gcm,
        )
        .with_auth_tag(vec![0; 16])
        .with_context({
            let mut ctx = HashMap::new();
            ctx.insert("purpose".to_string(), "test".to_string());
            ctx
        });

        assert!(ct.auth_tag.is_some());
        assert!(ct.context.is_some());
        assert_eq!(ct.dek_id, "dek-123");
    }

    #[test]
    fn test_key_spec_lengths() {
        assert_eq!(KeySpec::Aes256.key_material_length(), 32);
        assert_eq!(KeySpec::Rsa4096.key_material_length(), 512);
        assert_eq!(KeySpec::Ed25519.key_material_length(), 32);
    }

    #[test]
    fn test_rotation_config() {
        let mut config = RotationConfig::new(90);
        assert!(config.enabled);
        assert_eq!(config.rotation_period_days, 90);

        config.mark_rotated();
        let expected_next = config.last_rotation_date + chrono::Duration::days(90);
        assert_eq!(config.next_rotation_date, expected_next);
    }

    #[test]
    fn test_encryption_context() {
        let ctx = EncryptionContext::empty()
            .add("department", "engineering")
            .add("project", "kms");

        assert_eq!(ctx.context.len(), 2);
        assert_eq!(ctx.context.get("department").unwrap(), "engineering");
    }

    #[test]
    fn test_kms_health_status() {
        let healthy = KmsHealthStatus::healthy(15);
        assert!(healthy.healthy);
        assert_eq!(healthy.latency_ms, 15);

        let unhealthy = KmsHealthStatus::unhealthy("Connection refused");
        assert!(!unhealthy.healthy);
        assert_eq!(unhealthy.message, "Connection refused");
    }

    #[test]
    fn test_signature_algorithm_display() {
        assert_eq!(
            SignatureAlgorithm::Ed25519.to_string(),
            "Ed25519"
        );
        assert_eq!(
            SignatureAlgorithm::RsaPkcs1v15Sha256.to_string(),
            "RSA-PKCS1v15-SHA256"
        );
    }

    #[test]
    fn test_decrypted_dek_expiry() {
        let dek = DecryptedDek {
            plaintext_key: vec![0; 32],
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            decrypted_at: Utc::now(),
            source_key_id: "test".to_string(),
            expires_at: Utc::now() - chrono::Duration::hours(1),
        };

        assert!(dek.is_expired());
    }
}
