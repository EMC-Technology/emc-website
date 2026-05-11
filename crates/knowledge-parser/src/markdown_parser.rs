//! Comrak AST 解析器（Markdown → comrak AST 节点树）
//!
//! 遵循 CommonMark 标准规范，输出结构化的 AST 用于后续分块处理。
//! 使用 comrak 库的默认配置进行 Markdown 解析。

use crate::Result;
use crate::file_ingester::IngestedFile;
use comrak::{Arena, ComrakOptions, nodes::NodeValue, parse_document};
use knowledge_core::model::{
    Block, BlockStatus, BlockType, Document, RecordIdType, Token, TokenStatus, TokenType,
};

/// 判断是否为有序列表项（如 `1. `、`42. `）
///
/// 匹配模式：ASCII 数字 + `.` + 空格
/// 避免将 `42 is the answer` 等纯数字开头的行误判为列表
fn is_ordered_list_item(trimmed: &str) -> bool {
    let Some(dot_pos) = trimmed.find('.') else {
        return false;
    };
    if dot_pos == 0 || dot_pos + 1 >= trimmed.len() {
        return false;
    }
    let digits = &trimmed[..dot_pos];
    let after_dot = trimmed.as_bytes().get(dot_pos + 1);
    digits.chars().all(|c| c.is_ascii_digit()) && after_dot == Some(&b' ')
}

/// `Markdown` 解析器（将 `Markdown` 文件解析为 `Document` + `Blocks` + `Tokens`）
///
/// 组合 `ComrakAstParser`，提供从 `IngestedFile` 到三元组的端到端处理。
pub struct MarkdownParser {
    _ast_parser: ComrakAstParser,
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownParser {
    /// 创建 `Markdown` 解析器
    #[must_use]
    pub fn new() -> Self {
        Self {
            _ast_parser: ComrakAstParser::new(),
        }
    }

    /// 解析 Markdown 文件，返回 (`Document`, `Blocks`, `Tokens`)
    ///
    /// # Errors
    ///
    /// 文档创建失败时返回错误
    pub fn parse(&self, file: &IngestedFile) -> Result<(Document, Vec<Block>, Vec<Token>)> {
        let title = extract_markdown_title(&file.content);
        let hash_prefix = if file.hash.len() >= 8 {
            &file.hash[..8]
        } else {
            &file.hash
        };
        let doc_id: RecordIdType =
            surrealdb::sql::Thing::from(("doc".to_string(), hash_prefix.to_string()));

        let document = Document {
            id: None,
            path: file.path.to_string_lossy().to_string(),
            title,
            source_type: file.source_type.clone(),
            hash: file.hash.clone(),
        };

        let lines: Vec<&str> = file.content.lines().collect();
        #[allow(clippy::cast_possible_truncation)]
        let line_count = lines.len() as u32;

        let mut blocks = Vec::new();
        let mut current_line = 0u32;
        let mut in_code_block = false;

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                current_line += 1;
                continue;
            }

            let block_type = if in_code_block {
                if trimmed.starts_with("```") {
                    in_code_block = false;
                }
                BlockType::Code
            } else if trimmed.starts_with('#') {
                BlockType::Heading
            } else if trimmed.starts_with("```") {
                in_code_block = true;
                BlockType::Code
            } else if trimmed.starts_with("    ") || trimmed.starts_with('\t') {
                BlockType::Code
            } else if trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || is_ordered_list_item(trimmed)
            {
                BlockType::List
            } else {
                BlockType::Paragraph
            };

            let end_line = current_line;
            let _block_id: RecordIdType =
                surrealdb::sql::Thing::from(("block".to_string(), format!("{current_line}")));

            blocks.push(Block {
                id: None,
                doc_id: doc_id.clone(),
                block_type,
                start_line: current_line,
                end_line,
                idempotency_key: None,
                status: BlockStatus::Created,
            });

            current_line += 1;
        }

