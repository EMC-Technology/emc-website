use aws_config::BehaviorVersion;
use aws_sdk_kms::{Client, primitives::Blob, error::ProvideErrorMetadata};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::error::helpers;
use crate::kms::traits::*;
use crate::Result;

/// AWS KMS 适配器
///
/// 使用 AWS Key Management Service 作为企业级密钥管理后端。
///
/// # 特性
///
/// - Envelope Encryption (信封加密) 模式
/// - CMK (Customer Master Key) 管理
/// - 自动重试和错误处理
/// - IAM 权限集成
/// - CloudTrail 审计日志
///
/// # 前置条件
///
/// - AWS 凭证已配置（环境变量、~/.aws/credentials 或 IAM Role）
/// - 目标区域有权限访问 AWS KMS
/// - CMK 已创建或配置了自动创建选项
#[derive(Clone)]
pub struct AwsKms {
    /// AWS KMS 客户端
    client: Client,
    /// 默认 CMK ID（用于 DEK 加密）
    default_cmk_id: String,
    /// 重试配置
    retry_config: RetryConfig,
}

/// 重试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 初始重试延迟（毫秒）
    pub initial_delay_ms: u64,
    /// 最大重试延迟（毫秒）
    pub max_delay_ms: u64,
    /// 退避倍数
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
        }
    }
}

impl AwsKms {
    /// 创建新的 AWS KMS 适配器
    ///
    /// # 参数
    ///
    /// - `region`: AWS 区域（如 "us-east-1", "ap-northeast-1"）
    /// - `default_cmk_id`: 默认的 Customer Master Key ID 或 ARN
    ///
    /// # 错误
    ///
    /// AWS SDK 配置失败或凭证无效时返回错误。
    pub async fn new(region: &str, default_cmk_id: &str) -> Result<Self> {
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_sdk_kms::config::Region::new(region.to_string()))
            .load()
            .await;

        let client = Client::new(&config);

        Ok(Self {
            client,
            default_cmk_id: default_cmk_id.to_string(),
            retry_config: RetryConfig::default(),
        })
    }

    /// 使用自定义配置创建 AWS KMS 适配器
    pub async fn with_config(config: AwsKmsConfig) -> Result<Self> {
        let mut loader = aws_config::defaults(BehaviorVersion::latest());

        if let Some(ref region) = config.region {
            loader = loader.region(
                aws_sdk_kms::config::Region::new(region.clone())
            );
        }

        if let Some(ref profile) = config.profile {
            loader = loader.profile_name(profile);
        }

        let sdk_config = loader.load().await;
        let client = Client::new(&sdk_config);

        Ok(Self {
            client,
            default_cmk_id: config.default_cmk_id,
            retry_config: config.retry_config.unwrap_or_default(),
        })
    }

    /// 执行带重试的操作
    async fn execute_with_retry<F, Fut, T, E>(&self, operation: &str, f: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, E>>,
        E: Into<aws_sdk_kms::Error>,
    {
        let mut delay = Duration::from_millis(self.retry_config.initial_delay_ms);
        let mut last_error = None;

        for attempt in 0..=self.retry_config.max_retries {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let e = e.into();
                    let meta = e.meta();
                    let is_retryable = matches!(
                        meta.code(),
                        Some("ThrottlingException")
                            | Some("ServiceUnavailable")
                            | Some("InternalFailure")
                    );

                    if !is_retryable || attempt == self.retry_config.max_retries {
                        return Err(helpers::internal_error(&format!(
                            "AWS KMS {} 失败 (尝试 {}/{}): {}",
                            operation,
                            attempt + 1,
                            self.retry_config.max_retries + 1,
                            e
                        )));
                    }

                    tracing::warn!(
                        operation = %operation,
                        attempt = attempt + 1,
                        error = %e,
                        "AWS KMS 操作失败，准备重试"
                    );

                    last_error = Some(e);
                    tokio::time::sleep(delay).await;
                    delay = Duration::from_millis(
                        (delay.as_millis() as f64 * self.retry_config.backoff_multiplier)
                            .min(self.retry_config.max_delay_ms as f64) as u64,
                    );
                }
            }
        }

        Err(helpers::internal_error(&format!(
            "AWS KMS {} 最终失败: {:?}",
            operation, last_error
        )))
    }

    /// 将 AWS 密钥元数据转换为通用格式
    fn convert_key_metadata(key_id: &str, desc: &aws_sdk_kms::output::DescribeKeyOutput) -> KeyMetadata {
        let key_metadata = desc.key_metadata().unwrap_or_default();

        let key_usage = match key_metadata.key_usage().unwrap_or("ENCRYPT_DECRYPT") {
            "ENCRYPT_DECRYPT" => KeyUsage::EncryptDecrypt,
            "SIGN_VERIFY" => KeyUsage::SignVerify,
            _ => KeyUsage::Both,
        };

        let key_type = match key_metadata.key_spec().unwrap_or("SYMMETRIC_DEFAULT") {
            s if s.starts_with("RSA_") => KeyType::Rsa,
            s if s.contains("ECC") || s.contains("EC_") => KeyType::Ecc,
            "HMAC_224" | "HMAC_256" | "HMAC_384" | "HMAC_512" => KeyType::Hmac,
            _ => KeyType::Symmetric,
        };

        let key_spec = match key_metadata.key_spec().unwrap_or("SYMMETRIC_DEFAULT") {
            "AES_128" => KeySpec::Aes128,
            "AES_256" | "SYMMETRIC_DEFAULT" => KeySpec::Aes256,
            "RSA_2048" => KeySpec::Rsa2048,
            "RSA_3072" => KeySpec::Rsa3072,
            "RSA_4096" => KeySpec::Rsa4096,
            "ECC_NIST_P256" => KeySpec::EccP256,
            "ECC_NIST_P384" => KeySpec::EccP384,
            "HMAC_256" => KeySpec::Hmac256,
            _ => KeySpec::Aes256,
        };

        KeyMetadata {
            key_id: key_id.to_string(),
            key_usage,
            key_type,
            key_spec,
            creation_date: key_metadata
                .creation_date()
                .map(|d| DateTime::<Utc>::from(*d))
                .unwrap_or_else(Utc::now),
            enabled: key_metadata.enabled().unwrap_or(false),
            version: 1,
            description: key_metadata.description().map(String::from),
            tags: HashMap::new(),
            rotation_config: None,
            expiration_date: None,
        }
    }
}

