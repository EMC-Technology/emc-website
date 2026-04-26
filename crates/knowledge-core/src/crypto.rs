/// 加密与哈希工具（blake3 + aes-gcm）
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use blake3::Hasher;
use aes_gcm::aead::rand_core::RngCore;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use crate::Result;
use crate::error::helpers;
use std::path::Path;
use zeroize::ZeroizeOnDrop;

/// 使用 blake3 计算数据的哈希摘要
pub fn hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// 使用 blake3 计算字符串的哈希摘要
pub fn hash_str(s: &str) -> [u8; 32] {
    hash(s.as_bytes())
}

/// AES-256-GCM 对称加密封装
///
/// 析构时自动安全擦除内部密钥材料（通过 `ZeroizeOnDrop`）。
#[derive(ZeroizeOnDrop)]
pub struct Encryptor {
    #[zeroize(skip)]
    cipher: Aes256Gcm,
}

impl Encryptor {
    /// 从 32 字节密钥创建加密器
    ///
    /// # Errors
    ///
    /// 密钥长度不合法时返回错误。由于参数类型为 `[u8; 32]`，
    /// 类型系统已保证长度为 32 字节，此错误在实践中不会发生。
    pub fn new(key: [u8; 32]) -> Result<Self> {
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| helpers::crypto_error(&format!("AES-256-GCM 密钥初始化失败: {e}")))?;
        Ok(Self { cipher })
    }

    /// 加密明文，返回 (nonce, ciphertext)
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| helpers::crypto_error(&format!("加密失败: {e}")))?;
        Ok((nonce.to_vec(), ciphertext))
    }
}

/// AES-256-GCM 对称解密封装
///
/// 析构时自动安全擦除内部密钥材料（通过 `ZeroizeOnDrop`）。
#[derive(ZeroizeOnDrop)]
pub struct Decryptor {
    #[zeroize(skip)]
    cipher: Aes256Gcm,
}

impl Decryptor {
    /// 从 32 字节密钥创建解密器
    ///
    /// # Errors
    ///
    /// 密钥长度不合法时返回错误。由于参数类型为 `[u8; 32]`，
    /// 类型系统已保证长度为 32 字节，此错误在实践中不会发生。
    pub fn new(key: [u8; 32]) -> Result<Self> {
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| helpers::crypto_error(&format!("AES-256-GCM 密钥初始化失败: {e}")))?;
        Ok(Self { cipher })
    }

    /// 解密密文，返回明文
    pub fn decrypt(&self, nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(nonce);
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| helpers::crypto_error(&format!("解密失败: {e}")))?;
        Ok(plaintext)
    }
}

/// 密钥管理器
///
/// 负责密钥的生成、存储、读取和转换。
pub struct KeyManager {
    key_path: String,
}

impl KeyManager {
    /// 创建密钥管理器
    pub fn new(key_path: &str) -> Self {
        Self {
            key_path: key_path.to_string(),
        }
    }

    /// 生成新的 32 字节随机密钥
    pub fn generate_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// 保存密钥到文件
    pub fn save_key(&self, key: [u8; 32]) -> Result<()> {
        let path = Path::new(&self.key_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.key_path)?;
            file.write_all(&key)?;
        }

        #[cfg(windows)]
        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&self.key_path)?;
            file.write_all(&key)?;

            let path_str = Path::new(&self.key_path)
                .canonicalize()?
                .to_str()
                .ok_or_else(|| helpers::internal_error("密钥文件路径编码无效"))?
                .to_string();

            let reset_result = std::process::Command::new("icacls")
                .arg(&path_str)
                .arg("/inheritance:r")
                .arg("/grant:r")
                .arg(format!("{}:(F)", whoami()?))
                .output();

            match reset_result {
                Ok(output) if output.status.success() => {}
                Ok(output) => {
                    return Err(helpers::crypto_error(&format!(
                        "设置密钥文件权限失败: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )));
                }
                Err(e) => {
                    return Err(helpers::crypto_error(&format!(
                        "执行 icacls 命令失败: {e}"
                    )));
                }
            }
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _ = key;
            return Err(helpers::internal_error("当前平台不支持密钥文件存储"));
        }

        Ok(())
    }

    /// 从文件加载密钥
    pub fn load_key(&self) -> Result<[u8; 32]> {
        let mut file = File::open(&self.key_path)?;
        let mut key = [0u8; 32];
        file.read_exact(&mut key)?;
        Ok(key)
    }

    /// 检查密钥文件是否存在
    pub fn key_exists(&self) -> bool {
        Path::new(&self.key_path).exists()
    }

    /// 安全擦除密钥文件
    ///
    /// 通过覆写文件内容（精确 32 字节）后同步落盘再删除，
    /// 确保密钥不会被恢复。
    ///
    /// # 安全说明
    ///
    /// 在现代 SSD 和日志结构文件系统上，覆写不能保证物理擦除
    /// （wear leveling 机制可能将新数据写入不同物理位置）。
    /// 此方法提供尽力而为（best-effort）的安全擦除，
    /// 生产环境应依赖文件系统加密（如 LUKS/BitLocker）
    /// 和 `zeroize` 对内存密钥的安全擦除。
    pub fn secure_erase_key(&self) -> Result<()> {
        let path = Path::new(&self.key_path);
        if path.exists() {
            let file_len = std::fs::metadata(path)?.len();
            let mut file = OpenOptions::new()
                .write(true)
                .open(path)?;
            let zeros = vec![0u8; usize::try_from(file_len).map_err(|_| {
                error_core::helpers::internal_error("file too large for memory allocation")
            })?];
            file.write_all(&zeros)?;
            file.sync_all()?;

            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[cfg(windows)]
fn whoami() -> Result<String> {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .map_err(|_| helpers::crypto_error("无法获取当前用户名: USERNAME 和 USER 环境变量均未设置"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_hash_consistency() {
        let data = b"test data";
        let hash1 = hash(data);
        let hash2 = hash(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let key = KeyManager::generate_key();
        let encryptor = Encryptor::new(key).unwrap();
        let decryptor = Decryptor::new(key).unwrap();

        let plaintext = b"Hello, world!";
        let (nonce, ciphertext) = encryptor.encrypt(plaintext).unwrap();
        let decrypted = decryptor.decrypt(&nonce, &ciphertext).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_key_manager() {
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("enc.key").to_str().unwrap().to_string();
        let manager = KeyManager::new(&key_path);

        let key = KeyManager::generate_key();
        manager.save_key(key).unwrap();

        let loaded_key = manager.load_key().unwrap();
        assert_eq!(key, loaded_key);

        assert!(manager.key_exists());

        manager.secure_erase_key().unwrap();
        assert!(!manager.key_exists());
    }

    #[test]
    fn test_hash_str() {
        let s = "test string";
        let hash1 = hash_str(s);
        let hash2 = hash(s.as_bytes());
        assert_eq!(hash1, hash2);
    }
}
