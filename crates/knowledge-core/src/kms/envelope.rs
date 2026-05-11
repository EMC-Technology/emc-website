use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::Result;
use crate::error::helpers;
use crate::kms::traits::{DecryptedDek, EncryptionContext, EnvelopedData, KeyManagementService};

/// 信封加密 (Envelope Encryption) 实现
///
/// 信封加密是一种混合加密模式，结合了对称加密的高效性和非对称/托管密钥的安全性：
///
/// 1. **生成 DEK**：使用 KMS 生成数据加密密钥 (Data Encryption Key)
/// 2. **本地加密**：使用 DEK 对数据进行快速对称加密
/// 3. **存储**：将加密后的数据和加密的 DEK 一起存储
///
/// # 架构优势
///
/// - **性能**：大数据量使用本地对称加密，避免 KMS 瓶颈
/// - **安全**：DEK 由 KMS 保护，无需在应用层管理长期密钥
/// - **灵活**：支持多种 KMS 后端 (AWS, Vault, Local)
///
/// # 使用示例
///
/// ```ignore
/// let envelope = EnvelopeEncryption::new(kms, 100);
/// let encrypted = envelope.encrypt_data(b"sensitive data", &context).await?;
/// let decrypted = envelope.decrypt_data(&encrypted).await?;
/// ```
pub struct EnvelopeEncryption<KMS: KeyManagementService> {
    /// KMS 客户端
    kms: Arc<KMS>,
    /// DEK 缓存（减少 KMS 调用）
    dek_cache: Arc<tokio::sync::RwLock<LruCache<String, DecryptedDek>>>,
}

impl<KMS: KeyManagementService> EnvelopeEncryption<KMS> {
    /// 创建新的信封加密实例
    ///
    /// # 参数
    ///
    /// - `kms`: KMS 服务实例
    /// - `cache_size`: 最大缓存条目数
    pub fn new(kms: KMS, cache_size: usize) -> Self {
        let cache_size =
            NonZeroUsize::new(cache_size.max(1)).expect("1 is non-zero; max(1) guarantees this");
        Self {
            kms: Arc::new(kms),
            dek_cache: Arc::new(tokio::sync::RwLock::new(LruCache::new(cache_size))),
        }
    }

    /// 从 Arc<KMS> 创建实例
    pub fn from_arc(kms: Arc<KMS>, cache_size: usize) -> Self {
        let cache_size =
            NonZeroUsize::new(cache_size.max(1)).expect("1 is non-zero; max(1) guarantees this");
        Self {
            kms,
            dek_cache: Arc::new(tokio::sync::RwLock::new(LruCache::new(cache_size))),
        }
    }

    /// 加密数据（自动管理 DEK 生命周期）
    ///
    /// # 参数
    ///
    /// - `data`: 待加密的明文数据
    /// - `context`: 加密上下文（可选，用于审计和访问控制）
    ///
    /// # 返回
    ///
    /// 返回包含密文和加密 DEK 的 EnvelopedData 结构。
    pub async fn encrypt_data(
        &self,
        data: &[u8],
        context: &EncryptionContext,
    ) -> Result<EnvelopedData> {
        let kek_id = context
            .context
            .get("kek_id")
            .map_or("default", String::as_str);

        let encrypted_dek = self.kms.generate_dek(kek_id).await?;

        {
            let mut cache = self.dek_cache.write().await;
            let dek_plaintext = self.kms.decrypt_dek(&encrypted_dek).await?;
            let cache_key = encrypted_dek.key_id.clone();
            cache.put(
                cache_key,
                DecryptedDek {
                    plaintext_key: dek_plaintext,
                    algorithm: encrypted_dek.algorithm,
                    decrypted_at: chrono::Utc::now(),
                    source_key_id: encrypted_dek.key_id.clone(),
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
                },
            );
        }

        let ciphertext = self.kms.encrypt(&encrypted_dek, data).await?;

        Ok(EnvelopedData {
            ciphertext,
            encrypted_dek,
            context: Some(context.clone()),
            version: 1,
        })
    }

