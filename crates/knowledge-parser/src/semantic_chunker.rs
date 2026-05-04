//! 语义感知的分块器
//!
//! 基于句子嵌入相似度的智能文本分块，相比固定大小分割：
//! - **语义完整性**：在主题边界处切分，避免截断关键信息
//! - **上下文保留**：块间重叠保持连贯性
//! - **自适应大小**：根据内容密度动态调整块大小
//!
//! # 分块流程
//!
//! ```text
//! 原始文本 → 句子分割 → 句子嵌入 → 相似度聚类 → 合并成块
//! ```

use error_core::Result;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use surrealdb::sql::Uuid;
use tracing::{debug, instrument};

/// 句子嵌入器 trait
///
/// 抽象的句子级嵌入接口，支持多种后端：
/// - 本地模型（sentence-transformers via candle/ort）
/// - 远程 API（OpenAI Embeddings）
/// - 轻量模型（用于开发测试）
pub trait SentenceEmbedder: Send + Sync {
    /// 计算单个句子的嵌入向量
    fn embed(&self, sentence: &str) -> impl std::future::Future<Output = Result<Vec<f32>>> + Send;

    /// 批量计算句子嵌入（更高效）
    fn embed_batch(
        &self,
        sentences: &[String],
    ) -> impl std::future::Future<Output = Result<Vec<Vec<f32>>>> + Send;

    /// 返回嵌入向量维度
    fn dimension(&self) -> usize;
}

/// 语义分块器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticChunkerConfig {
    /// 相似度阈值（默认: 0.65），低于此值视为语义边界
    pub threshold: f64,
    /// 最大块大小（token 数，默认: 512）
    pub max_chunk_size: usize,
    /// 最小块大小（token 数，默认: 50），防止过度碎片化
    pub min_chunk_size: usize,
    /// 块间重叠（token 数，默认: 50），保持上下文连续性
    pub overlap_size: usize,
}

impl Default for SemanticChunkerConfig {
    fn default() -> Self {
        Self {
            threshold: 0.65,
            max_chunk_size: 512,
            min_chunk_size: 50,
            overlap_size: 50,
        }
    }
}

/// 分块结果
///
/// 包含分块内容、位置信息和元数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// 唯一标识符
    pub id: Uuid,
    /// 分块内容
    pub content: String,
    /// 估算 token 数量
    pub token_count: usize,
    /// 分块元数据
    pub metadata: ChunkMetadata,
}

/// 分块元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// 所属文档 ID
    pub document_id: String,
    /// 在文档中的分块索引（从 0 开始）
    pub chunk_index: usize,
    /// 在原文中的起始字符偏移
    pub start_offset: usize,
    /// 在原文中的结束字符偏移
    pub end_offset: usize,
    /// 包含的句子数量
    pub sentence_count: usize,
    /// 块内平均语义相似度（可选，用于质量评估）
    pub semantic_similarity: Option<f64>,
}

/// 语义感知的分块器
///
/// 核心算法：
/// 1. 将文本按标点符号分割为句子
/// 2. 计算每个句子的嵌入向量
/// 3. 计算相邻句子的余弦相似度
/// 4. 在相似度低于阈值的位置标记边界
/// 5. 合并相邻句子为块，控制大小限制
///
/// # 性能目标
///
/// - **P99 延迟**: < 1s / 10KB 文本（含嵌入计算）
/// - **吞吐量**: ~100 docs/s（批量模式）
///
/// # 示例
///
/// ```ignore
/// use knowledge_parser::semantic_chunker::{SemanticChunker, SentenceEmbedder, SemanticChunkerConfig};
/// use std::sync::Arc;
///
/// let embedder: Arc<dyn SentenceEmbedder> = /* ... */;
/// let chunker = SemanticChunker::new(embedder, SemanticChunkerConfig::default());
///
/// let chunks = chunker.chunk("这是一段很长的文本...", "doc-001").await?;
/// for chunk in &chunks {
///     println!("Chunk {}: {} tokens", chunk.metadata.chunk_index, chunk.token_count);
/// }
/// ```
pub struct SemanticChunker<E: SentenceEmbedder + Send + Sync + 'static> {
    /// 句子嵌入模型
    embedder: Arc<E>,
    /// 配置参数
    config: SemanticChunkerConfig,
}

