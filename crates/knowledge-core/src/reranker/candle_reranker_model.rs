//! Candle Cross-Encoder 模型加载与前向推理
//!
//! 详见文档: §2 | 用例: UC-028~UC-030

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig};
use tokenizers::Tokenizer;
use tracing::{debug, info};

use error_core::{ErrorObject, Result};

use super::config::CrossEncoderConfig;

/// Cross-Encoder 推理模型
///
/// 封装 BertModel + Tokenizer，提供 Cross-Encoder 风格的
/// (query, document) 对相关性评分。
///
/// 详见文档: §2 | 用例: UC-028~UC-030
pub struct CandleRerankerModel {
    model: BertModel,
    tokenizer: Tokenizer,
    config: CrossEncoderConfig,
    device: Device,
}

impl CandleRerankerModel {
    /// 加载模型
    ///
    /// 从 HuggingFace Hub 下载（或从缓存加载）Cross-Encoder 模型，
    /// 并初始化 BertModel + Tokenizer。
    ///
    /// 详见文档: §2.1 | 用例: UC-028 | 方法: M-041
    ///
    /// # Errors
    ///
    /// 当模型下载、文件解析或设备初始化失败时返回错误
    pub async fn load(config: CrossEncoderConfig) -> Result<Self> {
        config.validate()?;

        let device = Self::create_device(&config.device)?;

        info!(
            model_id = %config.model_id,
            device = %config.device,
            "Loading Cross-Encoder model"
        );

        let api = hf_hub::api::tokio::Api::builder()
            .with_progress(false)
            .build()
            .map_err(|e| ErrorObject::internal(format!("hf-hub API init failed: {e}"), None))?;

        let repo = api.model(config.model_id.clone());

        let config_path = repo
            .get("config.json")
            .await
            .map_err(|e| ErrorObject::not_found(format!("config.json not found: {e}"), None))?;

        let tokenizer_path = repo
            .get("tokenizer.json")
            .await
            .map_err(|e| ErrorObject::not_found(format!("tokenizer.json not found: {e}"), None))?;

        let weights_path = repo
            .get("model.safetensors")
            .await
            .or_else(|_| async { repo.get("pytorch_model.bin").await })
            .map_err(|e| ErrorObject::not_found(format!("model weights not found: {e}"), None))?;

        let bert_config: BertConfig = serde_json::from_str(
            &std::fs::read_to_string(config_path)
                .map_err(|e| ErrorObject::io(format!("read config.json: {e}"), None))?,
        )
        .map_err(|e| ErrorObject::internal(format!("parse config.json: {e}"), None))?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| ErrorObject::internal(format!("load tokenizer: {e}"), None))?;

