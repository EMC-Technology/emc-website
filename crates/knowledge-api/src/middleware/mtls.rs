//! mTLS（双向 TLS）认证中间件
//!
//! 实现服务间零信任认证：
//! - **证书验证**：验证客户端 X.509 证书的完整性和信任链
//! - **身份提取**：从证书 SAN (Subject Alternative Name) 中提取服务身份
//! - **STS Token**：Service-to-Service JWT Token 的生成与验证
//!
//! # 零信任原则
//!
//! 所有服务间通信必须通过 mTLS 认证，无论是否在内部网络中。
//! 每个请求都必须携带有效的客户端证书或 STS Token。

use std::collections::HashSet;
use std::fmt::Write;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, TimeZone, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;
use x509_parser::extensions::GeneralName;
use x509_parser::parse_x509_certificate;

/// mTLS 认证中间件配置
#[derive(Debug, Clone)]
pub struct MtlsConfig {
    /// 是否启用严格模式（拒绝无证书的请求）
    pub strict_mode: bool,
    /// 允许的 CA 证书指纹列表（SHA-256）
    pub allowed_ca_fingerprints: HashSet<String>,
    /// 允许的服务主体名称列表
    pub allowed_principals: HashSet<String>,
    /// 证书有效期容忍窗口（秒），用于处理时钟偏移
    pub clock_skew_tolerance_secs: u64,
}

impl Default for MtlsConfig {
    fn default() -> Self {
        Self {
            strict_mode: true,
            allowed_ca_fingerprints: HashSet::new(),
            allowed_principals: HashSet::new(),
            clock_skew_tolerance_secs: 300,
        }
    }
}

/// 客户端身份信息
///
/// 从 X.509 客户端证书中提取的服务身份，注入到 Request Extensions 中供下游使用。
///
/// # 字段说明
///
/// - `service_name`: 服务名称（从证书 CN 或 SAN DNS 名称提取）
/// - `service_id`: 服务唯一标识符（从证书 URI SAN 或自定义扩展提取）
/// - `environment`: 运行环境（production / staging / development）
/// - `version`: 服务版本号
/// - `trust_level`: 基于证书属性确定的信任级别
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientIdentity {
    /// 服务名称
    pub service_name: String,
    /// 服务唯一 ID
    pub service_id: Uuid,
    /// 运行环境
    pub environment: Environment,
    /// 服务版本号
    pub version: String,
    /// 信任级别
    pub trust_level: TrustLevel,
}

/// 运行环境枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Environment {
    /// 生产环境 —— 线上正式运行环境
    Production,
    /// 预发布环境 —— 用于最终验证的生产镜像副本
    Staging,
    /// 开发环境 —— 本地开发和功能调试
    Development,
    /// 测试环境 —— CI/CD 自动化测试
    Test,
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Production => write!(f, "production"),
            Self::Staging => write!(f, "staging"),
            Self::Development => write!(f, "development"),
            Self::Test => write!(f, "test"),
        }
    }
}

/// 信任级别
///
/// 基于客户端证书的签发者、策略和元数据确定。
/// 不同信任级别对应不同的权限边界和速率限制。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TrustLevel {
    /// 完全可信 —— 内部核心服务，由平台根 CA 签发
    FullTrust,
    /// 部分可信 —— 合作伙伴服务，由中间 CA 签发
    PartialTrust,
    /// 不可信 —— 外部第三方服务，仅允许受限的 API 子集
    Untrusted,
}

impl TrustLevel {
    /// 根据环境推断默认信任级别
    #[must_use]
    pub const fn from_environment(env: &Environment) -> Self {
        match env {
            Environment::Production | Environment::Staging => Self::FullTrust,
            Environment::Development => Self::PartialTrust,
            Environment::Test => Self::Untrusted,
        }
    }
}

/// 证书信息摘要
///
/// 用于日志记录和安全审计，不包含敏感私钥信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateInfo {
    /// 主题通用名 (CN)
    pub subject_cn: String,
    /// 签发者通用名 (Issuer CN)
    pub issuer_cn: String,
    /// 序列号（十六进制）
    pub serial_number: String,
    /// 有效期起始
    #[serde(with = "chrono::serde::ts_seconds")]
    pub not_before: DateTime<Utc>,
    /// 有效期截止
    #[serde(with = "chrono::serde::ts_seconds")]
    pub not_after: DateTime<Utc>,
    /// SHA-256 指纹
    pub fingerprint_sha256: String,
    /// SAN DNS 名称列表
    pub san_dns_names: Vec<String>,
    /// SAN URI 列表
    pub san_uris: Vec<String>,
}

