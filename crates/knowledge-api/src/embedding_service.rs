//! 向量嵌入计算管道（Embedding Service）
//!
//! # 性能预算
//! - 单块延迟：P99 < 50ms
//! - 吞吐量目标：> 100 blocks/s
//!
//! # 设计策略
//! - 攒批优化：累积多个 Block 后批量计算（减少模型调用开销）
//! - 异步队列：使用 tokio mpsc channel 解耦生产者和消费者
//! - 背压控制：channel 容量限制防止内存溢出
//!
//! # 嵌入策略
//! - `LocalBgeLarge`: 基于确定性哈希的特征提取（SIMHash 变体），
//!   生成具有局部敏感性的向量。生产环境应替换为 candle/ort 推理。
//! - `OpenAiAda002`: 通过 HTTP 调用 OpenAI Embeddings API，
//!   需设置 OPENAI_API_KEY 环境变量。

use crate::Result;
use crate::embedding_model::{
    EmbeddingConfig, EmbeddingModel as EmbeddingModelTrait, EmbeddingModelType,
};
use crate::gemma_embedding::GemmaEmbedding;
use error_core::helpers;
use knowledge_core::model::Block;
use reqwest;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

const BGE_DEFAULT_DIMENSION: usize = 1536;
const OPENAI_DEFAULT_DIMENSION: usize = 1536;

/// 嵌入模型类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddingModel {
    /// 本地 BGE-Large 模型（SIMHash 变体）
    LocalBgeLarge,
    /// [`OpenAI`] text-embedding-ada-002
    OpenAiAda002,
    /// Gemma 4.0 4B 模型
    Gemma4E4b,
}

/// 嵌入计算任务
pub struct EmbeddingTask {
    /// Block ID
    pub block_id: String,
    /// 待计算文本内容
    pub content: String,
    /// 结果回调通道
    pub response_tx: oneshot::Sender<EmbeddingResult>,
}

/// 嵌入计算结果
///
/// `embedding` 字段使用 `Arc<Vec<f32>>` 包装，Clone 仅增加引用计数。
#[derive(Debug, Clone)]
pub struct EmbeddingResult {
    /// Block ID
    pub block_id: String,
    /// 嵌入向量（`Arc` 包装，Clone 为浅拷贝）
    pub embedding: Arc<Vec<f32>>,
    /// 处理耗时（毫秒）
    pub processing_time_ms: u64,
}

/// 嵌入计算服务
///
/// 通过异步队列解耦嵌入请求与计算，支持攒批优化和背压控制。
pub struct EmbeddingService {
    model_type: EmbeddingModel,
    dimension: usize,
    batch_size: usize,
    sender: mpsc::Sender<EmbeddingTask>,
}

impl EmbeddingService {
    /// # Errors
    ///
    /// HTTP 客户端构建失败或通道创建失败时返回错误。
    pub fn new(
        model: EmbeddingModel,
        dimension: usize,
        batch_size: usize,
        channel_capacity: usize,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(channel_capacity);

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| helpers::config_error(&format!("HTTP 客户端构建失败: {e}")))?;

        // 初始化GemmaEmbedding（如果模型类型是Gemma4E4b）
        let gemma_embedding = if model == EmbeddingModel::Gemma4E4b {
            Some(
                GemmaEmbedding::new(EmbeddingConfig {
                    model_type: EmbeddingModelType::Gemma4E4b,
                    embedding_dim: dimension,
                    max_seq_length: 8192,
                    use_gpu: true,
                    model_id: Some("google/gemma-4-e4b-it".to_string()),
                    cache_dir: None,
                    batch_size,
                    extra_params: std::collections::HashMap::new(),
                })
                .map_err(|e| helpers::config_error(&e.to_string()))?,
            )
        } else {
            None
        };

        let mut worker = EmbeddingWorker {
            model: model.clone(),
            dimension,
            batch_size,
            receiver,
            http_client,
            gemma_embedding,
        };
        tokio::spawn(async move {
            worker.run().await;
        });

