use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use chrono::Utc;
use ring::{
    rand as ring_rand,
    signature::Ed25519KeyPair,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use zeroize::ZeroizeOnDrop;

use crate::error::helpers;
use crate::kms::traits::*;
use crate::Result;

/// 主密钥包装类型，确保析构时安全擦除
#[derive(Clone, ZeroizeOnDrop)]
struct MasterKey([u8; 32]);

impl MasterKey {
    fn new(key: [u8; 32]) -> Self {
        Self(key)
    }

    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::ops::Deref for MasterKey {
    type Target = [u8; 32];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// 本地文件系统 KMS 实现
///
/// 适用于开发和测试环境的轻量级 KMS 后端。
///
/// # 安全警告
///
/// **此实现仅用于开发和测试环境！**
/// 生产环境必须使用 AWS KMS 或 HashiCorp Vault 等专业 KMS 服务。
///
/// # 特性
///
/// - 密钥存储在本地文件系统（加密后）
/// - 支持 AES-256-GCM 对称加密
/// - 支持 Ed25519 数字签名
/// - 基于 Master Key 的密钥派生
/// - 线程安全（使用 RwLock）
pub struct LocalKms {
    /// 主密钥 (Master Key)，用于保护其他密钥
    /// 析构时通过 `ZeroizeOnDrop` 自动安全擦除
    master_key: MasterKey,
    /// 密钥存储目录
    keys_dir: PathBuf,
    /// 内存中的密钥缓存
    key_store: Arc<RwLock<HashMap<String, StoredKey>>>,
    /// 随机数生成器
    rng: Arc<RwLock<ring_rand::SystemRandom>>,
}

/// 存储的密钥结构
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredKey {
    /// 密钥元数据
    metadata: KeyMetadata,
    /// 加密后的密钥材料
    encrypted_material: Vec<u8>,
    /// 加密使用的 IV
    iv: Vec<u8>,
}

impl LocalKms {
    /// 创建新的 LocalKms 实例
    ///
    /// # 参数
    ///
    /// - `master_key`: 32 字节的主密钥，用于加密/解密其他密钥
    /// - `keys_dir`: 密钥存储目录路径
    ///
    /// # 错误
    ///
    /// 目录创建失败时返回错误。
    pub fn new(master_key: [u8; 32], keys_dir: impl AsRef<Path>) -> Result<Self> {
        let keys_dir = keys_dir.as_ref().to_path_buf();

        std::fs::create_dir_all(&keys_dir).map_err(|e| {
            helpers::internal_error(&format!("无法创建密钥目录 {:?}: {}", keys_dir, e))
        })?;

        Ok(Self {
            master_key: MasterKey::new(master_key),
            keys_dir,
            key_store: Arc::new(RwLock::new(HashMap::new())),
            rng: Arc::new(RwLock::new(ring_rand::SystemRandom::new())),
        })
    }

    /// 从配置文件加载或创建 LocalKms
    pub async fn from_config(config: &LocalKmsConfig) -> Result<Self> {
        let master_key = if config.master_key_path.exists() {
            Self::load_master_key(&config.master_key_path)?
        } else {
            let key = Self::generate_master_key();
            Self::save_master_key(&key, &config.master_key_path)?;
            key
        };

        let kms = Self::new(master_key, &config.keys_dir)?;

        // 加载已有密钥
        if config.keys_dir.exists() {
            kms.load_keys_from_disk().await?;
        }

        Ok(kms)
    }

    /// 生成随机主密钥
    fn generate_master_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut key);
        key
    }

    /// 从文件加载主密钥
    fn load_master_key(path: &Path) -> Result<[u8; 32]> {
        let data = std::fs::read(path).map_err(|e| {
            helpers::internal_error(&format!("无法读取主密钥文件 {:?}: {}", path, e))
        })?;

        if data.len() != 32 {
            return Err(helpers::crypto_error(
                "主密钥文件长度不正确（应为32字节）",
            ));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&data[..32]);
        Ok(key)
    }

    /// 保存主密钥到文件
    fn save_master_key(key: &[u8; 32], path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, key).map_err(|e| {
            helpers::internal_error(&format!("无法写入主密钥文件 {:?}: {}", path, e))
        })?;
        Ok(())
    }

    /// 使用主密钥加密数据
    fn encrypt_with_master_key(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        let cipher = Aes256Gcm::new_from_slice(self.master_key.as_bytes())
            .map_err(|e| helpers::crypto_error(&format!("AES 初始化失败: {}", e)))?;

        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| helpers::crypto_error(&format!("加密失败: {}", e)))?;

        Ok((ciphertext.to_vec(), nonce.to_vec()))
    }

    /// 使用主密钥解密数据
    fn decrypt_with_master_key(&self, ciphertext: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(self.master_key.as_bytes())
            .map_err(|e| helpers::crypto_error(&format!("AES 初始化失败: {}", e)))?;

        let nonce = Nonce::from_slice(iv);
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| helpers::crypto_error("解密失败：密文可能已损坏"))?;

        Ok(plaintext)
    }

    /// 从磁盘加载所有密钥
    async fn load_keys_from_disk(&self) -> Result<()> {
        let entries = std::fs::read_dir(&self.keys_dir).map_err(|e| {
            helpers::internal_error(&format!("无法读取密钥目录: {}", e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("key") {
                if let Ok(data) = std::fs::read(&path) {
                    if let Ok(stored_key) = bincode::deserialize::<StoredKey>(&data) {
                        let mut store = self.key_store.write().await;
                        store.insert(stored_key.metadata.key_id.clone(), stored_key);
                    }
                }
            }
        }

        Ok(())
    }

    /// 将密钥保存到磁盘
    async fn save_key_to_disk(&self, stored_key: &StoredKey) -> Result<()> {
        let filename = format!("{}.key", stored_key.metadata.key_id);
        let path = self.keys_dir.join(filename);

        let data = bincode::serialize(stored_key)
            .map_err(|e| helpers::internal_error(&format!("序列化密钥失败: {}", e)))?;

        std::fs::write(&path, &data).map_err(|e| {
            helpers::internal_error(&format!("无法写入密钥文件 {:?}: {}", path, e))
        })?;

        Ok(())
    }

    /// 从磁盘删除密钥文件
    fn delete_key_file(&self, key_id: &str) -> Result<()> {
        let filename = format!("{}.key", key_id);
        let path = self.keys_dir.join(filename);

        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                helpers::internal_error(&format!("无法删除密钥文件: {}", e))
            })?;
        }

        Ok(())
    }

    /// 解密获取密钥材料的明文
    async fn decrypt_key_material(&self, key_id: &str) -> Result<Vec<u8>> {
        let store = self.key_store.read().await;
        let stored_key = store.get(key_id).ok_or_else(|| {
            helpers::not_found("密钥", key_id)
        })?;

        self.decrypt_with_master_key(&stored_key.encrypted_material, &stored_key.iv)
    }

    /// 创建默认密钥元数据
    fn create_default_metadata(
        key_id: String,
        key_usage: KeyUsage,
        key_type: KeyType,
        key_spec: KeySpec,
    ) -> KeyMetadata {
        KeyMetadata {
            key_id,
            key_usage,
            key_type,
            key_spec,
            creation_date: Utc::now(),
            enabled: true,
            version: 1,
            description: None,
            tags: HashMap::new(),
            rotation_config: None,
            expiration_date: None,
        }
    }

    /// 生成随机密钥材料
    fn generate_key_material(spec: KeySpec) -> Vec<u8> {
        let len = spec.key_material_length();
        let mut material = vec![0u8; len];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut material);
        material
    }
}