    /// 解密数据
    ///
    /// # 参数
    ///
    /// - `enveloped`: 包含密文和加密 DEK 的结构体
    ///
    /// # 返回
    ///
    /// 解密后的明文数据。
    pub async fn decrypt_data(&self, enveloped: &EnvelopedData) -> Result<Vec<u8>> {
        let key_id = &enveloped.encrypted_dek.key_id;

        {
            let cache = self.dek_cache.read().await;
            if let Some(cached_dek) = cache.peek(key_id)
                && chrono::Utc::now() < cached_dek.expires_at
            {
                let cipher = Aes256Gcm::new_from_slice(&cached_dek.plaintext_key)
                    .map_err(|e| helpers::crypto_error(&format!("DEK 初始化失败: {e}")))?;

                if enveloped.ciphertext.nonce.is_empty() {
                    return Err(helpers::crypto_error("解密失败：密文缺少 Nonce"));
                }

                let nonce = Nonce::from_slice(&enveloped.ciphertext.nonce);
                let plaintext = cipher
                    .decrypt(nonce, enveloped.ciphertext.data.as_slice())
                    .map_err(|_| helpers::crypto_error("解密失败：密文可能已损坏"))?;

                return Ok(plaintext);
            }
        }

        self.kms
            .decrypt(&enveloped.encrypted_dek, &enveloped.ciphertext)
            .await
    }

    /// 批量加密多条数据（共享同一 DEK）
    ///
    /// 当需要加密多个相关数据时，此方法可以减少 DEK 生成次数。
    pub async fn encrypt_batch(
        &self,
        data_list: &[&[u8]],
        context: &EncryptionContext,
    ) -> Result<Vec<EnvelopedData>> {
        let mut results = Vec::with_capacity(data_list.len());

        for data in data_list {
            let enveloped = self.encrypt_data(data, context).await?;
            results.push(enveloped);
        }

        Ok(results)
    }

    /// 批量解密
    pub async fn decrypt_batch(&self, enveloped_list: &[EnvelopedData]) -> Result<Vec<Vec<u8>>> {
        let mut results = Vec::with_capacity(enveloped_list.len());

        for enveloped in enveloped_list {
            let plaintext = self.decrypt_data(enveloped).await?;
            results.push(plaintext);
        }

        Ok(results)
    }

    /// 清除缓存中的所有 DEK
    pub async fn clear_cache(&self) {
        let mut cache: tokio::sync::RwLockWriteGuard<'_, LruCache<String, DecryptedDek>> =
            self.dek_cache.write().await;
        cache.clear();
    }

    /// 获取当前缓存大小
    pub async fn cache_size(&self) -> usize {
        let cache: tokio::sync::RwLockReadGuard<'_, LruCache<String, DecryptedDek>> =
            self.dek_cache.read().await;
        cache.len()
    }

    /// 获取 KMS 引用
    pub fn kms(&self) -> &Arc<KMS> {
        &self.kms
    }
}

/// 信封加密配置
#[derive(Debug, Clone)]
pub struct EnvelopeConfig {
    /// DEK 缓存 TTL（秒）
    pub cache_ttl_secs: u64,
    /// 最大缓存条目数
    pub max_cache_size: usize,
    /// 默认 KEK ID
    pub default_kek_id: String,
}

impl Default for EnvelopeConfig {
    fn default() -> Self {
        Self {
            cache_ttl_secs: 300,
            max_cache_size: 100,
            default_kek_id: "default".to_string(),
        }
    }
}

/// 分块加密处理器
///
/// 用于处理大型数据的分块加密，每块独立加密但共享同一 DEK。
pub struct ChunkedEncryptor<KMS: KeyManagementService> {
    envelope: EnvelopeEncryption<KMS>,
    chunk_size: usize,
}