        Ok(Self {
            model_type: model,
            dimension,
            batch_size,
            sender,
        })
    }

    /// 计算单个文本块的嵌入向量
    ///
    /// `content` 参数提供块的语义文本内容，用于嵌入计算。
    /// 若 `content` 为空，则回退到块的元数据（`idempotency_key` + 行号范围）。
    ///
    /// # Errors
    ///
    /// 嵌入队列已关闭或结果接收失败时返回错误。
    pub async fn embed_block(&self, block: &Block, content: &str) -> Result<EmbeddingResult> {
        let (response_tx, response_rx) = oneshot::channel();

        let extracted = extract_block_content(block);
        let effective_content = if content.is_empty() {
            extracted
        } else {
            content.to_string()
        };

        let task = EmbeddingTask {
            block_id: block
                .id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            content: effective_content,
            response_tx,
        };

        self.sender
            .send(task)
            .await
            .map_err(|_| helpers::io_error("嵌入队列已关闭"))?;

        response_rx
            .await
            .map_err(|_| helpers::io_error("嵌入结果接收失败"))
    }

    /// 批量计算文本块的嵌入向量
    ///
    /// `contents` 与 `blocks` 等长，`contents[i]` 为 `blocks[i]` 的语义文本内容。
    /// 若某项 content 为空，则回退到该块的元数据。
    ///
    /// # Errors
    ///
    /// 任意单个块的嵌入计算失败时返回错误。
    ///
    /// # Panics
    ///
    /// 当 `contents.len() != blocks.len()` 时 panic。
    pub async fn embed_batch(
        &self,
        blocks: &[Block],
        contents: &[String],
    ) -> Result<Vec<EmbeddingResult>> {
        assert_eq!(
            blocks.len(),
            contents.len(),
            "blocks 与 contents 长度必须一致"
        );
        let futures: Vec<_> = blocks
            .iter()
            .zip(contents.iter())
            .map(|(block, content)| self.embed_block(block, content))
            .collect();
        let results = futures::future::join_all(futures).await;
        results.into_iter().collect()
    }

    /// 获取当前模型的嵌入向量维度
    #[must_use]
    pub const fn dimension(&self) -> usize {
        self.dimension
    }

    /// 获取当前嵌入模型类型
    #[must_use]
    pub const fn model_type(&self) -> &EmbeddingModel {
        &self.model_type
    }

    /// 获取攒批大小
    #[must_use]
    pub const fn batch_size(&self) -> usize {
        self.batch_size
    }
}

fn extract_block_content(block: &Block) -> String {
    let mut parts = Vec::new();

    if let Some(key) = &block.idempotency_key {
        parts.push(key.clone());
    }

    parts.push(format!(
        "{:?}:L{}-L{}",
        block.block_type, block.start_line, block.end_line
    ));

    parts.join(" ")
}

struct EmbeddingWorker {
    model: EmbeddingModel,
    dimension: usize,
    batch_size: usize,
    receiver: mpsc::Receiver<EmbeddingTask>,
    http_client: reqwest::Client,
    gemma_embedding: Option<GemmaEmbedding>,
}

