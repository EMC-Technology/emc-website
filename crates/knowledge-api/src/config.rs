//! 配置管理模块（从 config.toml / 环境变量加载 MECE-07 配置项）
//! 
//! # 配置层级优先级（从高到低）
//! 1. 命令行参数（预留，暂未实现）
//! 2. 环境变量（格式：KNOWLEDGE_<SECTION>_<KEY>）
//! 3. config.toml 文件
//! 4. 内置默认值

use error_core::helpers;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// 应用程序配置结构体
/// 
/// 对应 config.toml 文件的结构，包含所有可配置项。
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// 服务器配置
    pub server: ServerConfig,
    /// 数据库配置
    pub database: DatabaseConfig,
    /// 安全配置
    pub security: SecurityConfig,
    /// 解析器配置
    pub parser: ParserConfig,
}

/// 服务器配置
#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    /// HTTP 服务端口
    pub port: u16,
    /// 主机地址
    pub host: String,
    /// 是否启用 TLS
    pub tls: bool,
    /// TLS 证书路径
    pub cert_path: Option<String>,
    /// TLS 私钥路径
    pub key_path: Option<String>,
    /// 最大请求大小（字节）
    pub max_request_size: u64,
}

/// 数据库配置
#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    /// `SurrealDB` 连接地址
    pub addr: String,
    /// 命名空间
    pub namespace: String,
    /// 数据库名称
    pub database: String,
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
}

/// 安全配置
#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    /// 加密密钥文件路径
    pub enc_key_path: String,
    /// JWT 密钥
    pub jwt_secret: String,
    /// JWT 过期时间（小时）
    pub jwt_expiry_hours: u8,
    /// 审计日志路径
    pub audit_log_path: String,
}

/// 解析器配置
#[derive(Debug, Deserialize, Clone)]
pub struct ParserConfig {
    /// 最大文件大小（字节）
    pub max_file_size: u64,
    /// 块大小（字符）
    pub chunk_size: usize,
    /// 重叠大小（字符）
    pub chunk_overlap: usize,
    /// 是否启用代码解析
    pub enable_code_parsing: bool,
    /// 嵌入向量维度
    #[serde(default = "default_embedding_dim")]
    pub embedding_dim: usize,
}

const fn default_embedding_dim() -> usize {
    1536
}

/// 配置加载器
/// 
/// 负责从 config.toml 文件和环境变量加载配置。
pub struct ConfigLoader;

impl ConfigLoader {
    /// 从文件加载配置
    /// 
    /// # 参数
    /// - `path` - config.toml 文件路径
    /// 
    /// # Returns
    /// 加载后的配置结构体
    /// # Errors
    ///
    /// 文件打开失败、读取失败或格式错误时返回错误。
    pub fn load_from_file(path: &Path) -> crate::Result<Config> {
        let mut file = File::open(path).map_err(|e| {
            helpers::config_error(&format!("无法打开配置文件: {e}"))
        })?;
        
        let mut content = String::new();
        file.read_to_string(&mut content).map_err(|e| {
            helpers::config_error(&format!("无法读取配置文件: {e}"))
        })?;
        
        let config: Config = toml::from_str(&content).map_err(|e| {
            helpers::config_error(&format!("配置文件格式错误: {e}"))
        })?;
        
        Ok(config)
    }
    
    /// 从默认位置加载配置
    /// 
    /// 按以下顺序查找：
    /// 1. ./config.toml
    /// 2. ~/.knowledge/config.toml
    /// 3. /etc/knowledge/config.toml
    /// # Errors
    ///
    /// 所有配置查找路径均不存在时返回错误。
    pub fn load_default() -> crate::Result<Config> {
        let home_config = dir::home_dir().map(|h| h.join(".knowledge/config.toml"));

        if Path::new("./config.toml").exists() {
            return Self::load_from_file(Path::new("./config.toml"));
        }

        if let Some(ref hc) = home_config {
            if hc.exists() {
                return Self::load_from_file(hc);
            }
        }

        #[cfg(unix)]
        if Path::new("/etc/knowledge/config.toml").exists() {
            return Self::load_from_file(Path::new("/etc/knowledge/config.toml"));
        }

        #[cfg(not(unix))]
        {
            let etc_config = dir::program_data_dir().join("knowledge/config.toml");
            if etc_config.exists() {
                return Self::load_from_file(&etc_config);
            }
        }

        let config = Self::default_config();
        Self::validate(&config)?;
        Ok(config)
    }
    
    /// 获取默认配置
    #[must_use]
    pub fn default_config() -> Config {
        Config {
            server: ServerConfig {
                port: 3000,
                host: "127.0.0.1".to_string(),
                tls: false,
                cert_path: None,
                key_path: None,
                max_request_size: 10 * 1024 * 1024, // 10MB
            },
            database: DatabaseConfig {
                addr: "ws://localhost:8000".to_string(),
                namespace: "knowledge".to_string(),
                database: "knowledge".to_string(),
                username: std::env::var("KNOWLEDGE_DB_USERNAME")
                    .unwrap_or_default(),
                password: std::env::var("KNOWLEDGE_DB_PASSWORD")
                    .unwrap_or_default(),
            },
            security: SecurityConfig {
                enc_key_path: "./enc.key".to_string(),
                jwt_secret: std::env::var("KNOWLEDGE_JWT_SECRET")
                    .unwrap_or_default(),
                jwt_expiry_hours: 24,
                audit_log_path: "./audit.log".to_string(),
            },
            parser: ParserConfig {
                max_file_size: 1024 * 1024 * 1024, // 1GB
                chunk_size: 1000,
                chunk_overlap: 100,
                enable_code_parsing: true,
                embedding_dim: 1536,
            },
        }
    }
    
