//! 模型加载器工厂和注册中心

use super::candle_loader::CandleModelLoader;
use super::model_loader::{
    DeviceType, ModelBackend, ModelConfig, ModelInfo, ModelLoader, ModelLoaderError, Quantization,
};
use std::collections::HashMap;
use std::sync::Arc;

/// 模型加载器工厂
pub struct ModelLoaderFactory;

impl ModelLoaderFactory {
    /// 创建模型加载器实例
    ///
    /// # Errors
    ///
    /// 当后端不支持或配置无效时返回错误。
    pub fn create(config: &ModelConfig) -> Result<Box<dyn ModelLoader>, ModelLoaderError> {
        match &config.backend {
            ModelBackend::Candle => Ok(Box::new(CandleModelLoader::new(config.clone()))),
            ModelBackend::MistralRs => Err(ModelLoaderError::UnsupportedBackend(
                "mistral-rs feature 未启用".to_string(),
            )),
            ModelBackend::OnnxRuntime => Err(ModelLoaderError::UnsupportedBackend(
                "ONNX Runtime 后端尚未实现".to_string(),
            )),
            ModelBackend::LlamaCpp => Err(ModelLoaderError::UnsupportedBackend(
                "llama.cpp 后端尚未实现".to_string(),
            )),
        }
    }

    /// 列出所有支持的后端
    #[must_use]
    pub fn supported_backends() -> Vec<ModelBackend> {
        vec![ModelBackend::Candle]
    }
}

/// 便捷函数：快速加载模型
///
/// # Errors
///
/// 当模型加载失败时返回错误。
pub async fn load_model(
    model_id: &str,
    device: DeviceType,
) -> Result<Arc<dyn super::model_loader::LoadedModel>, ModelLoaderError> {
    let config = ModelConfig {
        model_id: model_id.to_string(),
        device,
        ..Default::default()
    };
    load_model_with_config(&config).await
}

/// 便捷函数：使用完整配置加载模型
///
/// # Errors
///
/// 当工厂创建或模型加载失败时返回错误。
pub async fn load_model_with_config(
    config: &ModelConfig,
) -> Result<Arc<dyn super::model_loader::LoadedModel>, ModelLoaderError> {
    let loader = ModelLoaderFactory::create(config)?;
    let model = loader.load().await?;
    Ok(Arc::from(model))
}

/// 模型注册中心
pub struct ModelRegistry {
    models: tokio::sync::RwLock<HashMap<String, Arc<dyn super::model_loader::LoadedModel>>>,
}

impl ModelRegistry {
    #[must_use]
    /// 创建新的空模型注册表
    ///
    /// 初始化一个空的模型缓存映射，后续可通过 [`get_or_load`] 按需加载模型。
    pub fn new() -> Self {
        Self {
            models: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// # Errors
    ///
    /// 当模型加载失败时返回错误。
    pub async fn get_or_load(
        &self,
        key: &str,
        config: &ModelConfig,
    ) -> Result<Arc<dyn super::model_loader::LoadedModel>, ModelLoaderError> {
        {
            let cache = self.models.read().await;
            if let Some(model) = cache.get(key) {
                return Ok(Arc::clone(model));
            }
        }

        let model = load_model_with_config(config).await?;
        {
            let mut cache = self.models.write().await;
            cache.insert(key.to_string(), Arc::clone(&model));
        }
        Ok(model)
    }

    /// # Errors
    ///
    /// 当模型不存在时返回错误。
    pub async fn unload(&self, key: &str) -> Result<(), ModelLoaderError> {
        let mut cache = self.models.write().await;
        if cache.remove(key).is_some() {
            Ok(())
        } else {
            Err(ModelLoaderError::LoadFailed(format!("模型不存在: {key}")))
        }
    }

    /// # Errors
    ///
    /// 此函数当前不会返回错误（空实现）。
    /// 列出所有已注册的模型信息
    #[must_use]
    pub fn list_models() -> Vec<(String, ModelInfo)> {
        Self::builtin_models()
    }

    /// 返回内置模型列表
    #[must_use]
    pub fn builtin_models() -> Vec<(String, ModelInfo)> {
        vec![
            ("hash-embedding".to_string(), ModelInfo {
                name: "hash-embedding".to_string(),
                version: "1.0.0".to_string(),
                architecture: "Hash".to_string(),
                parameter_count_billion: 0.0,
                embedding_dim: 64,
                vocab_size: 0,
                max_context_length: 0,
                model_size_bytes: 0,
                backend: ModelBackend::Candle,
                device: DeviceType::Cpu,
                quantization: Quantization::default(),
            }),
            ("gemma-2b-embedding".to_string(), ModelInfo {
                name: "gemma-2b-embedding".to_string(),
                version: "1.0.0".to_string(),
                architecture: "Gemma".to_string(),
                parameter_count_billion: 2.0,
                embedding_dim: 2048,
                vocab_size: 256_000,
                max_context_length: 8192,
                model_size_bytes: 0,
                backend: ModelBackend::Candle,
                device: DeviceType::Cpu,
                quantization: Quantization::default(),
            }),
        ]
    }

    /// 清空模型缓存
    ///
    /// 移除所有已加载的模型实例并释放内存。
    /// 注意：正在使用这些模型的并发请求可能会失败。
    pub async fn clear_cache(&self) {
        let mut cache = self.models.write().await;
        cache.clear();
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_factory_create_candle_loader() {
        let config = ModelConfig {
            model_id: "gemma-4-e4b-it".to_string(),
            backend: ModelBackend::Candle,
            ..Default::default()
        };

        let loader = ModelLoaderFactory::create(&config).unwrap();
        assert_eq!(loader.name(), "candle-loader");
    }

    #[tokio::test]
    async fn test_registry_basic() {
        let _registry = ModelRegistry::new();
        let models = ModelRegistry::list_models();
        assert!(!models.is_empty(), "内置模型列表不应为空");
        let names: Vec<&str> = models.iter().map(|(name, _)| name.as_str()).collect();
        assert!(names.contains(&"hash-embedding"), "应包含 hash-embedding 内置模型");
    }
}