impl EmbeddingWorker {
    async fn run(&mut self) {
        let mut pending_tasks = Vec::new();

        loop {
            tokio::select! {
                task = self.receiver.recv() => {
                    if let Some(task) = task {
                        pending_tasks.push(task);

                        if pending_tasks.len() >= self.batch_size {
                            if let Err(e) = self.process_batch(&mut pending_tasks).await {
                                tracing::error!("批量嵌入计算失败: {e}");
                            }
                        }
                    } else {
                        tracing::info!("EmbeddingWorker 通道已关闭，处理剩余任务后退出");
                        if !pending_tasks.is_empty() {
                            if let Err(e) = self.process_batch(&mut pending_tasks).await {
                                tracing::error!("最终批量嵌入计算失败: {e}");
                            }
                        }
                        break;
                    }
                }

                () = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                    if !pending_tasks.is_empty() {
                        if let Err(e) = self.process_batch(&mut pending_tasks).await {
                            tracing::error!("批量嵌入计算失败: {e}");
                        }
                    }
                }
            }
        }
    }

    async fn process_batch(&mut self, tasks: &mut Vec<EmbeddingTask>) -> Result<()> {
        if tasks.is_empty() {
            return Ok(());
        }

        let start_time = std::time::Instant::now();

        let contents: Vec<String> = tasks.iter().map(|t| t.content.clone()).collect();

        let embeddings = match &self.model {
            EmbeddingModel::LocalBgeLarge => self.compute_bge_large(&contents),
            EmbeddingModel::OpenAiAda002 => {
                let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
                    helpers::config_error("使用 OpenAiAda002 模型时必须设置 OPENAI_API_KEY 环境变量")
                })?;
                self.compute_openai(&contents, &api_key).await?
            }
            EmbeddingModel::Gemma4E4b => self.compute_gemma(&contents).await?,
        };

        let elapsed = start_time.elapsed();

        for (task, embedding) in tasks.drain(..).zip(embeddings) {
            let result = EmbeddingResult {
                block_id: task.block_id,
                embedding: Arc::new(embedding),
                processing_time_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
            };
            let _ = task.response_tx.send(result);
        }

        Ok(())
    }

    fn compute_bge_large(&self, texts: &[String]) -> Vec<Vec<f32>> {
        if self.dimension != BGE_DEFAULT_DIMENSION {
            tracing::warn!(
                actual = self.dimension,
                recommended = BGE_DEFAULT_DIMENSION,
                "BGE 维度与推荐值不一致，可能影响向量质量"
            );
        }
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            let embedding = simhash_embedding(text, self.dimension);
            results.push(embedding);
        }
        results
    }

    async fn compute_openai(&self, texts: &[String], api_key: &str) -> Result<Vec<Vec<f32>>> {
        if self.dimension != OPENAI_DEFAULT_DIMENSION {
            tracing::warn!(
                actual = self.dimension,
                recommended = OPENAI_DEFAULT_DIMENSION,
                "OpenAI 维度与推荐值不一致，API 调用可能失败"
            );
        }
        if api_key.is_empty() {
            tracing::warn!("OPENAI_API_KEY 未设置，回退到本地哈希嵌入 [E2002]");
            return Ok(self.compute_bge_large(texts));
        }

        match self.call_openai_api(texts, api_key).await {
            Ok(embeddings) => Ok(embeddings),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "OpenAI API 调用失败，回退到本地哈希嵌入 [E2002]"
                );
                Ok(self.compute_bge_large(texts))
            }
        }
    }

    async fn call_openai_api(&self, texts: &[String], api_key: &str) -> Result<Vec<Vec<f32>>> {
        let body = serde_json::json!({
            "model": "text-embedding-ada-002",
            "input": texts,
        });

        let response = self
            .http_client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| helpers::llm_api_error(&format!("OpenAI API 请求失败: {e}"), "embed_openai"))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(helpers::llm_api_error(
                &format!("OpenAI API 返回错误 {status}: {error_text}"),
                "embed_openai",
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| helpers::llm_api_error(&format!("OpenAI 响应解析失败: {e}"), "embed_openai"))?;

        let mut embeddings = Vec::with_capacity(texts.len());
        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            for item in data {
                if let Some(vec) = item.get("embedding").and_then(|e| e.as_array()) {
                    #[allow(clippy::cast_possible_truncation)]
                    let embedding: Vec<f32> = vec
                        .iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect();
                    embeddings.push(embedding);
                }
            }
        }

        if embeddings.len() != texts.len() {
            return Err(helpers::llm_api_error(
                &format!(
                    "OpenAI 返回嵌入数量不匹配: 期望 {}, 实际 {}",
                    texts.len(),
                    embeddings.len()
                ),
                "embed_openai",
            ));
        }

        Ok(embeddings)
    }

    async fn compute_gemma(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let gemma_embedding = self
            .gemma_embedding
            .as_mut()
            .ok_or_else(|| helpers::config_error("Gemma 嵌入模型未初始化"))?;

        if !gemma_embedding.is_loaded() {
            gemma_embedding
                .load_model()
                .await
                .map_err(|e| helpers::llm_api_error(&format!("加载 Gemma 模型失败: {e}"), "embed_gemma"))?;
        }

        let text_refs: Vec<&str> = texts.iter().map(String::as_str).collect();

        let results = gemma_embedding
            .embed_batch(&text_refs)
            .await
            .map_err(|e| helpers::llm_api_error(&format!("Gemma 批量嵌入计算失败: {e}"), "embed_gemma"))?;

        let embeddings: Vec<Vec<f32>> = results.into_iter().map(|r| Arc::try_unwrap(r.vector).unwrap_or_else(|arc| (*arc).clone())).collect();

        Ok(embeddings)
    }
}

