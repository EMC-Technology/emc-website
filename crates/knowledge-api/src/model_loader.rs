//! 通用模型加载器抽象层
//!
//! 提供与具体ML框架无关的统一模型加载接口。
//! 支持多种后端：candle、mistral.rs、ONNX Runtime、llama.cpp等。
//!
//! # 架构设计
//!
//! ```
//! ┌─────────────────────────────────────┐
//! │       Application Layer             │
//! │   (KnowledgeVM / EmbeddingService)  │
//! └─────────────┬───────────────────────┘
//!               │
//! ┌─────────────▼───────────────────────┐
//! │     EmbeddingModel Trait           │
//! │   (业务逻辑层)                      │
//! └─────────────┬───────────────────────┘
//!               │
//! ┌─────────────▼───────────────────────┐
//! │      ModelLoader Trait            │
//! │   (框架无关的加载抽象)              │
//! ├─────────────────────────────────────┤
//! │  CandleLoader  │  MistralRsLoader  │
//! │  OnnxLoader    │  LlamaCppLoader   │
//! └─────────────────────────────────────┘
//! ```
//!
//! # 使用示例
//!
//! ```ignore
//! let config = ModelConfig {
//!     model_id: "gemma-4-e4b-it".to_string(),
//!     backend: ModelBackend::Candle,
//!     ..Default::default()
//! };
//!
//! let loader = ModelLoaderFactory::create(&config).await?;
//! let model = loader.load().await?;
//! let embedding = model.embed("Hello, world!").await?;
//! ```

use async_trait::async_trait;
use std::path::PathBuf;
use thiserror::Error;

/// 模型加载器错误类型
#[derive(Error, Debug)]
pub enum ModelLoaderError {
    /// 不支持的模型架构或模型 ID
    #[error("不支持的模型: {0}")]
    UnsupportedModel(String),

    /// 不支持的推理后端框架
    #[error("不支持的推理后端: {0}")]
    UnsupportedBackend(String),

    /// 模型文件路径不存在或无法访问
    #[error("模型文件未找到: {0}")]
    ModelNotFound(PathBuf),

    /// 模型权重加载或初始化失败
    #[error("模型加载失败: {0}")]
    LoadFailed(String),

    /// 分词器（Tokenizer）加载或初始化失败
    #[error("Tokenizer加载失败: {0}")]
    TokenizerFailed(String),

    /// 模型推理过程失败（如前向传播错误）
    #[error("推理失败: {0}")]
    InferenceFailed(String),

    /// 计算设备（GPU/CPU）初始化失败
    #[error("设备初始化失败: {0}")]
    DeviceInitFailed(String),

    /// 配置参数无效或缺失必要字段
    #[error("配置错误: {0}")]
    ConfigError(String),

    /// 文件读写等 IO 操作错误
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    /// JSON 序列化/反序列化错误
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    /// 从远程仓库下载模型文件失败
    #[error("模型下载失败: {0}")]
    DownloadFailed(String),
}

impl From<super::hf_downloader::ModelDownloadError> for ModelLoaderError {
    fn from(e: super::hf_downloader::ModelDownloadError) -> Self {
        Self::DownloadFailed(e.to_string())
    }
}

impl From<ModelLoaderError> for error_core::ErrorObject {
    fn from(err: ModelLoaderError) -> Self {
        use error_core::helpers;
        match err {
            ModelLoaderError::UnsupportedModel(msg) => {
                helpers::embedding_config_error(&format!("不支持的模型: {msg}"))
            }
            ModelLoaderError::UnsupportedBackend(msg) => {
                helpers::embedding_config_error(&format!("不支持的后端: {msg}"))
            }
            ModelLoaderError::ModelNotFound(path) => {
                helpers::not_found("model", &path.display().to_string())
            }
            ModelLoaderError::LoadFailed(msg) => helpers::embedding_model_load_failed(&msg),
            ModelLoaderError::TokenizerFailed(msg) => helpers::embedding_tokenizer_error(&msg),
            ModelLoaderError::InferenceFailed(msg) => helpers::embedding_inference_failed(&msg),
            ModelLoaderError::DeviceInitFailed(msg) => {
                helpers::embedding_model_load_failed(&format!("设备初始化失败: {msg}"))
            }
            ModelLoaderError::ConfigError(msg) => helpers::embedding_config_error(&msg),
            ModelLoaderError::Io(e) => helpers::embedding_io_error(&e.to_string()),
            ModelLoaderError::Serialization(e) => helpers::serde_error(&e.to_string()),
            ModelLoaderError::DownloadFailed(msg) => helpers::net_api_error(&msg),
        }
    }
}

/// 推理后端类型
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModelBackend {
    /// `HuggingFace` candle 框架（默认推荐）
    Candle,

    /// mistral.rs 框架（高性能优化）
    MistralRs,

    /// ONNX Runtime（跨平台）
    OnnxRuntime,

    /// llama.cpp Rust 绑定（量化优化）
    LlamaCpp,
}

