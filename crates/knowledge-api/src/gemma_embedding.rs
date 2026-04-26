//! GEMMA 4.0 E4B 嵌入模型实现
//!
//! 基于 Google Gemma 4 E4B 模型的语义嵌入实现。
//! 支持 Candle 后端进行本地推理，输出 2560 维语义向量。
//!
//! # 模型规格
//!
//! | 属性 | 值 |
//! |------|-----|
//! | `gemma-4-e4b-it` | E4B | 2560 | 通用语义嵌入 |
//! | 上下文长度 | 128K tokens |
//! | 许可证 | Apache 2.0 |

use super::candle_loader::CandleModelLoader;
use super::embedding_model::{EmbeddingConfig, EmbeddingModelInfo, EmbeddingModelType, EmbeddingResult, EmbeddingModel as EmbeddingModelTrait};
use super::model_loader::{LoadedModel, ModelConfig, ModelLoader, PoolingStrategy};
use super::EmbeddingError;

/// Gemma 4.0 E4B 嵌入模型实现
///
/// 基于 Candle 框架的高性能文本嵌入模型，支持 GPU 加速和批处理优化。
/// 适用于语义搜索、文档相似度计算、RAG 检索增强等场景。
pub struct GemmaEmbedding {
    /// 嵌入模型配置参数
    config: EmbeddingConfig,
    /// 模型元信息（名称、维度、版本等）
    model_info: EmbeddingModelInfo,
    /// 已加载的模型实例（None 表示尚未加载）
    loaded_model: Option<Box<dyn LoadedModel>>,
}

impl GemmaEmbedding {
    /// 创建新的 Gemma 嵌入模型实例
    ///
    /// # Errors
    ///
    /// - `EmbeddingError::ConfigError`：配置错误
    pub fn new(config: EmbeddingConfig) -> Result<Self, EmbeddingError> {
        let model_info = EmbeddingModelInfo {
            name: "GEMMA 4.0 E4B".to_string(),
            model_type: EmbeddingModelType::Gemma4E4b,
            embedding_dim: config.embedding_dim,
            max_seq_length: config.max_seq_length,
            vocab_size: 256_000,
            is_loaded: false,
            backend: "candle".to_string(),
            version: "1.0.0".to_string(),
            model_size_bytes: 0,
            gpu_supported: config.use_gpu,
            device: if config.use_gpu { "gpu" } else { "cpu" }.to_string(),
        };

        Ok(Self {
            config,
            model_info,
            loaded_model: None,
        })
    }

    /// 使用默认配置创建 Gemma 嵌入模型实例
    ///
    /// # Errors
    ///
    /// - `EmbeddingError::ConfigError`：配置错误
    pub fn new_with_defaults() -> Result<Self, EmbeddingError> {
        Self::new(EmbeddingConfig {
            model_type: EmbeddingModelType::Gemma4E4b,
            embedding_dim: 2560,
            max_seq_length: 8192,
            use_gpu: true,
            model_id: Some("google/gemma-4-e4b-it".to_string()),
            cache_dir: None,
            batch_size: 32,
            extra_params: std::collections::HashMap::new(),
        })
    }

    /// 加载 Gemma 模型
    ///
    /// # Errors
    ///
    /// - `EmbeddingError::ModelLoadFailed`：模型加载失败
    /// - `EmbeddingError::ConfigError`：配置错误
    pub async fn load_model(&mut self) -> Result<(), EmbeddingError> {
        let model_id = self.config.model_id.clone()
            .unwrap_or_else(|| "google/gemma-4-e4b-it".to_string());

        let device = if self.config.use_gpu {
            #[cfg(any(feature = "cuda", feature = "flash-attn"))]
            {
                super::model_loader::DeviceType::Cuda { device_id: 0 }
            }
            #[cfg(not(any(feature = "cuda", feature = "flash-attn")))]
            {
                tracing::warn!("use_gpu=true 但 CUDA 未编译，回退到 CPU 设备");
                super::model_loader::DeviceType::Cpu
            }
        } else {
            super::model_loader::DeviceType::Cpu
        };

        let loader_config = ModelConfig {
            model_id: model_id.clone(),
            backend: super::model_loader::ModelBackend::Candle,
            device,
            quantization: super::model_loader::Quantization::Fp16,
            max_seq_length: self.config.max_seq_length,
            batch_size: self.config.batch_size,
            cache_dir: self.config.cache_dir.as_ref().map(std::path::PathBuf::from),
            use_flash_attention: false,
            num_threads: 4,
            extra_params: self.config.extra_params.clone(),
        };

        let loader = CandleModelLoader::new(loader_config);
        let model = loader.load().await
            .map_err(|e| EmbeddingError::ModelLoadFailed(e.to_string()))?;

        self.loaded_model = Some(model);
        self.model_info.is_loaded = true;

        tracing::info!(
            model = "gemma-4-e4b",
            dim = self.config.embedding_dim,
            "GEMMA 4.0 E4B模型加载完成"
        );

        Ok(())
    }

    #[must_use]
    /// 检查模型是否已加载到内存中
    ///
    /// 返回 `true` 表示模型已就绪可以执行推理，
    /// 返回 `false` 表示需要先调用 [`load_model`] 加载模型。
    pub fn is_loaded(&self) -> bool {
        self.loaded_model.is_some()
    }

    #[must_use]
    /// 获取模型元信息引用
    ///
    /// 返回包含模型名称、嵌入维度、是否已加载等信息的不可变引用。
    pub const fn model_info(&self) -> &EmbeddingModelInfo {
        &self.model_info
    }
}

impl EmbeddingModelTrait for GemmaEmbedding {
    fn embed(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError> {
        let model = self.loaded_model.as_ref()
            .ok_or_else(|| EmbeddingError::ModelNotLoaded("Gemma模型未加载".to_string()))?;

        if text.trim().is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        let start = std::time::Instant::now();

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(
                model.embed(text, PoolingStrategy::Eos)
            )
        })
        .map_err(|e| EmbeddingError::InferenceFailed(e.to_string()))?;

        let inference_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        Ok(EmbeddingResult {
            vector: result,
            token_count: 0,
            inference_time_ms,
            model_info: self.model_info.clone(),
        })
    }

    fn embedding_dim(&self) -> usize {
        self.config.embedding_dim
    }

    fn model_name(&self) -> &'static str {
        "gemma-4-e4b"
    }
}
