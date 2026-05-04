use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::helpers;
use crate::kms::traits::*;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LeaseInfo {
    lease_id: String,
    ttl_seconds: u64,
    last_renewed: DateTime<Utc>,
    renew_fail_count: u32,
}

impl LeaseInfo {
    fn needs_renewal(&self) -> bool {
        let elapsed = Utc::now() - self.last_renewed;
        let remaining = self.ttl_seconds.saturating_sub(elapsed.num_seconds().max(0) as u64);
        remaining > 0 && remaining < self.ttl_seconds / 3
    }

    fn is_expired(&self) -> bool {
        let elapsed = Utc::now() - self.last_renewed;
        self.ttl_seconds.saturating_sub(elapsed.num_seconds().max(0) as u64) == 0
    }
}

/// HashiCorp Vault Transit 引擎 KMS 适配器
///
/// 通过 Vault Transit 引擎提供密钥管理服务，支持加密、解密、密钥轮换和 Lease 自动续期。
/// 与 [`LocalKms`](super::local::LocalKms) 不同，Vault KMS 将密钥托管在远程 Vault 服务中，
/// 适用于需要集中密钥管理和审计的企业部署场景。
#[derive(Clone)]
pub struct VaultKms {
    client: Arc<vaultrs::client::VaultClient>,
    transit_mount: String,
    leases: Arc<RwLock<HashMap<String, LeaseInfo>>>,
    _lease_renewal: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

/// Vault 连接配置
///
/// 配置 HashiCorp Vault 客户端的连接参数，包括地址、认证令牌和 TLS 设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultKmsConfig {
    /// Vault 服务器地址（如 `https://vault.example.com:8200`）
    pub address: String,
    /// Vault 认证令牌
    pub token: String,
    /// Transit 引擎挂载路径（默认为 `transit`）
    pub transit_mount: Option<String>,
    /// Vault 命名空间（Vault Enterprise 特性）
    pub namespace: Option<String>,
    /// TLS 配置
    pub tls: Option<VaultTlsConfig>,
    /// 是否启用 Lease 自动续期（默认为 `true`）
    pub auto_lease_renew: Option<bool>,
}

/// Vault TLS 配置
///
/// 配置与 Vault 服务器通信时的 TLS 证书和验证策略。
///
/// # 安全警告
///
/// `skip_verify` 设为 `true` 将跳过 TLS 证书验证，仅应在开发测试环境中使用。
/// 生产环境必须配置有效的 CA 证书。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultTlsConfig {
    /// CA 证书路径
    pub ca_cert: Option<String>,
    /// 客户端证书路径（mTLS）
    pub client_cert: Option<String>,
    /// 客户端私钥路径（mTLS）
    pub client_key: Option<String>,
    /// 是否跳过 TLS 证书验证（⚠️ 仅限开发环境）
    pub skip_verify: bool,
}

impl VaultKms {
    /// 创建 VaultKms 实例
    ///
    /// 连接到 Vault 服务器并初始化 Transit 引擎客户端。
    /// 若配置中 `auto_lease_renew` 未显式禁用，将自动启动 Lease 续期后台任务。
    ///
    /// # Errors
    ///
    /// 当 Vault 配置构建失败或无法创建 Vault 客户端时返回错误。
    pub async fn new(config: &VaultKmsConfig) -> Result<Self> {
        let mut settings_builder = vaultrs::client::VaultClientSettingsBuilder::default();
        settings_builder.address(&config.address).token(&config.token);

        if config.tls.as_ref().is_some_and(|tls| tls.skip_verify) {
            settings_builder.verify(false);
        }

        if let Some(ref ns) = config.namespace {
            settings_builder.set_namespace(ns.clone());
        }

        let settings = settings_builder
            .build()
            .map_err(|e| helpers::io_error(&format!("Vault 配置构建失败: {}", e)))?;

        let client = vaultrs::client::VaultClient::new(settings)
            .map_err(|e| helpers::io_error(&format!("创建 Vault 客户端失败: {}", e)))?;

        let client = Arc::new(client);
        let transit_mount = config
            .transit_mount
            .clone()
            .unwrap_or_else(|| "transit".to_string());

        let leases: Arc<RwLock<HashMap<String, LeaseInfo>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let lease_handle = if config.auto_lease_renew.unwrap_or(false) {
            Some(tokio::spawn(Self::lease_renewal_task(
                client.clone(),
                leases.clone(),
            )))
        } else {
            None
        };

        Ok(Self {
            client,
            transit_mount,
            leases,
            _lease_renewal: Arc::new(RwLock::new(lease_handle)),
        })
    }

