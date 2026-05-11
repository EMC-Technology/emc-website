//! 嵌入模型类型定义与配置

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// 嵌入错误类型
#[derive(Error, Debug)]
pub enum EmbeddingError {
    /// 模型配置参数无效或缺失
    #[error("配置错误: {0}")]
    ConfigError(String),
    /// 尝试在模型未加载时执行推理
    #[error("模型未加载: {0}")]
    ModelNotLoaded(String),
    /// 模型权重文件加载失败
    #[error("模型加载失败: {0}")]
    ModelLoadFailed(String),
    /// 嵌入向量推理计算失败
    #[error("推理失败: {0}")]
    InferenceFailed(String),
    /// 分词器初始化或编码失败
    #[error("Tokenizer错误: {0}")]
    TokenizerError(String),
    /// 输入文本为空字符串
    #[error("输入为空")]
    EmptyInput,
    /// 文件读写等系统 IO 错误
    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<EmbeddingError> for error_core::ErrorObject {
    fn from(err: EmbeddingError) -> Self {
        use error_core::helpers;
        match err {
            EmbeddingError::ConfigError(msg) => helpers::embedding_config_error(&msg),
            EmbeddingError::ModelNotLoaded(msg) => helpers::embedding_model_not_loaded(&msg),
            EmbeddingError::ModelLoadFailed(msg) => helpers::embedding_model_load_failed(&msg),
            EmbeddingError::InferenceFailed(msg) => helpers::embedding_inference_failed(&msg),
            EmbeddingError::TokenizerError(msg) => helpers::embedding_tokenizer_error(&msg),
            EmbeddingError::EmptyInput => helpers::embedding_empty_input(),
            EmbeddingError::IoError(e) => helpers::embedding_io_error(&e.to_string()),
        }
    }
}

/// 嵌入模型类型枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EmbeddingModelType {
    /// 哈希嵌入（无需模型，用于测试）
    Hash,
    /// GEMMA 4.0 E4B 嵌入模型
    Gemma4E4b,
    /// `OpenAI text-embedding-ada-002` (未来)
    OpenAIAda002,
    /// 自定义模型
    Custom(String),
}

impl std::fmt::Display for EmbeddingModelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hash => write!(f, "hash"),
            Self::Gemma4E4b => write!(f, "gemma-4-e4b"),
            Self::OpenAIAda002 => write!(f, "openai-ada-002"),
            Self::Custom(name) => write!(f, "custom-{name}"),
        }
    }
}

impl std::str::FromStr for EmbeddingModelType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "hash" => Ok(Self::Hash),
            "gemma-4-e4b" | "gemma4e4b" | "gemma-4-4b" | "gemma4_4b" => Ok(Self::Gemma4E4b),
            "openai-ada-002" | "openai" => Ok(Self::OpenAIAda002),
            other if other.starts_with("custom-") => Ok(Self::Custom(
                other.strip_prefix("custom-").unwrap_or(other).to_string(),
            )),
            _ => Err(format!("未知的嵌入模型类型: {s}")),
        }
    }
}

/// 嵌入模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// 模型类型
    pub model_type: EmbeddingModelType,
    /// 嵌入维度
    pub embedding_dim: usize,
    /// 最大序列长度
    pub max_seq_length: usize,
    /// 是否使用GPU
    pub use_gpu: bool,
    /// `HuggingFace` 模型 ID
    pub model_id: Option<String>,
    /// 模型缓存目录
    pub cache_dir: Option<String>,
    /// 批处理大小
    pub batch_size: usize,
    /// 额外参数
    pub extra_params: HashMap<String, serde_json::Value>,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_type: EmbeddingModelType::Gemma4E4b,
            embedding_dim: 2560,
            max_seq_length: 8192,
            use_gpu: true,
            model_id: Some("google/gemma-4-e4b-it".to_string()),
            cache_dir: None,
            batch_size: 32,
            extra_params: HashMap::new(),
        }
    }
}

/// 嵌入模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModelInfo {
    /// 模型名称
    pub name: String,
    /// 模型类型
    pub model_type: EmbeddingModelType,
    /// 嵌入维度
    pub embedding_dim: usize,
    /// 最大序列长度
    pub max_seq_length: usize,
    /// 词汇表大小
    pub vocab_size: usize,
    /// 是否已加载
    pub is_loaded: bool,
    /// 推理后端
    pub backend: String,
    /// 版本
    pub version: String,
    /// 模型文件大小（字节）
    pub model_size_bytes: u64,
    /// 是否支持GPU
    pub gpu_supported: bool,
    /// 运行设备
    pub device: String,
}

