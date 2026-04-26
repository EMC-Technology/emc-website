//! Code 解析流水线（DAG 分支 B 完整流程）
//!
//! # 处理链路
//!
//! ```text
//! IngestedFile → TreeSitterParser (AST) → CodeTokenMapper (Token映射)
//!                                              ↓
//!                                    Document + Block(1) + Tokens(N)
//! ```
//!
//! # 设计说明
//!
//! 本模块是 DAG 分支 B 的顶层编排器，组合以下子组件：
//! - [`TreeSitterParser`]：tree-sitter AST 解析器
//! - [`GrammarCachePool`]：Grammar 缓存池（懒加载）
//! - [`CodeTokenMapper`]：AST 节点 → 系统 Token 映射器
//!
//! # 核心不变量
//!
//! - **代码文件通常生成 1 个 Block**（整个文件作为一个代码块）
//! - **所有 Token 来自 tree-sitter AST 节点**（不走 icu_segmenter）
//! - **保持符号完整性的关键在于 CodeTokenMapper 的映射逻辑**
//!
//! # 性能预算
//!
//! P99 < 500ms（≤1000 行代码文件）

use std::path::Path;

use knowledge_core::model::{Block, BlockType, Document, SourceType, Token, RecordIdType};
use crate::Result;
use error_core::helpers;

use crate::code_token_mapper::CodeTokenMapper;
use crate::file_ingester::IngestedFile;
use crate::tree_sitter_parser::TreeSitterParser;

/// 单个代码文件的解析输出三元组
///
/// 包含文档元数据、代码块列表和 Token 列表。
pub type ParseOutput = (Document, Vec<Block>, Vec<Token>);

/// 文件扩展名到语言标识符的映射表
///
/// 用于从 IngestedFile.path 自动推断编程语言。
static EXTENSION_LANG_MAP: &[(&str, &str)] = &[
    ("rs", "rust"),
    ("py", "python"),
    ("pyw", "python"),
    ("js", "javascript"),
    ("mjs", "javascript"),
    ("cjs", "javascript"),
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("jsx", "javascript"),
    ("go", "go"),
    ("c", "c"),
    ("h", "c"),
    ("cpp", "cpp"),
    ("cc", "cpp"),
    ("cxx", "cpp"),
    ("hpp", "cpp"),
    ("hxx", "cpp"),
    ("java", "java"),
    ("rb", "ruby"),
    ("rbw", "ruby"),
    ("kt", "kotlin"),
    ("kts", "kotlin"),
    ("swift", "swift"),
    ("zig", "zig"),
    ("toml", "toml"),
    ("yaml", "yaml"),
    ("yml", "yaml"),
    ("json", "json"),
];

/// Code 解析流水线（DAG 分支 B 完整流程）
///
/// 组合 [`TreeSitterParser`]、[`GrammarCachePool`] 和 [`CodeTokenMapper`]，
/// 提供从 [`IngestedFile`] 到 (`Document`, `Block`, `Token`) 三元组的端到端处理能力。
///
/// # 线程安全说明
///
/// `CodePipeline` 内部持有 `TreeSitterParser`，而 `tree_sitter::Parser`
/// 不是 `Send`。因此本结构体也不实现 `Send`。
///
/// 如需跨线程使用，请使用 `Arc<Mutex<CodePipeline>>` 包装。
pub struct CodePipeline {
    parser: TreeSitterParser,
}

impl Default for CodePipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl CodePipeline {
    /// 创建新的 Code 解析流水线实例
    #[must_use]
    pub fn new() -> Self {
        Self {
            parser: TreeSitterParser::new(),
        }
    }