#[async_trait]
impl KeyManagementService for LocalKms {
    async fn generate_dek(&self, key_id: &str) -> Result<EncryptedKey> {
        let algorithm = EncryptionAlgorithm::Aes256Gcm;
        let dek_material = Self::generate_key_material(KeySpec::Aes256);

        let (encrypted_dek, iv) = self.encrypt_with_master_key(&dek_material)?;

        Ok(EncryptedKey::new(
            format!("dek-{}", uuid::Uuid::new_v4()),
            encrypted_dek,
            iv,
            algorithm,
            key_id.to_string(),
        ))
    }

    async fn encrypt(&self, dek: &EncryptedKey, plaintext: &[u8]) -> Result<Ciphertext> {
        let dek_plaintext = self.decrypt_with_master_key(&dek.ciphertext_blob, &dek.iv)?;

        match dek.algorithm {
            EncryptionAlgorithm::Aes256Gcm => {
                let cipher = Aes256Gcm::new_from_slice(&dek_plaintext)
                    .map_err(|e| helpers::crypto_error(&format!("DEK 初始化失败: {}", e)))?;

                let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
                let ciphertext = cipher
                    .encrypt(&nonce, plaintext)
                    .map_err(|e| helpers::crypto_error(&format!("加密失败: {}", e)))?;

                Ok(Ciphertext::new(
                    ciphertext.to_vec(),
                    dek.key_id.clone(),
                    EncryptionAlgorithm::Aes256Gcm,
                )
                .with_nonce(nonce.to_vec()))
            }
            _ => Err(helpers::unsupported_format(
                &format!("{:?} 算法尚未实现", dek.algorithm),
            )),
        }
    }