/// 嵌入模型 Trait
#[async_trait::async_trait]
pub trait EmbeddingModel: Send + Sync {
    /// 生成文本的嵌入向量
    ///
    /// # Errors
    ///
    /// - `EmbeddingError::EmptyInput`：输入文本为空
    /// - `EmbeddingError::ModelNotLoaded`：模型未加载
    /// - `EmbeddingError::InferenceFailed`：推理失败
    /// - `EmbeddingError::TokenizerError`：分词器错误
    async fn embed(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError>;

    /// 批量生成文本的嵌入向量
    ///
    /// # Errors
    ///
    /// - `EmbeddingError::EmptyInput`：输入文本为空
    /// - `EmbeddingError::ModelNotLoaded`：模型未加载
    /// - `EmbeddingError::InferenceFailed`：推理失败
    /// - `EmbeddingError::TokenizerError`：分词器错误
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<EmbeddingResult>, EmbeddingError>;

    /// 获取嵌入维度
    fn embedding_dim(&self) -> usize;

    /// 获取模型名称
    fn model_name(&self) -> &str;

    /// 获取模型名称（同 `model_name`）
    fn name(&self) -> &str {
        self.model_name()
    }

    /// 检查模型是否已初始化
    fn is_initialized(&self) -> bool {
        false
    }

    /// 初始化模型
    ///
    /// # Errors
    ///
    /// - `EmbeddingError::ModelLoadFailed`：模型加载失败
    /// - `EmbeddingError::ConfigError`：配置错误
    fn initialize(&mut self) -> Result<(), EmbeddingError> {
        Ok(())
    }
    /// 返回嵌入模型的元信息
    ///
    /// 包含模型名称、类型、维度、设备信息等只读属性，
    /// 用于模型注册表查询和健康检查端点。
    #[must_use]
    fn info(&self) -> EmbeddingModelInfo {
        EmbeddingModelInfo {
            name: self.model_name().to_string(),
            model_type: EmbeddingModelType::Hash,
            embedding_dim: self.embedding_dim(),
            max_seq_length: 8192,
            vocab_size: 0,
            is_loaded: self.is_initialized(),
            backend: "unknown".to_string(),
            version: "1.0.0".to_string(),
            model_size_bytes: 0,
            gpu_supported: false,
            device: "cpu".to_string(),
        }
    }
}

/// 嵌入结果
///
/// `vector` 字段使用 `Arc<Vec<f32>>` 包装，使得 Clone 操作仅增加引用计数
/// 而非深拷贝整个嵌入向量（通常数千维），显著降低内存分配开销。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    /// 嵌入向量（`Arc` 包装，Clone 为浅拷贝）
    pub vector: Arc<Vec<f32>>,
    /// 使用的token数量
    pub token_count: usize,
    /// 推理耗时（毫秒）
    pub inference_time_ms: f64,
    /// 模型信息
    pub model_info: EmbeddingModelInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_error_display() {
        assert!(format!("{}", EmbeddingError::ConfigError("bad".to_string())).contains("bad"));
        assert!(format!("{}", EmbeddingError::ModelNotLoaded("x".to_string())).contains('x'));
        assert!(
            format!("{}", EmbeddingError::ModelLoadFailed("fail".to_string())).contains("fail")
        );
        assert!(format!("{}", EmbeddingError::InferenceFailed("err".to_string())).contains("err"));
        assert!(format!("{}", EmbeddingError::TokenizerError("tok".to_string())).contains("tok"));
        assert!(format!("{}", EmbeddingError::EmptyInput).contains("空"));
    }

    #[test]
    fn test_embedding_error_into_error_object() {
        let errors = vec![
            EmbeddingError::ConfigError("c".to_string()),
            EmbeddingError::ModelNotLoaded("m".to_string()),
            EmbeddingError::ModelLoadFailed("l".to_string()),
            EmbeddingError::InferenceFailed("i".to_string()),
            EmbeddingError::TokenizerError("t".to_string()),
            EmbeddingError::EmptyInput,
        ];
        for err in errors {
            let _obj: error_core::ErrorObject = err.into();
        }
    }