    /// 执行完整的代码文件解析流程
    ///
    /// 处理步骤：
    /// 1. 根据 file.path 扩展名确定编程语言
    /// 2. 加载对应的 `tree-sitter` grammar（`GrammarCachePool`）
    /// 3. 调用 `TreeSitterParser.parse()` 生成 AST
    /// 4. 提取 AST 节点列表
    /// 5. 调用 `CodeTokenMapper.map_nodes_to_tokens()` 转换为 Token
    /// 6. 构建 `Document` + `Block`(代码块) + `Token`
    ///
    /// # 特殊说明
    ///
    /// - 代码文件通常生成 **1 个 `Block`**（整个文件作为一个代码块）
    /// - 所有 Token 来自 `tree-sitter` AST 节点（**不走 `icu_segmenter`**）
    /// - 保持符号完整性的关键在于 `CodeTokenMapper` 的映射逻辑
    ///
    /// # 参数
    ///
    /// * `file` - 由 [`FileIngester`] 处理后的输入文件对象
    ///
    /// # Errors
    ///
    /// * 不支持的文件格式（对应 `ErrorObject` code `ERR-USR-PARSE-002`） - 文件扩展名不支持或 grammar 未安装
    /// * 解析失败（对应 `ErrorObject` code `ERR-FS-PARSE-001`） - tree-sitter 解析失败
    ///
    /// # Example
    ///
    /// ```ignore
    /// let pipeline = CodePipeline::new();
    /// let ingester = FileIngester::new();
    /// let file = ingester.ingest(Path::new("src/main.rs")).await?;
    /// let (doc, blocks, tokens) = pipeline.process(&file)?;
    /// println!("文档: {}, 块数: {}, Token 数: {}", doc.path, blocks.len(), tokens.len());
    /// ```
    pub fn process(
        &mut self,
        file: &IngestedFile,
    ) -> Result<(Document, Vec<Block>, Vec<Token>)> {
        if file.source_type != SourceType::Code {
            return Err(helpers::unsupported_format(&format!(
                "CodePipeline 仅支持 Code 类型文件，实际类型: {:?}",
                file.source_type
            )));
        }

        let language = Self::detect_language_from_path(&file.path)?;

        let ast = self.parser.parse(&file.content, language)?;

        let line_count = count_lines(&file.content);
        let block_id: RecordIdType = surrealdb::sql::Thing::from((
            "block".to_string(),
            format!("code_{}", generate_short_hash(&file.path)),
        ));

        let block = Block {
            id: None,
            doc_id: surrealdb::sql::Thing::from(("document".to_string(), "pending".to_string())),
            block_type: BlockType::Code,
            start_line: 0,
            end_line: line_count.saturating_sub(1),
            embedding: None,
            idempotency_key: Some(crate::idempotency::IdempotencyKeyGenerator::generate_for_block(
                &file.hash,
                0,
                line_count.saturating_sub(1),
                &file.content,
            )),
        };

        let tokens =
            CodeTokenMapper::map_nodes_to_tokens(&ast.nodes, 0, &block_id.to_string());

        let title = extract_title_from_path(&file.path);

        let document = Document {
            id: None,
            path: file.path.to_string_lossy().to_string(),
            title,
            source_type: SourceType::Code,
            hash: file.hash.clone(),
        };

        Ok((document, vec![block], tokens))
    }

    /// 从文件路径推断编程语言标识符
    ///
    /// 通过查找 `EXTENSION_LANG_MAP` 表确定语言，
    /// 如果扩展名不在表中则返回错误。
    fn detect_language_from_path(path: &Path) -> Result<&'static str> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| {
                EXTENSION_LANG_MAP
                    .iter()
                    .find(|(e, _)| *e == ext)
                    .map(|(_, lang)| *lang)
            })
            .ok_or_else(|| {
                helpers::unsupported_format(&format!(
                "无法识别的代码文件扩展名: {:?}（支持的扩展名: {}）",
                path.extension(),
                supported_extensions()
            ))
            })
    }

    /// 检查给定路径是否可由此流水线处理
    ///
    /// 用于在 DAG 引擎中做路由判断。
    #[must_use]
    pub fn can_process(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| EXTENSION_LANG_MAP.iter().any(|(e, _)| *e == ext))
    }

    /// 批量处理多个代码文件
    ///
    /// 当启用 `parallel` feature 时，使用 rayon 线程池并行解析文件；
    /// 未启用时退化为顺序迭代。结果顺序与输入一致（确定性）。
    ///
    /// # 线程安全
    ///
    /// 每个文件在独立线程中创建自己的 `TreeSitterParser` 实例，
    /// 避免跨线程共享非 `Send` 的 `tree_sitter::Parser`。
    /// `GrammarCachePool` 使用 `OnceLock` 保证线程安全的懒加载。
    ///
    /// # 异步上下文使用
    ///
    /// 本方法为同步（CPU 密集型），在异步运行时中应通过
    /// `tokio::task::spawn_blocking` 调用以避免阻塞异步调度器。
    ///
    /// # Example
    ///
    /// ```ignore
    /// let files = vec![file1, file2, file3];
    /// let results = CodePipeline::process_batch(&files);
    /// for (i, result) in results.into_iter().enumerate() {
    ///     match result {
    ///         Ok((doc, blocks, tokens)) => { /* ... */ },
    ///         Err(e) => eprintln!("文件 {} 解析失败: {}", files[i].path.display(), e),
    ///     }
    /// }
    /// ```
    pub fn process_batch(files: &[IngestedFile]) -> Vec<Result<ParseOutput>> {
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            files
                .par_iter()
                .map(process_single_file)
                .collect()
        }

        #[cfg(not(feature = "parallel"))]
        {
            files
                .iter()
                .map(process_single_file)
                .collect()
        }
    }
}