    async fn decrypt(&self, dek: &EncryptedKey, ciphertext: &Ciphertext) -> Result<Vec<u8>> {
        let dek_plaintext = self.decrypt_with_master_key(&dek.ciphertext_blob, &dek.iv)?;

        match dek.algorithm {
            EncryptionAlgorithm::Aes256Gcm => {
                let cipher = Aes256Gcm::new_from_slice(&dek_plaintext)
                    .map_err(|e| helpers::crypto_error(&format!("DEK 初始化失败: {}", e)))?;

                if ciphertext.nonce.is_empty() {
                    return Err(helpers::crypto_error(
                        "解密失败：密文缺少 Nonce，可能由旧版本系统生成",
                    ));
                }

                let nonce = Nonce::from_slice(&ciphertext.nonce);
                let plaintext = cipher
                    .decrypt(nonce, ciphertext.data.as_slice())
                    .map_err(|_| helpers::crypto_error("解密失败：数据可能已损坏或 Nonce 不匹配"))?;

                Ok(plaintext)
            }
            _ => Err(helpers::unsupported_format(
                &format!("{:?} 算法尚未实现", dek.algorithm),
            )),
        }
    }

    async fn sign(&self, key_id: &str, data: &[u8]) -> Result<Signature> {
        let key_material = self.decrypt_key_material(key_id).await?;

        let key_pair = Ed25519KeyPair::from_pkcs8_maybe_unchecked(&key_material)
            .map_err(|e| helpers::crypto_error(&format!("无效的 Ed25519 密钥: {}", e)))?;

        let signature = key_pair.sign(data);

        Ok(Signature::new(
            signature.as_ref().to_vec(),
            SignatureAlgorithm::Ed25519,
            key_id.to_string(),
        ))
    }

    async fn verify(&self, key_id: &str, data: &[u8], signature: &Signature) -> Result<bool> {
        let key_material = self.decrypt_key_material(key_id).await?;

        let public_key =
            ring::signature::UnparsedPublicKey::new(
                &ring::signature::ED25519,
                &key_material,
            );

        match public_key.verify(data, &signature.value) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn rotate_key(&self, key_id: &str) -> Result<RotationResult> {
        let mut store = self.key_store.write().await;
        let stored_key = store.get_mut(key_id).ok_or_else(|| {
            helpers::not_found("密钥", key_id)
        })?;

        let new_material = Self::generate_key_material(stored_key.metadata.key_spec);
        let (encrypted_material, iv) = self.encrypt_with_master_key(&new_material)?;

        stored_key.encrypted_material = encrypted_material;
        stored_key.iv = iv;
        stored_key.metadata.version += 1;

        if let Some(ref mut rotation_config) = stored_key.metadata.rotation_config {
            rotation_config.mark_rotated();
        }

        let new_version = stored_key.metadata.version;

        drop(store);
        let stored_key_ref = self.key_store.read().await.get(key_id).cloned();
        if let Some(sk) = stored_key_ref {
            self.save_key_to_disk(&sk).await?;
        }

        Ok(RotationResult {
            key_id: key_id.to_string(),
            new_version,
            rotated_at: Utc::now(),
            pending_deletion_date: None,
            status: RotationStatus::Completed,
        })
    }

    async fn list_keys(&self) -> Result<Vec<KeyMetadata>> {
        let store = self.key_store.read().await;
        let keys = store.values().map(|sk| sk.metadata.clone()).collect();
        Ok(keys)
    }

    async fn describe_key(&self, key_id: &str) -> Result<KeyMetadata> {
        let store = self.key_store.read().await;
        store
            .get(key_id)
            .map(|sk| sk.metadata.clone())
            .ok_or_else(|| helpers::not_found("密钥", key_id))
    }

    async fn destroy_key(&self, key_id: &str) -> Result<()> {
        let mut store = self.key_store.write().await;
        store.remove(key_id);
        drop(store);

        self.delete_key_file(key_id)?;

        tracing::warn!(key_id = %key_id, "密钥已被销毁（不可逆操作）");
        Ok(())
    }

    async fn health_check(&self) -> Result<KmsHealthStatus> {
        let start = std::time::Instant::now();

        let store = self.key_store.read().await;
        let _ = store.len(); 

        let latency = start.elapsed().as_millis() as u64;
        Ok(KmsHealthStatus::healthy(latency))
    }
}

impl LocalKms {
    /// 创建新的对称加密密钥
    pub async fn create_key(
        &self,
        key_id: &str,
        key_spec: KeySpec,
        description: Option<&str>,
    ) -> Result<KeyMetadata> {
        let metadata = Self::create_default_metadata(
            key_id.to_string(),
            KeyUsage::EncryptDecrypt,
            KeyType::Symmetric,
            key_spec,
        );

        let material = Self::generate_key_material(key_spec);
        let (encrypted_material, iv) = self.encrypt_with_master_key(&material)?;

        let stored_key = StoredKey {
            metadata: KeyMetadata {
                description: description.map(String::from),
                ..metadata
            },
            encrypted_material,
            iv,
        };

        {
            let mut store = self.key_store.write().await;
            store.insert(key_id.to_string(), stored_key.clone());
        }

        self.save_key_to_disk(&stored_key).await?;

        Ok(stored_key.metadata)
    }