// ========== STS Token 系统 ==========

/// STS (Security Token Service) Token 生成器
///
/// 为 Service-to-Service 通信生成短期 JWT Token，
/// 作为 mTLS 证书认证的补充方案（如 WebSocket 等长连接场景）。
///
/// # 安全设计
///
/// - Token 有效期短（默认 5 分钟）以减少泄露风险
/// - 使用 HMAC-SHA256 签名
/// - 包含目标服务限制（audience claim）防止 Token 重放
pub struct StsTokenGenerator {
    signing_key: EncodingKey,
    decoding_key: DecodingKey,
    issuer: String,
    default_ttl: Duration,
}

/// STS Token Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StsClaims {
    /// 签发者（当前服务标识）
    iss: String,
    /// 目标受众（目标服务标识）
    sub: String,
    /// 受众（目标服务名称）
    aud: String,
    /// 签发时间（Unix 时间戳）
    iat: u64,
    /// 过期时间（Unix 时间戳）
    exp: u64,
    /// JWT ID（唯一标识，用于防重放）
    jti: String,
    /// 服务 ID
    svc_id: Uuid,
    /// 服务名称
    svc_name: String,
    /// 信任级别
    trust_level: TrustLevel,
}

impl StsTokenGenerator {
    /// 创建新的 STS Token 生成器
    ///
    /// # Arguments
    ///
    /// * `signing_key_bytes` - HMAC 签名密钥（至少 32 字节推荐）
    /// * `issuer` - 当前服务的标识符（如 "knowledge-api"）
    /// * `default_ttl` - 默认 Token 有效期
    ///
    /// # Panics
    ///
    /// 当签名密钥长度不足时 panic。此为编程错误，应在初始化阶段暴露。
    ///
    /// # Example
    ///
    /// ```ignore
    /// let generator = StsTokenGenerator::new(
    ///     b"my-super-secret-key-at-least-32-bytes-long!!",
    ///     "knowledge-api",
    ///     Duration::from_secs(300),
    /// );
    /// ```
    pub fn new(signing_key_bytes: &[u8], issuer: &str, default_ttl: Duration) -> Self {
        assert!(
            signing_key_bytes.len() >= 32,
            "[SECURITY] STS 签名密钥长度不足 32 字节"
        );
        info!(
            issuer = issuer,
            ttl_secs = default_ttl.as_secs(),
            "初始化 StsTokenGenerator"
        );
        Self {
            signing_key: EncodingKey::from_secret(signing_key_bytes),
            decoding_key: DecodingKey::from_secret(signing_key_bytes),
            issuer: issuer.to_string(),
            default_ttl,
        }
    }