/// 计算文本的行数（换行符数量 + 1）
#[allow(clippy::cast_possible_truncation)]
fn count_lines(text: &str) -> u32 {
    text.chars().filter(|&c| c == '\n').count() as u32 + 1
}

/// 处理单个代码文件的完整解析流程（无状态版本）
///
/// 创建独立的 `TreeSitterParser` 实例，适用于并行场景。
/// 逻辑与 `CodePipeline::process()` 完全一致，但不依赖 `&mut self`。
fn process_single_file(file: &IngestedFile) -> Result<ParseOutput> {
    if file.source_type != SourceType::Code {
        return Err(helpers::unsupported_format(&format!(
            "CodePipeline 仅支持 Code 类型文件，实际类型: {:?}",
            file.source_type
        )));
    }

    let language = CodePipeline::detect_language_from_path(&file.path)?;

    let mut parser = TreeSitterParser::new();
    let ast = parser.parse(&file.content, language)?;

    let line_count = count_lines(&file.content);
    let block_id: RecordIdType = surrealdb::sql::Thing::from((
        "block".to_string(),
        format!("code_{}", generate_short_hash(&file.path)),
    ));

    let block = Block {
        id: None,
        doc_id: surrealdb::sql::Thing::from(("document".to_string(), "pending".to_string())),
        block_type: BlockType::Code,
        start_line: 0,
        end_line: line_count.saturating_sub(1),
        embedding: None,
        idempotency_key: Some(crate::idempotency::IdempotencyKeyGenerator::generate_for_block(
            &file.hash,
            0,
            line_count.saturating_sub(1),
            &file.content,
        )),
    };

    let tokens =
        CodeTokenMapper::map_nodes_to_tokens(&ast.nodes, 0, &block_id.to_string());

    let title = extract_title_from_path(&file.path);

    let document = Document {
        id: None,
        path: file.path.to_string_lossy().to_string(),
        title,
        source_type: SourceType::Code,
        hash: file.hash.clone(),
    };

    Ok((document, vec![block], tokens))
}

/// 生成短哈希用于 block ID（取 BLAKE3 前 8 字符）
///
/// 路径分隔符统一为 `/` 以确保跨平台确定性：
/// 同一文件在 Windows (`\`) 和 Unix (`/`) 上产生相同的 block ID。
fn generate_short_hash(path: &std::path::Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let hash = blake3::hash(normalized.as_bytes());
    hash.to_hex()[..8].to_string()
}

