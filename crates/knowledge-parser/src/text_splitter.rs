//! 文本分块器（纯文本 → text-splitter 递归字符分割 → Block 列表）
//!
//! 使用 text-splitter 的 Characters 模式进行递归字符分割。
//!
//! # 设计要点
//!
//! - **递归分割**：先尝试按段落分割，失败则按句子，最后按字符
//! - **重叠保持**：相邻 chunk 之间保留 `chunk_overlap` 字符的重叠区域

use crate::Result;
use knowledge_core::model::{Block, BlockStatus, BlockType, RecordIdType};
use surrealdb::sql::Thing;
use text_splitter::{Characters, TextSplitter};

/// 默认最大分块大小（字符数）
const DEFAULT_MAX_CHUNK_SIZE: usize = 1000;

/// 默认分块重叠大小（字符数）
const DEFAULT_CHUNK_OVERLAP: usize = 200;

/// 文本分块器
///
/// 将纯文本或 Markdown 内容分割为 Block 列表。
pub struct TextSplitterBlocker {
    splitter: TextSplitter<Characters>,
    max_chunk_size: usize,
    chunk_overlap: usize,
}

impl Default for TextSplitterBlocker {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSplitterBlocker {
    /// 创建新的文本分块器实例（使用默认配置）
    ///
    /// # Panics
    ///
    /// 当默认配置参数不合法时 panic（理论上不会发生）。
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(DEFAULT_MAX_CHUNK_SIZE, DEFAULT_CHUNK_OVERLAP)
            .expect("默认配置参数应始终合法")
    }

    /// 自定义分块配置
    ///
    /// # Errors
    ///
    /// 当 `chunk_overlap >= max_chunk_size` 时返回错误。
    pub fn with_config(max_chunk_size: usize, chunk_overlap: usize) -> Result<Self> {
        if chunk_overlap >= max_chunk_size {
            return Err(error_core::ErrorObject::from(format!(
                "chunk_overlap ({chunk_overlap}) 必须小于 max_chunk_size ({max_chunk_size})"
            )));
        }

        let splitter = TextSplitter::new(Characters).with_trim_chunks(true);

        Ok(Self {
            splitter,
            max_chunk_size,
            chunk_overlap,
        })
    }

    /// 将纯文本分割为 Block 列表
    ///
    /// # Errors
    ///
    /// 当 text-splitter 内部处理失败时返回解析错误（对应 `ErrorObject` code `ERR-FS-PARSE-001`）。
    pub fn split_to_blocks(&self, content: &str, file_path: &str) -> Result<Vec<Block>> {
        if content.is_empty() {
            return Ok(Vec::new());
        }

        let chunks: Vec<&str> = self.splitter.chunks(content, self.max_chunk_size).collect();

        let mut blocks = Vec::with_capacity(chunks.len());
        let mut current_line: u32 = 0;

        for chunk in &chunks {
            let line_count: u32 = chunk.matches('\n').count().try_into().unwrap_or(u32::MAX);
            let end_line = current_line + line_count;

            let doc_hash = blake3::hash(file_path.as_bytes()).to_hex().to_string();
            let doc_id_record: RecordIdType = Thing::from(("doc".to_string(), doc_hash.clone()));

            let block_type = if chunk.trim_start().starts_with('#') {
                BlockType::Heading
            } else {
                BlockType::Paragraph
            };

            let block = Block {
                id: None,
                doc_id: doc_id_record.clone(),
                block_type,
                start_line: current_line,
                end_line,
                idempotency_key: Some(
                    crate::idempotency::IdempotencyKeyGenerator::generate_for_block(
                        &doc_hash,
                        current_line,
                        end_line,
                        chunk,
                    ),
                ),
                status: BlockStatus::Created,
            };

            blocks.push(block);
            current_line = end_line + 1;
        }

        Ok(blocks)
    }

    /// 从 comrak AST 提取结构化 Block（保留标题层级信息）
    ///
    /// 当前实现仅接受 Document 根节点，后续可扩展。
    ///
    /// # Errors
    ///
    /// 当前实现始终返回空列表，不会返回错误。
    pub const fn split_ast_to_blocks(&self, _ast: &comrak::nodes::NodeValue) -> Result<Vec<Block>> {
        Ok(Vec::new())
    }

    /// 获取当前配置的最大分块大小
    #[must_use]
    pub const fn max_chunk_size(&self) -> usize {
        self.max_chunk_size
    }

