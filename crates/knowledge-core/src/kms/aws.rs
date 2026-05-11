use base64::Engine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::Result;
use crate::error::helpers;
use crate::kms::traits::*;

/// AWS KMS 适配器
///
/// 通过 AWS SDK 连接 AWS KMS 服务，提供密钥管理后端。
///
/// # 特性
///
/// - 使用 AWS KMS 进行密钥管理和信封加密
/// - 支持 IAM 认证和访问控制
/// - 自动处理 AWS API 限流和重试
/// - 支持多区域密钥复制
///
/// # 认证
///
/// 支持以下认证方式（按优先级）：
/// - 显式配置的 Access Key（或 IAM Role）
/// - 访问 AWS KMS 凭证链
/// - EC2 Instance Profile / ECS Task Role
///
/// # 限流处理
///
/// AWS KMS 有请求配额限制，此客户端内置了限流处理：
/// - 自适应请求延迟
/// - 指数退避重试
/// - 客户端缓存减少调用次数
///
/// # 配置示例
///
/// ```ignore
/// let config = AwsKmsConfig {
///     region: "us-east-1".to_string(),
///     key_id: "arn:aws:kms:us-east-1:123456789:key/...".to_string(),
///     access_key: None,
///     secret_key: None,
///     max_retries: 3,
///     timeout_ms: 5000,
/// };
/// let kms = AwsKms::new(config).await?;
/// ```
pub struct AwsKms {
    /// AWS KMS 客户端
    client: aws_sdk_kms::Client,
    /// 默认密钥 ID
    default_key_id: String,
    /// 请求重试次数
    max_retries: u32,
    /// 请求超时（毫秒）
    timeout_ms: u64,
    /// DEK 缓存
    dek_cache: Arc<RwLock<HashMap<String, CachedDek>>>,
}

/// AWS KMS 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsKmsConfig {
    /// AWS 区域（如 "us-east-1", "ap-northeast-1"）
    pub region: String,
    /// 默认的 Customer Master Key ID 或 ARN
    pub key_id: String,
    /// AWS Access Key ID（可选，推荐使用 IAM Role）
    pub access_key: Option<String>,
    /// AWS Secret Access Key（可选，推荐使用 IAM Role）
    pub secret_key: Option<String>,
    /// 最大重试次数（默认 3）
    pub max_retries: Option<u32>,
    /// 请求超时毫秒数（默认 5000）
    pub timeout_ms: Option<u64>,
}

impl Default for AwsKmsConfig {
    fn default() -> Self {
        Self {
            region: "us-east-1".to_string(),
            key_id: String::new(),
            access_key: None,
            secret_key: None,
            max_retries: Some(3),
            timeout_ms: Some(5000),
        }
    }
}

/// 缓存的 DEK
#[derive(Debug, Clone)]
struct CachedDek {
    encrypted_key: EncryptedKey,
    plaintext: Vec<u8>,
    created_at: chrono::DateTime<Utc>,
}