/// 从文件路径提取标题（使用文件名不含扩展名）
fn extract_title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// 获取所有支持的扩展名字符串（用于错误消息）
fn supported_extensions() -> String {
    EXTENSION_LANG_MAP
        .iter()
        .map(|(ext, _)| format!(".{ext}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use error_core::prelude::ErrorSource;
    use super::*;
    use std::io::Write as IoWrite;
    use std::path::PathBuf;
    use knowledge_core::model::TokenType;

    fn create_temp_code_file(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("knowledge_parser_code_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("创建临时文件失败");
        write!(f, "{content}").expect("写入临时文件失败");
        path
    }

    fn cleanup_temp_dir() {
        let dir = std::env::temp_dir().join("knowledge_parser_code_test");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 测试 Rust 文件的完整处理链路
    #[tokio::test]
    async fn test_full_code_pipeline_rust() {
        let rust_code = r#"
fn main() {
    let message = "Hello from Rust!";
    println!("{}", message);
}
"#;
        let path = create_temp_code_file("test_pipeline.rs", rust_code);

        let mut pipeline = CodePipeline::new();
        let file = crate::file_ingester::IngestedFile {
            path,
            content: rust_code.to_string(),
            hash: "a".repeat(64),
            file_size: rust_code.len() as u64,
            source_type: SourceType::Code,
        };

        let result = pipeline.process(&file);
        if let Ok((doc, blocks, tokens)) = result {
            assert_eq!(
                doc.source_type,
                SourceType::Code,
                "Document 的 source_type 应为 Code"
            );
            assert!(
                doc.path.ends_with("test_pipeline.rs"),
                "Document 路径应包含文件名"
            );

            assert_eq!(blocks.len(), 1, "代码文件应产生 1 个 Block");
            assert_eq!(
                blocks[0].block_type,
                BlockType::Code,
                "Block 类型应为 Code"
            );

            assert!(
                !tokens.is_empty(),
                "应产生至少一个 Token"
            );

            let has_keyword = tokens
                .iter()
                .any(|t| t.token_type == TokenType::Keyword);
            assert!(
                has_keyword,
                "Rust 代码应包含 Keyword 类型的 Token（如 'fn', 'let'）"
            );

            let has_identifier = tokens
                .iter()
                .any(|t| t.token_type == TokenType::Identifier);
            assert!(
                has_identifier,
                "Rust 代码应包含 Identifier 类型的 Token（如 'main', 'message'）"
            );
        } else if let Err(ref e) = result {
            if e.code().contains("PARSE") && e.source() == ErrorSource::USR {
                eprintln!("Rust grammar 未安装（可选依赖），跳过测试: {}", e.message());
            } else {
                panic!("意外的错误: {e}");
            }
        }

        cleanup_temp_dir();
    }

    /// 测试代码文件生成单个 Block
    #[tokio::test]
    async fn test_code_file_single_block() {
        let python_code = "
def calculate_sum(a, b):
    return a + b

result = calculate_sum(10, 20)
print(result)
";
        let path = create_temp_code_file("single_block.py", python_code);

        let mut pipeline = CodePipeline::new();
        let file = crate::file_ingester::IngestedFile {
            path,
            content: python_code.to_string(),
            hash: "b".repeat(64),
            file_size: python_code.len() as u64,
            source_type: SourceType::Code,
        };

        let result = pipeline.process(&file);
        if let Ok((_doc, blocks, _tokens)) = result {
            assert_eq!(blocks.len(), 1, "代码文件应恰好生成 1 个 Block");

            let block = &blocks[0];
            assert_eq!(block.start_line, 0, "起始行应为 0");
            assert!(
                block.end_line > 0,
                "结束行应大于 0（文件有多行内容）"
            );
            assert_eq!(block.block_type, BlockType::Code);
        } else if let Err(ref e) = result {
            if e.code().contains("PARSE") && e.source() == ErrorSource::USR {
                eprintln!("Rust grammar 未安装（可选依赖），跳过测试: {}", e.message());
            } else {
                panic!("意外的错误: {e}");
            }
        }

        cleanup_temp_dir();
    }

    /// 测试不支持的语言返回正确错误
    #[tokio::test]
    async fn test_unsupported_extension_error() {
        let mut pipeline = CodePipeline::new();
        let file = crate::file_ingester::IngestedFile {
            path: PathBuf::from("/path/to/file.xyz"),
            content: "some content".to_string(),
            hash: "c".repeat(64),
            file_size: 12,
            source_type: SourceType::Code,
        };

        let result = pipeline.process(&file);
        assert!(result.is_err());

        match result.unwrap_err() {
            err if err.code().contains("PARSE") && err.source() == ErrorSource::USR => {
                assert!(
                    err.message().contains(".xyz") || err.message().contains("无法识别"),
                    "错误消息应提及不支持的扩展名: {}",
                    err.message()
                );
            }
            other => panic!("期望 UnsupportedFormat 错误，实际: {other}"),
        }
    }

    /// 测试非 Code 类型文件被拒绝
    #[tokio::test]
    async fn test_non_code_file_rejected() {
        let mut pipeline = CodePipeline::new();
        let file = crate::file_ingester::IngestedFile {
            path: PathBuf::from("/path/to/readme.md"),
            content: "# Hello".to_string(),
            hash: "d".repeat(64),
            file_size: 7,
            source_type: SourceType::Markdown,
        };

        let result = pipeline.process(&file);
        assert!(result.is_err());

        match result.unwrap_err() {
            err if err.code().contains("PARSE") && err.source() == ErrorSource::USR => {
                assert!(
                    err.message().contains("仅支持 Code"),
                    "错误消息应说明仅支持 Code 类型: {}",
                    err.message()
                );
            }
            other => panic!("期望 UnsupportedFormat 错误，实际: {other}"),
        }
    }

    /// 测试 `can_process` 方法正确判断可处理的文件
    #[test]
    fn test_can_process_detection() {
        assert!(
            CodePipeline::can_process(Path::new("main.rs")),
            ".rs 文件应被识别为可处理"
        );
        assert!(
            CodePipeline::can_process(Path::new("script.py")),
            ".py 文件应被识别为可处理"
        );
        assert!(
            CodePipeline::can_process(Path::new("app.ts")),
            ".ts 文件应被识别为可处理"
        );
        assert!(
            CodePipeline::can_process(Path::new("Cargo.toml")),
            ".toml 文件应被识别为可处理"
        );
        assert!(
            !CodePipeline::can_process(Path::new("readme.md")),
            ".md 文件不应由 CodePipeline 处理"
        );
        assert!(
            !CodePipeline::can_process(Path::new("data.txt")),
            ".txt 文件不应由 CodePipeline 处理"
        );
        assert!(
            !CodePipeline::can_process(Path::new("no_extension")),
            "无扩展名文件不应由 CodePipeline 处理"
        );
    }

    /// 测试多种语言的扩展名映射
    #[test]
    fn test_multi_language_extension_mapping() {
        let test_cases = vec![
            ("main.rs", Some("rust")),
            ("app.py", Some("python")),
            ("index.js", Some("javascript")),
            ("server.ts", Some("typescript")),
            ("main.go", Some("go")),
            ("lib.c", Some("c")),
            ("app.cpp", Some("cpp")),
            ("Main.java", Some("java")),
            ("script.rb", Some("ruby")),
            ("App.kt", Some("kotlin")),
            ("main.swift", Some("swift")),
            ("build.zig", Some("zig")),
            ("Cargo.toml", Some("toml")),
            ("config.yaml", Some("yaml")),
            ("data.json", Some("json")),
            ("readme.md", None),
            ("image.png", None),
        ];

        for (filename, expected_lang) in test_cases {
            let path = Path::new(filename);
            let result = CodePipeline::detect_language_from_path(path);

            match expected_lang {
                Some(lang) => {
                    assert_eq!(
                        result.ok(),
                        Some(lang),
                        "{filename} 应映射到 {expected_lang:?}"
                    );
                }
                None => {
                    assert!(
                        result.is_err(),
                        "{filename} 应返回错误（不支持的语言）"
                    );
                }
            }
        }
    }

    /// 测试空代码文件的处理
    #[tokio::test]
    async fn test_empty_code_file() {
        let mut pipeline = CodePipeline::new();
        let file = crate::file_ingester::IngestedFile {
            path: PathBuf::from("/empty.rs"),
            content: String::new(),
            hash: "e".repeat(64),
            file_size: 0,
            source_type: SourceType::Code,
        };

        let result = pipeline.process(&file);
        if let Ok((_, blocks, tokens)) = result {
            assert_eq!(blocks.len(), 1, "即使空文件也应创建 1 个 Block");
            assert_eq!(tokens.len(), 0, "空文件不应产生任何 Token");
        } else if let Err(ref e) = result {
            if e.code().contains("PARSE") && e.source() == ErrorSource::USR {
                eprintln!("Rust grammar 未安装（可选依赖），跳过测试: {}", e.message());
            } else {
                panic!("意外的错误: {e}");
            }
        }
    }

    /// 测试行号计算的准确性
    #[test]
    fn test_line_count_accuracy() {
        assert_eq!(count_lines(""), 1, "空文本应有 1 行");
        assert_eq!(count_lines("line1"), 1, "单行无换行应有 1 行");
        assert_eq!(count_lines("line1\n"), 2, "单行带换行应有 2 行");
        assert_eq!(count_lines("line1\nline2\nline3"), 3, "三行应有 3 行");
    }
}
