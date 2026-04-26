//! tree-sitter AST 解析器（按语言加载 grammar → AST 解析 → `named_nodes` 提取）
//!
//! # 核心设计约束
//!
//! - 直接读取 `named_nodes` 和 `anonymous_nodes`
//! - 每个 AST 节点直接映射为系统 Token
//! - **绝对保证代码符号不被自然语言分词器撕裂**（如 `Vec<i32>`、`->` 符号）
//!
//! # 架构说明
//!
//! 本模块是 Code 解析路径的核心组件，负责：
//! 1. 接收源代码文本和语言标识符
//! 2. 通过 GrammarCachePool 加载对应语言的 tree-sitter grammar
//! 3. 调用 tree-sitter 引擎生成语法树
//! 4. 递归遍历语法树提取所有节点信息

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use crate::Result;
use error_core::helpers;

use crate::grammar_cache::GrammarCachePool;

/// AST 节点表示（从 tree-sitter Node 提取的轻量级快照）
///
/// 保留节点在源码中的位置信息和语义类型，用于后续 Token 映射。
/// 设计为独立于 tree-sitter 生命周期的值类型，避免借用检查复杂性。
#[derive(Debug, Clone)]
pub struct AstNode {
    /// 节点的语法类型名称（如 `identifier`、`function_definition`）
    pub kind: String,
    /// 节点在源文本中的起始字节偏移
    pub start_byte: usize,
    /// 节点在源文本中的结束字节偏移（不包含）
    pub end_byte: usize,
    /// 节点在源文本中的起始字符偏移（UTF-8 字符数，从 0 开始）
    ///
    /// 与 `start_byte` 不同，此值按 Unicode 字符计数，
    /// 与 `MarkdownPipeline` 的 `global_offset` 语义一致，
    /// 确保跨流水线的原子寻址唯一性。
    pub start_char: u32,
    /// 节点在源文本中的起始行号（从 0 开始）
    pub start_row: usize,
    /// 节点在源文本中的起始列号（从 0 开始）
    pub start_column: usize,
    /// 是否为命名节点（named node）
    ///
    /// 命名节点具有语义意义（如标识符、关键字），
    /// 匿名节点通常是语法构造符号（如括号、运算符）
    pub is_named: bool,
    /// 节点对应的源文本内容
    pub text: String,
}

impl AstNode {
    /// 计算节点的字节长度
    #[inline]
    #[must_use]
    pub const fn byte_length(&self) -> usize {
        self.end_byte - self.start_byte
    }
}

/// tree-sitter 解析结果封装
///
/// 包含完整的语法树和提取出的节点列表，
/// 以及解析过程中产生的错误诊断信息。
#[derive(Debug)]
pub struct TreeSitterAst {
    /// 从根节点递归提取的所有 AST 节点（按深度优先序排列）
    pub nodes: Vec<AstNode>,
    /// 解析是否成功完成（无语法错误）
    pub is_valid: bool,
}

/// tree-sitter AST 解析器（按语言加载 grammar → AST 解析 → `named_nodes` 提取）
///
/// # 关键设计约束
///
/// - 直接读取 `named_nodes` 和 `anonymous_nodes`
/// - 每个 AST 节点直接映射为系统 Token
/// - **绝对保证代码符号不被自然语言分词器撕裂**（如 `Vec<i32>`、`->` 符号）
///
/// # 线程安全
///
/// `TreeSitterParser` 内部维护 `Parser` 缓存，每次调用 `parse` 时会复用已创建的 `Parser` 实例。
/// 由于 `tree_sitter::Parser` 不是 `Send`，本结构体也不实现 `Send`，
/// 应在单线程或使用 `Arc<Mutex<>>` 包装后跨线程使用。
pub struct TreeSitterParser {
    parsers: HashMap<String, tree_sitter::Parser>,
}