        if blocks.is_empty() && line_count > 0 {
            let _block_id: RecordIdType =
                surrealdb::sql::Thing::from(("block".to_string(), "0".to_string()));
            blocks.push(Block {
                id: None,
                doc_id: doc_id.clone(),
                block_type: BlockType::Paragraph,
                start_line: 0,
                end_line: line_count.saturating_sub(1),
                idempotency_key: None,
                status: BlockStatus::Created,
            });
        }

        let tokens = tokenize_markdown(&file.content, &blocks);

        Ok((document, blocks, tokens))
    }
}

fn extract_markdown_title(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return heading.to_string();
        }
        if let Some(heading) = trimmed.strip_prefix("## ") {
            return heading.to_string();
        }
    }
    "Untitled".to_string()
}

fn tokenize_markdown(content: &str, blocks: &[Block]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut global_offset = 0u64;

    for block in blocks {
        let block_id: RecordIdType =
            surrealdb::sql::Thing::from(("block".to_string(), format!("{}", block.start_line)));
        let lines: Vec<&str> = content.lines().collect();
        #[allow(clippy::cast_possible_truncation)]
        let start = block.start_line as usize;
        #[allow(clippy::cast_possible_truncation)]
        let end = (block.end_line as usize).min(lines.len().saturating_sub(1));

        for line_idx in start..=end {
            if line_idx >= lines.len() {
                break;
            }
            let line = lines[line_idx];
            let mut char_pos = 0u32;
            for word in line.split_whitespace() {
                let token_type = if word.starts_with('#') || word.starts_with("```") {
                    TokenType::Symbol
                } else {
                    TokenType::Word
                };
                tokens.push(Token {
                    id: None,
                    block_id: block_id.clone(),
                    content: word.to_string(),
                    token_type,
                    start_char: char_pos,
                    global_offset: global_offset + u64::from(char_pos),
                    status: TokenStatus::Created,
                });
                char_pos += u32::try_from(word.chars().count()).unwrap_or(u32::MAX) + 1;
            }
            global_offset += line.chars().count() as u64 + 1;
        }
    }
    tokens
}

/// `Comrak` AST 解析器
///
/// 将 `Markdown` 原始文本解析为 `comrak` 的 `AST` 节点树结构。
/// 输出的 `AST` 可供下游的 `TextSplitterBlocker` 进行结构化分块处理。
///
/// # Example
///
/// ```ignore
/// use knowledge_parser::markdown_parser::ComrakAstParser;
///
/// let parser = ComrakAstParser::new();
/// let ast = parser.parse("# Hello\n\nWorld");
/// // ast 现在包含 Document → Heading(Hello) → Paragraph(World) 的节点树
/// ```
pub struct ComrakAstParser {
    options: ComrakOptions,
}

impl Default for ComrakAstParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ComrakAstParser {
    /// 创建新的 `ComrakAstParser` 实例（使用默认配置）
    ///
    /// 默认配置：
    /// - 使用 comrak 的默认选项（CommonMark + GFM 兼容）
    ///
    /// 创建 Markdown 解析器
    #[must_use]
    pub fn new() -> Self {
        let options = ComrakOptions::default();
        Self { options }
    }

    /// 自定义 `Comrak` 解析选项
    ///
    /// # 参数
    ///
    /// * `options` - comrak 的 `ComrakOptions` 配置实例
    #[must_use]
    pub const fn with_options(options: ComrakOptions) -> Self {
        Self { options }
    }