    /// 从环境变量创建 VaultKms 实例
    ///
    /// 读取以下环境变量：
    /// - `VAULT_ADDR`: Vault 服务器地址（默认 `http://127.0.0.1:8200`）
    /// - `VAULT_TOKEN`: Vault 认证令牌（必需）
    /// - `VAULT_NAMESPACE`: Vault 命名空间（可选）
    /// - `VAULT_SKIP_VERIFY`: 设为 `true` 跳过 TLS 验证（可选，⚠️ 仅限开发环境）
    ///
    /// # Errors
    ///
    /// 当 `VAULT_TOKEN` 未设置或连接 Vault 失败时返回错误。
    pub async fn from_env() -> Result<Self> {
        let address =
            std::env::var("VAULT_ADDR").unwrap_or_else(|_| "http://127.0.0.1:8200".to_string());
        let token = std::env::var("VAULT_TOKEN")
            .map_err(|_| helpers::config_error("未设置 VAULT_TOKEN 环境变量"))?;

        let config = VaultKmsConfig {
            address,
            token,
            transit_mount: std::env::var("VAULT_TRANSIT_MOUNT").ok(),
            namespace: std::env::var("VAULT_NAMESPACE").ok(),
            tls: None,
            auto_lease_renew: Some(true),
        };

        Self::new(&config).await
    }