impl<KMS: KeyManagementService> ChunkedEncryptor<KMS> {
    /// 创建分块加密器
    pub fn new(envelope: EnvelopeEncryption<KMS>, chunk_size: usize) -> Self {
        Self {
            envelope,
            chunk_size,
        }
    }

    /// 加密大型数据（自动分块）
    pub async fn encrypt_large(
        &self,
        data: &[u8],
        context: &EncryptionContext,
    ) -> Result<ChunkedEncryptedData> {
        if data.len() <= self.chunk_size {
            let enveloped = self.envelope.encrypt_data(data, context).await?;
            return Ok(ChunkedEncryptedData {
                chunks: vec![enveloped],
                original_size: data.len(),
                chunk_count: 1,
            });
        }

        let chunks: Vec<&[u8]> = data.chunks(self.chunk_size).collect();
        let mut encrypted_chunks = Vec::with_capacity(chunks.len());

        for chunk in chunks {
            let enveloped = self.envelope.encrypt_data(chunk, context).await?;
            encrypted_chunks.push(enveloped);
        }

        let chunk_count = encrypted_chunks.len();
        Ok(ChunkedEncryptedData {
            chunks: encrypted_chunks,
            original_size: data.len(),
            chunk_count,
        })
    }

    /// 解密分块数据
    pub async fn decrypt_large(&self, encrypted: &ChunkedEncryptedData) -> Result<Vec<u8>> {
        let mut result = Vec::with_capacity(encrypted.original_size);

        for chunk in &encrypted.chunks {
            let plaintext = self.envelope.decrypt_data(chunk).await?;
            result.extend_from_slice(&plaintext);
        }

        Ok(result)
    }
}

/// 分块加密的数据结构
#[derive(Debug, Clone)]
pub struct ChunkedEncryptedData {
    /// 加密的分块列表
    pub chunks: Vec<EnvelopedData>,
    /// 原始数据大小
    pub original_size: usize,
    /// 分块数量
    pub chunk_count: usize,
}

/// 数据加密工具函数集合
pub mod utils {
    use super::EnvelopeEncryption;
    use crate::Result;
    use crate::error::helpers;
    use crate::kms::traits::{EncryptionContext, EnvelopedData, KeyManagementService};
    use base64::Engine;