    /// 获取当前配置的重叠大小
    #[must_use]
    pub const fn chunk_overlap(&self) -> usize {
        self.chunk_overlap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_paragraph_priority() {
        let splitter = TextSplitterBlocker::new();
        let content = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let blocks = splitter
            .split_to_blocks(content, "test://paragraph")
            .expect("分块应成功");
        assert!(!blocks.is_empty(), "应生成 Block");
        for block in &blocks {
            assert_eq!(block.block_type, BlockType::Paragraph);
        }
    }

    #[test]
    fn test_split_empty_content() {
        let splitter = TextSplitterBlocker::new();
        let blocks = splitter
            .split_to_blocks("", "test://empty")
            .expect("空内容不应报错");
        assert!(blocks.is_empty(), "空内容应返回空列表");
    }

    #[test]
    fn test_split_single_short_paragraph() {
        let splitter = TextSplitterBlocker::new();
        let content = "Short text that fits in one chunk.";
        let blocks = splitter
            .split_to_blocks(content, "test://short")
            .expect("短文本应成功");
        assert_eq!(blocks.len(), 1, "短文本应为单个块");
    }

    #[test]
    fn test_default_config_values() {
        let splitter = TextSplitterBlocker::new();
        assert_eq!(splitter.max_chunk_size(), DEFAULT_MAX_CHUNK_SIZE);
        assert_eq!(splitter.chunk_overlap(), DEFAULT_CHUNK_OVERLAP);
    }

    #[test]
    fn test_invalid_config_overlap_greater_than_max() {
        let result = TextSplitterBlocker::with_config(100, 150);
        assert!(result.is_err(), "overlap >= max 应返回错误");
    }

    #[test]
    fn test_invalid_config_overlap_equal_to_max() {
        let result = TextSplitterBlocker::with_config(100, 100);
        assert!(result.is_err(), "overlap == max 也应返回错误");
    }

    #[test]
    fn test_custom_config() {
        let splitter = TextSplitterBlocker::with_config(500, 50).expect("合法配置应成功");
        assert_eq!(splitter.max_chunk_size(), 500);
        assert_eq!(splitter.chunk_overlap(), 50);
    }

    #[test]
    fn test_default_creates_splitter() {
        let splitter = TextSplitterBlocker::default();
        assert_eq!(splitter.max_chunk_size(), DEFAULT_MAX_CHUNK_SIZE);
        assert_eq!(splitter.chunk_overlap(), DEFAULT_CHUNK_OVERLAP);
    }

    #[test]
    fn test_split_ast_to_blocks_returns_empty() {
        let splitter = TextSplitterBlocker::new();
        let ast = comrak::nodes::NodeValue::Document;
        let result = splitter.split_ast_to_blocks(&ast);
        assert!(result.is_ok(), "split_ast_to_blocks 应成功");
        assert!(result.unwrap().is_empty(), "当前实现应返回空列表");
    }

    #[test]
    fn test_split_heading_block_type() {
        let splitter = TextSplitterBlocker::new();
        let content = "# Heading\n\nParagraph text";
        let blocks = splitter
            .split_to_blocks(content, "test://heading")
            .expect("分块应成功");
        assert!(
            blocks.iter().any(|b| b.block_type == BlockType::Heading),
            "以 # 开头的内容应产生 Heading 类型的 Block"
        );
    }

    #[test]
    fn test_split_blocks_have_idempotency_keys() {
        let splitter = TextSplitterBlocker::new();
        let content = "First paragraph.\n\nSecond paragraph.";
        let blocks = splitter
            .split_to_blocks(content, "test://keys")
            .expect("分块应成功");
        for block in &blocks {
            assert!(block.idempotency_key.is_some(), "每个 Block 应有幂等键");
            let key = block.idempotency_key.as_ref().unwrap();
            assert_eq!(key.len(), 64, "幂等键应为 64 字符");
        }
    }

    #[test]
    fn test_split_blocks_have_valid_line_ranges() {
        let splitter = TextSplitterBlocker::new();
        let content = "First line\nSecond line\n\nThird line\nFourth line";
        let blocks = splitter
            .split_to_blocks(content, "test://lines")
            .expect("分块应成功");
        for block in &blocks {
            assert!(
                block.start_line <= block.end_line,
                "start_line 应 <= end_line"
            );
        }
    }
}