#[async_trait]
impl KeyManagementService for AwsKms {
    async fn generate_dek(&self, key_id: &str) -> Result<EncryptedKey> {
        let cmk_id = if key_id.is_empty() {
            &self.default_cmk_id
        } else {
            key_id
        };

        let result = self
            .execute_with_retry("GenerateDataKey", || async {
                self.client
                    .generate_data_key()
                    .key_id(cmk_id)
                    .key_spec(aws_sdk_kms::types::DataKeySpec::Aes256)
                    .send()
                    .await
            })
            .await?;

        let plaintext = result.plaintext().map(|b| b.as_ref().to_vec()).unwrap_or_default();
        let ciphertext_blob = result.ciphertext_blob().map(|b| b.as_ref().to_vec()).unwrap_or_default();

        Ok(EncryptedKey::new(
            format!("dek-{}", uuid::Uuid::new_v4()),
            ciphertext_blob,
            vec![],
            EncryptionAlgorithm::Aes256Gcm,
            cmk_id.to_string(),
        ))
    }

    async fn encrypt(&self, dek: &EncryptedKey, plaintext: &[u8]) -> Result<Ciphertext> {
        let result = self
            .execute_with_retry("Encrypt", || async {
                self.client
                    .encrypt()
                    .key_id(&dek.encrypted_with)
                    .plaintext(Blob::new(plaintext))
                    .send()
                    .await
            })
            .await;

        match result {
            Ok(output) => {
                let ciphertext_blob = output
                    .ciphertext_blob()
                    .map(|b| b.as_ref().to_vec())
                    .unwrap_or_default();

                Ok(Ciphertext::new(
                    ciphertext_blob,
                    dek.key_id.clone(),
                    EncryptionAlgorithm::Aes256Gcm,
                ))
            }
            Err(e) => Err(e),
        }
    }

    async fn decrypt(&self, dek: &EncryptedKey, ciphertext: &Ciphertext) -> Result<Vec<u8>> {
        let result = self
            .execute_with_retry("Decrypt", || async {
                self.client
                    .decrypt()
                    .key_id(&dek.encrypted_with)
                    .ciphertext_blob(Blob::new(ciphertext.data.clone()))
                    .send()
                    .await
            })
            .await?;

        let plaintext = result.plaintext().map(|b| b.as_ref().to_vec()).unwrap_or_default();
        Ok(plaintext)
    }

    async fn sign(&self, key_id: &str, data: &[u8]) -> Result<Signature> {
        let result = self
            .execute_with_retry("Sign", || async {
                self.client
                    .sign()
                    .key_id(key_id)
                    .message(Blob::new(data))
                    .signing_algorithm(aws_sdk_kms::types::SigningAlgorithmSpec::RsassaPkcs1V15Sha256)
                    .message_type(aws_sdk_kms::types::MessageType::Raw)
                    .send()
                    .await
            })
            .await?;

        let signature = result.signature().map(|b| b.as_ref().to_vec()).unwrap_or_default();
        let algorithm = match result.signing_algorithm() {
            Some(aws_sdk_kms::types::SigningAlgorithmSpec::RsassaPkcs1V15Sha256) => SignatureAlgorithm::RsaPkcs1v15Sha256,
            Some(aws_sdk_kms::types::SigningAlgorithmSpec::RsassaPssSha256) => SignatureAlgorithm::RsaPssSha256,
            Some(aws_sdk_kms::types::SigningAlgorithmSpec::EcdsaSha256) => SignatureAlgorithm::EcdsaP256Sha256,
            _ => SignatureAlgorithm::Ed25519,
        };

        Ok(Signature::new(signature, algorithm, key_id.to_string()))
    }