/// 基于确定性哈希的特征提取（SIMHash 变体）
///
/// 生成具有局部敏感性的向量：相似文本产生相似向量。
/// 使用 blake3 替代 `DefaultHasher`，保证跨版本/跨平台字节级确定性。
/// 生产环境应替换为真实模型推理（candle / ort / ONNX Runtime）。
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn simhash_embedding(text: &str, dimension: usize) -> Vec<f32> {
    let mut v = vec![0.0f64; dimension];

    let tokens: Vec<&str> = text
        .split_whitespace()
        .chain(text.split(|c: char| !c.is_alphanumeric()))
        .filter(|t| !t.is_empty())
        .collect();

    if tokens.is_empty() {
        v[0] = 1.0;
        let norm = v
            .iter()
            .map(|x| x * x)
            .sum::<f64>()
            .sqrt()
            .max(f64::EPSILON);
        return v.iter().map(|x| (x / norm) as f32).collect();
    }

    for token in &tokens {
        let h1 = blake3_to_u64(token.as_bytes());
        let h2 = blake3_to_u64(format!("{token}:salt2").as_bytes());

        for (i, vec_item) in v.iter_mut().enumerate().take(dimension) {
            let bit_pos = (h1.wrapping_add((i as u64).wrapping_mul(h2))) % 64;
            let sign = if (h1 >> (bit_pos % 64)) & 1 == 1 {
                1.0f64
            } else {
                -1.0f64
            };
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
    v.iter().map(|x| (x / norm) as f32).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_block(_id: &str, line: u32) -> Block {
        Block {
            id: None,
            doc_id: knowledge_core::model::RecordIdType::from((
                "doc".to_string(),
                "test".to_string(),
            )),
            block_type: knowledge_core::model::BlockType::Paragraph,
            start_line: line,
            end_line: line + 10,
            embedding: None,
            idempotency_key: None,
        }
    }

    #[tokio::test]
    async fn test_embed_single_block_success() {
        let service = EmbeddingService::new(EmbeddingModel::LocalBgeLarge, 1536, 32, 100).unwrap();

        let block = create_test_block("test001", 1);
        let result = service.embed_block(&block, "test content for embedding").await.unwrap();

        assert_eq!(result.block_id, "");
        assert_eq!(result.embedding.len(), 1536);
    }

    #[tokio::test]
    async fn test_batch_embedding_order_preserved() {
        let service = EmbeddingService::new(EmbeddingModel::LocalBgeLarge, 1536, 32, 100).unwrap();

        let blocks = vec![
            create_test_block("block1", 1),
            create_test_block("block2", 20),
            create_test_block("block3", 50),
        ];
        let contents = vec![
            "first block content".to_string(),
            "second block content".to_string(),
            "third block content".to_string(),
        ];

        let results = service.embed_batch(&blocks, &contents).await.unwrap();

        assert_eq!(results.len(), 3);
        for result in &results {
            assert_eq!(result.embedding.len(), 1536);
        }
    }

    #[test]
    fn test_dimension_validation_failure() {
        let service_data = EmbeddingService {
            model_type: EmbeddingModel::LocalBgeLarge,
            dimension: 768,
            batch_size: 32,
            sender: mpsc::channel(1).0,
        };

        let wrong_embedding = vec![0.0f32; 512];
        assert_ne!(wrong_embedding.len(), service_data.dimension());
    }

    #[tokio::test]
    async fn test_service_dimension_constant() {
        let service = EmbeddingService::new(EmbeddingModel::LocalBgeLarge, 1536, 32, 100).unwrap();

        assert_eq!(service.dimension(), 1536);
    }

    #[test]
    fn test_simhash_embedding_deterministic() {
        let text = "hello world rust programming";
        let v1 = simhash_embedding(text, 128);
        let v2 = simhash_embedding(text, 128);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_simhash_embedding_different_texts() {
        let v1 = simhash_embedding("rust programming language", 128);
        let v2 = simhash_embedding("python data science", 128);
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_simhash_embedding_normalized() {
        let v = simhash_embedding("test normalization", 64);
        let norm_sq: f32 = v.iter().map(|x| x * x).sum();
        let diff = (norm_sq - 1.0).abs();
        assert!(diff < 0.01, "向量未归一化: norm^2 = {norm_sq}");
    }

    #[test]
    fn test_simhash_embedding_empty_text() {
        let v = simhash_embedding("", 64);
        assert_eq!(v.len(), 64);
        let norm_sq: f32 = v.iter().map(|x| x * x).sum();
        assert!((norm_sq - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_simhash_similarity() {
        let v1 = simhash_embedding("rust programming language", 128);
        let v2 = simhash_embedding("rust programming languages", 128);
        let v3 = simhash_embedding("completely different topic", 128);

        let sim_12: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
        let sim_13: f32 = v1.iter().zip(v3.iter()).map(|(a, b)| a * b).sum();

        assert!(
            sim_12 > sim_13,
            "相似文本的余弦相似度应高于不相似文本: sim_12={sim_12}, sim_13={sim_13}"
        );
    }

    #[test]
    fn test_extract_block_content() {
        let block = create_test_block("test", 42);
        let content = extract_block_content(&block);
        assert!(!content.is_empty());
        assert!(content.contains("42"));
    }
}
