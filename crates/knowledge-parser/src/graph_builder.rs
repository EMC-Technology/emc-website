//! 图构建器（组装完整实体图 + 幂等键生成）
//!
//! GraphBuilder 将 Markdown/Code 解析产出的 Document、Block、Token、Reference
//! 组装为完整的 BuiltGraph，并为每个实体生成 BLAKE3 幂等键。

use crate::Result;
use crate::code_pipeline::CodePipeline;
use crate::file_ingester::IngestedFile;
use crate::idempotency::IdempotencyKeyGenerator;
use crate::markdown_pipeline::MarkdownPipeline;
use crate::symbol_resolver::SymbolResolver;
use knowledge_core::model::{Block, Document, Reference, SourceType, Token};

/// 图构建器的完整输出
#[derive(Debug)]
pub struct BuiltGraph {
    /// 文档元数据
    pub document: Document,
    /// 所有 Block 实体
    pub blocks: Vec<Block>,
    /// 所有 Token 实体
    pub tokens: Vec<Token>,
    /// 所有 Reference 边
    pub references: Vec<Reference>,
}

/// 图构建器：组装完整实体图 + 幂等键生成
///
/// `GraphBuilder` 是解析流水线的最终阶段，负责：
/// 1. 调用 `MarkdownPipeline` 或 `CodePipeline` 解析文件
/// 2. 为每个 Block/Token 生成 BLAKE3 幂等键
/// 3. 运行 `SymbolResolver` 提取引用关系
/// 4. 组装为 `BuiltGraph`
pub struct GraphBuilder {
    markdown_pipeline: MarkdownPipeline,
    code_pipeline: CodePipeline,
    symbol_resolver: SymbolResolver,
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphBuilder {
    /// 创建新的图构建器
    #[must_use]
    pub fn new() -> Self {
        Self {
            markdown_pipeline: MarkdownPipeline::new(),
            code_pipeline: CodePipeline::new(),
            symbol_resolver: SymbolResolver::new(),
        }
    }

    /// 重置内部状态（允许复用构建器实例）
    pub fn reset(&mut self) {
        self.symbol_resolver = SymbolResolver::new();
    }

    /// 从 Markdown 文件构建图
    ///
    /// # Errors
    ///
    /// Markdown 解析或文档创建失败时返回错误
    pub fn build_from_markdown(&mut self, file: &IngestedFile) -> Result<BuiltGraph> {
        let (document, blocks, tokens) = self.markdown_pipeline.process(file)?;

        let doc_id_str = document
            .id
            .as_ref()
            .map(std::string::ToString::to_string)
            .unwrap_or_default();
        let references = self.symbol_resolver.resolve(&tokens, &doc_id_str);

        let blocks = Self::assign_idempotency_keys(blocks, &document.hash, &file.content);

        Ok(BuiltGraph {
            document,
            blocks,
            tokens,
            references,
        })
    }

    /// 从代码文件构建图
    ///
    /// # Errors
    ///
    /// 代码解析或文档创建失败时返回错误
    pub fn build_from_code(&mut self, file: &IngestedFile) -> Result<BuiltGraph> {
        let (document, blocks, tokens) = self.code_pipeline.process(file)?;

        let doc_id_str = document
            .id
            .as_ref()
            .map(std::string::ToString::to_string)
            .unwrap_or_default();
        let references = self.symbol_resolver.resolve(&tokens, &doc_id_str);

        let blocks = Self::assign_idempotency_keys(blocks, &document.hash, &file.content);

        Ok(BuiltGraph {
            document,
            blocks,
            tokens,
            references,
        })
    }

    /// 根据 `SourceType` 自动路由到对应的解析路径
    ///
    /// # Errors
    ///
    /// 解析或文档创建失败时返回错误
    pub fn build(&mut self, file: &IngestedFile) -> Result<BuiltGraph> {
        match file.source_type {
            SourceType::Markdown | SourceType::Plain => self.build_from_markdown(file),
            SourceType::Code => self.build_from_code(file),
        }
    }

    /// 为 Block 列表分配幂等键
    ///
    /// 幂等键基于 `doc_hash + start_line + end_line + content_hash` 生成，
    /// 确保内容变更时幂等键不同，支持增量更新检测。
    fn assign_idempotency_keys(
        blocks: Vec<Block>,
        doc_hash: &str,
        file_content: &str,
    ) -> Vec<Block> {
        blocks
            .into_iter()
            .map(|mut block| {
                if block.idempotency_key.is_none() {
                    let block_content =
                        extract_block_text(file_content, block.start_line, block.end_line);
                    block.idempotency_key = Some(IdempotencyKeyGenerator::generate_for_block(
                        doc_hash,
                        block.start_line,
                        block.end_line,
                        &block_content,
                    ));
                }
                block
            })
            .collect()
    }
}

/// 从文件内容中提取指定行范围的文本
fn extract_block_text(content: &str, start_line: u32, end_line: u32) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let start = start_line as usize;
    let end = (end_line as usize + 1).min(lines.len());
    if start >= lines.len() {
        return String::new();
    }
    lines[start..end].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use knowledge_core::model::RecordIdType;