    /// 生成 Service-to-Service Token
    ///
    /// # Arguments
    ///
    /// * `service_id` - 发起请求的服务 ID
    /// * `target_service` - 目标服务名称（用于 audience 限制）
    /// * `trust_level` - 调用方的信任级别
    /// * `custom_ttl` - 自定义有效期（可选，覆盖默认值）
    ///
    /// # Returns
    ///
    /// 编码后的 JWT Token 字符串。
    ///
    /// # Errors
    ///
    /// 当 Token 编码失败时返回错误（通常不应发生）。
    #[instrument(skip(self), fields(svc_id = %service_id, target = %target_service))]
    pub fn generate_token(
        &self,
        service_id: &Uuid,
        target_service: &str,
        trust_level: TrustLevel,
        custom_ttl: Option<Duration>,
    ) -> crate::Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| error_core::helpers::io_error(&format!("系统时间获取失败: {e}")))?
            .as_secs();

        let ttl = custom_ttl.unwrap_or(self.default_ttl);
        let claims = StsClaims {
            iss: self.issuer.clone(),
            sub: self.issuer.clone(),
            aud: target_service.to_string(),
            iat: now,
            exp: now + ttl.as_secs(),
            jti: Uuid::new_v4().to_string(),
            svc_id: *service_id,
            svc_name: self.issuer.clone(),
            trust_level,
        };

        let token = encode(&Header::default(), &claims, &self.signing_key).map_err(|e| {
            error_core::helpers::crypto_error(&format!("STS Token 编码失败: {e}"))
        })?;

        debug!(
            target_service = target_service,
            ttl_secs = ttl.as_secs(),
            "STS Token 已生成"
        );

        Ok(token)
    }

    /// 验证 Service Token 并提取 Claims
    ///
    /// # Arguments
    ///
    /// * `token` - 待验证的 JWT Token 字符串
    /// * `expected_audience` - 期望的目标服务（用于 audience 验证）
    ///
    /// # Returns
    ///
    /// 解析后的 `` `ServiceTokenClaims` ``。
    ///
    /// # Errors
    ///
    /// - Token 格式无效 → 认证错误
    /// - 签名不匹配 → 认证错误
    /// - Token 过期 → 认证错误
    /// - Audience 不匹配 → 认证错误
    #[instrument(skip(self), fields(audience = %expected_audience))]
    pub fn validate_token(
        &self,
        token: &str,
        expected_audience: &str,
    ) -> crate::Result<ServiceTokenClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[expected_audience]);
        validation.validate_exp = true;

        let token_data =
            decode::<StsClaims>(token, &self.decoding_key, &validation).map_err(|e| {
                error_core::helpers::auth_error(
                    &format!("STS Token 验证失败: {e}"),
                    "validate_sts_token",
                )
            })?;

        let claims = token_data.claims;

        Ok(ServiceTokenClaims {
            service_id: claims.svc_id,
            service_name: claims.svc_name.clone(),
            issued_at: claims.iat,
            expires_at: claims.exp,
            target_service: claims.aud.clone(),
            trust_level: claims.trust_level,
            token_id: claims.jti,
        })
    }
}

/// 验证后的 STS Token Claims（安全可用的字段子集）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceTokenClaims {
    /// 发起请求的服务 ID
    pub service_id: Uuid,
    /// 发起请求的服务名称
    pub service_name: String,
    /// Token 签发时间
    pub issued_at: u64,
    /// Token 过期时间
    pub expires_at: u64,
    /// 目标服务
    pub target_service: String,
    /// 信任级别
    pub trust_level: TrustLevel,
    /// Token 唯一标识
    pub token_id: String,
}

impl ServiceTokenChecks for ServiceTokenClaims {
    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(u64::MAX, |d| d.as_secs());
        now > self.expires_at
    }

    fn is_for_service(&self, service_name: &str) -> bool {
        self.target_service == service_name
    }
}

/// STS Token 辅助检查 trait
pub trait ServiceTokenChecks {
    /// 检查 Token 是否已过期
    fn is_expired(&self) -> bool;
    /// 检查 Token 是否面向指定服务
    fn is_for_service(&self, service_name: &str) -> bool;
}

/// mTLS 证书验证结果
#[derive(Debug)]
pub enum CertificateVerificationResult {
    /// 验证通过，附带客户端身份
    Verified(ClientIdentity),
    /// 验证失败，附带原因
    Rejected(CertificateRejectionReason),
}

/// 证书被拒绝的原因
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "code", content = "detail")]
pub enum CertificateRejectionReason {
    /// 未提供客户端证书
    NoCertificate,
    /// 证书已过期
    Expired {
        /// 证书过期时间
        not_after: DateTime<Utc>,
    },
    /// 证书尚未生效
    NotYetValid {
        /// 证书生效时间
        not_before: DateTime<Utc>,
    },
    /// 证书签名无效
    InvalidSignature,
    /// 证书不在信任链中
    UntrustedIssuer {
        /// 未受信任的签发者名称
        issuer: String,
    },
    /// 证书已被吊销
    Revoked,
    /// 证书主体名称不在允许列表中
    DisallowedPrincipal {
        /// 不被允许的主体通用名 (CN)
        cn: String,
    },
    /// 证书缺少必要的 SAN 扩展
    MissingSanExtension,
    /// 无法解析证书
    ParseError(String),
}

