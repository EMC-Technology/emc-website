//! Markdown/Plain 解析流水线（DAG 分支 A 完整流程）
//!
//! 处理链路：
//! ```text
//! IngestedFile → ComrakAstParser (标题提取) → TextSplitterBlocker → IcuTokenizer
//!                                                       ↓
//!                                         Document + Blocks + Tokens
//! ```
//!
//! # 设计要点
//!
//! - **策略模式**：根据 `source_type` 选择解析路径：
//!   - `SourceType::Markdown` → 先走 ComrakAstParser 提取标题，再走 TextSplitterBlocker 分块
//!   - `SourceType::Plain` → 直接走 TextSplitterBlocker
//! - **偏移量追踪**：自动计算每个 Block 和 Token 的 global_offset，
//!   使用 u64 类型防止大文件溢出
//! - **性能预算**：P99 < 500ms（≤1000 行文件）

use crate::Result;
use crate::code_token_mapper::CodeTokenMapper;
use crate::file_ingester::IngestedFile;
use crate::icu_tokenizer::IcuTokenizer;
use crate::markdown_parser::ComrakAstParser;
use crate::text_splitter::TextSplitterBlocker;
use error_core::helpers;
use knowledge_core::model::{Block, BlockType, Document, SourceType, Token};

/// Markdown/Plain 解析流水线
///
/// 组合 `ComrakAstParser`、`TextSplitterBlocker`、`IcuTokenizer` 三个组件，
/// 实现从原始文件到结构化知识实体的完整转换流程。
///
/// # Example
///
/// ```ignore
/// use knowledge_parser::{FileIngester, MarkdownPipeline};
///
/// #[tokio::main]
/// async fn main() {
///     let ingester = FileIngester::new();
///     let file = ingester.ingest(Path::new("doc.md")).await?;
///
///     let pipeline = MarkdownPipeline::new();
///     let (document, blocks, tokens) = pipeline.process(&file).await?;
///
///     println!("Document: {}", document.title);
///     println!("Blocks: {}", blocks.len());
///     println!("Tokens: {}", tokens.len());
/// }
/// ```
pub struct MarkdownPipeline {
    parser: ComrakAstParser,
    splitter: TextSplitterBlocker,
    tokenizer: IcuTokenizer,
    code_mapper: CodeTokenMapper,
}

impl Default for MarkdownPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownPipeline {
    /// 创建新的 Markdown 解析流水线实例（使用默认配置）
    ///
    /// 默认配置：
    /// - `ComrakAstParser`：使用 comrak 默认选项
    /// - `TextSplitterBlocker`：`max_chunk_size=1000`, `chunk_overlap=200`
    /// - `IcuTokenizer`：`WordSegmenter`（UAX#29 词级分词规范）
    /// - `CodeTokenMapper`：代码块符号完整性映射器
    #[must_use]
    pub fn new() -> Self {
        Self {
            parser: ComrakAstParser::new(),
            splitter: TextSplitterBlocker::new(),
            tokenizer: IcuTokenizer::new(),
            code_mapper: CodeTokenMapper::new(),
        }
    }

    /// 自定义各组件配置
    ///
    /// # 参数
    ///
    /// * `parser` - `Comrak` `AST` 解析器实例
    /// * `splitter` - 文本分块器实例
    /// * `tokenizer` - `ICU` 分词器实例
    #[must_use]
    pub const fn with_components(
        parser: ComrakAstParser,
        splitter: TextSplitterBlocker,
        tokenizer: IcuTokenizer,
    ) -> Self {
        Self {
            parser,
            splitter,
            tokenizer,
            code_mapper: CodeTokenMapper::new(),
        }
    }