    async fn verify(&self, key_id: &str, data: &[u8], signature: &Signature) -> Result<bool> {
        let result = self
            .execute_with_retry("Verify", || async {
                self.client
                    .verify()
                    .key_id(key_id)
                    .message(Blob::new(data))
                    .signature(Blob::new(signature.value.clone()))
                    .signing_algorithm(match signature.algorithm {
                        SignatureAlgorithm::RsaPkcs1v15Sha256 => aws_sdk_kms::types::SigningAlgorithmSpec::RsassaPkcs1V15Sha256,
                        SignatureAlgorithm::RsaPssSha256 => aws_sdk_kms::types::SigningAlgorithmSpec::RsassaPssSha256,
                        SignatureAlgorithm::EcdsaP256Sha256 => aws_sdk_kms::types::SigningAlgorithmSpec::EcdsaSha256,
                        _ => aws_sdk_kms::types::SigningAlgorithmSpec::RsassaPkcs1V15Sha256,
                    })
                    .message_type(aws_sdk_kms::types::MessageType::Raw)
                    .send()
                    .await
            })
            .await?;

        Ok(result.signature_valid())
    }

    async fn rotate_key(&self, key_id: &str) -> Result<RotationResult> {
        self.execute_with_retry("RotateKey", || async {
            self.client
                .rotate_key_on_demand()
                .key_id(key_id)
                .send()
                .await
        })
        .await?;

        Ok(RotationResult {
            key_id: key_id.to_string(),
            new_version: 0,
            rotated_at: Utc::now(),
            pending_deletion_date: None,
            status: RotationStatus::Completed,
        })
    }

    async fn list_keys(&self) -> Result<Vec<KeyMetadata>> {
        let output = self
            .execute_with_retry("ListKeys", || async {
                self.client.list_keys().send().await
            })
            .await?;

        let mut keys = Vec::new();

        for key_entry in output.keys() {
            if let Some(key_id) = key_entry.key_id() {
                match self.describe_key(key_id).await {
                    Ok(metadata) => keys.push(metadata),
                    Err(_) => continue,
                }
            }
        }

        Ok(keys)
    }

    async fn describe_key(&self, key_id: &str) -> Result<KeyMetadata> {
        let output = self
            .execute_with_retry("DescribeKey", || async {
                self.client
                    .describe_key()
                    .key_id(key_id)
                    .send()
                    .await
            })
            .await?;

        Ok(Self::convert_key_metadata(key_id, &output))
    }

    async fn destroy_key(&self, key_id: &str) -> Result<()> {
        tracing::warn!(key_id = %key_id, "正在调度密钥销毁");

        self.execute_with_retry("ScheduleKeyDeletion", || async {
            self.client
                .schedule_key_deletion()
                .key_id(key_id)
                .pending_window_in_days(7)
                .send()
                .await
        })
        .await?;

        Ok(())
    }

    async fn health_check(&self) -> Result<KmsHealthStatus> {
        let start = std::time::Instant::now();

        match self.client.list_keys().limit(1).send().await {
            Ok(_) => {
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

/// AWS KMS 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsKmsConfig {
    /// AWS 区域
    pub region: Option<String>,
    /// AWS Profile 名称
    pub profile: Option<String>,
    /// 默认 CMK ID
    pub default_cmk_id: String,
    /// 重试配置
    pub retry_config: Option<RetryConfig>,
}

impl AwsKmsConfig {
    /// 从环境变量加载配置
    pub fn from_env() -> Option<Self> {
        let region = std::env::var("AWS_REGION").ok();
        let cmk_id = std::env::var("AWS_KMS_KEY_ID").ok();

        cmk_id.map(|default_cmk_id| Self {
            region,
            profile: None,
            default_cmk_id,
            retry_config: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 100);
    }

    #[test]
    fn test_encryption_algorithm_conversion() {
        assert_eq!(EncryptionAlgorithm::Aes256Gcm.display_name(), "AES-256-GCM");
    }

    #[test]
    fn test_aws_config_from_env_missing() {
        // 确保没有设置这些环境变量
        std::env::remove_var("AWS_KMS_KEY_ID");
        assert!(AwsKmsConfig::from_env().is_none());
    }

    #[tokio::test]
    async fn test_convert_key_metadata() {
        let mock_desc = aws_sdk_kms::output::DescribeKeyOutput::builder()
            .key_metadata(
                aws_sdk_kms::types::KeyMetadata::builder()
                    .key_id("test-key")
                    .arn("arn:aws:kms:us-east-1:123456789:key/test-key")
                    .aws_account_id("123456789")
                    .creation_date(DateTime::<Utc>::now())
                    .enabled(true)
                    .key_spec("AES_256")
                    .key_usage("ENCRYPT_DECRYPT")
                    .description("Test key")
                    .build(),
            )
            .build();

        let metadata = AwsKms::convert_key_metadata("test-key", &mock_desc);
        assert_eq!(metadata.key_id, "test-key");
        assert_eq!(metadata.key_spec, KeySpec::Aes256);
        assert_eq!(metadata.key_usage, KeyUsage::EncryptDecrypt);
        assert!(metadata.enabled);
    }
}
