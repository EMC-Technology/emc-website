//! 基于哈希的伪嵌入实现
//!
//! 用于测试、原型开发和无GPU环境的fallback方案。
//!
//! # 确定性保证
//!
//! 使用 blake3 替代 `std::hash::DefaultHasher`（SipHash）以保证跨版本/跨平台的哈希确定性。
//! `DefaultHasher` 的输出在不同 Rust 版本间可能变化，违反本项目"0 随机性"设计哲学。
//! blake3 提供密码学级确定性：相同输入永远产生相同输出，不受编译器版本或平台影响。

use crate::embedding_model::{
    EmbeddingConfig, EmbeddingError, EmbeddingModel as EmbeddingModelTrait, EmbeddingModelInfo,
    EmbeddingModelType, EmbeddingResult,
};
use std::sync::Arc;

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
/// 2. 对每个 token 使用 blake3 计算双哈希 `(h1, h2)`
/// 3. 使用哈希值确定向量各维度的符号和权重
/// 4. L2 归一化得到单位向量
///
/// ## 确定性
///
/// blake3 保证：相同 token 在任何 Rust 版本、任何操作系统、任何 CPU 架构上
/// 产生字节级一致的哈希输出。这符合本项目"0 随机性，0 黑盒推断"的设计哲学。
pub struct HashEmbedding {
    config: EmbeddingConfig,
    initialized: bool,
}

/// 从 blake3 哈希输出中提取 u64（小端序）
///
/// blake3 输出 32 字节，取前 8 字节作为 u64。
/// 此函数是确定性的：相同输入永远产生相同输出。
#[inline]
fn blake3_to_u64(data: &[u8]) -> u64 {
    let hash = blake3::hash(data);
    u64::from_le_bytes(hash.as_bytes()[..8].try_into().expect("blake3 输出至少 8 字节"))
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
    ///
    /// 使用 blake3 替代 `DefaultHasher`，确保跨版本/跨平台确定性。
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
            let norm = v
                .iter()
                .map(|x| x * x)
                .sum::<f64>()
                .sqrt()
                .max(f64::EPSILON);
            #[allow(clippy::cast_possible_truncation)]
            return v.iter().map(|x| (x / norm) as f32).collect();
        }

        for token in &query_tokens {
            let h1 = blake3_to_u64(token.as_bytes());
            let h2 = blake3_to_u64(format!("{token}:salt2").as_bytes());

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

#[async_trait::async_trait]
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

    async fn embed(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError> {
        if text.trim().is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        let start = std::time::Instant::now();
        let vector = self.compute_hash_embedding(text);
        let inference_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        let token_count = text.split_whitespace().count().max(
            text.split(|c: char| !c.is_alphanumeric())
                .filter(|t| !t.is_empty())
                .count(),
        );

        Ok(EmbeddingResult {
            vector: Arc::new(vector),
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

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<EmbeddingResult>, EmbeddingError> {
        let start = std::time::Instant::now();
        let mut results = Vec::with_capacity(texts.len());

        for text in texts {
            if text.trim().is_empty() {
                return Err(EmbeddingError::EmptyInput);
            }

            let vector = self.compute_hash_embedding(text);
            let token_count = text.split_whitespace().count().max(
                text.split(|c: char| !c.is_alphanumeric())
                    .filter(|t| !t.is_empty())
                    .count(),
            );

            results.push(EmbeddingResult {
                vector: Arc::new(vector),
                token_count,
                inference_time_ms: 0.0,
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
            });
        }

        let total_time_ms = start.elapsed().as_secs_f64() * 1000.0;
        #[allow(clippy::cast_precision_loss)]
        for result in &mut results {
            result.inference_time_ms = total_time_ms / texts.len().max(1) as f64;
        }

        Ok(results)
    }
}