impl std::fmt::Display for ModelBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Candle => write!(f, "candle"),
            Self::MistralRs => write!(f, "mistral-rs"),
            Self::OnnxRuntime => write!(f, "onnx-runtime"),
            Self::LlamaCpp => write!(f, "llama-cpp"),
        }
    }
}

/// 设备类型
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum DeviceType {
    /// CPU设备
    #[default]
    Cpu,

    /// CUDA GPU 设备 —— 需指定 GPU 设备编号
    Cuda {
        /// CUDA GPU 设备 ID（从 0 开始）
        device_id: usize,
    },

    /// Apple Metal (MPS)
    Metal,
}

/// 量化精度
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum Quantization {
    /// FP32 (无量化)
    Fp32,

    /// FP16 (半精度)
    #[default]
    Fp16,

    /// INT8 (8位整数)
    Int8,

    /// INT4 (4位整数，极端压缩)
    Int4,
}

/// 通用模型配置，支持 `` `ONNX` `` 和 `` `GGUF` `` 格式
///
/// 统一的模型配置，适用于所有后端。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelConfig {
    /// 模型标识符（HuggingFace repo ID 或本地路径）
    pub model_id: String,

    /// 推理后端
    pub backend: ModelBackend,

    /// 计算设备
    pub device: DeviceType,

    /// 量化精度
    pub quantization: Quantization,

    /// 最大序列长度
    pub max_seq_length: usize,

    /// 批处理大小
    pub batch_size: usize,

    /// 模型缓存目录
    pub cache_dir: Option<PathBuf>,

    /// 是否使用Flash Attention（如果后端支持）
    pub use_flash_attention: bool,

    /// 线程数（用于CPU并行计算）
    pub num_threads: usize,

    /// 额外参数（传递给底层框架）
    pub extra_params: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model_id: "gemma-4-e4b-it".to_string(),
            backend: ModelBackend::Candle,
            device: DeviceType::default(),
            quantization: Quantization::default(),
            max_seq_length: 512,
            batch_size: 32,
            cache_dir: None,
            use_flash_attention: false,
            num_threads: 4,
            extra_params: std::collections::HashMap::new(),
        }
    }
}

/// 模型元信息
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// 模型名称
    pub name: String,

    /// 模型版本
    pub version: String,

    /// 架构类型（如 "Gemma", "Llama", "Qwen"）
    pub architecture: String,

    /// 参数量（十亿）
    pub parameter_count_billion: f64,

    /// 嵌入维度
    pub embedding_dim: usize,

    /// 词汇表大小
    pub vocab_size: usize,

    /// 最大上下文长度
    pub max_context_length: usize,

    /// 模型文件大小（字节）
    pub model_size_bytes: u64,

    /// 使用的后端
    pub backend: ModelBackend,

    /// 使用的设备
    pub device: DeviceType,

    /// 量化方式
    pub quantization: Quantization,
}

/// Tokenizer 信息
#[derive(Debug, Clone)]
pub struct TokenizerInfo {
    /// Tokenizer 类型（如 `SentencePiece`、`BPE`、`WordPiece`）
    pub tokenizer_type: String,

    /// 特殊token映射
    pub special_tokens: std::collections::HashMap<String, u32>,

    /// 词汇表大小
    pub vocab_size: usize,
}

/// 推理结果
#[derive(Debug, Clone)]
pub struct InferenceResult {
    /// 输出向量
    pub output: Vec<f32>,

    /// 隐藏状态（可选，用于提取句子嵌入）
    pub hidden_states: Option<Vec<Vec<f32>>>,

    /// 使用的token数量
    pub token_count: usize,

    /// 推理耗时（毫秒）
    pub inference_time_ms: f64,

    /// 内存峰值占用（字节）
    pub peak_memory_bytes: Option<u64>,
}

/// 通用模型加载器 Trait
///
/// 定义所有模型加载器必须实现的接口。
/// 与具体的ML框架（candle、ONNX等）完全解耦。
#[async_trait]
pub trait ModelLoader: Send + Sync {
    /// 获取加载器名称
    fn name(&self) -> &str;

    /// 支持的后端类型
    fn supported_backend(&self) -> ModelBackend;

    /// 检查是否支持指定模型
    ///
    /// 参数:
    /// * `model_id` - 模型标识符
    ///
    /// 返回:
    /// * `Ok(true)` 如果支持
    /// * `Err` 如果无法确定
    /// # Errors
    ///
    /// 当无法确定模型是否支持时返回错误。
    fn supports_model(&self, model_id: &str) -> Result<bool, ModelLoaderError>;

    /// 加载模型（懒加载）
    ///
    /// 此方法应：
    /// 1. 验证模型文件存在性
    /// 2. 加载模型权重
    /// 3. 初始化Tokenizer
    /// 4. 配置计算设备
    async fn load(&self) -> Result<Box<dyn LoadedModel>, ModelLoaderError>;