    /// 执行完整的 Markdown/Plain 文件解析流程
    ///
    /// 处理步骤：
    /// 1. 创建 Document 实体（从 `IngestedFile` 提取元信息）
    /// 2. 根据 `source_type` 选择解析路径：
    ///    - `Markdown` → 使用 `ComrakAstParser` 提取标题 + `TextSplitterBlocker` 分块
    ///    - `Plain` → 直接走 `TextSplitterBlocker`
    /// 3. 对每个 Block 根据 `block_type` 选择分词策略：
    ///    - `BlockType::Code` → 使用 `CodeTokenMapper`（保持代码符号完整性）
    ///    - 其他 → 使用 `IcuTokenizer`（词级分词）
    /// 4. 计算 `global_offset`（累加每个块的字符数）
    /// 5. 返回完整实体集合：(Document, Vec<Block>, Vec<Token>)
    ///
    /// # 性能预算
    ///
    /// P99 < 500ms（≤1000 行文件）
    ///
    /// # 参数
    ///
    /// * `file` - 已完成文件读取和哈希计算的 `IngestedFile` 实例
    ///
    /// # Returns
    ///
    /// 成功时返回元组 `(Document, Vec<Block>, Vec<Token>)`。
    ///
    /// # Errors
    ///
    /// | 场景 | 错误变体 | 错误码 |
    /// |------|----------|--------|
    /// | 文档创建失败（哈希格式错误）| `ParseError` | E3001 |
    /// | 分块失败 | `ParseError` | E3001 |
    pub fn process(&self, file: &IngestedFile) -> Result<(Document, Vec<Block>, Vec<Token>)> {
        let title = Self::extract_title(&self.parser, &file.content);
        let path_string = file.path.to_string_lossy().into_owned();
        let document = Document::new(
            path_string.clone(),
            title,
            file.source_type.clone(),
            &file.hash,
        )?;

        let blocks = self.create_blocks(&file.content, &file.source_type, &path_string)?;

        let mut all_tokens = Vec::new();
        let mut global_offset: u64 = 0;

        for block in &blocks {
            let block_content = Self::extract_block_content(&file.content, block);
            let tokens = if block.block_type == BlockType::Code {
                self.code_mapper
                    .tokenize_code_block(&block_content, global_offset, &block.id)
            } else {
                let block_id = block.id.clone().unwrap_or_else(|| {
                    #[cfg(feature = "db")]
                    {
                        surrealdb::sql::Thing::from(("block".to_string(), "temp".to_string()))
                    }
                    #[cfg(not(feature = "db"))]
                    {
                        "block:temp".to_string()
                    }
                });
                self.tokenizer
                    .tokenize_block(&block_content, global_offset, &block_id)
            };
            global_offset += block_content.chars().count() as u64;
            all_tokens.extend(tokens);
        }

        Ok((document, blocks, all_tokens))
    }

    /// 从文件内容中提取标题
    ///
    /// 提取策略：
    /// 1. 使用 `ComrakAstParser` 解析标题
    /// 2. 若无标题，使用第一行非空文本
    /// 3. 若仍无，使用 "Untitled"
    fn extract_title(parser: &ComrakAstParser, content: &str) -> String {
        let headings = parser.parse(content);
        if let Some(first_heading) = headings.first() {
            return first_heading.clone();
        }

        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        "Untitled".to_string()
    }

    /// 根据源类型创建 Block 列表
    ///
    /// - `Markdown` → 使用 `TextSplitterBlocker` 分块（保留段落结构）
    /// - `Plain` → 使用 `TextSplitterBlocker` 分块
    /// - `Code` → 返回错误（应使用 `CodePipeline`）
    fn create_blocks(
        &self,
        content: &str,
        source_type: &SourceType,
        file_path: &str,
    ) -> Result<Vec<Block>> {
        match source_type {
            SourceType::Markdown | SourceType::Plain => {
                self.splitter.split_to_blocks(content, file_path)
            }
            SourceType::Code => Err(helpers::unsupported_format(
                "Code 类型应使用 CodePipeline 处理",
            )),
        }
    }

    /// 从原始内容中提取指定 Block 的文本内容
    ///
    /// 根据 `Block` 的 `start_line`/`end_line` 进行精确切片。
    fn extract_block_content(content: &str, block: &Block) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let start = block.start_line as usize;
        let end = (block.end_line as usize + 1).min(lines.len());

        if start >= lines.len() {
            return String::new();
        }

        lines[start..end].join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use error_core::prelude::ErrorSource;
    use std::path::PathBuf;

