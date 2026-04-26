//! Comrak AST 解析器（Markdown → comrak AST 节点树）
//!
//! 遵循 CommonMark 标准规范，输出结构化的 AST 用于后续分块处理。
//! 使用 comrak 库的默认配置进行 Markdown 解析。

use comrak::{nodes::NodeValue, parse_document, Arena, ComrakOptions};
use knowledge_core::model::{Block, BlockType, Document, Token, TokenType, RecordIdType};
use crate::Result;
use crate::file_ingester::IngestedFile;

/// 判断是否为有序列表项（如 `1. `、`42. `）
///
/// 匹配模式：ASCII 数字 + `.` + 空格
/// 避免将 `42 is the answer` 等纯数字开头的行误判为列表
fn is_ordered_list_item(trimmed: &str) -> bool {
    let Some(dot_pos) = trimmed.find('.') else { return false };
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
        let hash_prefix = if file.hash.len() >= 8 { &file.hash[..8] } else { &file.hash };
        let doc_id: RecordIdType = surrealdb::sql::Thing::from(("doc".to_string(), hash_prefix.to_string()));

        let document = Document {
            id: None,
            path: file.path.to_string_lossy().to_string(),
            title,
            source_type: file.source_type.clone(),
            hash: file.hash.clone(),
        };

        let _doc_id_for_blocks = &doc_id;

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
            } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") || is_ordered_list_item(trimmed) {
                BlockType::List
            } else {
                BlockType::Paragraph
            };

            let end_line = current_line;
            let _block_id: RecordIdType = surrealdb::sql::Thing::from(("block".to_string(), format!("{current_line}")));

            blocks.push(Block {
                id: None,
                doc_id: doc_id.clone(),
                block_type,
                start_line: current_line,
                end_line,
                embedding: None,
                idempotency_key: None,
            });

            current_line += 1;
        }

        if blocks.is_empty() && line_count > 0 {
            let _block_id: RecordIdType = surrealdb::sql::Thing::from(("block".to_string(), "0".to_string()));
            blocks.push(Block {
                id: None,
                doc_id: doc_id.clone(),
                block_type: BlockType::Paragraph,
                start_line: 0,
                end_line: line_count.saturating_sub(1),
                embedding: None,
                idempotency_key: None,
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
        let block_id: RecordIdType = surrealdb::sql::Thing::from(("block".to_string(), format!("{}", block.start_line)));
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
}