    fn record_id_to_string(id: &RecordIdType) -> String {
        format!("{}:{}", id.tb, id.id)
    }

    fn make_ingested_file(content: &str, source_type: SourceType) -> IngestedFile {
        IngestedFile {
            path: if source_type == SourceType::Code {
                std::path::PathBuf::from("/test.rs")
            } else {
                std::path::PathBuf::from("/test.md")
            },
            content: content.to_string(),
            hash: "a".repeat(64),
            file_size: content.len() as u64,
            source_type,
        }
    }

    #[test]
    fn test_build_from_markdown_simple_input_returns_built_graph() {
        let mut builder = GraphBuilder::new();
        let file = make_ingested_file("# Hello\n\nWorld", SourceType::Markdown);
        let result = builder.build_from_markdown(&file);
        assert!(
            result.is_ok(),
            "简单 Markdown 应成功构建: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_build_from_markdown_empty_input_returns_empty_graph() {
        let mut builder = GraphBuilder::new();
        let file = make_ingested_file("", SourceType::Markdown);
        let result = builder.build_from_markdown(&file);
        assert!(result.is_ok(), "空 Markdown 应成功构建");

        let graph = result.unwrap();
        assert!(graph.blocks.is_empty(), "空内容不应产生 Block");
        assert!(graph.tokens.is_empty(), "空内容不应产生 Token");
    }

    #[test]
    fn test_build_from_markdown_assigns_idempotency_keys() {
        let mut builder = GraphBuilder::new();
        let file = make_ingested_file("# Title\n\nParagraph text", SourceType::Markdown);
        let graph = builder.build_from_markdown(&file).unwrap();

        for block in &graph.blocks {
            assert!(block.idempotency_key.is_some(), "每个 Block 应被分配幂等键");
            let key = block.idempotency_key.as_ref().unwrap();
            assert_eq!(key.len(), 64, "幂等键应为 64 字符 BLAKE3 hex");
        }
    }

    #[test]
    fn test_build_from_markdown_preserves_document_metadata() {
        let mut builder = GraphBuilder::new();
        let file = make_ingested_file("# My Title\n\nBody", SourceType::Markdown);
        let graph = builder.build_from_markdown(&file).unwrap();

        assert_eq!(graph.document.path, "/test.md");
        assert_eq!(graph.document.source_type, SourceType::Markdown);
        assert_eq!(graph.document.hash, "a".repeat(64));
    }

    #[test]
    fn test_build_dispatches_by_source_type_markdown() {
        let mut builder = GraphBuilder::new();
        let file = make_ingested_file("# Hello", SourceType::Markdown);
        let result = builder.build(&file);
        assert!(result.is_ok(), "SourceType::Markdown 应走 Markdown 路径");
    }

    #[test]
    fn test_build_dispatches_by_source_type_plain() {
        let mut builder = GraphBuilder::new();
        let file = make_ingested_file("Plain text", SourceType::Plain);
        let result = builder.build(&file);
        assert!(result.is_ok(), "SourceType::Plain 应走 Markdown 路径");
    }

    #[test]
    fn test_record_id_to_string_formats_correctly() {
        let doc_id: RecordIdType =
            surrealdb::sql::Thing::from(("doc".to_string(), "abc123".to_string()));
        let s = record_id_to_string(&doc_id);
        assert_eq!(s, "doc:abc123");
    }

    #[test]
    fn test_built_graph_debug_trait() {
        let graph = BuiltGraph {
            document: knowledge_core::model::Document::new(
                "/test.md",
                "Test",
                SourceType::Markdown,
                "a".repeat(64),
            )
            .unwrap(),
            blocks: vec![],
            tokens: vec![],
            references: vec![],
        };
        let debug_str = format!("{graph:?}");
        assert!(!debug_str.is_empty(), "BuiltGraph 应实现 Debug");
    }

    #[test]
    fn test_reset_allows_reuse_after_build() {
        let mut builder = GraphBuilder::new();
        let file = make_ingested_file("# First", SourceType::Markdown);
        let _ = builder.build_from_markdown(&file);

        builder.reset();

        let file2 = make_ingested_file("# Second", SourceType::Markdown);
        let result = builder.build_from_markdown(&file2);
        assert!(result.is_ok(), "reset 后应能再次构建");
    }

    #[test]
    fn test_build_from_markdown_heading_creates_heading_block() {
        let mut builder = GraphBuilder::new();
        let file = make_ingested_file("# Title\n\nParagraph", SourceType::Markdown);
        let graph = builder.build_from_markdown(&file).unwrap();

        assert!(
            graph
                .blocks
                .iter()
                .any(|b| b.block_type == knowledge_core::model::BlockType::Heading),
            "Markdown 标题行应产生 Heading 类型的 Block"
        );
    }

    #[test]
    fn test_default_creates_valid_builder() {
        let mut builder = GraphBuilder::default();
        let file = make_ingested_file("# Test", SourceType::Markdown);
        let result = builder.build_from_markdown(&file);
        assert!(
            result.is_ok(),
            "Default 构建器应能正常工作: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_build_from_code_with_rust_file() {
        let mut builder = GraphBuilder::new();
        let file = make_ingested_file("fn main() { println!(\"hello\"); }", SourceType::Code);
        let result = builder.build_from_code(&file);
        match result {
            Ok(graph) => {
                assert_eq!(graph.document.source_type, SourceType::Code);
                assert!(!graph.blocks.is_empty(), "代码文件应产生 Block");
            }
            Err(e)
                if e.code().contains("PARSE")
                    && e.source() == error_core::prelude::ErrorSource::USR =>
            {
                eprintln!("Rust grammar 未安装，跳过: {}", e.message());
            }
            Err(e) => panic!("意外的错误: {e}"),
        }
    }

    #[test]
    fn test_build_routes_code_to_code_pipeline() {
        let mut builder = GraphBuilder::new();
        let file = make_ingested_file("fn foo() {}", SourceType::Code);
        let result = builder.build(&file);
        match result {
            Ok(graph) => {
                assert_eq!(graph.document.source_type, SourceType::Code);
            }
            Err(e) if e.code().contains("PARSE") => {
                eprintln!("Grammar 未安装，跳过: {}", e.message());
            }
            Err(e) => panic!("意外的错误: {e}"),
        }
    }

    #[test]
    fn test_extract_block_text_normal_range() {
        let content = "line0\nline1\nline2\nline3";
        let text = extract_block_text(content, 1, 2);
        assert_eq!(text, "line1\nline2");
    }

    #[test]
    fn test_extract_block_text_start_beyond_content() {
        let content = "line0\nline1";
        let text = extract_block_text(content, 10, 15);
        assert!(text.is_empty(), "start 超出内容范围应返回空字符串");
    }

    #[test]
    fn test_extract_block_text_end_clamped_to_content() {
        let content = "line0\nline1\nline2";
        let text = extract_block_text(content, 1, 10);
        assert_eq!(text, "line1\nline2", "end 应被裁剪到内容范围内");
    }

    #[test]
    fn test_extract_block_text_single_line() {
        let content = "only line";
        let text = extract_block_text(content, 0, 0);
        assert_eq!(text, "only line");
    }

    #[test]
    fn test_extract_block_text_empty_content() {
        let text = extract_block_text("", 0, 0);
        assert!(text.is_empty(), "空内容应返回空字符串");
    }
}
