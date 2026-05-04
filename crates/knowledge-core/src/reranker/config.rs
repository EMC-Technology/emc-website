//! Cross-Encoder 推理配置
//!
//! 详见文档: §6 | 用例: UC-027

use error_core::{ErrorObject, Result};

/// Cross-Encoder 推理配置
///
/// 详见文档: §6 | 用例: UC-027
#[derive(Debug, Clone)]
pub struct CrossEncoderConfig {
    /// HuggingFace 模型 ID（如 "cross-encoder/ms-marco-MiniLM-L-6-v2"）
    pub model_id: String,
    /// 最大序列长度（token 数）
    pub max_seq_length: usize,
    /// 推理批量大小
    pub batch_size: usize,
    /// 推理设备（"cpu" 或 "cuda"）
    pub device: String,
    /// 模型缓存目录
    pub cache_dir: Option<String>,
}

impl Default for CrossEncoderConfig {
    /// 创建默认配置
    ///
    /// 详见文档: §6 | 用例: UC-027 | 方法: M-038
    fn default() -> Self {
        Self {
            model_id: "cross-encoder/ms-marco-MiniLM-L-6-v2".to_string(),
            max_seq_length: 512,
            batch_size: 32,
            device: "cpu".to_string(),
            cache_dir: None,
        }
    }
}

impl CrossEncoderConfig {
    /// 验证配置有效性
    ///
    /// 详见文档: §6 | 用例: UC-027 | 方法: M-039
    ///
    /// # Errors
    ///
    /// 当配置参数不在有效范围内时返回错误
    pub fn validate(&self) -> Result<()> {
        if self.model_id.is_empty() {
            return Err(ErrorObject::validation("model_id must not be empty", None));
        }
        if self.max_seq_length == 0 {
            return Err(ErrorObject::validation("max_seq_length must be > 0", None));
        }
        if self.batch_size == 0 {
            return Err(ErrorObject::validation("batch_size must be > 0", None));
        }
        if self.device != "cpu" && self.device != "cuda" {
            return Err(ErrorObject::validation(
                "device must be 'cpu' or 'cuda'",
                None,
            ));
        }
        Ok(())
    }

    /// 从环境变量创建配置
    ///
    /// 详见文档: §6 | 用例: UC-027 | 方法: M-040
    ///
    /// 支持的环境变量：
    /// - `RERANKER_MODEL_ID`：模型 ID
    /// - `RERANKER_MAX_SEQ_LENGTH`：最大序列长度
    /// - `RERANKER_BATCH_SIZE`：批量大小
    /// - `RERANKER_DEVICE`：推理设备
    /// - `RERANKER_CACHE_DIR`：缓存目录
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(v) = std::env::var("RERANKER_MODEL_ID") {
            config.model_id = v;
        }
        if let Ok(v) = std::env::var("RERANKER_MAX_SEQ_LENGTH") {
            if let Ok(n) = v.parse() {
                config.max_seq_length = n;
            }
        }
        if let Ok(v) = std::env::var("RERANKER_BATCH_SIZE") {
            if let Ok(n) = v.parse() {
                config.batch_size = n;
            }
        }
        if let Ok(v) = std::env::var("RERANKER_DEVICE") {
            config.device = v;
        }
        if let Ok(v) = std::env::var("RERANKER_CACHE_DIR") {
            config.cache_dir = Some(v);
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = CrossEncoderConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_empty_model_id() {
        let config = CrossEncoderConfig {
            model_id: String::new(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_zero_max_seq_length() {
        let config = CrossEncoderConfig {
            max_seq_length: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_zero_batch_size() {
        let config = CrossEncoderConfig {
            batch_size: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_invalid_device() {
        let config = CrossEncoderConfig {
            device: "tpu".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_cuda_device() {
        let config = CrossEncoderConfig {
            device: "cuda".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }
}