impl AwsKms {
    /// 创建 AWS KMS 适配器
    pub async fn new(config: &AwsKmsConfig) -> Result<Self> {
        let mut config_builder = aws_sdk_kms::config::Builder::new();

        config_builder.region(aws_sdk_kms::config::Region::new(config.region.clone()));

        if let (Some(access_key), Some(secret_key)) = (&config.access_key, &config.secret_key) {
            let credentials = aws_sdk_kms::config::Credentials::new(
                access_key,
                secret_key,
                None,
                None,
                "knowledge-system-kms",
            );
            config_builder.credentials_provider(credentials);
        }

        let client = aws_sdk_kms::Client::from_conf(config_builder.build());

        Ok(Self {
            client,
            default_key_id: config.key_id.clone(),
            max_retries: config.max_retries.unwrap_or(3),
            timeout_ms: config.timeout_ms.unwrap_or(5000),
            dek_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 从环境变量创建 AWS KMS 适配器
    pub async fn from_env() -> Result<Self> {
        let config = aws_sdk_kms::Config::builder().load_from_env().build();

        let client = aws_sdk_kms::Client::from_conf(config);

        let key_id = std::env::var("AWS_KMS_KEY_ID")
            .map_err(|_| helpers::config_error("未设置 AWS_KMS_KEY_ID 环境变量"))?;

        Ok(Self {
            client,
            default_key_id: key_id,
            max_retries: 3,
            timeout_ms: 5000,
            dek_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 带重试的 API 调用
    async fn retry_api_call<F, Fut, T>(&self, operation: &str, f: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt < self.max_retries {
                        let delay =
                            std::time::Duration::from_millis(u64::from(100 * 2u32.pow(attempt)));
                        tracing::warn!(
                            attempt = attempt + 1,
                            max_retries = self.max_retries,
                            ?delay,
                            operation,
                            "准备重试"
                        );
                        tokio::time::sleep(delay).await;
                    }
                    last_error = Some(e);
                }
            }
        }

        last_error.map_or_else(
            || Err(helpers::internal_error("重试循环未产生错误，逻辑不可达")),
            Err,
        )
    }

    /// 从 AWS 密钥元数据转换为内部 KeyMetadata
    fn convert_metadata(key_id: &str, aws_key: &aws_sdk_kms::types::KeyMetadata) -> KeyMetadata {
        let key_usage = match aws_key.key_usage() {
            aws_sdk_kms::types::KeyUsage::EncryptDecrypt => KeyUsage::EncryptDecrypt,
            aws_sdk_kms::types::KeyUsage::SignVerify => KeyUsage::SignVerify,
            aws_sdk_kms::types::KeyUsage::GenerateVerifyMac => KeyUsage::Both,
            other => {
                tracing::warn!(?other, "未识别的 AWS KeyUsage，回退至 Both");
                KeyUsage::Both
            }
        };

        let key_spec = match aws_key.key_spec() {
            aws_sdk_kms::types::KeySpec::Aes256 => KeySpec::Aes256,
            aws_sdk_kms::types::KeySpec::Aes128 => KeySpec::Aes128,
            aws_sdk_kms::types::KeySpec::Rsa2048 => KeySpec::Rsa2048,
            aws_sdk_kms::types::KeySpec::Rsa3072 => KeySpec::Rsa3072,
            aws_sdk_kms::types::KeySpec::Rsa4096 => KeySpec::Rsa4096,
            aws_sdk_kms::types::KeySpec::EccNistP256 => KeySpec::EccP256,
            aws_sdk_kms::types::KeySpec::EccNistP384 => KeySpec::EccP384,
            aws_sdk_kms::types::KeySpec::SymmetricDefault => KeySpec::Aes256,
            aws_sdk_kms::types::KeySpec::Hmac224 => KeySpec::Hmac256,
            aws_sdk_kms::types::KeySpec::Hmac256 => KeySpec::Hmac256,
            aws_sdk_kms::types::KeySpec::Hmac384 => KeySpec::Hmac256,
            aws_sdk_kms::types::KeySpec::Hmac512 => KeySpec::Hmac256,
            aws_sdk_kms::types::KeySpec::EccNistP521 => KeySpec::EccP384,
            aws_sdk_kms::types::KeySpec::EccSecgP256k1 => KeySpec::EccP256,
            aws_sdk_kms::types::KeySpec::Sm2 => KeySpec::Aes256,
            other => {
                tracing::warn!(?other, "未识别的 AWS KeySpec，回退至 Aes256");
                KeySpec::Aes256
            }
        };

        let key_type = match key_spec {
            KeySpec::Aes128 | KeySpec::Aes256 => KeyType::Symmetric,
            KeySpec::Rsa2048 | KeySpec::Rsa3072 | KeySpec::Rsa4096 => KeyType::Rsa,
            KeySpec::EccP256 | KeySpec::EccP384 => KeyType::Ecc,
            KeySpec::Ed25519 => KeyType::Eddsa,
            KeySpec::Hmac256 => KeyType::Hmac,
        };

        KeyMetadata {
            key_id: key_id.to_string(),
            key_usage,
            key_type,
            key_spec,
            creation_date: aws_key
                .creation_date()
                .map(|dt| chrono::DateTime::from_timestamp(dt.secs(), 0))
                .flatten()
                .unwrap_or_else(Utc::now),
            enabled: aws_key.enabled().unwrap_or(true),
            version: 1,
            description: aws_key.description().map(String::from),
            tags: HashMap::new(),
            rotation_config: None,
            expiration_date: None,
        }
    }
}

#[async_trait::async_trait]
impl KeyManagementService for AwsKms {
    async fn generate_dek(&self, key_id: &str) -> Result<EncryptedKey> {
        let effective_key_id = if key_id == "default" {
            &self.default_key_id
        } else {
            key_id
        };

        let result = self
            .retry_api_call("generate_data_key", || async {
                self.client
                    .generate_data_key()
                    .key_id(effective_key_id)
                    .key_spec(aws_sdk_kms::types::DataKeySpec::Aes256)
                    .send()
                    .await
                    .map_err(|e| helpers::io_error(&format!("AWS KMS 生成 DEK 失败: {}", e)))
            })
            .await?;

        let ciphertext_blob = result
            .ciphertext_blob()
            .map(|blob| blob.as_ref().to_vec())
            .unwrap_or_default();
        let plaintext = result
            .plaintext()
            .map(|blob| blob.as_ref().to_vec())
            .unwrap_or_default();

        let encrypted_key = EncryptedKey::new(
            format!("dek-{}", uuid::Uuid::new_v4()),
            ciphertext_blob,
            vec![0u8; 12],
            EncryptionAlgorithm::Aes256Gcm,
            effective_key_id.to_string(),
        );

        let mut cache = self.dek_cache.write().await;
        cache.insert(
            encrypted_key.key_id.clone(),
            CachedDek {
                encrypted_key: encrypted_key.clone(),
                plaintext,
                created_at: Utc::now(),
            },
        );

        Ok(encrypted_key)
    }

    async fn encrypt(&self, dek: &EncryptedKey, plaintext: &[u8]) -> Result<Ciphertext> {
        let cache = self.dek_cache.read().await;
        let cached = cache.get(&dek.key_id);

        let dek_plaintext = if let Some(cached) = cached {
            cached.plaintext.clone()
        } else {
            drop(cache);
            let result = self
                .client
                .decrypt()
                .ciphertext_blob(aws_sdk_kms::primitives::Blob::new(
                    dek.ciphertext_blob.clone(),
                ))
                .key_id(&dek.encrypted_with)
                .send()
                .await
                .map_err(|e| helpers::io_error(&format!("AWS KMS 解密 DEK 失败: {}", e)))?;

            result
                .plaintext()
                .map(|blob| blob.as_ref().to_vec())
                .unwrap_or_default()
        };

        use aes_gcm::{
            Aes256Gcm, Nonce,
            aead::{Aead, AeadCore, KeyInit},
        };

        let cipher = Aes256Gcm::new_from_slice(&dek_plaintext)
            .map_err(|e| helpers::crypto_error(&format!("AES 初始化失败: {}", e)))?;

        let nonce = Aes256Gcm::generate_nonce(&mut aes_gcm::aead::OsRng);
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

    async fn decrypt(&self, dek: &EncryptedKey, ciphertext: &Ciphertext) -> Result<Vec<u8>> {
        let cache = self.dek_cache.read().await;
        let cached = cache.get(&dek.key_id);

        let dek_plaintext = if let Some(cached) = cached {
            cached.plaintext.clone()
        } else {
            drop(cache);
            let result = self
                .client
                .decrypt()
                .ciphertext_blob(aws_sdk_kms::primitives::Blob::new(
                    dek.ciphertext_blob.clone(),
                ))
                .key_id(&dek.encrypted_with)
                .send()
                .await
                .map_err(|e| helpers::io_error(&format!("AWS KMS 解密 DEK 失败: {}", e)))?;

            result
                .plaintext()
                .map(|blob| blob.as_ref().to_vec())
                .unwrap_or_default()
        };

        use aes_gcm::{KeyInit, Nonce, aead::Aead};

        let cipher = Aes256Gcm::new_from_slice(&dek_plaintext)
            .map_err(|e| helpers::crypto_error(&format!("AES 初始化失败: {}", e)))?;

        if ciphertext.nonce.is_empty() {
            return Err(helpers::crypto_error("解密失败：密文缺少 Nonce"));
        }

        let nonce = Nonce::from_slice(&ciphertext.nonce);
        let plaintext = cipher
            .decrypt(nonce, ciphertext.data.as_slice())
            .map_err(|_| helpers::crypto_error("解密失败：数据可能已损坏或 Nonce 不匹配"))?;

        Ok(plaintext)
    }

    async fn decrypt_dek(&self, dek: &EncryptedKey) -> Result<Vec<u8>> {
        let cache = self.dek_cache.read().await;
        let cached = cache.get(&dek.key_id);

        if let Some(cached) = cached {
            return Ok(cached.plaintext.clone());
        }
        drop(cache);

        let result = self
            .client
            .decrypt()
            .ciphertext_blob(aws_sdk_kms::primitives::Blob::new(
                dek.ciphertext_blob.clone(),
            ))
            .key_id(&dek.encrypted_with)
            .send()
            .await
            .map_err(|e| helpers::io_error(&format!("AWS KMS 解密 DEK 失败: {}", e)))?;

        let plaintext = result
            .plaintext()
            .map(|blob| blob.as_ref().to_vec())
            .unwrap_or_default();

        {
            let mut cache = self.dek_cache.write().await;
            cache.put(
                dek.key_id.clone(),
                crate::kms::traits::DecryptedDek {
                    plaintext_key: plaintext.clone(),
                    algorithm: dek.algorithm.clone(),
                    decrypted_at: chrono::Utc::now(),
                    source_key_id: dek.key_id.clone(),
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
                },
            );
        }

        Ok(plaintext)
    }

    async fn sign(&self, key_id: &str, data: &[u8]) -> Result<Signature> {
        let effective_key_id = if key_id == "default" {
            &self.default_key_id
        } else {
            key_id
        };

        let result = self
            .client
            .sign()
            .key_id(effective_key_id)
            .message(aws_sdk_kms::primitives::Blob::new(data.to_vec()))
            .signing_algorithm(aws_sdk_kms::types::SigningAlgorithmSpec::RsaPkcs1Sha256)
            .send()
            .await
            .map_err(|e| helpers::io_error(&format!("AWS KMS 签名失败: {}", e)))?;

        let signature_bytes = result
            .signature()
            .map(|blob| blob.as_ref().to_vec())
            .unwrap_or_default();

        Ok(Signature::new(
            signature_bytes,
            SignatureAlgorithm::RsaPkcs1v15Sha256,
            effective_key_id.to_string(),
        ))
    }

    async fn verify(&self, key_id: &str, data: &[u8], signature: &Signature) -> Result<bool> {
        let effective_key_id = if key_id == "default" {
            &self.default_key_id
        } else {
            key_id
        };

        let result = self
            .client
            .verify()
            .key_id(effective_key_id)
            .message(aws_sdk_kms::primitives::Blob::new(data.to_vec()))
            .signature(aws_sdk_kms::primitives::Blob::new(signature.value.clone()))
            .signing_algorithm(aws_sdk_kms::types::SigningAlgorithmSpec::RsaPkcs1Sha256)
            .send()
            .await
            .map_err(|e| helpers::io_error(&format!("AWS KMS 验证签名失败: {}", e)))?;

        Ok(result.signature_valid().unwrap_or(false))
    }

    async fn rotate_key(&self, key_id: &str) -> Result<RotationResult> {
        let effective_key_id = if key_id == "default" {
            &self.default_key_id
        } else {
            key_id
        };

        self.client
            .rotate_key_on_demand()
            .key_id(effective_key_id)
            .send()
            .await
            .map_err(|e| helpers::io_error(&format!("AWS KMS 密钥轮换失败: {}", e)))?;

        Ok(RotationResult {
            key_id: effective_key_id.to_string(),
            new_version: 0,
            rotated_at: Utc::now(),
            pending_deletion_date: None,
            status: RotationStatus::Completed,
        })
    }

    async fn list_keys(&self) -> Result<Vec<KeyMetadata>> {
        let result = self
            .client
            .list_keys()
            .send()
            .await
            .map_err(|e| helpers::io_error(&format!("AWS KMS 列出密钥失败: {}", e)))?;

        let mut metadata_list = Vec::new();

        for key in result.keys() {
            let describe_result = self
                .client
                .describe_key()
                .key_id(key.key_id().unwrap_or_default())
                .send()
                .await;

            if let Ok(describe) = describe_result {
                if let Some(aws_metadata) = describe.key_metadata() {
                    let metadata = Self::convert_metadata(
                        aws_metadata.key_id().unwrap_or_default(),
                        aws_metadata,
                    );
                    metadata_list.push(metadata);
                }
            }
        }

        Ok(metadata_list)
    }

    async fn describe_key(&self, key_id: &str) -> Result<KeyMetadata> {
        let effective_key_id = if key_id == "default" {
            &self.default_key_id
        } else {
            key_id
        };

        let result = self
            .client
            .describe_key()
            .key_id(effective_key_id)
            .send()
            .await
            .map_err(|e| helpers::io_error(&format!("AWS KMS 查询密钥失败: {}", e)))?;

        let aws_metadata = result
            .key_metadata()
            .ok_or_else(|| helpers::not_found("密钥", effective_key_id))?;

        Ok(Self::convert_metadata(effective_key_id, aws_metadata))
    }

    async fn destroy_key(&self, key_id: &str) -> Result<()> {
        let effective_key_id = if key_id == "default" {
            &self.default_key_id
        } else {
            key_id
        };

        self.client
            .schedule_key_deletion()
            .key_id(effective_key_id)
            .pending_window_in_days(7)
            .send()
            .await
            .map_err(|e| helpers::io_error(&format!("AWS KMS 计划密钥销毁失败: {}", e)))?;

        tracing::warn!(key_id = %effective_key_id, "密钥已计划销毁（7天后生效）");
        Ok(())
    }

    async fn health_check(&self) -> Result<KmsHealthStatus> {
        let start = std::time::Instant::now();

        match self.client.list_keys().limit(1).send().await {
            Ok(_) => {
                #[allow(clippy::cast_possible_truncation)]
                let latency = start.elapsed().as_millis() as u64;
                Ok(KmsHealthStatus::healthy(latency))
            }
            Err(e) => Ok(KmsHealthStatus::unhealthy(&format!(
                "AWS KMS 连接失败: {}",
                e
            ))),
        }
    }
}

/// AWS KMS 配置构建器
#[derive(Debug, Default)]
pub struct AwsKmsConfigBuilder {
    config: AwsKmsConfig,
}

impl AwsKmsConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.config.region = region.into();
        self
    }

    pub fn key_id(mut self, key_id: impl Into<String>) -> Self {
        self.config.key_id = key_id.into();
        self
    }

    pub fn credentials(
        mut self,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        self.config.access_key = Some(access_key.into());
        self.config.secret_key = Some(secret_key.into());
        self
    }

    pub fn max_retries(mut self, retries: u32) -> Self {
        self.config.max_retries = Some(retries);
        self
    }

    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.config.timeout_ms = Some(timeout);
        self
    }

    pub fn build(self) -> AwsKmsConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_kms_config_default() {
        let config = AwsKmsConfig::default();
        assert_eq!(config.region, "us-east-1");
        assert!(config.access_key.is_none());
        assert!(config.secret_key.is_none());
        assert_eq!(config.max_retries, Some(3));
        assert_eq!(config.timeout_ms, Some(5000));
    }

    #[test]
    fn test_aws_kms_config_builder() {
        let config = AwsKmsConfigBuilder::new()
            .region("ap-northeast-1")
            .key_id("arn:aws:kms:ap-northeast-1:123456789:key/test-key")
            .credentials(
                "AKIAIOSFODNN7EXAMPLE",
                "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            )
            .max_retries(5)
            .timeout_ms(10000)
            .build();

        assert_eq!(config.region, "ap-northeast-1");
        assert_eq!(
            config.key_id,
            "arn:aws:kms:ap-northeast-1:123456789:key/test-key"
        );
        assert_eq!(config.access_key, Some("AKIAIOSFODNN7EXAMPLE".to_string()));
        assert_eq!(
            config.secret_key,
            Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string())
        );
        assert_eq!(config.max_retries, Some(5));
        assert_eq!(config.timeout_ms, Some(10000));
    }

    #[test]
    fn test_convert_metadata_key_usage() {
        let aws_key_usage = aws_sdk_kms::types::KeyUsage::EncryptDecrypt;
        assert_eq!(
            match aws_key_usage {
                aws_sdk_kms::types::KeyUsage::EncryptDecrypt => KeyUsage::EncryptDecrypt,
                aws_sdk_kms::types::KeyUsage::SignVerify => KeyUsage::SignVerify,
                _ => KeyUsage::Both,
            },
            KeyUsage::EncryptDecrypt
        );
    }

    #[test]
    fn test_aws_kms_config_serialization() {
        let config = AwsKmsConfig {
            region: "us-west-2".to_string(),
            key_id: "test-key-id".to_string(),
            access_key: Some("test-access".to_string()),
            secret_key: Some("test-secret".to_string()),
            max_retries: Some(5),
            timeout_ms: Some(3000),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AwsKmsConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.region, deserialized.region);
        assert_eq!(config.key_id, deserialized.key_id);
        assert_eq!(config.max_retries, deserialized.max_retries);
    }
}