    fn create_test_ingested_file(content: &str, source_type: SourceType) -> IngestedFile {
        let hash = "a".repeat(64);

        IngestedFile {
            path: PathBuf::from("test.md"),
            content: content.to_string(),
            hash,
            file_size: content.len() as u64,
            source_type,
        }
    }

    #[tokio::test]
    async fn test_full_markdown_pipeline() {
        let pipeline = MarkdownPipeline::new();

        let markdown_content =
            "# Test Document\n\nThis is the first paragraph.\n\nThis is the second paragraph.";
        let file = create_test_ingested_file(markdown_content, SourceType::Markdown);

        let result = pipeline.process(&file);

        assert!(
            result.is_ok(),
            "Markdown 文件应成功处理: {:?}",
            result.err()
        );
        let (document, blocks, tokens) = result.unwrap();

        assert_eq!(document.title, "Test Document", "标题应从第一个 H1 提取");
        assert_eq!(document.source_type, SourceType::Markdown);
        assert!(!blocks.is_empty(), "应至少生成一个 Block");
        assert!(!tokens.is_empty(), "应至少生成一个 Token");
    }

    #[tokio::test]
    async fn test_plain_text_pipeline() {
        let pipeline = MarkdownPipeline::new();

        let plain_content = "This is plain text.\n\nAnother paragraph.";
        let file = create_test_ingested_file(plain_content, SourceType::Plain);

        let result = pipeline.process(&file);

        assert!(result.is_ok(), "纯文本文件应成功处理: {:?}", result.err());
        let (document, blocks, _) = result.unwrap();

        assert_eq!(document.source_type, SourceType::Plain);
        assert!(!blocks.is_empty(), "纯文本应生成 Block");
    }

    #[tokio::test]
    async fn test_code_source_type_rejected() {
        let pipeline = MarkdownPipeline::new();

        let code_content = "fn main() {}";
        let file = create_test_ingested_file(code_content, SourceType::Code);

        let result = pipeline.process(&file);

        assert!(result.is_err(), "Code 类型应被拒绝");
        match result.unwrap_err() {
            err if err.code().contains("PARSE") && err.source() == ErrorSource::USR => {
                assert!(err.message().contains("Code"), "错误消息应提及 Code 类型");
            }
            other => panic!("期望 UnsupportedFormat, 实际: {other}"),
        }
    }

    #[test]
    fn test_title_extraction_from_heading() {
        let pipeline = MarkdownPipeline::new();

        let content = "# My Title\n\nContent here.";
        let file = create_test_ingested_file(content, SourceType::Markdown);

        let (document, _, _) = pipeline.process(&file).expect("处理应成功");

        assert_eq!(document.title, "My Title", "标题应从 H1 标题提取");
    }

    #[test]
    fn test_title_fallback_to_first_line() {
        let pipeline = MarkdownPipeline::new();

        let content = "First line without heading.\n\nMore content.";
        let file = create_test_ingested_file(content, SourceType::Plain);

        let (document, _, _) = pipeline.process(&file).expect("处理应成功");

        assert_eq!(
            document.title, "First line without heading.",
            "无标题时应回退到第一行"
        );
    }

    #[test]
    fn test_empty_content_handling() {
        let pipeline = MarkdownPipeline::new();

        let file = create_test_ingested_file("", SourceType::Plain);

        let (document, blocks, tokens) = pipeline.process(&file).expect("空内容不应报错");

        assert_eq!(document.title, "Untitled", "空内容标题应为 Untitled");
        assert!(blocks.is_empty(), "空内容不应生成 Block");
        assert!(tokens.is_empty(), "空内容不应生成 Token");
    }

    #[test]
    fn test_default_pipeline_creation() {
        let pipeline = MarkdownPipeline::new();

        assert_eq!(
            pipeline.splitter.max_chunk_size(),
            1000,
            "默认 max_chunk_size 应为 1000"
        );
        assert_eq!(
            pipeline.splitter.chunk_overlap(),
            200,
            "默认 chunk_overlap 应为 200"
        );
    }