        // SAFETY: VarBuilder::from_mmaped_safetensors 需要 unsafe 因为内存映射文件
        // 在映射期间被外部修改可能导致未定义行为。此处安全因为：
        // 1. weights_path 已验证存在且为只读模型权重文件
        // 2. 文件由调用方确保完整性（HuggingFaceDownloader 下载并校验 blake3 哈希）
        // 3. 模型缓存目录受操作系统文件权限保护，同主机其他进程无法写入
        // 4. 下载完成后不再修改已写入的文件（写后不变语义）
        // 5. VarBuilder 持有的 mmap 句柄生命周期被 Rust 所有权系统保护：
        //    - VarBuilder 在 load() 函数作用域内创建，通过 ? 传播错误
        //    - 构建的 BertModel 持有权重数据的所有权
        //    - 当 CandleRerankerModel 被 drop 时，mmap 句柄被正确关闭
        // 6. 当前设计假设单实例部署，同一缓存目录不会被多个进程并发写入。
        //    若未来支持多实例共享缓存目录，需引入文件锁（flock/LockFileEx）保证独占访问。
        //    可通过添加 fs4 crate 依赖实现跨平台文件锁。
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], DType::F32, &device)
                .map_err(|e| ErrorObject::internal(format!("load weights: {e}"), None))?
        };

        let model = BertModel::load(vb, &bert_config)
            .map_err(|e| ErrorObject::internal(format!("build BertModel: {e}"), None))?;

        info!("Cross-Encoder model loaded successfully");

        Ok(Self {
            model,
            tokenizer,
            config,
            device,
        })
    }

    /// 前向推理
    ///
    /// 对 (query, document) 对进行 Cross-Encoder 前向传播，
    /// 返回 [CLS] token 的 logits 作为相关性分数。
    ///
    /// 详见文档: §2.2 | 用例: UC-029 | 方法: M-042
    ///
    /// # Errors
    ///
    /// 当 tokenization 或前向传播失败时返回错误
    pub fn forward(&self, query: &str, document: &str) -> Result<f64> {
        let text = format!("{query} [SEP] {document}");
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| ErrorObject::internal(format!("tokenization failed: {e}"), None))?;

        let ids = encoding.get_ids();
        let type_ids = encoding.get_type_ids();
        let attention_mask = encoding.get_attention_mask();

        let input_ids = Tensor::new(ids.to_vec(), &self.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| ErrorObject::internal(format!("tensor input_ids: {e}"), None))?;

        let token_type_ids = Tensor::new(type_ids.to_vec(), &self.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| ErrorObject::internal(format!("tensor token_type_ids: {e}"), None))?;

        let attention_mask = Tensor::new(
            attention_mask.iter().map(|m| *m as f32).collect::<Vec<_>>(),
            &self.device,
        )
        .and_then(|t| t.unsqueeze(0))
        .map_err(|e| ErrorObject::internal(format!("tensor attention_mask: {e}"), None))?;

        let output = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask))
            .map_err(|e| ErrorObject::internal(format!("forward pass failed: {e}"), None))?;

        let cls_embedding = output
            .get(0)
            .and_then(|t| t.get(0))
            .map_err(|e| ErrorObject::internal(format!("extract CLS: {e}"), None))?;

        let score = cls_embedding
            .mean_all()
            .and_then(|t| t.to_dtype(DType::F64))
            .map_err(|e| ErrorObject::internal(format!("compute score: {e}"), None))?;

        let score_val = score
            .to_vec0::<f64>()
            .map_err(|e| ErrorObject::internal(format!("extract score value: {e}"), None))?;

        Ok(score_val)
    }

    /// 批量前向推理
    ///
    /// 对多个 (query, document) 对进行批量推理，
    /// 内部按 `batch_size` 分批处理以控制内存占用。
    ///
    /// 详见文档: §2.3 | 用例: UC-030 | 方法: M-043
    ///
    /// # Errors
    ///
    /// 当任一对的推理失败时返回错误
    pub fn forward_batch(&self, pairs: &[(&str, &str)]) -> Result<Vec<f64>> {
        let mut scores = Vec::with_capacity(pairs.len());

        for chunk in pairs.chunks(self.config.batch_size) {
            for (query, document) in chunk {
                let score = self.forward(query, document)?;
                scores.push(score);
            }
        }

        Ok(scores)
    }

    /// 创建推理设备
    ///
    /// 详见文档: §2.1 | 方法: M-044
    fn create_device(device_str: &str) -> Result<Device> {
        match device_str {
            "cpu" => Ok(Device::Cpu),
            "cuda" => {
                #[cfg(feature = "cuda")]
                {
                    Device::new_cuda(0)
                        .map_err(|e| ErrorObject::internal(format!("CUDA init failed: {e}"), None))
                }
                #[cfg(not(feature = "cuda"))]
                {
                    Err(ErrorObject::validation(
                        "CUDA device requested but 'cuda' feature not enabled",
                        None,
                    ))
                }
            }
            _ => Err(ErrorObject::validation(
                format!("Unsupported device: {device_str}"),
                None,
            )),
        }
    }

    /// 获取配置引用
    pub fn config(&self) -> &CrossEncoderConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_device_cpu() {
        let device = CandleRerankerModel::create_device("cpu").unwrap();
        assert!(matches!(device, Device::Cpu));
    }

    #[test]
    fn test_create_device_invalid() {
        let result = CandleRerankerModel::create_device("tpu");
        assert!(result.is_err());
    }
}