    /// 卸载模型并释放资源
    async fn unload(&mut self) -> Result<(), ModelLoaderError>;

    /// 获取模型信息（无需完整加载）
    async fn get_model_info(&self) -> Result<ModelInfo, ModelLoaderError>;
}

/// 已加载的模型实例
///
/// 提供统一的推理接口，屏蔽底层框架差异。
#[async_trait]
pub trait LoadedModel: Send + Sync {
    /// 获取模型信息
    fn info(&self) -> &ModelInfo;

    /// 获取Tokenizer信息
    fn tokenizer_info(&self) -> &TokenizerInfo;

    /// 文本编码为Token IDs
    async fn encode(&self, text: &str) -> Result<Vec<u32>, ModelLoaderError>;

    /// Token IDs 解码为文本
    async fn decode(&self, token_ids: &[u32]) -> Result<String, ModelLoaderError>;

    /// 单文本推理
    ///
    /// 用于嵌入向量生成、文本分类等任务。
    async fn infer(&self, text: &str) -> Result<InferenceResult, ModelLoaderError>;

    /// 批量推理
    ///
    /// 高性能批处理，自动padding到相同长度。
    async fn infer_batch(
        &self,
        texts: &[String],
    ) -> Result<Vec<InferenceResult>, ModelLoaderError> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.infer(text).await?);
        }
        Ok(results)
    }

    /// 提取句子嵌入
    ///
    /// 从推理结果中提取固定维度的句子表示。
    /// 常用策略：
    /// - [EOS] token位置的隐藏状态
    /// - 平均池化（Mean Pooling）
    /// - 最大池化（Max Pooling）
    async fn embed(
        &self,
        text: &str,
        pooling_strategy: PoolingStrategy,
    ) -> Result<Vec<f32>, ModelLoaderError> {
        let result = self.infer(text).await?;
        Ok(self.extract_embedding(&result, pooling_strategy))
    }

    /// 批量提取句子嵌入
    ///
    /// 从批量推理结果中提取固定维度的句子表示。
    /// 常用策略：
    /// - [EOS] token位置的隐藏状态
    /// - 平均池化（Mean Pooling）
    /// - 最大池化（Max Pooling）
    async fn embed_batch(
        &self,
        texts: &[&str],
        pooling_strategy: PoolingStrategy,
    ) -> Result<Vec<Vec<f32>>, ModelLoaderError> {
        let texts: Vec<String> = texts.iter().map(|&s| s.to_string()).collect();
        let results = self.infer_batch(&texts).await?;
        let mut embeddings = Vec::with_capacity(results.len());
        for result in results {
            embeddings.push(self.extract_embedding(&result, pooling_strategy));
        }
        Ok(embeddings)
    }

    /// 从推理结果中提取嵌入向量
    fn extract_embedding(&self, result: &InferenceResult, strategy: PoolingStrategy) -> Vec<f32> {
        match &result.hidden_states {
            Some(states) if !states.is_empty() => {
                match strategy {
                    PoolingStrategy::Eos => {
                        // 使用最后一个位置的状态
                        states.last().cloned().unwrap_or_default()
                    }
                    PoolingStrategy::Mean => {
                        // 平均池化所有位置
                        let seq_len = states.len();
                        if seq_len == 0 {
                            return vec![0.0; self.info().embedding_dim];
                        }
                        let dim = states[0].len();
                        let mut pooled = vec![0.0f64; dim];
                        for state in states {
                            for (i, val) in state.iter().enumerate() {
                                pooled[i] += f64::from(*val);
                            }
                        }
                        #[allow(clippy::cast_precision_loss)]
                        #[allow(clippy::cast_possible_truncation)]
                        pooled
                            .iter()
                            .map(|x| (*x / (seq_len as f64)) as f32)
                            .collect()
                    }
                    PoolingStrategy::Max => {
                        // 最大池化
                        let dim = states[0].len();
                        let mut pooled = vec![f32::MIN; dim];
                        for state in states {
                            for (i, val) in state.iter().enumerate() {
                                pooled[i] = pooled[i].max(*val);
                            }
                        }
                        pooled
                    }
                    PoolingStrategy::Cls => {
                        // 使用[CLS] token位置的状态（第一个位置）
                        states.first().cloned().unwrap_or_default()
                    }
                }
            }
            _ => result.output.clone(),
        }
    }
}

/// 池化策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum PoolingStrategy {
    /// 使用[EOS] (End of Sequence) token位置的隐藏状态
    #[default]
    Eos,

    /// 平均池化所有token的隐藏状态
    Mean,

    /// 最大池化所有token的隐藏状态
    Max,

    /// 使用[CLS] token位置的隐藏状态（BERT风格）
    Cls,
}