impl<E: SentenceEmbedder + Send + Sync + 'static> SemanticChunker<E> {
    /// 创建新的语义分块器
    ///
    /// # 参数
    ///
    /// * `embedder` - 句子嵌入器实例（Arc 包装以共享所有权）
    /// * `config` - 分块配置参数
    pub const fn new(embedder: Arc<E>, config: SemanticChunkerConfig) -> Self {
        Self { embedder, config }
    }

    /// 使用默认配置创建分块器
    pub fn with_defaults(embedder: Arc<E>) -> Self {
        Self {
            embedder,
            config: SemanticChunkerConfig::default(),
        }
    }

    /// 执行语义分块
    ///
    /// 完整的分块流水线：句子分割 → 嵌入计算 → 边界检测 → 块合并。
    ///
    /// # 参数
    ///
    /// * `text` - 待分块的原始文本
    /// * `document_id` - 文档标识符（用于元数据）
    ///
    /// # 返回
    ///
    /// 分块结果列表，按在原文中的顺序排列
    /// # Errors
    ///
    /// 嵌入计算失败或嵌入数量与句子数量不匹配时返回错误
    #[instrument(skip(self, text))]
    pub async fn chunk(&self, text: &str, document_id: &str) -> Result<Vec<Chunk>> {
        if text.is_empty() || text.trim().is_empty() {
            return Ok(vec![]);
        }

        debug!(doc_id = document_id, text_len = text.len(), "开始语义分块");

        // Step 1: 句子分割
        let sentences = Self::split_sentences(text);
        debug!(sentence_count = sentences.len(), "句子分割完成");

        if sentences.is_empty() {
            return Ok(vec![]);
        }

        // Step 2: 计算句子嵌入
        let embeddings = self.embedder.embed_batch(&sentences).await?;
        debug!(embeddings_count = embeddings.len(), "嵌入计算完成");

        if embeddings.len() != sentences.len() {
            return Err(error_core::helpers::validation_error(&format!(
                "嵌入数量 ({}) 与句子数量 ({}) 不匹配",
                embeddings.len(),
                sentences.len()
            ), "compute_semantic_embeddings"));
        }

        // Step 3: 检测语义边界
        let boundaries = self.find_semantic_boundaries(&embeddings);
        debug!(boundary_count = boundaries.len(), "语义边界检测完成");

        // Step 4: 合并为块
        let chunks =
            self.merge_into_chunks(text, &sentences, &embeddings, &boundaries, document_id);

        debug!(chunk_count = chunks.len(), "分块完成");

        Ok(chunks)
    }

    /// 递归字符分割（fallback 策略）
    ///
    /// 当嵌入模型不可用时使用的简单分割策略，
    /// 按优先级尝试不同的分隔符直到达到目标大小。
    ///
    /// # 参数
    ///
    /// * `text` - 待分割的文本
    /// * `document_id` - 文档标识符
    /// * `separators` - 分隔符优先级列表（从粗到细）
    ///
    /// # 默认分隔符序列
    ///
    /// ```text
    /// ["\n\n", "\n", "。", "！", "？", ".", "!", "?", ";", "；", ",", "，", " "]
    /// ```
    #[must_use]
    pub fn recursive_split(
        &self,
        text: &str,
        document_id: &str,
        separators: &[&str],
    ) -> Vec<Chunk> {
        if text.is_empty() {
            return vec![];
        }

        let separators = if separators.is_empty() {
            DEFAULT_SEPARATORS.as_slice()
        } else {
            separators
        };

        let mut chunks = Vec::new();
        let mut current_pos = 0;
        let mut chunk_index = 0;

        while current_pos < text.len() {
            let remaining = &text[current_pos..];
            let (content, advance) = self.find_next_chunk(remaining, separators);

            if content.trim().is_empty() {
                current_pos += advance.max(1);
                continue;
            }

            let start_offset = current_pos;
            let end_offset = current_pos + content.len();

            chunks.push(Chunk {
                id: {
                    let hash = blake3::hash(
                        format!("{document_id}:{chunk_index}:{start_offset}").as_bytes(),
                    );
                    let hex = hash.to_hex();
                    Uuid::from_str(&format!(
                        "{}-{}-{}-{}-{}",
                        &hex[0..8],
                        &hex[8..12],
                        &hex[12..16],
                        &hex[16..20],
                        &hex[20..32]
                    ))
                    .unwrap_or_else(|()| Uuid::new())
                },
                content: content.trim().to_string(),
                token_count: Self::estimate_tokens(content),
                metadata: ChunkMetadata {
                    document_id: document_id.to_string(),
                    chunk_index,
                    start_offset,
                    end_offset,
                    sentence_count: count_sentences(content),
                    semantic_similarity: None,
                },
            });

            chunk_index += 1;
            current_pos = end_offset;
        }

        chunks
    }

    // ====================================================================
    // 内部方法
    // ====================================================================

    /// 按标点符号分割句子
    ///
    /// 支持中英文混合文本的句子边界检测：
    /// - 中文：。！？；\n\n
    /// - 英文：.!?\n\n
    fn split_sentences(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current = String::new();
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            current.push(c);

            if is_sentence_terminator(c) {
                // 检查后续是否为空白或引号闭合
                let mut has_trailing_space = false;
                while let Some(&next) = chars.peek() {
                    if next.is_whitespace() {
                        chars.next();
                        has_trailing_space = true;
                    } else if next == '"' || next == '\'' || next == '」' {
                        if let Some(c) = chars.next() {
                            current.push(c);
                        }
                    } else {
                        break;
                    }
                }

                if !current.trim().is_empty() {
                    sentences.push(current.trim().to_string());
                }
                current = String::new();

                // 保留换行作为分隔符
                if has_trailing_space && !current.ends_with('\n') {
                    current.push(' ');
                }
            }
        }

        // 处理末尾残余
        if !current.trim().is_empty() {
            sentences.push(current.trim().to_string());
        }

        sentences
    }

    /// 检测语义边界
    ///
    /// 计算相邻句子嵌入的余弦相似度，
    /// 当相似度低于阈值时标记为潜在边界点。
    ///
    /// 返回边界索引列表（表示在第 i 和 i+1 个句子之间切分）。
    fn find_semantic_boundaries(&self, embeddings: &[Vec<f32>]) -> Vec<usize> {
        let mut boundaries = Vec::new();

        for i in 0..embeddings.len().saturating_sub(1) {
            let sim = cosine_similarity_vec(&embeddings[i], &embeddings[i + 1]);
            if sim < self.config.threshold {
                boundaries.push(i);
            }
        }

        boundaries
    }

    /// 合并句子为块
    ///
    /// 根据语义边界和大小限制将句子组合成最终分块。
    fn merge_into_chunks(
        &self,
        original_text: &str,
        sentences: &[String],
        _embeddings: &[Vec<f32>],
        boundaries: &[usize],
        document_id: &str,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut current_sentences: Vec<usize> = Vec::new();
        let mut current_tokens = 0usize;
        let mut chunk_index = 0usize;
        let boundary_set: std::collections::HashSet<_> = boundaries.iter().copied().collect();

        let sentence_offsets = Self::compute_sentence_offsets(original_text, sentences);

        for (idx, sentence) in sentences.iter().enumerate() {
            let sent_tokens = Self::estimate_tokens(sentence);

            let should_split = current_tokens + sent_tokens > self.config.max_chunk_size
                || boundary_set.contains(&(idx.saturating_sub(1)))
                    && current_tokens >= self.config.min_chunk_size;

            if should_split && !current_sentences.is_empty() {
                chunks.push(Self::create_chunk_from_sentences(
                    sentences,
                    &current_sentences,
                    document_id,
                    chunk_index,
                    &sentence_offsets,
                ));

                chunk_index += 1;

                let overlap_count = self.calculate_overlap_count(&current_sentences);
                current_sentences = current_sentences
                    .iter()
                    .copied()
                    .skip(current_sentences.len().saturating_sub(overlap_count))
                    .collect();
                current_tokens = current_sentences
                    .iter()
                    .map(|&i| Self::estimate_tokens(&sentences[i]))
                    .sum();
            }

            current_sentences.push(idx);
            current_tokens += sent_tokens;
        }

        if !current_sentences.is_empty() {
            chunks.push(Self::create_chunk_from_sentences(
                sentences,
                &current_sentences,
                document_id,
                chunk_index,
                &sentence_offsets,
            ));
        }

        chunks
    }

    fn compute_sentence_offsets(original_text: &str, sentences: &[String]) -> Vec<(usize, usize)> {
        let mut offsets = Vec::with_capacity(sentences.len());
        let mut search_start = 0;
        for sentence in sentences {
            let trimmed = sentence.trim();
            if let Some(pos) = original_text[search_start..].find(trimmed) {
                let abs_start = search_start + pos;
                let abs_end = abs_start + trimmed.len();
                offsets.push((abs_start, abs_end.min(original_text.len())));
                search_start = abs_end;
            } else {
                offsets.push((search_start, original_text.len()));
                search_start = original_text.len();
            }
        }
        offsets
    }

    /// 从句子索引列表创建 Chunk
    fn create_chunk_from_sentences(
        sentences: &[String],
        indices: &[usize],
        document_id: &str,
        chunk_index: usize,
        sentence_offsets: &[(usize, usize)],
    ) -> Chunk {
        let content: String = indices
            .iter()
            .map(|&i| sentences[i].as_str())
            .collect::<Vec<&str>>()
            .join(" ");

        let start_offset = indices
            .first()
            .and_then(|&i| sentence_offsets.get(i))
            .map_or(0, |&(s, _)| s);
        let end_offset = indices
            .last()
            .and_then(|&i| sentence_offsets.get(i))
            .map_or_else(|| content.len(), |&(_, e)| e);

        Chunk {
            id: {
                let hash =
                    blake3::hash(format!("{document_id}:{chunk_index}:{start_offset}").as_bytes());
                let hex = hash.to_hex();
                Uuid::from_str(&format!(
                    "{}-{}-{}-{}-{}",
                    &hex[0..8],
                    &hex[8..12],
                    &hex[12..16],
                    &hex[16..20],
                    &hex[20..32]
                ))
                .unwrap_or_else(|()| Uuid::new())
            },
            content,
            token_count: Self::estimate_tokens_content(
                &indices.iter().map(|&i| &sentences[i]).collect::<Vec<_>>(),
            ),
            metadata: ChunkMetadata {
                document_id: document_id.to_string(),
                chunk_index,
                start_offset,
                end_offset,
                sentence_count: indices.len(),
                semantic_similarity: None,
            },
        }
    }

    /// 计算重叠句子数
    fn calculate_overlap_count(&self, current_indices: &[usize]) -> usize {
        if current_indices.len() <= 1 {
            return 0;
        }

        let target_overlap_tokens = self.config.overlap_size;
        let mut overlap_count = 0;
        let mut accumulated_tokens = 0;

        for _ in current_indices.iter().rev() {
            if accumulated_tokens >= target_overlap_tokens {
                break;
            }
            overlap_count += 1;
            // 粗略估算：假设每句平均 20 tokens
            accumulated_tokens += self.config.max_chunk_size / 50;
        }

        overlap_count
    }

    /// 估算字符串的 token 数量（粗粒度：中文字符计为 1 token，英文单词计为 1 token）
    fn estimate_tokens(text: &str) -> usize {
        let chinese_chars = text.chars().filter(|c| is_chinese(*c)).count();
        let english_words = text
            .split_whitespace()
            .filter(|w| !w.chars().any(is_chinese))
            .count();
        chinese_chars + english_words
    }

    fn estimate_tokens_content(texts: &[&String]) -> usize {
        texts.iter().map(|t| Self::estimate_tokens(t)).sum()
    }

    /// 递归查找下一个分块
    fn find_next_chunk<'a>(&self, text: &'a str, separators: &[&str]) -> (&'a str, usize) {
        for &sep in separators {
            if let Some(pos) = text.find(sep) {
                if text.len() <= self.config.max_chunk_size {
                    if pos > 0 {
                        return (&text[..pos + sep.len()], pos + sep.len());
                    }
                } else if pos > self.config.min_chunk_size {
                    return (&text[..pos + sep.len()], pos + sep.len());
                }
            }
        }

        if text.len() <= self.config.max_chunk_size {
            (text, text.len())
        } else {
            let split_at = self.config.max_chunk_size.min(text.len());
            let split_at = crate::floor_char_boundary(text, split_at);
            (&text[..split_at], split_at)
        }
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 默认分隔符序列（从粗到细）
const DEFAULT_SEPARATORS: [&str; 13] = [
    "\n\n", // 段落分隔
    "\n",   // 行分隔
    "。",   // 中文句号
    "！",   // 中文感叹号
    "？",   // 中文问号
    ". ",   // 英文句号+空格
    "! ",   // 英文感叹号+空格
    "? ",   // 英文问号+空格
    ";",    // 英文分号
    "；",   // 中文分号
    ", ",   // 英文逗号+空格
    "，",   // 中文逗号
    " ",    // 空格（最后手段）
];

/// 判断是否为句子终止符
const fn is_sentence_terminator(c: char) -> bool {
    matches!(c, '。' | '！' | '？' | '.' | '!' | '?')
}

/// 判断是否为中文字符
const fn is_chinese(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '\u{F900}'..='\u{FAFF}')
}