    #[test]
    fn test_embedding_model_type_display() {
        assert_eq!(format!("{}", EmbeddingModelType::Hash), "hash");
        assert_eq!(format!("{}", EmbeddingModelType::Gemma4E4b), "gemma-4-e4b");
        assert_eq!(
            format!("{}", EmbeddingModelType::OpenAIAda002),
            "openai-ada-002"
        );
        assert_eq!(
            format!("{}", EmbeddingModelType::Custom("my".to_string())),
            "custom-my"
        );
    }

    #[test]
    fn test_embedding_model_type_from_str() {
        assert_eq!(
            "hash".parse::<EmbeddingModelType>(),
            Ok(EmbeddingModelType::Hash)
        );
        assert_eq!(
            "gemma-4-e4b".parse::<EmbeddingModelType>(),
            Ok(EmbeddingModelType::Gemma4E4b)
        );
        assert_eq!(
            "gemma4e4b".parse::<EmbeddingModelType>(),
            Ok(EmbeddingModelType::Gemma4E4b)
        );
        assert_eq!(
            "gemma-4-4b".parse::<EmbeddingModelType>(),
            Ok(EmbeddingModelType::Gemma4E4b)
        );
        assert_eq!(
            "gemma4_4b".parse::<EmbeddingModelType>(),
            Ok(EmbeddingModelType::Gemma4E4b)
        );
        assert_eq!(
            "openai-ada-002".parse::<EmbeddingModelType>(),
            Ok(EmbeddingModelType::OpenAIAda002)
        );
        assert_eq!(
            "openai".parse::<EmbeddingModelType>(),
            Ok(EmbeddingModelType::OpenAIAda002)
        );
        assert_eq!(
            "custom-mymodel".parse::<EmbeddingModelType>(),
            Ok(EmbeddingModelType::Custom("mymodel".to_string()))
        );
        assert!("unknown".parse::<EmbeddingModelType>().is_err());
    }

    #[test]
    fn test_embedding_config_default() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.model_type, EmbeddingModelType::Gemma4E4b);
        assert_eq!(config.embedding_dim, 2560);
        assert!(config.use_gpu);
        assert_eq!(config.batch_size, 32);
    }

    #[test]
    fn test_embedding_model_info_serialization() {
        let info = EmbeddingModelInfo {
            name: "test-model".to_string(),
            model_type: EmbeddingModelType::Hash,
            embedding_dim: 256,
            max_seq_length: 512,
            vocab_size: 30000,
            is_loaded: true,
            backend: "cpu".to_string(),
            version: "1.0".to_string(),
            model_size_bytes: 1024,
            gpu_supported: false,
            device: "cpu".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let de: EmbeddingModelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(de.name, "test-model");
        assert_eq!(de.embedding_dim, 256);
    }

    #[test]
    fn test_embedding_result_serialization() {
        let result = EmbeddingResult {
            vector: Arc::new(vec![0.1, 0.2, 0.3]),
            token_count: 10,
            inference_time_ms: 5.0,
            model_info: EmbeddingModelInfo {
                name: "test".to_string(),
                model_type: EmbeddingModelType::Hash,
                embedding_dim: 3,
                max_seq_length: 512,
                vocab_size: 0,
                is_loaded: true,
                backend: "cpu".to_string(),
                version: "1.0".to_string(),
                model_size_bytes: 0,
                gpu_supported: false,
                device: "cpu".to_string(),
            },
        };
        let json = serde_json::to_string(&result).unwrap();
        let de: EmbeddingResult = serde_json::from_str(&json).unwrap();
        assert_eq!(de.token_count, 10);
        assert_eq!(de.vector.len(), 3);
    }

    #[test]
    fn test_embedding_model_type_serialization_roundtrip() {
        let types = [
            EmbeddingModelType::Hash,
            EmbeddingModelType::Gemma4E4b,
            EmbeddingModelType::OpenAIAda002,
            EmbeddingModelType::Custom("test".to_string()),
        ];
        for t in &types {
            let json = serde_json::to_string(t).unwrap();
            let de: EmbeddingModelType = serde_json::from_str(&json).unwrap();
            assert_eq!(*t, de);
        }
    }
}
