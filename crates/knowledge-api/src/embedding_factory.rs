//! 嵌入模型工厂

use super::embedding_model::{
    EmbeddingConfig, EmbeddingModel as EmbeddingModelTrait, EmbeddingModelType,
};
use super::gemma_embedding::GemmaEmbedding;
use crate::embedding_model::EmbeddingError;

/// 嵌入模型工厂，根据配置创建对应的嵌入模型实例
pub struct EmbeddingFactory;

impl EmbeddingFactory {
    /// 创建嵌入模型实例
    ///
    /// # 参数
    ///
    /// * `config` - 嵌入模型配置
    ///
    /// # Returns
    ///
    /// 返回包装在 `Box<dyn EmbeddingModelTrait>` 中的嵌入模型实例
    ///
    /// # Errors
    ///
    /// - `EmbeddingError::ConfigError`：配置错误或模型类型不支持
    /// - `EmbeddingError::ModelLoadFailed`：模型加载失败
    pub fn create(config: EmbeddingConfig) -> Result<Box<dyn EmbeddingModelTrait>, EmbeddingError> {
        match config.model_type {
            EmbeddingModelType::Gemma4E4b => {
                tracing::info!(
                    dim = config.embedding_dim,
                    model_id = ?config.model_id,
                    "创建GEMMA 4.0 E4B嵌入模型"
                );
                let gemma = GemmaEmbedding::new(config)?;
                Ok(Box::new(gemma))
            }
            EmbeddingModelType::Hash => Err(EmbeddingError::ConfigError(
                "Hash嵌入模型不支持通过工厂创建，请直接使用HashEmbedding".to_string(),
            )),
            EmbeddingModelType::OpenAIAda002 => Err(EmbeddingError::ConfigError(
                "OpenAI嵌入模型尚未实现".to_string(),
            )),
            EmbeddingModelType::Custom(name) => Err(EmbeddingError::ConfigError(format!(
                "自定义嵌入模型 '{name}' 尚未实现"
            ))),
        }
    }

    /// 创建默认的 Gemma 4 E4B 嵌入模型配置
    ///
    /// # Returns
    ///
    /// 返回预配置的 `EmbeddingConfig` 实例
    #[must_use]
    pub fn default_gemma_e4b_config() -> EmbeddingConfig {
        EmbeddingConfig {
            model_type: EmbeddingModelType::Gemma4E4b,
            embedding_dim: 2560,
            max_seq_length: 8192,
            use_gpu: true,
            model_id: Some("google/gemma-4-e4b-it".to_string()),
            cache_dir: None,
            batch_size: 32,
            extra_params: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_gemma_e4b_config() {
        let config = EmbeddingFactory::default_gemma_e4b_config();
        assert_eq!(config.model_type, EmbeddingModelType::Gemma4E4b);
        assert_eq!(config.embedding_dim, 2560);
        assert_eq!(config.max_seq_length, 8192);
        assert!(config.use_gpu);
        assert_eq!(config.model_id.as_deref(), Some("google/gemma-4-e4b-it"));
        assert!(config.cache_dir.is_none());
        assert_eq!(config.batch_size, 32);
    }

    #[test]
    fn test_create_hash_model_returns_error() {
        let config = EmbeddingConfig {
            model_type: EmbeddingModelType::Hash,
            embedding_dim: 256,
            max_seq_length: 512,
            use_gpu: false,
            model_id: None,
            cache_dir: None,
            batch_size: 1,
            extra_params: std::collections::HashMap::new(),
        };
        let result = EmbeddingFactory::create(config);
        assert!(result.is_err());
        match result.err() {
            Some(EmbeddingError::ConfigError(msg)) => assert!(msg.contains("Hash")),
            other => panic!("Expected ConfigError, got: {other:?}"),
        }
    }

    #[test]
    fn test_create_openai_model_returns_error() {
        let config = EmbeddingConfig {
            model_type: EmbeddingModelType::OpenAIAda002,
            embedding_dim: 1536,
            max_seq_length: 8192,
            use_gpu: false,
            model_id: None,
            cache_dir: None,
            batch_size: 1,
            extra_params: std::collections::HashMap::new(),
        };
        let result = EmbeddingFactory::create(config);
        assert!(result.is_err());
        match result.err() {
            Some(EmbeddingError::ConfigError(msg)) => assert!(msg.contains("OpenAI")),
            other => panic!("Expected ConfigError, got: {other:?}"),
        }
    }

    #[test]
    fn test_create_custom_model_returns_error() {
        let config = EmbeddingConfig {
            model_type: EmbeddingModelType::Custom("my-model".to_string()),
            embedding_dim: 768,
            max_seq_length: 512,
            use_gpu: false,
            model_id: None,
            cache_dir: None,
            batch_size: 1,
            extra_params: std::collections::HashMap::new(),
        };
        let result = EmbeddingFactory::create(config);
        assert!(result.is_err());
        match result.err() {
            Some(EmbeddingError::ConfigError(msg)) => assert!(msg.contains("my-model")),
            other => panic!("Expected ConfigError, got: {other:?}"),
        }
    }
}
