/// KMS 核心 trait 定义
pub mod traits;

/// 本地文件系统 KMS 实现
#[cfg(feature = "kms-local")]
pub mod local;

/// 信封加密（Envelope Encryption）实现
#[cfg(feature = "kms-local")]
pub mod envelope;

/// 密钥轮换调度器
#[cfg(feature = "kms-local")]
pub mod rotation;

/// AWS KMS 后端
#[cfg(feature = "kms-aws")]
pub mod aws;

/// HashiCorp Vault 后端
#[cfg(feature = "kms-vault")]
pub mod vault;

pub use traits::*;