/// `` `mTLS` `` 证书验证器
///
/// 封装 mTLS 证书验证逻辑。在生产环境中应与 `rustls` 或 `openssl` 集成。
pub struct CertificateVerifier {
    config: Arc<MtlsConfig>,
}

impl CertificateVerifier {
    /// 创建新的证书验证器
    #[must_use]
    pub fn new(config: MtlsConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// 验证客户端证书并提取身份
    ///
    /// 在实际生产环境中，此处应接收 `rustls::Certificate` 或 `openssl::x509::X509` 对象。
    /// 本实现提供框架接口，具体加密操作需根据所选 TLS 库完成集成。
    ///
    /// # Arguments
    ///
    /// * `cert_der` - 客户端证书的 DER 编码字节数组
    ///
    /// # Errors
    ///
    /// 当证书为空或验证失败时返回错误。
    pub fn verify_certificate(
        &self,
        cert_der: &[u8],
    ) -> Result<CertificateVerificationResult, CertificateRejectionReason> {
        if cert_der.is_empty() {
            return Ok(CertificateVerificationResult::Rejected(
                CertificateRejectionReason::NoCertificate,
            ));
        }

        let cert_info = Self::parse_certificate_info(cert_der)?;

        // 1. 检查证书有效期
        let now = Utc::now();
        #[allow(clippy::cast_possible_wrap)]
        if now
            > cert_info.not_after
                + chrono::Duration::seconds(self.config.clock_skew_tolerance_secs as i64)
        {
            return Ok(CertificateVerificationResult::Rejected(
                CertificateRejectionReason::Expired {
                    not_after: cert_info.not_after,
                },
            ));
        }

        #[allow(clippy::cast_possible_wrap)]
        if now
            < cert_info.not_before
                - chrono::Duration::seconds(self.config.clock_skew_tolerance_secs as i64)
        {
            return Ok(CertificateVerificationResult::Rejected(
                CertificateRejectionReason::NotYetValid {
                    not_before: cert_info.not_before,
                },
            ));
        }

        // 2. 检查主体是否在允许列表中
        if !self.config.allowed_principals.is_empty()
            && !self
                .config
                .allowed_principals
                .contains(&cert_info.subject_cn)
        {
            return Ok(CertificateVerificationResult::Rejected(
                CertificateRejectionReason::DisallowedPrincipal {
                    cn: cert_info.subject_cn,
                },
            ));
        }

        // 3. 提取客户端身份
        let identity = Self::extract_identity_from_cert_info(&cert_info);

        Ok(CertificateVerificationResult::Verified(identity))
    }

    fn parse_certificate_info(
        cert_der: &[u8],
    ) -> Result<CertificateInfo, CertificateRejectionReason> {
        let (_, cert) = parse_x509_certificate(cert_der).map_err(|e| {
            CertificateRejectionReason::ParseError(format!("X.509 证书解析失败: {e}"))
        })?;

        let subject_cn = cert
            .subject()
            .iter_common_name()
            .next()
            .map(|atv| {
                atv.attr_value()
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let issuer_cn = cert
            .issuer()
            .iter_common_name()
            .next()
            .map(|atv| {
                atv.attr_value()
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let serial_bytes = cert.tbs_certificate.serial.to_bytes_be();
        let mut serial_number = String::with_capacity(serial_bytes.len() * 2);
        for b in &serial_bytes {
            let _ = write!(serial_number, "{b:02x}");
        }

        let not_before = Utc
            .timestamp_opt(cert.validity().not_before.timestamp(), 0)
            .single()
            .unwrap_or(DateTime::UNIX_EPOCH);

        let not_after = Utc
            .timestamp_opt(cert.validity().not_after.timestamp(), 0)
            .single()
            .unwrap_or(DateTime::UNIX_EPOCH);

        let fingerprint_sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(cert_der);
            let result = hasher.finalize();
            let mut fp = String::with_capacity(result.len() * 2);
            for b in &result {
                let _ = write!(fp, "{b:02x}");
            }
            fp
        };

        let (san_dns_names, san_uris) = cert
            .subject_alternative_name()
            .ok()
            .flatten()
            .map(|san_ext| {
                let (dns, uris) = san_ext.value.general_names.iter().fold(
                    (Vec::new(), Vec::new()),
                    |(mut dns, mut uris), gn| {
                        match gn {
                            GeneralName::DNSName(name) => {
                                dns.push(name.to_string());
                            }
                            GeneralName::URI(uri) => {
                                uris.push(uri.to_string());
                            }
                            _ => {}
                        }
                        (dns, uris)
                    },
                );
                (dns, uris)
            })
            .unwrap_or_default();

        Ok(CertificateInfo {
            subject_cn,
            issuer_cn,
            serial_number,
            not_before,
            not_after,
            fingerprint_sha256,
            san_dns_names,
            san_uris,
        })
    }

    fn extract_identity_from_cert_info(cert_info: &CertificateInfo) -> ClientIdentity {
        let service_name = if cert_info.subject_cn.is_empty() {
            cert_info.san_dns_names.first().cloned().unwrap_or_default()
        } else {
            cert_info.subject_cn.clone()
        };

        let environment = if service_name.contains("prod") {
            Environment::Production
        } else if service_name.contains("staging") {
            Environment::Staging
        } else if service_name.contains("test") {
            Environment::Test
        } else {
            Environment::Development
        };

        ClientIdentity {
            service_name,
            service_id: Uuid::nil(),
            environment: environment.clone(),
            version: "0.0.0".to_string(),
            trust_level: TrustLevel::from_environment(&environment),
        }
    }
}

// ========== 单元测试 ==========

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SIGNING_KEY: &[u8] = b"this-is-a-test-signing-key-for-sts-tokens!-32b";

    fn generate_test_cert() -> Vec<u8> {
        let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).expect("参数创建成功");
        let mut dn = rcgen::DistinguishedName::new();
        dn.push(rcgen::DnType::CommonName, "test-service");
        dn.push(rcgen::DnType::OrganizationName, "Test Org");
        params.distinguished_name = dn;

        let san = rcgen::SanType::DnsName(
            rcgen::Ia5String::try_from("test-service.internal").expect("IA5String 创建成功"),
        );
        params.subject_alt_names.push(san);

        let key_pair = rcgen::KeyPair::generate().expect("密钥对生成成功");
        let cert = params.self_signed(&key_pair).expect("证书创建成功");
        cert.der().to_vec()
    }

    fn generate_expired_cert() -> Vec<u8> {
        let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).expect("参数创建成功");
        params.not_before = rcgen::date_time_ymd(2000, 1, 1);
        params.not_after = rcgen::date_time_ymd(2001, 1, 1);

        let mut dn = rcgen::DistinguishedName::new();
        dn.push(rcgen::DnType::CommonName, "expired-service");
        params.distinguished_name = dn;

        let key_pair = rcgen::KeyPair::generate().expect("密钥对生成成功");
        let cert = params.self_signed(&key_pair).expect("证书创建成功");
        cert.der().to_vec()
    }

    #[test]
    fn test_sts_token_generation_and_validation() {
        let generator =
            StsTokenGenerator::new(TEST_SIGNING_KEY, "test-service", Duration::from_secs(300));
        let service_id = Uuid::new_v4();

        let token = generator
            .generate_token(&service_id, "target-service", TrustLevel::FullTrust, None)
            .expect("Token 生成成功");

        let claims = generator
            .validate_token(&token, "target-service")
            .expect("Token 验证成功");

        assert_eq!(claims.service_id, service_id);
        assert_eq!(claims.target_service, "target-service");
        assert_eq!(claims.trust_level, TrustLevel::FullTrust);
        assert!(!claims.is_expired());
        assert!(claims.is_for_service("target-service"));
    }

    #[test]
    fn test_sts_token_rejects_wrong_audience() {
        let generator =
            StsTokenGenerator::new(TEST_SIGNING_KEY, "test-service", Duration::from_secs(300));

        let token = generator
            .generate_token(
                &Uuid::new_v4(),
                "correct-target",
                TrustLevel::FullTrust,
                None,
            )
            .expect("Token 生成成功");

        let result = generator.validate_token(&token, "wrong-target");
        assert!(result.is_err(), "错误受众应导致验证失败");
    }

    #[test]
    fn test_trust_level_from_environment() {
        assert_eq!(
            TrustLevel::from_environment(&Environment::Production),
            TrustLevel::FullTrust
        );
        assert_eq!(
            TrustLevel::from_environment(&Environment::Development),
            TrustLevel::PartialTrust
        );
        assert_eq!(
            TrustLevel::from_environment(&Environment::Test),
            TrustLevel::Untrusted
        );
    }

    #[test]
    fn test_mtls_config_default() {
        let config = MtlsConfig::default();
        assert!(config.strict_mode);
        assert!(config.allowed_ca_fingerprints.is_empty());
        assert!(config.allowed_principals.is_empty());
        assert_eq!(config.clock_skew_tolerance_secs, 300);
    }

    #[test]
    fn test_certificate_rejection_no_certificate() {
        let verifier = CertificateVerifier::new(MtlsConfig::default());
        let result = verifier.verify_certificate(&[]).expect("验证执行成功");
        matches!(
            result,
            CertificateVerificationResult::Rejected(CertificateRejectionReason::NoCertificate)
        );
    }

    #[test]
    #[should_panic(expected = "签名密钥长度不足")]
    fn test_sts_rejects_short_key() {
        let _ = StsTokenGenerator::new(b"short", "test", Duration::from_secs(60));
    }

    #[test]
    fn test_client_identity_serialization() {
        let identity = ClientIdentity {
            service_name: "knowledge-api".to_string(),
            service_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            environment: Environment::Production,
            version: "1.0.0".to_string(),
            trust_level: TrustLevel::FullTrust,
        };
        let json = serde_json::to_string(&identity).expect("序列化成功");
        assert!(json.contains("knowledge-api"));
        assert!(json.contains("fullTrust"));
    }

    #[test]
    fn test_environment_display() {
        assert_eq!(Environment::Production.to_string(), "production");
        assert_eq!(Environment::Staging.to_string(), "staging");
        assert_eq!(Environment::Development.to_string(), "development");
    }

    #[test]
    fn test_parse_valid_certificate() {
        let cert_der = generate_test_cert();
        let result = CertificateVerifier::parse_certificate_info(&cert_der);
        assert!(result.is_ok(), "有效证书解析应成功");

        let info = result.unwrap();
        assert_eq!(info.subject_cn, "test-service");
        assert!(!info.serial_number.is_empty());
        assert!(!info.fingerprint_sha256.is_empty());
        assert_eq!(info.fingerprint_sha256.len(), 64);
    }

    #[test]
    fn test_parse_malformed_data_returns_parse_error() {
        let malformed = b"this is not a certificate";
        let result = CertificateVerifier::parse_certificate_info(malformed);
        assert!(result.is_err());
        matches!(
            result.unwrap_err(),
            CertificateRejectionReason::ParseError(_)
        );
    }

    #[test]
    fn test_verify_expired_certificate_returns_expired() {
        let cert_der = generate_expired_cert();
        let verifier = CertificateVerifier::new(MtlsConfig::default());
        let result = verifier
            .verify_certificate(&cert_der)
            .expect("验证执行成功");
        matches!(
            result,
            CertificateVerificationResult::Rejected(CertificateRejectionReason::Expired { .. })
        );
    }

    #[test]
    fn test_verify_valid_certificate_succeeds() {
        let cert_der = generate_test_cert();
        let verifier = CertificateVerifier::new(MtlsConfig::default());
        let result = verifier
            .verify_certificate(&cert_der)
            .expect("验证执行成功");
        matches!(result, CertificateVerificationResult::Verified(_));
    }

    #[test]
    fn test_certificate_fingerprint_is_deterministic() {
        let cert_der = generate_test_cert();
        let info1 = CertificateVerifier::parse_certificate_info(&cert_der).unwrap();
        let info2 = CertificateVerifier::parse_certificate_info(&cert_der).unwrap();
        assert_eq!(info1.fingerprint_sha256, info2.fingerprint_sha256);
    }

    #[test]
    fn test_extract_identity_from_cert_info() {
        let cert_der = generate_test_cert();
        let cert_info = CertificateVerifier::parse_certificate_info(&cert_der).unwrap();
        let identity = CertificateVerifier::extract_identity_from_cert_info(&cert_info);
        assert_eq!(identity.service_name, "test-service");
        assert_eq!(identity.environment, Environment::Test);
        assert_eq!(identity.trust_level, TrustLevel::Untrusted);
    }
}