    /// 创建签名密钥对 (Ed25519)
    pub async fn create_signing_key(
        &self,
        key_id: &str,
        description: Option<&str>,
    ) -> Result<KeyMetadata> {
        let pkcs8_doc = Ed25519KeyPair::generate_pkcs8(&ring_rand::SystemRandom::new())
            .map_err(|e| helpers::crypto_error(&format!("生成 Ed25519 密钥对失败: {}", e)))?;

        let pkcs8_bytes = pkcs8_doc.as_ref().to_vec();
        let (encrypted_material, iv) = self.encrypt_with_master_key(&pkcs8_bytes)?;

        let metadata = Self::create_default_metadata(
            key_id.to_string(),
            KeyUsage::SignVerify,
            KeyType::Eddsa,
            KeySpec::Ed25519,
        );

        let stored_key = StoredKey {
            metadata: KeyMetadata {
                description: description.map(String::from),
                ..metadata
            },
            encrypted_material,
            iv,
        };

        {
            let mut store = self.key_store.write().await;
            store.insert(key_id.to_string(), stored_key.clone());
        }

        self.save_key_to_disk(&stored_key).await?;

        Ok(stored_key.metadata)
    }
}

/// 本地 KMS 配置
#[derive(Debug, Clone)]
pub struct LocalKmsConfig {
    /// 主密钥文件路径
    pub master_key_path: PathBuf,
    /// 密钥存储目录
    pub keys_dir: PathBuf,
}

impl LocalKmsConfig {
    /// 使用默认路径创建配置
    pub fn with_defaults(base_dir: impl AsRef<Path>) -> Self {
        let base = base_dir.as_ref();
        Self {
            master_key_path: base.join(".master-key"),
            keys_dir: base.join("keys"),
        }
    }
}

impl Default for LocalKmsConfig {
    fn default() -> Self {
        Self::with_defaults(dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("knowledge-system")
            .join("kms"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_and_encrypt_decrypt() {
        let dir = tempdir().unwrap();
        let master_key = LocalKms::generate_master_key();
        let kms = LocalKms::new(master_key, dir.path()).unwrap();

        let metadata = kms.create_key("test-key-1", KeySpec::Aes256, Some("测试密钥")).await.unwrap();
        assert_eq!(metadata.key_id, "test-key-1");
        assert_eq!(metadata.key_spec, KeySpec::Aes256);
        assert!(metadata.enabled);

        let dek = kms.generate_dek("test-key-1").await.unwrap();
        let plaintext = b"Hello, LocalKMS!";
        let ciphertext = kms.encrypt(&dek, plaintext).await.unwrap();

        let decrypted = kms.decrypt(&dek, &ciphertext).await.unwrap();
        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[tokio::test]
    async fn test_sign_and_verify() {
        let dir = tempdir().unwrap();
        let master_key = LocalKms::generate_master_key();
        let kms = LocalKms::new(master_key, dir.path()).unwrap();

        kms.create_signing_key("sign-key-1", Some("签名密钥")).await.unwrap();

        let data = b"Data to sign";
        let signature = kms.sign("sign-key-1", data).await.unwrap();

        let verified = kms.verify("sign-key-1", data, &signature).await.unwrap();
        assert!(verified);

        let wrong_data = b"Wrong data";
        let verified_wrong = kms.verify("sign-key-1", wrong_data, &signature).await.unwrap();
        assert!(!verified_wrong);
    }

    #[tokio::test]
    async fn test_list_and_describe_keys() {
        let dir = tempdir().unwrap();
        let master_key = LocalKms::generate_master_key();
        let kms = LocalKms::new(master_key, dir.path()).unwrap();

        kms.create_key("key-a", KeySpec::Aes256, None).await.unwrap();
        kms.create_key("key-b", KeySpec::Aes128, None).await.unwrap();

        let keys = kms.list_keys().await.unwrap();
        assert_eq!(keys.len(), 2);

        let desc = kms.describe_key("key-a").await.unwrap();
        assert_eq!(desc.key_id, "key-a");
        assert_eq!(desc.key_type, KeyType::Symmetric);
    }

    #[tokio::test]
    async fn test_rotate_key() {
        let dir = tempdir().unwrap();
        let master_key = LocalKms::generate_master_key();
        let kms = LocalKms::new(master_key, dir.path()).unwrap();

        kms.create_key("rotate-me", KeySpec::Aes256, None).await.unwrap();

        let result = kms.rotate_key("rotate-me").await.unwrap();
        assert_eq!(result.status, RotationStatus::Completed);
        assert_eq!(result.new_version, 2);

        let desc = kms.describe_key("rotate-me").await.unwrap();
        assert_eq!(desc.version, 2);
    }

    #[tokio::test]
    async fn test_destroy_key() {
        let dir = tempdir().unwrap();
        let master_key = LocalKms::generate_master_key();
        let kms = LocalKms::new(master_key, dir.path()).unwrap();

        kms.create_key("to-delete", KeySpec::Aes256, None).await.unwrap();
        assert!(kms.list_keys().await.unwrap().len() == 1);

        kms.destroy_key("to-delete").await.unwrap();
        assert!(kms.list_keys().await.unwrap().is_empty());

        let err = kms.describe_key("to-delete").await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_health_check() {
        let dir = tempdir().unwrap();
        let master_key = LocalKms::generate_master_key();
        let kms = LocalKms::new(master_key, dir.path()).unwrap();

        let health = kms.health_check().await.unwrap();
        assert!(health.healthy);
        assert!(health.latency_ms < 1000);
    }

    #[tokio::test]
    async fn test_persistence() {
        let dir = tempdir().unwrap();
        let master_key = LocalKms::generate_master_key();

        {
            let kms = LocalKms::new(master_key, dir.path()).unwrap();
            kms.create_key("persisted-key", KeySpec::Aes256, None).await.unwrap();
        }

        let kms2 = LocalKms::new(master_key, dir.path()).unwrap();
        kms2.load_keys_from_disk().await.unwrap();

        let keys = kms2.list_keys().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key_id, "persisted-key");
    }

    #[tokio::test]
    async fn test_encryption_context_in_ciphertext() {
        let dir = tempdir().unwrap();
        let master_key = LocalKms::generate_master_key();
        let kms = LocalKms::new(master_key, dir.path()).unwrap();

        kms.create_key("ctx-key", KeySpec::Aes256, None).await.unwrap();
        let dek = kms.generate_dek("ctx-key").await.unwrap();

        let mut ctx = HashMap::new();
        ctx.insert("purpose".to_string(), "test".to_string());

        let ct = Ciphertext::new(b"test".to_vec(), dek.key_id.clone(), EncryptionAlgorithm::Aes256Gcm)
            .with_context(ctx);

        assert!(ct.context.is_some());
        assert_eq!(ct.context.unwrap().context.get("purpose").unwrap(), "test");
    }

    #[proptest::proptest]
    fn test_encrypt_decrypt_roundtrip(#[proptest::strategy("0..1000usize")] size: usize) {
        use std::hint::black_box;

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let dir = tempdir().unwrap();
            let master_key = LocalKms::generate_master_key();
            let kms = LocalKms::new(master_key, dir.path()).unwrap();

            kms.create_key("fuzz-key", KeySpec::Aes256, None).await.unwrap();
            let dek = kms.generate_dek("fuzz-key").await.unwrap();

            let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
            let black_box(black_box(&data));

            let ct = kms.encrypt(&dek, &data).await.unwrap();
            let decrypted = kms.decrypt(&dek, &ct).await.unwrap();
            assert_eq!(data, decrypted);
        });
    }
}