/// 计算两个向量的余弦相似度
fn cosine_similarity_vec(a: &[f32], b: &[f32]) -> f64 {
    const NORM_THRESHOLD: f64 = 1e-10;
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let norm_a: f64 = a.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();

    if norm_a < NORM_THRESHOLD || norm_b < NORM_THRESHOLD {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// 统计句子数量
fn count_sentences(text: &str) -> usize {
    text.chars()
        .filter(|c| is_sentence_terminator(*c))
        .count()
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    /// 测试用 Mock Embedder（返回零向量）
    struct MockEmbedder {
        dimension: usize,
    }

    impl SentenceEmbedder for MockEmbedder {
        async fn embed(&self, _sentence: &str) -> Result<Vec<f32>> {
            Ok(vec![0.0; self.dimension])
        }

        async fn embed_batch(&self, sentences: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(sentences
                .iter()
                .map(|_| vec![0.0; self.dimension])
                .collect())
        }

        fn dimension(&self) -> usize {
            self.dimension
        }
    }

    fn create_test_chunker() -> SemanticChunker<MockEmbedder> {
        let embedder = Arc::new(MockEmbedder { dimension: 384 });
        SemanticChunker::with_defaults(embedder)
    }

    #[test]
    fn test_config_default_values() {
        let config = SemanticChunkerConfig::default();
        assert!((config.threshold - 0.65).abs() < f64::EPSILON);
        assert_eq!(config.max_chunk_size, 512);
        assert_eq!(config.min_chunk_size, 50);
        assert_eq!(config.overlap_size, 50);
    }

    #[tokio::test]
    async fn test_chunk_empty_text() {
        let chunker = create_test_chunker();
        let result = chunker.chunk("", "doc-empty").await.expect("分块失败");
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_chunk_single_sentence() {
        let chunker = create_test_chunker();
        let text = "这是一个单句测试。";
        let result = chunker.chunk(text, "doc-001").await.expect("分块失败");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].metadata.sentence_count, 1);
    }

    #[tokio::test]
    async fn test_chunk_multiple_sentences() {
        let chunker = create_test_chunker();
        let text = "这是第一句话。这是第二句话。这是第三句话。这是第四句话。";
        let result = chunker.chunk(text, "doc-002").await.expect("分块失败");

        assert!(!result.is_empty());
        for (i, chunk) in result.iter().enumerate() {
            assert_eq!(chunk.metadata.chunk_index, i);
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn test_recursive_split_basic() {
        let chunker = create_test_chunker();
        let text = "第一段。\n\n第二段。\n\n第三段。";
        let chunks = chunker.recursive_split(text, "doc-003", &["\n\n"]);

        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].content.contains("第一段"));
        assert!(chunks[2].content.contains("第三段"));
    }

    #[test]
    fn test_recursive_split_preserves_content() {
        let chunker = create_test_chunker();
        let text = "A".repeat(1000);
        let chunks = chunker.recursive_split(&text, "doc-004", &[" "]);

        let combined: String = chunks.iter().map(|c| c.content.as_str()).collect();
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation
        )]
        let threshold = (text.len() as f64 * 0.9) as usize;
        assert!(combined.len() >= threshold, "递归分割应覆盖大部分原文内容");
    }

    #[test]
    fn test_recursive_split_empty() {
        let chunker = create_test_chunker();
        let chunks = chunker.recursive_split("", "doc-empty", &[]);

        assert!(chunks.is_empty());
    }

    #[test]
    fn test_sentence_splitting_mixed_language() {
        let text = "Hello world. 你好世界。This is a test. 这是一个测试。";
        let sentences = SemanticChunker::<MockEmbedder>::split_sentences(text);

        assert_eq!(sentences.len(), 4);
    }

    #[test]
    fn test_estimate_tokens() {
        let chinese_only = "你好世界";
        assert_eq!(
            SemanticChunker::<MockEmbedder>::estimate_tokens(chinese_only),
            4
        );

        let english_only = "Hello world test";
        assert_eq!(
            SemanticChunker::<MockEmbedder>::estimate_tokens(english_only),
            3
        );
    }

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity_vec(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity_vec(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_chunk_serialization() {
        let chunk = Chunk {
            id: Uuid::from_str("00000000-0000-0000-0000-000000000001")
                .unwrap_or_else(|()| Uuid::new()),
            content: "test content".to_string(),
            token_count: 10,
            metadata: ChunkMetadata {
                document_id: "doc-123".to_string(),
                chunk_index: 0,
                start_offset: 0,
                end_offset: 12,
                sentence_count: 1,
                semantic_similarity: Some(0.85),
            },
        };

        let serialized = serde_json::to_string(&chunk).expect("序列化失败");
        let deserialized: Chunk = serde_json::from_str(&serialized).expect("反序列化失败");

        assert_eq!(deserialized.id, chunk.id);
        assert_eq!(deserialized.token_count, chunk.token_count);
    }
}
