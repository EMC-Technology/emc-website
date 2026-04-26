//! 基于哈希的伪嵌入实现
//!
/// 用于测试、原型开发和无GPU环境的fallback方案。
use crate::embedding_model::{EmbeddingConfig, EmbeddingError, EmbeddingModel as EmbeddingModelTrait, EmbeddingModelInfo, EmbeddingModelType, EmbeddingResult};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 基于哈希的伪嵌入模型
///
/// # ⚠️ 重要警告
///
/// 此模型**不具备语义理解能力**，仅适用于：
/// - 单元测试
/// - 原型开发
/// - 无GPU/无API访问权限的环境
///
/// ## 算法原理
///
/// 1. 将文本分词为 token 序列
/// 2. 对每个 token 计算双哈希 `(h1, h2)`
/// 3. 使用哈希值确定向量各维度的符号和权重
/// 4. L2 归一化得到单位向量
pub struct HashEmbedding {
    config: EmbeddingConfig,
    initialized: bool,
}

impl HashEmbedding {
    /// 创建新的 `HashEmbedding` 实例
    #[must_use]
    pub const fn new(config: EmbeddingConfig) -> Self {
        Self {
            config,
            initialized: false,
        }
    }

    /// 使用默认配置创建
    #[must_use]
    pub fn with_dimension(dim: usize) -> Self {
        Self::new(EmbeddingConfig {
            model_type: super::EmbeddingModelType::Hash,
            embedding_dim: dim,
            ..Default::default()
        })
    }

    /// 计算哈希嵌入向量的核心算法
    fn compute_hash_embedding(&self, text: &str) -> Vec<f32> {
        let dimension = self.config.embedding_dim;
        let mut v = vec![0.0f64; dimension];

        let query_tokens: Vec<&str> = text
            .split_whitespace()
            .chain(text.split(|c: char| !c.is_alphanumeric()))
            .filter(|t| !t.is_empty())
            .collect();

        if query_tokens.is_empty() {
            v[0] = 1.0;
            let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt().max(f64::EPSILON);
            #[allow(clippy::cast_possible_truncation)]
            return v.iter().map(|x| (x / norm) as f32).collect();
        }

        for token in &query_tokens {
            let mut hasher = DefaultHasher::new();
            token.hash(&mut hasher);
            let h1 = hasher.finish();

            hasher = DefaultHasher::new();
            format!("{token}:salt2").hash(&mut hasher);
            let h2 = hasher.finish();

            for (i, vec_item) in v.iter_mut().enumerate().take(dimension) {
                let bit_pos = (h1.wrapping_add((i as u64).wrapping_mul(h2))) % 64;
                let sign = if (h1 >> (bit_pos % 64)) & 1 == 1 {
                    1.0f64
                } else {
                    -1.0f64
                };
                #[allow(clippy::cast_precision_loss)]
                let weight = 1.0 + (h2 % 100) as f64 / 100.0;
                *vec_item += sign * weight;
            }
        }

        let norm = v
            .iter()
            .map(|x| x * x)
            .sum::<f64>()
            .sqrt()
            .max(f64::EPSILON);
        #[allow(clippy::cast_possible_truncation)]
        v.iter().map(|x| (x / norm) as f32).collect()
    }
}

impl EmbeddingModelTrait for HashEmbedding {
    fn name(&self) -> &'static str {
        "hash-embedding"
    }

    fn embedding_dim(&self) -> usize {
        self.config.embedding_dim
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn initialize(&mut self) -> Result<(), EmbeddingError> {
        tracing::info!(
            dim = self.config.embedding_dim,
            "初始化HashEmbedding伪嵌入模型"
        );
        self.initialized = true;
        Ok(())
    }

    fn embed(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError> {
        if text.trim().is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        let start = std::time::Instant::now();
        let vector = self.compute_hash_embedding(text);
        let inference_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        let token_count = text
            .split_whitespace()
            .count()
            .max(text.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).count());

        Ok(EmbeddingResult {
            vector,
            token_count,
            inference_time_ms,
            model_info: EmbeddingModelInfo {
                name: "HashEmbedding".to_string(),
                model_type: EmbeddingModelType::Hash,
                embedding_dim: self.config.embedding_dim,
                max_seq_length: self.config.max_seq_length,
                vocab_size: 0,
                is_loaded: true,
                backend: "hash".to_string(),
                version: "1.0.0".to_string(),
                model_size_bytes: 0,
                gpu_supported: false,
                device: "cpu".to_string(),
            },
        })
    }

    fn info(&self) -> EmbeddingModelInfo {
        EmbeddingModelInfo {
            name: "HashEmbedding".to_string(),
            model_type: EmbeddingModelType::Hash,
            version: "1.0.0".to_string(),
            embedding_dim: self.config.embedding_dim,
            max_seq_length: self.config.max_seq_length,
            vocab_size: 0,
            is_loaded: true,
            backend: "hash".to_string(),
            model_size_bytes: 0,
            gpu_supported: false,
            device: "cpu".to_string(),
        }
    }

    fn model_name(&self) -> &'static str {
        "hash"
    }
}