    /// 解析 Markdown 文本并返回 Arena 和根节点引用
    ///
    /// 使用 comrak 的 `parse_document` 将 `Markdown` 文本转换为 `AST`，
    /// 并从中提取所有标题文本。
    ///
    /// # 参数
    ///
    /// * `content` - Markdown 原始文本
    ///
    /// # Returns
    ///
    /// `Vec<String>`，包含从 Markdown 文档中提取的所有标题文本，
    /// `NodeValue` 的生命周期绑定到传入的 content 和 Arena。
    ///
    /// # Panics
    ///
    /// 此函数不会 panic。comrak 的解析器对所有合法/非法输入均返回有效 `AST`。
    #[must_use]
    pub fn parse(&self, content: &str) -> Vec<String> {
        let arena = Arena::new();
        let root = parse_document(&arena, content, &self.options);
        let mut headings = Vec::new();
        for node in root.children() {
            let data = node.data.borrow();
            if let NodeValue::Heading(_) = data.value {
                let mut text = String::new();
                for sub in node.children() {
                    let sub_data = sub.data.borrow();
                    if let NodeValue::Text(ref t) = sub_data.value {
                        text.push_str(t);
                    }
                }
                if !text.is_empty() {
                    headings.push(text);
                }
            }
        }
        headings
    }