    #[test]
    fn test_custom_components_pipeline() {
        let custom_parser = ComrakAstParser::new();
        let custom_splitter = TextSplitterBlocker::with_config(200, 50).expect("测试配置应合法");
        let custom_tokenizer = IcuTokenizer::new();

        let pipeline =
            MarkdownPipeline::with_components(custom_parser, custom_splitter, custom_tokenizer);

        assert_eq!(pipeline.splitter.max_chunk_size(), 200, "自定义配置应生效");
        assert_eq!(pipeline.splitter.chunk_overlap(), 50, "自定义配置应生效");
    }

    #[test]
    fn test_global_offset_continuity_across_blocks() {
        let pipeline = MarkdownPipeline::new();

        let content = "First block.\n\nSecond block.";
        let file = create_test_ingested_file(content, SourceType::Plain);

        let (_, _, tokens) = pipeline.process(&file).expect("处理应成功");

        if tokens.len() >= 2 {
            assert!(
                tokens[1].global_offset > tokens[0].global_offset,
                "全局偏移应在块间递增"
            );
        }
    }

    #[test]
    fn test_default_creates_pipeline() {
        let pipeline = MarkdownPipeline::default();
        let file = create_test_ingested_file("# Test", SourceType::Markdown);
        let result = pipeline.process(&file);
        assert!(
            result.is_ok(),
            "Default 创建的流水线应能正常工作: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_process_with_code_block_uses_code_mapper() {
        let pipeline = MarkdownPipeline::new();
        let content = "# Title\n\n```rust\nfn main() {}\n```\n\nParagraph text.";
        let file = create_test_ingested_file(content, SourceType::Markdown);
        let result = pipeline.process(&file);
        assert!(
            result.is_ok(),
            "包含代码块的 Markdown 应成功处理: {:?}",
            result.err()
        );
        let (_, _, tokens) = result.unwrap();
        assert!(!tokens.is_empty(), "应生成 Token");
    }

    #[test]
    fn test_title_fallback_to_first_non_empty_line() {
        let pipeline = MarkdownPipeline::new();
        let content = "\n\n  \nFirst meaningful line\nMore content";
        let file = create_test_ingested_file(content, SourceType::Plain);
        let (document, _, _) = pipeline.process(&file).expect("处理应成功");
        assert_eq!(
            document.title, "First meaningful line",
            "标题应回退到第一个非空行"
        );
    }

    #[test]
    fn test_process_plain_text_creates_paragraph_blocks() {
        let pipeline = MarkdownPipeline::new();
        let content = "This is a paragraph.\n\nAnother paragraph.";
        let file = create_test_ingested_file(content, SourceType::Plain);
        let (_, blocks, _) = pipeline.process(&file).expect("处理应成功");
        assert!(!blocks.is_empty(), "纯文本应产生 Block");
        for block in &blocks {
            assert_eq!(
                block.block_type,
                BlockType::Paragraph,
                "纯文本 Block 应为 Paragraph 类型"
            );
        }
    }

    #[test]
    fn test_process_markdown_with_multiple_headings() {
        let pipeline = MarkdownPipeline::new();
        let content = "# Title 1\n\nContent 1\n\n## Title 2\n\nContent 2";
        let file = create_test_ingested_file(content, SourceType::Markdown);
        let (document, blocks, _) = pipeline.process(&file).expect("处理应成功");
        assert_eq!(document.title, "Title 1", "标题应从第一个 H1 提取");
        assert!(!blocks.is_empty(), "多个标题应产生 Block");
    }

    #[test]
    fn test_process_with_inline_code_and_regular_text() {
        let pipeline = MarkdownPipeline::new();
        let content = "# Heading\n\nText with `inline code` here.";
        let file = create_test_ingested_file(content, SourceType::Markdown);
        let result = pipeline.process(&file);
        assert!(
            result.is_ok(),
            "包含行内代码的 Markdown 应成功处理: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_process_document_has_correct_hash() {
        let pipeline = MarkdownPipeline::new();
        let file = create_test_ingested_file("# Test", SourceType::Markdown);
        let (document, _, _) = pipeline.process(&file).expect("处理应成功");
        assert_eq!(
            document.hash,
            "a".repeat(64),
            "Document hash 应与 IngestedFile 一致"
        );
    }
}