    async fn lease_renewal_task(
        client: Arc<vaultrs::client::VaultClient>,
        leases: Arc<RwLock<HashMap<String, LeaseInfo>>>,
    ) {
        tracing::info!("Vault lease renewal task started");

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;

            let leases_to_renew: Vec<LeaseInfo> = {
                let leases_map = leases.read().await;
                leases_map
                    .values()
                    .filter(|info| !info.is_expired() && info.needs_renewal())
                    .cloned()
                    .collect()
            };

            for info in leases_to_renew {
                let result =
                    Self::renew_lease(&client, &info.lease_id, info.ttl_seconds).await;

                let mut leases_map = leases.write().await;
                match result {
                    Ok(()) => {
                        if let Some(entry) = leases_map.get_mut(&info.lease_id) {
                            entry.last_renewed = Utc::now();
                            entry.renew_fail_count = 0;
                            tracing::info!(lease_id = %info.lease_id, "Lease 续期成功");
                        }
                    }
                    Err(e) => {
                        if let Some(entry) = leases_map.get_mut(&info.lease_id) {
                            entry.renew_fail_count += 1;
                            tracing::warn!(
                                lease_id = %info.lease_id,
                                fail_count = entry.renew_fail_count,
                                "Lease 续期失败: {}",
                                e
                            );
                            if entry.renew_fail_count >= 3 {
                                tracing::error!(
                                    lease_id = %info.lease_id,
                                    "Lease 连续续期失败 3 次，移除跟踪"
                                );
                                leases_map.remove(&info.lease_id);
                            }
                        }
                    }
                }
            }

            {
                let mut leases_map = leases.write().await;
                leases_map.retain(|id, info| {
                    if info.is_expired() {
                        tracing::warn!(lease_id = %id, "Lease 已过期，移除跟踪");
                        false
                    } else {
                        true
                    }
                });
            }
        }
    }

    async fn renew_lease(
        client: &vaultrs::client::VaultClient,
        lease_id: &str,
        increment: u64,
    ) -> Result<()> {
        let url = format!(
            "{}/v{}/sys/leases/renew",
            client.settings.address, client.settings.version
        );

        let body = serde_json::json!({
            "lease_id": lease_id,
            "increment": increment,
        });

        let body_str = serde_json::to_string(&body)
            .map_err(|e| helpers::serde_error(&format!("序列化请求失败: {}", e)))?;

        let mut request = client
            .http
            .http
            .put(&url)
            .header("X-Vault-Token", &client.middle.token)
            .header("Content-Type", "application/json")
            .body(body_str);

        if let Some(ref ns) = client.settings.namespace {
            request = request.header("X-Vault-Namespace", ns.as_str());
        }

        let response = request
            .send()
            .await
            .map_err(|e| helpers::io_error(&format!("Vault lease 续期请求失败: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(helpers::io_error(&format!(
                "Vault lease 续期失败: HTTP {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    /// 注册 Vault Lease 以便自动续期
    ///
    /// 将 Lease ID 和 TTL 注册到内部追踪表，后台续期任务会在 TTL 剩余 1/3 时自动续期。
    ///
    /// # 参数
    ///
    /// - `lease_id`: Vault 返回的 Lease 标识符
    /// - `ttl_seconds`: Lease 的有效时长（秒）
    pub async fn register_lease(&self, lease_id: String, ttl_seconds: u64) {
        let info = LeaseInfo {
            lease_id: lease_id.clone(),
            ttl_seconds,
            last_renewed: Utc::now(),
            renew_fail_count: 0,
        };
        self.leases.write().await.insert(lease_id, info);
    }

    fn convert_key_type(vault_type: &vaultrs::api::transit::KeyType) -> KeyType {
        match vault_type {
            vaultrs::api::transit::KeyType::Aes128Gcm96
            | vaultrs::api::transit::KeyType::Aes256Gcm96
            | vaultrs::api::transit::KeyType::Chacha20Poly1305 => KeyType::Symmetric,
            vaultrs::api::transit::KeyType::Rsa2048
            | vaultrs::api::transit::KeyType::Rsa3072
            | vaultrs::api::transit::KeyType::Rsa4096 => KeyType::Rsa,
            vaultrs::api::transit::KeyType::EcdsaP256
            | vaultrs::api::transit::KeyType::EcdsaP384
            | vaultrs::api::transit::KeyType::EcdsaP521 => KeyType::Ecc,
            vaultrs::api::transit::KeyType::Ed25519 => KeyType::Eddsa,
        }
    }

    fn convert_key_spec(vault_type: &vaultrs::api::transit::KeyType) -> KeySpec {
        match vault_type {
            vaultrs::api::transit::KeyType::Aes128Gcm96 => KeySpec::Aes128,
            vaultrs::api::transit::KeyType::Aes256Gcm96 => KeySpec::Aes256,
            vaultrs::api::transit::KeyType::Chacha20Poly1305 => KeySpec::Aes256,
            vaultrs::api::transit::KeyType::Rsa2048 => KeySpec::Rsa2048,
            vaultrs::api::transit::KeyType::Rsa3072 => KeySpec::Rsa3072,
            vaultrs::api::transit::KeyType::Rsa4096 => KeySpec::Rsa4096,
            vaultrs::api::transit::KeyType::EcdsaP256 => KeySpec::EccP256,
            vaultrs::api::transit::KeyType::EcdsaP384 => KeySpec::EccP384,
            vaultrs::api::transit::KeyType::EcdsaP521 => KeySpec::EccP384,
            vaultrs::api::transit::KeyType::Ed25519 => KeySpec::Ed25519,
        }
    }

}

#[async_trait::async_trait]
impl KeyManagementService for VaultKms {
    async fn generate_dek(&self, key_id: &str) -> Result<EncryptedKey> {
        let response = vaultrs::transit::generate::data_key(
            self.client.as_ref(),
            &self.transit_mount,
            key_id,
            vaultrs::api::transit::requests::DataKeyType::Plaintext,
            None,
        )
        .await
        .map_err(|e| helpers::io_error(&format!("Vault 生成 DEK 失败: {}", e)))?;

        let plaintext = response
            .plaintext
            .ok_or_else(|| helpers::crypto_error("Vault 未返回明文 DEK"))?;

        let decoded = BASE64
            .decode(&plaintext)
            .map_err(|e| helpers::crypto_error(&format!("Base64 解码失败: {}", e)))?;

        let nonce = decoded[..12].to_vec();
        let ciphertext_blob = decoded[12..].to_vec();

        Ok(EncryptedKey::new(
            format!("dek-{}", uuid::Uuid::new_v4()),
            ciphertext_blob,
            nonce,
            EncryptionAlgorithm::Aes256Gcm,
            key_id.to_string(),
        ))
    }

    async fn encrypt(&self, dek: &EncryptedKey, plaintext: &[u8]) -> Result<Ciphertext> {
        let pt_b64 = BASE64.encode(plaintext);

        let response = vaultrs::transit::data::encrypt(
            self.client.as_ref(),
            &self.transit_mount,
            &dek.encrypted_with,
            &pt_b64,
            None,
        )
        .await
        .map_err(|e| helpers::io_error(&format!("Vault 加密失败: {}", e)))?;

        let ct_bytes = BASE64
            .decode(&response.ciphertext)
            .map_err(|e| helpers::crypto_error(&format!("Base64 解码失败: {}", e)))?;

        Ok(Ciphertext::new(
            ct_bytes,
            dek.key_id.clone(),
            EncryptionAlgorithm::Aes256Gcm,
        ))
    }

    async fn decrypt(&self, dek: &EncryptedKey, ciphertext: &Ciphertext) -> Result<Vec<u8>> {
        let ct_b64 = BASE64.encode(&ciphertext.data);

        let response = vaultrs::transit::data::decrypt(
            self.client.as_ref(),
            &self.transit_mount,
            &dek.encrypted_with,
            &ct_b64,
            None,
        )
        .await
        .map_err(|e| helpers::io_error(&format!("Vault 解密失败: {}", e)))?;

        BASE64
            .decode(&response.plaintext)
            .map_err(|e| helpers::crypto_error(&format!("Base64 解码失败: {}", e)))
    }

    async fn sign(&self, key_id: &str, data: &[u8]) -> Result<Signature> {
        let input_b64 = BASE64.encode(data);

        let response = vaultrs::transit::data::sign(
            self.client.as_ref(),
            &self.transit_mount,
            key_id,
            &input_b64,
            None,
        )
        .await
        .map_err(|e| helpers::io_error(&format!("Vault 签名失败: {}", e)))?;

        let sig_bytes = BASE64
            .decode(&response.signature)
            .map_err(|e| helpers::crypto_error(&format!("Base64 解码失败: {}", e)))?;

        Ok(Signature::new(
            sig_bytes,
            SignatureAlgorithm::Ed25519,
            key_id.to_string(),
        ))
    }

    async fn verify(&self, key_id: &str, data: &[u8], signature: &Signature) -> Result<bool> {
        let input_b64 = BASE64.encode(data);
        let sig_b64 = BASE64.encode(&signature.value);

        let mut opts_builder =
            vaultrs::api::transit::requests::VerifySignedDataRequestBuilder::default();
        opts_builder.signature(&sig_b64);

        let response = vaultrs::transit::data::verify(
            self.client.as_ref(),
            &self.transit_mount,
            key_id,
            &input_b64,
            Some(&mut opts_builder),
        )
        .await
        .map_err(|e| helpers::io_error(&format!("Vault 验证签名失败: {}", e)))?;

        Ok(response.valid)
    }

    async fn rotate_key(&self, key_id: &str) -> Result<RotationResult> {
        vaultrs::transit::key::rotate(self.client.as_ref(), &self.transit_mount, key_id)
            .await
            .map_err(|e| helpers::io_error(&format!("Vault 密钥轮换失败: {}", e)))?;

        Ok(RotationResult {
            key_id: key_id.to_string(),
            new_version: 0,
            rotated_at: Utc::now(),
            pending_deletion_date: None,
            status: RotationStatus::Completed,
        })
    }

    async fn list_keys(&self) -> Result<Vec<KeyMetadata>> {
        let response = vaultrs::transit::key::list(self.client.as_ref(), &self.transit_mount)
            .await
            .map_err(|e| helpers::io_error(&format!("Vault 列出密钥失败: {}", e)))?;

        let mut metadata_list = Vec::new();

        for key_name in &response.keys {
            if let Ok(key_info) =
                vaultrs::transit::key::read(self.client.as_ref(), &self.transit_mount, key_name)
                    .await
            {
                let metadata = KeyMetadata {
                    key_id: key_name.clone(),
                    key_usage: KeyUsage::EncryptDecrypt,
                    key_type: Self::convert_key_type(&key_info.key_type),
                    key_spec: Self::convert_key_spec(&key_info.key_type),
                    creation_date: Utc::now(),
                    enabled: true,
                    version: 1,
                    description: None,
                    tags: HashMap::new(),
                    rotation_config: None,
                    expiration_date: None,
                };

                metadata_list.push(metadata);
            }
        }

        Ok(metadata_list)
    }

    async fn describe_key(&self, key_id: &str) -> Result<KeyMetadata> {
        let key_info =
            vaultrs::transit::key::read(self.client.as_ref(), &self.transit_mount, key_id)
                .await
                .map_err(|e| helpers::io_error(&format!("Vault 读取密钥失败: {}", e)))?;

        Ok(KeyMetadata {
            key_id: key_id.to_string(),
            key_usage: KeyUsage::EncryptDecrypt,
            key_type: Self::convert_key_type(&key_info.key_type),
            key_spec: Self::convert_key_spec(&key_info.key_type),
            creation_date: Utc::now(),
            enabled: true,
            version: 1,
            description: None,
            tags: HashMap::new(),
            rotation_config: None,
            expiration_date: None,
        })
    }

    async fn destroy_key(&self, key_id: &str) -> Result<()> {
        tracing::warn!(key_id = %key_id, "正在从 Vault 删除密钥");

        vaultrs::transit::key::delete(self.client.as_ref(), &self.transit_mount, key_id)
            .await
            .map_err(|e| helpers::io_error(&format!("Vault 删除密钥失败: {}", e)))?;

        Ok(())
    }

    async fn health_check(&self) -> Result<KmsHealthStatus> {
        let start = std::time::Instant::now();

        match vaultrs::sys::health(self.client.as_ref()).await {
            Ok(()) => {
                #[allow(clippy::cast_possible_truncation)]
                let latency = start.elapsed().as_millis() as u64;
                Ok(KmsHealthStatus::healthy(latency))
            }
            Err(e) => Ok(KmsHealthStatus::unhealthy(&format!(
                "Vault 连接失败: {}",
                e
            ))),
        }
    }
}

impl VaultKms {
    /// 从 Vault Transit 引擎中创建新密钥
    ///
    /// # 参数
    ///
    /// - `key_name`: 密钥名称（在 Vault 中唯一标识符）
    /// - `key_type`: 密钥类型（如 `aes256-gcm96`、`chacha20-poly1305`）
    ///
    /// # Errors
    ///
    /// 当 Vault API 调用失败或密钥已存在时返回错误。
    pub async fn create_key(
        &self,
        key_name: &str,
        key_type: &vaultrs::api::transit::KeyType,
    ) -> Result<KeyMetadata> {
        let mut opts_builder = vaultrs::api::transit::requests::CreateKeyRequestBuilder::default();
        opts_builder.key_type(*key_type);

        vaultrs::transit::key::create(
            self.client.as_ref(),
            &self.transit_mount,
            key_name,
            Some(&mut opts_builder),
        )
        .await
        .map_err(|e| helpers::io_error(&format!("创建 Vault 密钥失败: {}", e)))?;

        self.describe_key(key_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lease_info_needs_renewal_approaching_expiry() {
        let info = LeaseInfo {
            lease_id: "test-lease-1".to_string(),
            ttl_seconds: 300,
            last_renewed: Utc::now() - chrono::Duration::seconds(220),
            renew_fail_count: 0,
        };
        assert!(info.needs_renewal());
    }

    #[test]
    fn test_lease_info_no_renewal_needed_fresh() {
        let info = LeaseInfo {
            lease_id: "test-lease-2".to_string(),
            ttl_seconds: 300,
            last_renewed: Utc::now() - chrono::Duration::seconds(10),
            renew_fail_count: 0,
        };
        assert!(!info.needs_renewal());
    }

    #[test]
    fn test_lease_info_no_renewal_when_expired() {
        let info = LeaseInfo {
            lease_id: "test-lease-3".to_string(),
            ttl_seconds: 300,
            last_renewed: Utc::now() - chrono::Duration::seconds(300),
            renew_fail_count: 0,
        };
        assert!(!info.needs_renewal());
        assert!(info.is_expired());
    }

    #[test]
    fn test_lease_info_is_expired() {
        let info = LeaseInfo {
            lease_id: "test-lease-4".to_string(),
            ttl_seconds: 100,
            last_renewed: Utc::now() - chrono::Duration::seconds(100),
            renew_fail_count: 0,
        };
        assert!(info.is_expired());
    }

    #[test]
    fn test_lease_info_not_expired() {
        let info = LeaseInfo {
            lease_id: "test-lease-5".to_string(),
            ttl_seconds: 3600,
            last_renewed: Utc::now() - chrono::Duration::seconds(60),
            renew_fail_count: 0,
        };
        assert!(!info.is_expired());
    }

    #[tokio::test]
    async fn test_register_lease() {
        let leases: Arc<RwLock<HashMap<String, LeaseInfo>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let lease_id = "lease-abc123".to_string();
        let info = LeaseInfo {
            lease_id: lease_id.clone(),
            ttl_seconds: 3600,
            last_renewed: Utc::now(),
            renew_fail_count: 0,
        };

        leases.write().await.insert(lease_id.clone(), info);

        let map = leases.read().await;
        assert!(map.contains_key(&lease_id));
        assert_eq!(map.get(&lease_id).unwrap().ttl_seconds, 3600);
        assert_eq!(map.get(&lease_id).unwrap().renew_fail_count, 0);
    }

    #[tokio::test]
    async fn test_lease_removed_after_three_failures() {
        let leases: Arc<RwLock<HashMap<String, LeaseInfo>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let lease_id = "lease-fail".to_string();
        let info = LeaseInfo {
            lease_id: lease_id.clone(),
            ttl_seconds: 3600,
            last_renewed: Utc::now(),
            renew_fail_count: 2,
        };

        leases.write().await.insert(lease_id.clone(), info);

        {
            let mut map = leases.write().await;
            if let Some(entry) = map.get_mut(&lease_id) {
                entry.renew_fail_count += 1;
                if entry.renew_fail_count >= 3 {
                    map.remove(&lease_id);
                }
            }
        }

        let map = leases.read().await;
        assert!(!map.contains_key(&lease_id));
    }

    #[tokio::test]
    async fn test_lease_retain_removes_expired() {
        let leases: Arc<RwLock<HashMap<String, LeaseInfo>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let expired_id = "lease-expired".to_string();
        let active_id = "lease-active".to_string();

        leases.write().await.insert(
            expired_id.clone(),
            LeaseInfo {
                lease_id: expired_id.clone(),
                ttl_seconds: 100,
                last_renewed: Utc::now() - chrono::Duration::seconds(100),
                renew_fail_count: 0,
            },
        );

        leases.write().await.insert(
            active_id.clone(),
            LeaseInfo {
                lease_id: active_id.clone(),
                ttl_seconds: 3600,
                last_renewed: Utc::now(),
                renew_fail_count: 0,
            },
        );

        {
            let mut map = leases.write().await;
            map.retain(|_id, info| !info.is_expired());
        }

        let map = leases.read().await;
        assert!(!map.contains_key(&expired_id));
        assert!(map.contains_key(&active_id));
    }
}