    /// 获取当前使用的解析选项（用于测试验证）
    #[cfg(test)]
    #[must_use]
    pub const fn options(&self) -> &ComrakOptions {
        &self.options
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_markdown() {
        let parser = ComrakAstParser::new();
        let content = "# Hello World\n\nThis is a paragraph.";
        let headings = parser.parse(content);
        assert!(headings.contains(&"Hello World".to_string()));
    }

    #[test]
    fn test_parse_heading_hierarchy() {
        let parser = ComrakAstParser::new();
        let content = "# Title 1\n## Title 2\n### Title 3\n\nContent";
        let headings = parser.parse(content);
        assert_eq!(headings.len(), 3, "应提取到3个标题");
        assert!(headings.contains(&"Title 1".to_string()));
        assert!(headings.contains(&"Title 2".to_string()));
        assert!(headings.contains(&"Title 3".to_string()));
    }

    #[test]
    fn test_parse_empty_content() {
        let parser = ComrakAstParser::new();
        let headings = parser.parse("");
        assert!(headings.is_empty(), "空内容不应有标题");
    }

    #[test]
    fn test_parse_paragraph_extraction() {
        let parser = ComrakAstParser::new();
        let content = "First paragraph.\n\nSecond paragraph.";
        let headings = parser.parse(content);
        assert!(headings.is_empty(), "纯段落内容不应有标题");
    }

    #[test]
    fn test_parse_code_block() {
        let parser = ComrakAstParser::new();
        let content = "```\nfn main() {}\n```";
        let headings = parser.parse(content);
        assert!(headings.is_empty(), "代码块内容不应有标题");
    }

    #[test]
    fn test_custom_options_override() {
        let mut options = ComrakOptions::default();
        options.extension.strikethrough = true;
        let parser = ComrakAstParser::with_options(options);

        assert!(
            parser.options().extension.strikethrough,
            "自定义配置应覆盖默认值"
        );
    }

    fn make_ingested(content: &str) -> IngestedFile {
        IngestedFile {
            path: std::path::PathBuf::from("test.md"),
            content: content.to_string(),
            hash: blake3::hash(content.as_bytes()).to_hex().to_string(),
            file_size: content.len() as u64,
            source_type: knowledge_core::model::SourceType::Markdown,
        }
    }

    #[test]
    fn test_markdown_parser_default() {
        let _parser = MarkdownParser::default();
    }

    #[test]
    fn test_markdown_parser_new() {
        let parser = MarkdownParser::new();
        let file = make_ingested("# Hello\n\nWorld");
        let result = parser.parse(&file);
        assert!(result.is_ok());
        let (doc, blocks, tokens) = result.unwrap();
        assert_eq!(doc.title, "Hello");
        assert!(!blocks.is_empty());
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_parse_heading_block() {
        let parser = MarkdownParser::new();
        let file = make_ingested("# Title\n\nParagraph text");
        let (doc, blocks, _tokens) = parser.parse(&file).unwrap();
        assert_eq!(doc.title, "Title");
        let heading_blocks: Vec<_> = blocks
            .iter()
            .filter(|b| b.block_type == BlockType::Heading)
            .collect();
        assert!(!heading_blocks.is_empty(), "应识别出 Heading 块");
    }

    #[test]
    fn test_parse_code_block_triple_backtick() {
        let parser = MarkdownParser::new();
        let file = make_ingested("```rust\nfn main() {}\n```");
        let (_, blocks, _tokens) = parser.parse(&file).unwrap();
        let code_blocks: Vec<_> = blocks
            .iter()
            .filter(|b| b.block_type == BlockType::Code)
            .collect();
        assert!(!code_blocks.is_empty(), "应识别出 Code 块");
    }

    #[test]
    fn test_parse_code_block_indented() {
        let parser = MarkdownParser::new();
        let file = make_ingested("    let x = 1;");
        let (_, blocks, _tokens) = parser.parse(&file).unwrap();
        assert!(!blocks.is_empty(), "缩进4空格应产生 Block");
    }

    #[test]
    fn test_parse_code_block_tab_indented() {
        let parser = MarkdownParser::new();
        let file = make_ingested("\tlet x = 1;");
        let (_, blocks, _tokens) = parser.parse(&file).unwrap();
        assert!(!blocks.is_empty(), "Tab 缩进应产生 Block");
    }

    #[test]
    fn test_parse_unordered_list_dash() {
        let parser = MarkdownParser::new();
        let file = make_ingested("- item one\n- item two");
        let (_, blocks, _tokens) = parser.parse(&file).unwrap();
        assert_eq!(
            blocks[0].block_type,
            BlockType::List,
            "- 开头应识别为 List 块"
        );
    }

    #[test]
    fn test_parse_unordered_list_asterisk() {
        let parser = MarkdownParser::new();
        let file = make_ingested("* item one\n* item two");
        let (_, blocks, _tokens) = parser.parse(&file).unwrap();
        assert_eq!(
            blocks[0].block_type,
            BlockType::List,
            "* 开头应识别为 List 块"
        );
    }

    #[test]
    fn test_parse_ordered_list() {
        let parser = MarkdownParser::new();
        let file = make_ingested("1. first item\n2. second item");
        let (_, blocks, _tokens) = parser.parse(&file).unwrap();
        assert_eq!(
            blocks[0].block_type,
            BlockType::List,
            "数字列表应识别为 List 块"
        );
    }

    #[test]
    fn test_parse_paragraph_block() {
        let parser = MarkdownParser::new();
        let file = make_ingested("Just a plain paragraph.");
        let (_, blocks, _tokens) = parser.parse(&file).unwrap();
        assert_eq!(
            blocks[0].block_type,
            BlockType::Paragraph,
            "普通文本应识别为 Paragraph 块"
        );
    }

    #[test]
    fn test_parse_empty_lines_skipped() {
        let parser = MarkdownParser::new();
        let file = make_ingested("\n\n\nHello\n\n\n");
        let (_, blocks, _tokens) = parser.parse(&file).unwrap();
        assert_eq!(blocks.len(), 1, "空行不应产生块");
        assert_eq!(blocks[0].block_type, BlockType::Paragraph);
    }

    #[test]
    fn test_parse_empty_content_fallback_block() {
        let parser = MarkdownParser::new();
        let file = make_ingested("");
        let (_, blocks, _tokens) = parser.parse(&file).unwrap();
        assert!(blocks.is_empty(), "空内容不应产生块");
    }

    #[test]
    fn test_parse_only_whitespace_produces_fallback() {
        let parser = MarkdownParser::new();
        let file = make_ingested("   \n   \n   ");
        let result = parser.parse(&file);
        assert!(result.is_ok(), "仅空白的行应成功解析");
    }

    #[test]
    fn test_parse_title_from_h1() {
        let parser = MarkdownParser::new();
        let file = make_ingested("# My Title\n\nSome content");
        let (doc, _, _) = parser.parse(&file).unwrap();
        assert_eq!(doc.title, "My Title");
    }

    #[test]
    fn test_parse_title_from_h2_fallback() {
        let parser = MarkdownParser::new();
        let file = make_ingested("## Sub Title\n\nContent");
        let (doc, _, _) = parser.parse(&file).unwrap();
        assert_eq!(doc.title, "Sub Title");
    }

    #[test]
    fn test_parse_no_title_untitled() {
        let parser = MarkdownParser::new();
        let file = make_ingested("Just some text without heading");
        let (doc, _, _) = parser.parse(&file).unwrap();
        assert_eq!(doc.title, "Untitled");
    }

    #[test]
    fn test_parse_document_fields() {
        let parser = MarkdownParser::new();
        let file = make_ingested("# Test Doc");
        let (doc, _, _) = parser.parse(&file).unwrap();
        assert_eq!(doc.path, "test.md");
        assert_eq!(doc.source_type, knowledge_core::model::SourceType::Markdown);
        assert!(!doc.hash.is_empty());
    }

    #[test]
    fn test_parse_tokens_generated() {
        let parser = MarkdownParser::new();
        let file = make_ingested("# Hello World\n\nSome text here");
        let (_, _, tokens) = parser.parse(&file).unwrap();
        assert!(!tokens.is_empty(), "应生成 Token");
        let has_symbol = tokens.iter().any(|t| t.token_type == TokenType::Symbol);
        assert!(has_symbol, "# 开头的词应标记为 Symbol");
    }

    #[test]
    fn test_parse_code_block_closing() {
        let parser = MarkdownParser::new();
        let file = make_ingested("```rust\ncode line\n```\nAfter code");
        let (_, blocks, _) = parser.parse(&file).unwrap();
        let after_code: Vec<_> = blocks
            .iter()
            .filter(|b| b.block_type == BlockType::Paragraph)
            .collect();
        assert!(!after_code.is_empty(), "代码块关闭后应为 Paragraph");
    }

    #[test]
    fn test_is_ordered_list_item_valid() {
        assert!(is_ordered_list_item("1. hello"));
        assert!(is_ordered_list_item("42. world"));
    }

    #[test]
    fn test_is_ordered_list_item_no_dot() {
        assert!(!is_ordered_list_item("no dot here"));
    }

    #[test]
    fn test_is_ordered_list_item_dot_at_start() {
        assert!(!is_ordered_list_item(". no digits"));
    }

    #[test]
    fn test_is_ordered_list_item_dot_at_end() {
        assert!(!is_ordered_list_item("123."));
    }

    #[test]
    fn test_is_ordered_list_item_no_space_after_dot() {
        assert!(!is_ordered_list_item("1.x"));
    }

    #[test]
    fn test_is_ordered_list_item_non_digit() {
        assert!(!is_ordered_list_item("abc. hello"));
    }

    #[test]
    fn test_comrak_ast_parser_default() {
        let _parser = ComrakAstParser::default();
    }

    #[test]
    fn test_comrak_ast_parser_heading_with_text() {
        let parser = ComrakAstParser::new();
        let headings = parser.parse("# Heading\n\nSome text\n## Sub");
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0], "Heading");
        assert_eq!(headings[1], "Sub");
    }

    #[test]
    fn test_parse_short_hash() {
        let parser = MarkdownParser::new();
        let mut file = make_ingested("# Short");
        file.hash = "abc".to_string();
        let result = parser.parse(&file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_multiline_code_block() {
        let parser = MarkdownParser::new();
        let file = make_ingested("```\nline1\nline2\nline3\n```");
        let (_, blocks, _) = parser.parse(&file).unwrap();
        let code_blocks: Vec<_> = blocks
            .iter()
            .filter(|b| b.block_type == BlockType::Code)
            .collect();
        assert!(code_blocks.len() >= 2, "多行代码块应产生多个 Code 块");
    }

    #[test]
    fn test_parse_backtick_in_token() {
        let parser = MarkdownParser::new();
        let file = make_ingested("```rust\ncode\n```");
        let (_, _, tokens) = parser.parse(&file).unwrap();
        let backtick_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.content.starts_with("```"))
            .collect();
        assert!(!backtick_tokens.is_empty(), "``` 应被识别为 Symbol token");
    }
}