    /// 验证配置的有效性
    /// # Errors
    ///
    /// 配置项不合法时返回错误（如端口为 0、地址为空等）。
    pub fn validate(config: &Config) -> crate::Result<()> {
        if config.server.port == 0 {
            return Err(helpers::validation_error("ServerConfig", "服务器端口不能为 0"));
        }
        
        if config.database.addr.is_empty() {
            return Err(helpers::validation_error("DatabaseConfig", "数据库地址不能为空"));
        }
        
        if config.database.username.is_empty() || config.database.password.is_empty() {
            return Err(helpers::config_error(
                "数据库凭据未设置，请设置环境变量 KNOWLEDGE_DB_USERNAME 和 KNOWLEDGE_DB_PASSWORD",
            ));
        }

        if config.security.jwt_secret.is_empty() {
            return Err(helpers::config_error(
                "JWT 密钥未设置，请设置环境变量 KNOWLEDGE_JWT_SECRET",
            ));
        }
        let enc_key_path = Path::new(&config.security.enc_key_path);
        if enc_key_path.exists() {
            let metadata = enc_key_path.metadata().map_err(|e| {
                helpers::config_error(&format!("无法读取密钥文件: {e}"))
            })?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let mode = metadata.mode();
                if (mode & 0o777) != 0o600 {
                    return Err(helpers::config_error("密钥文件权限必须为 600"));
                }
            }

            #[cfg(not(unix))]
            {
                let _ = metadata;
            }
        }

        if config.server.tls && (config.server.cert_path.is_none() || config.server.key_path.is_none()) {
            return Err(helpers::validation_error("ServerConfig", "启用 TLS 时必须设置 cert_path 和 key_path"));
        }

        if config.parser.max_file_size == 0 {
            return Err(helpers::validation_error("ParserConfig", "最大文件大小不能为 0"));
        }

        if config.parser.chunk_overlap >= config.parser.chunk_size {
            return Err(helpers::validation_error("ParserConfig", &format!(
                "chunk_overlap ({}) 必须小于 chunk_size ({})",
                config.parser.chunk_overlap, config.parser.chunk_size
            )));
        }

        if config.parser.embedding_dim == 0 {
            return Err(helpers::validation_error("ParserConfig", "嵌入向量维度不能为 0"));
        }
        
        Ok(())
    }
}

// 辅助模块，用于获取用户主目录
mod dir {
    use std::env;
    use std::path::PathBuf;
    
    pub fn home_dir() -> Option<PathBuf> {
        env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }

    pub fn program_data_dir() -> PathBuf {
        env::var_os("PROGRAMDATA")
            .map_or_else(|| PathBuf::from("C:\\ProgramData"), PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    
    #[test]
    fn test_default_config() {
        let config = ConfigLoader::default_config();
        
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.database.addr, "ws://localhost:8000");
        assert_eq!(config.security.enc_key_path, "./enc.key");
        assert_eq!(config.parser.max_file_size, 1024 * 1024 * 1024);
    }
    
    #[test]
    fn test_load_from_file() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        
        let config_content = r#"
[server]
port = 8080
host = "0.0.0.0"
tls = false
max_request_size = 20971520

[database]
addr = "ws://localhost:8000"
namespace = "test"
database = "test"
username = "test"
password = "test"

[security]
enc_key_path = "./test.key"
jwt_secret = "test-secret"
jwt_expiry_hours = 12
audit_log_path = "./test-audit.log"

[parser]
max_file_size = 536870912
chunk_size = 2000
chunk_overlap = 200
enable_code_parsing = true
embedding_dim = 768
"#;
        
        let mut file = File::create(&config_path).unwrap();
        file.write_all(config_content.as_bytes()).unwrap();
        
        let config = ConfigLoader::load_from_file(&config_path).unwrap();
        
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.database.namespace, "test");
        assert_eq!(config.security.jwt_expiry_hours, 12);
        assert_eq!(config.parser.chunk_size, 2000);
    }
    
    #[test]
    fn test_validate_config() {
        let mut config = ConfigLoader::default_config();
        config.database.username = "test_user".to_string();
        config.database.password = "test_pass".to_string();
        config.security.jwt_secret = "test-secret".to_string();
        let result = ConfigLoader::validate(&config);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_validate_invalid_port() {
        let mut config = ConfigLoader::default_config();
        config.database.username = "test_user".to_string();
        config.database.password = "test_pass".to_string();
        config.security.jwt_secret = "test-secret".to_string();
        config.server.port = 0;
        
        let result = ConfigLoader::validate(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message().contains("端口"));
    }
    
    #[test]
    fn test_validate_empty_database_addr() {
        let mut config = ConfigLoader::default_config();
        config.database.username = "test_user".to_string();
        config.database.password = "test_pass".to_string();
        config.security.jwt_secret = "test-secret".to_string();
        config.database.addr = String::new();
        
        let result = ConfigLoader::validate(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message().contains("数据库地址"));
    }
}