    /// 快速加密字符串为 Base64 编码的 EnvelopedData JSON
    pub async fn encrypt_to_base64<KMS: KeyManagementService>(
        envelope: &EnvelopeEncryption<KMS>,
        plaintext: &str,
        context: &EncryptionContext,
    ) -> Result<String> {
        let enveloped = envelope.encrypt_data(plaintext.as_bytes(), context).await?;
        let json = serde_json::to_string(&enveloped)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(json.as_bytes()))
    }

    /// 从 Base64 解码并解密
    pub async fn decrypt_from_base64<KMS: KeyManagementService>(
        envelope: &EnvelopeEncryption<KMS>,
        encoded: &str,
    ) -> Result<String> {
        let json_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| {
                crate::error::helpers::validation_error(
                    &format!("Base64 解码失败: {e}"),
                    "decrypt_from_base64",
                )
            })?;

        let json_str = String::from_utf8(json_bytes)
            .map_err(|e| helpers::serde_error(&format!("UTF-8 解码失败: {e}")))?;

        let enveloped: EnvelopedData = serde_json::from_str(&json_str)
            .map_err(|e| helpers::serde_error(&format!("JSON 反序列化失败: {e}")))?;

        let plaintext = envelope.decrypt_data(&enveloped).await?;
        String::from_utf8(plaintext)
            .map_err(|e| helpers::serde_error(&format!("UTF-8 解码失败: {e}")))
    }

    /// 检查 EnvelopedData 是否有效
    pub fn is_valid_enveloped_data(data: &EnvelopedData) -> bool {
        !data.ciphertext.data.is_empty() && !data.encrypted_dek.ciphertext_blob.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{ChunkedEncryptor, EnvelopeConfig, EnvelopeEncryption, utils};
    use crate::kms::local::LocalKms;
    use crate::kms::traits::EncryptionContext;
    use base64::Engine;
    use tempfile::TempDir;

    async fn create_test_kms() -> (LocalKms, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let master_key = crate::kms::local::LocalKms::generate_master_key();
        let kms = LocalKms::new(master_key, dir.path()).unwrap();
        kms.create_key("test-kek", crate::kms::traits::KeySpec::Aes256, None)
            .await
            .unwrap();
        (kms, dir)
    }

    #[tokio::test]
    async fn test_encrypt_decrypt_roundtrip() {
        let (kms, _dir) = create_test_kms().await;
        let envelope = EnvelopeEncryption::new(kms, 10);

        let context = EncryptionContext::empty().add("purpose", "test");
        let plaintext = b"Hello, Envelope Encryption!";

        let encrypted = envelope.encrypt_data(plaintext, &context).await.unwrap();
        assert!(utils::is_valid_enveloped_data(&encrypted));

        let decrypted = envelope.decrypt_data(&encrypted).await.unwrap();
        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[tokio::test]
    async fn test_encrypt_with_context() {
        let (kms, _dir) = create_test_kms().await;
        let envelope = EnvelopeEncryption::new(kms, 10);

        let context = EncryptionContext::empty()
            .add("department", "engineering")
            .add("project", "kms")
            .add("kek_id", "test-kek");

        let encrypted = envelope
            .encrypt_data(b"secret data", &context)
            .await
            .unwrap();

        assert!(encrypted.context.is_some());
        let ctx = encrypted.context.unwrap();
        assert_eq!(ctx.context.get("department").unwrap(), "engineering");
    }

    #[tokio::test]
    async fn test_batch_encryption() {
        let (kms, _dir) = create_test_kms().await;
        let envelope = EnvelopeEncryption::new(kms, 10);

        let data_list: Vec<&[u8]> = vec![b"data1", b"data2", b"data3"];
        let context = EncryptionContext::empty();

        let encrypted_list = envelope.encrypt_batch(&data_list, &context).await.unwrap();
        assert_eq!(encrypted_list.len(), 3);

        let decrypted_list = envelope.decrypt_batch(&encrypted_list).await.unwrap();
        for (i, decrypted) in decrypted_list.iter().enumerate() {
            assert_eq!(*decrypted, data_list[i]);
        }
    }

    #[tokio::test]
    async fn test_chunked_encryption() {
        let (kms, _dir) = create_test_kms().await;
        let envelope = EnvelopeEncryption::new(kms, 10);
        let chunked = ChunkedEncryptor::new(envelope, 10);

        let large_data: Vec<u8> = (0..100u8).collect();
        let context = EncryptionContext::empty();

        let encrypted = chunked.encrypt_large(&large_data, &context).await.unwrap();
        assert!(encrypted.chunk_count > 1);
        assert_eq!(encrypted.original_size, 100);

        let decrypted = chunked.decrypt_large(&encrypted).await.unwrap();
        assert_eq!(large_data, decrypted);
    }

    #[tokio::test]
    async fn test_base64_roundtrip() {
        let (kms, _dir) = create_test_kms().await;
        let envelope = EnvelopeEncryption::new(kms, 10);
        let context = EncryptionContext::empty();

        let encoded = utils::encrypt_to_base64(&envelope, "test message", &context)
            .await
            .unwrap();

        assert!(!encoded.is_empty());

        let decoded = utils::decrypt_from_base64(&envelope, &encoded)
            .await
            .unwrap();

        assert_eq!(decoded, "test message");
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let (kms, _dir) = create_test_kms().await;
        let envelope = EnvelopeEncryption::new(kms, 10);
        let context = EncryptionContext::empty();

        envelope.encrypt_data(b"cache me", &context).await.unwrap();

        let size_before = envelope.cache_size().await;
        assert!(size_before > 0);

        envelope.clear_cache().await;
        let size_after = envelope.cache_size().await;
        assert_eq!(size_after, 0);
    }

    #[tokio::test]
    async fn test_envelope_config_default() {
        let config = EnvelopeConfig::default();
        assert_eq!(config.cache_ttl_secs, 300);
        assert_eq!(config.max_cache_size, 100);
        assert_eq!(config.default_kek_id, "default");
    }

    #[tokio::test]
    async fn test_from_arc_creation() {
        let (kms, _dir) = create_test_kms().await;
        let kms_arc = std::sync::Arc::new(kms);
        let envelope = EnvelopeEncryption::from_arc(kms_arc.clone(), 10);
        assert!(std::sync::Arc::ptr_eq(envelope.kms(), &kms_arc));
    }

    #[tokio::test]
    async fn test_kms_reference() {
        let (kms, _dir) = create_test_kms().await;
        let envelope = EnvelopeEncryption::new(kms, 10);
        let _kms_ref = envelope.kms();
    }

    #[tokio::test]
    async fn test_chunked_encryption_single_chunk() {
        let (kms, _dir) = create_test_kms().await;
        let envelope = EnvelopeEncryption::new(kms, 10);
        let chunked = ChunkedEncryptor::new(envelope, 1000);

        let small_data = b"small data";
        let context = EncryptionContext::empty();

        let encrypted = chunked.encrypt_large(small_data, &context).await.unwrap();
        assert_eq!(encrypted.chunk_count, 1);
        assert_eq!(encrypted.original_size, small_data.len());

        let decrypted = chunked.decrypt_large(&encrypted).await.unwrap();
        assert_eq!(small_data, decrypted.as_slice());
    }

    #[tokio::test]
    async fn test_decrypt_from_base64_invalid_input() {
        let (kms, _dir) = create_test_kms().await;
        let envelope = EnvelopeEncryption::new(kms, 10);

        let result = utils::decrypt_from_base64(&envelope, "!!!not-base64!!!").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_decrypt_from_base64_invalid_utf8_json() {
        let (kms, _dir) = create_test_kms().await;
        let envelope = EnvelopeEncryption::new(kms, 10);

        let invalid_json = base64::engine::general_purpose::STANDARD.encode(b"\xff\xfe".as_slice());
        let result = utils::decrypt_from_base64(&envelope, &invalid_json).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_decrypt_from_base64_invalid_json_structure() {
        let (kms, _dir) = create_test_kms().await;
        let envelope = EnvelopeEncryption::new(kms, 10);

        let valid_json_but_wrong_structure = base64::engine::general_purpose::STANDARD
            .encode(r#"{"not":"valid_enveloped_data"}"#.as_bytes());
        let result = utils::decrypt_from_base64(&envelope, &valid_json_but_wrong_structure).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_valid_enveloped_data_empty() {
        use crate::kms::traits::{Ciphertext, EncryptedKey, EncryptionAlgorithm, EnvelopedData};

        let empty_data = EnvelopedData {
            ciphertext: Ciphertext {
                data: vec![],
                nonce: vec![0u8; 12],
                auth_tag: None,
                dek_id: "test".to_string(),
                encrypted_at: chrono::Utc::now(),
                algorithm: EncryptionAlgorithm::Aes256Gcm,
                context: None,
            },
            encrypted_dek: EncryptedKey {
                key_id: "test".to_string(),
                ciphertext_blob: vec![],
                iv: vec![0u8; 12],
                algorithm: EncryptionAlgorithm::Aes256Gcm,
                created_at: chrono::Utc::now(),
                encrypted_with: "kek".to_string(),
            },
            context: None,
            version: 1,
        };
        assert!(!utils::is_valid_enveloped_data(&empty_data));
    }
}