impl Default for TreeSitterParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeSitterParser {
    /// 创建新的解析器实例（空缓存）
    #[must_use]
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
        }
    }

    /// 解析代码文件为 tree-sitter AST
    ///
    /// 执行流程：
    /// 1. 通过 `GrammarCachePool` 获取目标语言的 `Language` 对象
    /// 2. 获取或创建该语言的 `Parser` 实例（带缓存）
    /// 3. 调用 tree-sitter 引擎生成语法树
    /// 4. 递归遍历语法树提取所有节点
    ///
    /// # 参数
    ///
    /// * `source` - 代码源文本（必须是有效的 UTF-8）
    /// * `language` - 编程语言标识符（如 "rust", "python", "javascript"）
    ///
    /// # 支持的语言（根据扩展名自动选择）
    ///
    /// | 扩展名 | 语言标识 |
    /// |--------|----------|
    /// | `.rs` | rust |
    /// | `.py` | python |
    /// | `.js` / `.ts` | javascript / typescript |
    /// | `.go` | go |
    /// | `.c` / `.cpp` | c / cpp |
    /// | `.java` | java |
    /// | `.rb` | ruby |
    /// | `.kt` | kotlin |
    /// | `.swift` | swift |
    /// | `.zig` | zig |
    /// | `.toml` | toml |
    /// | `.yaml` / `.yml` | yaml |
    /// | `.json` | json |
    ///
    /// # Errors
    ///
    /// * 不支持的格式（对应 `ErrorObject` code `ERR-USR-PARSE-002`） - 语言不被支持或 grammar 加载失败
    /// * 解析失败（对应 `ErrorObject` code `ERR-FS-PARSE-001`） - 源代码包含无法恢复的语法错误
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut parser = TreeSitterParser::new();
    /// let ast = parser.parse("fn main() {}", "rust")?;
    /// println!("提取了 {} 个 AST 节点", ast.nodes.len());
    /// ```
    pub fn parse(&mut self, source: &str, language: &str) -> Result<TreeSitterAst> {
        let language_obj = GrammarCachePool::get_language(language)?;

        let parser = match self.parsers.entry(language.to_string()) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                let mut p = tree_sitter::Parser::new();
                p.set_language(language_obj).map_err(|err| {
                    helpers::internal_error(&format!(
                        "设置 tree-sitter language 失败 (language={language}): {err}"
                    ))
                })?;
                e.insert(p)
            }
        };

        let source_bytes = source.as_bytes();
        let tree = parser.parse(source_bytes, None).ok_or_else(|| {
            helpers::parse_error(&format!("tree-sitter 解析 {language} 语言源码失败"))
        })?;

        let root = tree.root_node();
        let is_valid = !root.has_error();

        let nodes = self.extract_nodes(&root, source_bytes);

        Ok(TreeSitterAst { nodes, is_valid })
    }

    /// 遍历 AST 提取所有节点（递归深度优先遍历）
    ///
    /// 采用深度优先策略访问语法树的所有节点，
    /// 包括命名节点和匿名节点，确保不遗漏任何语法元素。
    ///
    /// # 参数
    ///
    /// * `root` - 当前遍历的根节点
    /// * `source` - 完整的源文本字节切片（用于提取节点文本内容）
    ///
    /// # Returns
    ///
    /// 按深度优先顺序排列的 `AstNode` 列表
    #[must_use]
    pub fn extract_nodes(&self, root: &tree_sitter::Node, source: &[u8]) -> Vec<AstNode> {
        let mut nodes = Vec::new();
        let source_str = std::str::from_utf8(source).unwrap_or("");
        Self::extract_nodes_recursive(root, source_str, &mut nodes);
        nodes
    }

    fn extract_nodes_recursive(node: &tree_sitter::Node, source: &str, nodes: &mut Vec<AstNode>) {
        let text = if let Some(s) = source.get(node.start_byte()..node.end_byte()) {
            s.to_string()
        } else {
            tracing::warn!(
                "AST 节点 UTF-8 解码失败: kind={}, byte_range={}-{}",
                node.kind(),
                node.start_byte(),
                node.end_byte()
            );
            String::new()
        };

        let start_char =
            u32::try_from(source[..node.start_byte()].chars().count()).unwrap_or(u32::MAX);

        nodes.push(AstNode {
            kind: node.kind().to_string(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_char,
            start_row: node.start_position().row,
            start_column: node.start_position().column,
            is_named: node.is_named(),
            text,
        });

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::extract_nodes_recursive(&child, source, nodes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use error_core::prelude::ErrorSource;

    /// 测试辅助：创建一个简单的 Rust 解析器并解析基本代码
    #[test]
    fn test_parse_rust_code_basic() {
        let mut parser = TreeSitterParser::new();
        let source = r#"
fn main() {
    let x = 42;
    println!("Hello, world!");
}
"#;

        match parser.parse(source, "rust") {
            Ok(ast) => {
                assert!(!ast.nodes.is_empty(), "Rust 代码应产生至少一个 AST 节点");
                assert!(ast.is_valid, "合法的 Rust 代码应标记为有效");

                let has_function = ast
                    .nodes
                    .iter()
                    .any(|n| n.kind == "function_item" || n.kind == "function_definition");
                assert!(has_function, "应包含函数定义节点");

                let has_identifier = ast.nodes.iter().any(|n| n.kind == "identifier");
                assert!(has_identifier, "应包含标识符节点");
            }
            Err(e) if e.code().contains("PARSE") && e.source() == ErrorSource::USR => {
                eprintln!("Rust grammar 未安装（可选依赖），跳过测试: {}", e.message());
            }
            Err(e) => panic!("意外的错误类型: {e}"),
        }
    }

    /// 测试 Python 函数定义解析
    #[test]
    fn test_parse_python_function() {
        let mut parser = TreeSitterParser::new();
        let source = r#"
def greet(name: str) -> str:
    return f"Hello, {name}!"
"#;

        match parser.parse(source, "python") {
            Ok(ast) => {
                assert!(!ast.nodes.is_empty(), "Python 代码应产生至少一个 AST 节点");

                let has_function_def = ast.nodes.iter().any(|n| n.kind == "function_definition");
                assert!(has_function_def, "应包含 function_definition 节点");

                let has_identifier = ast.nodes.iter().any(|n| n.kind == "identifier");
                assert!(has_identifier, "应包含 identifier 节点");
            }
            Err(e) if e.code().contains("PARSE") && e.source() == ErrorSource::USR => {
                eprintln!(
                    "Python grammar 未安装（可选依赖），跳过测试: {}",
                    e.message()
                );
            }
            Err(e) => panic!("意外的错误类型: {e}"),
        }
    }

    /// 测试不支持的语言返回正确的错误
    #[test]
    fn test_unsupported_language_error() {
        let mut parser = TreeSitterParser::new();
        let result = parser.parse("print('hello')", "cobol");

        assert!(result.is_err());
        match result.unwrap_err() {
            err if err.code().contains("PARSE") && err.source() == ErrorSource::USR => {
                assert!(
                    err.message().contains("cobol"),
                    "错误消息应包含语言名称: {}",
                    err.message()
                );
            }
            other => panic!("期望 UnsupportedFormat 错误，实际: {other}"),
        }
    }

    /// 测试 AST 节点的位置信息准确性
    #[test]
    fn test_ast_node_position_accuracy() {
        let mut parser = TreeSitterParser::new();
        let source = "fn test() {}";

        match parser.parse(source, "rust") {
            Ok(ast) => {
                let fn_node = ast
                    .nodes
                    .iter()
                    .find(|n| n.text == "fn")
                    .expect("应找到 'fn' 关键字节点");

                assert_eq!(fn_node.start_byte, 0, "'fn' 关键字应从字节位置 0 开始");
                assert_eq!(fn_node.byte_length(), 2, "'fn' 关键字的长度应为 2 字节");
            }
            Err(e) if e.code().contains("PARSE") && e.source() == ErrorSource::USR => {
                eprintln!("Rust grammar 未安装（可选依赖），跳过测试: {}", e.message());
            }
            Err(e) => panic!("意外的错误类型: {e}"),
        }
    }

    /// 测试解析器缓存机制
    #[test]
    fn test_parser_cache_reuse() {
        let mut parser = TreeSitterParser::new();
        let source1 = "let a = 1;";
        let source2 = "let b = 2;";

        match (parser.parse(source1, "rust"), parser.parse(source2, "rust")) {
            (Ok(_), Ok(_)) => {
                assert_eq!(
                    parser.parsers.len(),
                    1,
                    "同一语言的多次解析应复用同一个 Parser 实例"
                );
            }
            (Err(e1), Err(e2))
                if e1.code().contains("PARSE")
                    && e1.source() == ErrorSource::USR
                    && e2.code().contains("PARSE")
                    && e2.source() == ErrorSource::USR =>
            {
                eprintln!("Rust grammar 未安装（可选依赖），跳过测试");
            }
            (Err(e), _) | (_, Err(e)) => {
                panic!("意外的错误: {e}");
            }
        }
    }

    /// 测试无效代码仍能产生 AST（带有错误标记）
    #[test]
    fn test_invalid_code_produces_ast_with_errors() {
        let mut parser = TreeSitterParser::new();
        let invalid_rust = "fn bad { this is not valid rust syntax }}}";

        match parser.parse(invalid_rust, "rust") {
            Ok(ast) => {
                assert!(!ast.nodes.is_empty(), "即使语法无效也应产生部分 AST 节点");
                assert!(!ast.is_valid, "无效代码应将 is_valid 标记为 false");
            }
            Err(e) if e.code().contains("PARSE") && e.source() == ErrorSource::USR => {
                eprintln!("Rust grammar 未安装（可选依赖），跳过测试: {}", e.message());
            }
            Err(e) => panic!("意外的错误类型: {e}"),
        }
    }
}
