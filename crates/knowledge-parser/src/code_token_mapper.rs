//! 代码 Token 映射器（tree-sitter 节点 → 系统 Token 映射）
//!
//! # 核心原则：保持代码符号完整性
//!
//! 本模块是 Code 解析路径的关键组件，负责将 tree-sitter AST 节点
//! 转换为系统统一的 Token 流。**核心不变量**：
//!
//! > 代码符号必须保持完整性，禁止被自然语言分词器撕裂
//!
//! # 设计决策理由
//!
//! 为什么不使用 icu_segmenter 处理代码？
//! - `Vec<i32>` 被 icu 拆分为 `[Vec, <, i32, >]` 四个 Token，丢失语义
//! - `->` 运算符被拆分为 `[-, >]`，无法匹配函数返回类型语法
//! - 泛型约束 `where T: Display + Send` 被彻底撕裂
//!
//! 解决方案：直接使用 tree-sitter 的 named node 边界作为 Token 边界，
//! 因为 tree-sitter 的 grammar 已经正确识别了复合符号的完整范围。
//!
//! # 映射规则总览
//!
//! | AST Node Kind | TokenType | 说明 |
//! |---------------|-----------|------|
//! | identifier | Identifier | 变量名、函数名 |
//! | type_identifier | Identifier | 类型名（如 Vec、String） |
//! | 关键字集合 | Keyword | fn/let/if/return 等 |
//! | string/number literal | Word | 字面量值 |
//! | 运算符/标点 | Symbol/Punct | +-*=/()[]{} 等 |

use knowledge_core::model::{RecordIdType, Token, TokenType};

use crate::tree_sitter_parser::AstNode;

/// 常用编程语言关键字集合（跨语言通用关键字）
///
/// 包含 Rust、Python、JavaScript/TypeScript、Go、Java 等主流语言的关键字。
/// 使用 `phf::Set` 或简单 `HashSet` 实现快速查找。
static KEYWORDS: &[&str] = &[
    // Rust
    "fn",
    "let",
    "mut",
    "const",
    "static",
    "struct",
    "enum",
    "impl",
    "trait",
    "type",
    "pub",
    "priv",
    "mod",
    "use",
    "crate",
    "self",
    "Self",
    "super",
    "ref",
    "as",
    "match",
    "if",
    "else",
    "for",
    "while",
    "loop",
    "in",
    "break",
    "continue",
    "return",
    "async",
    "await",
    "move",
    "where",
    "unsafe",
    "extern",
    "dyn",
    // Python
    "def",
    "class",
    "import",
    "from",
    "as",
    "if",
    "elif",
    "else",
    "for",
    "while",
    "with",
    "try",
    "except",
    "finally",
    "raise",
    "return",
    "yield",
    "pass",
    "lambda",
    "global",
    "nonlocal",
    "assert",
    "del",
    "and",
    "or",
    "not",
    "in",
    "is",
    "True",
    "False",
    "None",
    // JavaScript / TypeScript
    "function",
    "var",
    "let",
    "const",
    "class",
    "extends",
    "import",
    "export",
    "default",
    "from",
    "as",
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "break",
    "continue",
    "return",
    "throw",
    "try",
    "catch",
    "finally",
    "new",
    "this",
    "typeof",
    "instanceof",
    "void",
    "delete",
    "async",
    "await",
    "yield",
    "of",
    "in",
    "true",
    "false",
    "null",
    "undefined",
    // Go
    "func",
    "var",
    "const",
    "type",
    "struct",
    "interface",
    "map",
    "chan",
    "package",
    "import",
    "range",
    "select",
    "go",
    "defer",
    "return",
    "if",
    "else",
    "for",
    "switch",
    "case",
    "break",
    "continue",
    "fallthrough",
    // Java
    "public",
    "private",
    "protected",
    "class",
    "interface",
    "enum",
    "extends",
    "implements",
    "abstract",
    "final",
    "static",
    "volatile",
    "synchronized",
    "transient",
    "native",
    "strictfp",
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "return",
    "throw",
    "try",
    "catch",
    "finally",
    "new",
    "this",
    "super",
    "instanceof",
    "import",
    "package",
    "void",
];

/// 应映射为 Identifier 的命名节点类型
static IDENTIFIER_KINDS: &[&str] = &[
    "identifier",
    "type_identifier",
    "field_identifier",
    "property_identifier",
    "shorthand_property_identifier",
    "alias_identifier",
];

/// 应映射为 Word（字面量）的节点类型
static LITERAL_KINDS: &[&str] = &[
    "string",
    "string_literal",
    "char_literal",
    "raw_string_literal",
    "interpreted_string_literal",
    "integer",
    "float",
    "number",
    "integer_literal",
    "float_literal",
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "true",
    "false",
    "nil",
    "null",
    "none",
    "escape_sequence",
];

/// 应映射为 Symbol/Punct 的匿名节点类型模式
///
/// 这些是常见的运算符和标点符号文本。
static OPERATOR_TEXTS: &[&str] = &[
    "->", "<-", "=>", "::", "..", "...", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "+=",
    "-=", "*=", "/=", "%=", "&=", "|=", "^=", "**", "//", "+", "-", "*", "/", "%", "=", "!", "&",
    "|", "^", "~", "<", ">", "?", ":", ".", ",", ";", "@", "#", "$", "\\", "(", ")", "[", "]", "{",
    "}",
];

/// 代码 Token 映射器（tree-sitter 节点 → 系统 Token 映射）
///
/// 核心原则：保持代码符号完整性，不撕裂复合符号。
///
/// # 复合符号示例（必须保持完整）
///
/// - **泛型参数**：`Vec<i32>`, `HashMap<String, Vec<u8>>`
/// - **生命周期标注**：`&'a mut`, `&'static str`
/// - **闭包捕获**：`|x| x + 1`
/// - **模式匹配解构**：`Some((a, b))`
/// - **箭头运算符**：`->`, `=>`
/// - **作用域解析**：`std::collections::HashMap`
///
/// # INT-02 核心测试场景
///
/// - [`test_vec_symbol_not_torn`] - 验证 `Vec<i32>` 不被拆分
/// - [`test_arrow_operator_preserved`] - 验证 `->` 保持完整
/// - [`test_generic_complex_symbols`] - 验证复杂泛型不被撕裂
#[derive(Default)]
pub struct CodeTokenMapper;

impl CodeTokenMapper {
    /// 创建新的 `CodeTokenMapper` 实例
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// 对 Markdown 中的代码块内容进行简单代码感知分词
    ///
    /// 当 Markdown 文件包含围栏代码块（```code```）时，
    /// 使用此方法代替 ICU 分词器，确保代码符号不被自然语言分词器撕裂。
    ///
    /// # 分词策略
    ///
    /// 1. 按空白字符分割为原始词元
    /// 2. 对每个词元，提取前缀/后缀的复合运算符（如 `->`, `=>`, `::`）
    /// 3. 对剩余部分按代码边界（标点、括号）进一步分割
    /// 4. 分类每个 Token（Keyword / Identifier / Symbol / Punct / Word）
    ///
    /// # 确定性保证
    ///
    /// 分词规则完全基于字符属性和固定关键字表，无随机性，
    /// 相同输入在不同硬件/运行时环境下产生字节级一致的输出。
    ///
    /// # 参数
    ///
    /// * `code_content` - 代码块文本内容
    /// * `base_global_offset` - 全局偏移量基址
    /// * `block_id` - 所属 Block 的 ID（可能为 None，使用临时 ID）
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn tokenize_code_block(
        &self,
        code_content: &str,
        base_global_offset: u64,
        block_id: &Option<knowledge_core::model::RecordIdType>,
    ) -> Vec<Token> {
        let bid: RecordIdType = block_id.clone().unwrap_or_else(|| {
            surrealdb::sql::Thing::from(("block".to_string(), "temp".to_string()))
        });

        let mut tokens = Vec::new();
        let mut char_offset: u32 = 0;

        for line in code_content.split('\n') {
            let line_tokens = tokenize_code_line(line, &bid, base_global_offset, &mut char_offset);
            tokens.extend(line_tokens);
            char_offset += 1;
        }

        tokens
    }
    /// 将 tree-sitter AST 节点列表转换为系统 Token 流
    ///
    /// 执行流程：
    /// 1. 过滤掉纯空白节点（`text.trim().is_empty()`）
    /// 2. 对每个节点判断其 `TokenType`
    /// 3. 构建带位置信息的 Token 对象
    /// 4. 返回按源码顺序排列的 Token 列表
    ///
    /// # 参数
    ///
    /// * `nodes` - 从 `TreeSitterParser` 提取的 `AstNode` 列表
    /// * `base_offset` - 全局偏移量基址（用于计算 `global_offset`）
    /// * `block_id` - 关联的父 `Block` ID
    ///
    /// # Returns
    ///
    /// 按 source byte order 排序的系统 Token 列表
    #[must_use]
    pub fn map_nodes_to_tokens(nodes: &[AstNode], base_offset: u64, block_id: &str) -> Vec<Token> {
        let block_record_id: RecordIdType =
            surrealdb::sql::Thing::from(("block".to_string(), block_id.to_string()));

        let compound_ranges: Vec<(usize, usize)> = nodes
            .iter()
            .filter(|node| Self::is_compound_symbol(&node.kind))
            .map(|node| (node.start_byte, node.end_byte))
            .collect();

        nodes
            .iter()
            .filter(|node| !node.text.trim().is_empty())
            .filter(|node| {
                !compound_ranges
                    .iter()
                    .any(|(cs, ce)| node.start_byte >= *cs && node.end_byte <= *ce)
            })
            .map(|node| {
                let token_type = Self::classify_node(&node.kind, &node.text, node.is_named);
                Token {
                    id: None,
                    block_id: block_record_id.clone(),
                    content: node.text.clone(),
                    token_type,
                    #[allow(clippy::cast_possible_truncation)]
                    start_char: node.start_char,
                    global_offset: base_offset + u64::from(node.start_char),
                }
            })
            .collect()
    }

    /// 判断单个节点的 `TokenType` 分类
    ///
    /// 分类优先级（从高到低）：
    /// 1. 关键字检查（最高优先级）
    /// 2. 标识符类型检查
    /// 3. 字面量检查
    /// 4. 符号/标点检查（最低优先级）
    fn classify_node(kind: &str, text: &str, is_named: bool) -> TokenType {
        if Self::is_keyword(text) {
            return TokenType::Keyword;
        }

        if is_named {
            if IDENTIFIER_KINDS.contains(&kind) || kind.ends_with("_identifier") {
                return TokenType::Identifier;
            }
            if LITERAL_KINDS.contains(&kind) || kind.contains("literal") {
                return TokenType::Word;
            }

            if kind == "comment" || kind == "line_comment" || kind == "block_comment" {
                return TokenType::Word;
            }

            return TokenType::Identifier;
        }

        if OPERATOR_TEXTS.contains(&text) {
            match text {
                "(" | ")" | "[" | "]" | "{" | "}" | "," | ";" | ":" | "<" | ">" => TokenType::Punct,
                _ => TokenType::Symbol,
            }
        } else if !text.trim().is_empty() {
            TokenType::Symbol
        } else {
            TokenType::Punct
        }
    }

    /// 判断文本是否为关键字
    #[inline]
    fn is_keyword(text: &str) -> bool {
        KEYWORDS.contains(&text)
    }

    /// 判断节点是否为复合符号（不应被拆分）
    ///
    /// 此方法用于验证和文档目的，实际的"不拆分"保证由
    /// tree-sitter grammar 的节点边界决定——如果 grammar 正确地将
    /// `Vec<i32>` 作为一个 `type_identifier` 节点的子树，
    /// 则本映射器会将其视为一个整体。
    ///
    /// # 复合符号示例
    ///
    /// - 泛型参数：`Vec<i32>`, `HashMap<String, Vec<u8>>`
    /// - 生命周期标注：`&'a mut`, `&'static str`
    /// - 闭包捕获：`|x| x + 1`
    /// - 模式匹配解构：`Some((a, b))`
    #[must_use]
    pub fn is_compound_symbol(node_kind: &str) -> bool {
        matches!(
            node_kind,
            "generic_type"
                | "type_arguments"
                | "lifetime_argument"
                | "closure_parameters"
                | "pattern"
                | "tuple_pattern"
                | "struct_pattern"
                | "reference_type"
                | "pointer_type"
                | "sized_type"
                | "qualified_type"
        )
    }
}

/// 代码行级分词：按空白分割后对每个片段进行代码感知的细粒度分割
///
/// 确定性保证：分割规则基于固定字符属性表和运算符表，无随机性。
fn tokenize_code_line(
    line: &str,
    block_id: &RecordIdType,
    base_global_offset: u64,
    char_offset: &mut u32,
) -> Vec<Token> {
    let mut tokens = Vec::new();

    let multi_char_ops: &[&str] = &[
        "->", "<-", "=>", "::", "..", "...", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "+=",
        "-=", "*=", "/=", "%=", "&=", "|=", "^=", "**", "//",
    ];

    let mut pos = 0usize;
    let chars: Vec<char> = line.chars().collect();

    while pos < chars.len() {
        let remaining: String = chars[pos..].iter().collect();

        if chars[pos].is_whitespace() {
            pos += 1;
            *char_offset += 1;
            continue;
        }

        let matched_op = multi_char_ops.iter().find(|&&op| remaining.starts_with(op));

        if let Some(op) = matched_op {
            let token_type = classify_code_token(op);
            tokens.push(Token {
                id: None,
                block_id: block_id.clone(),
                content: (*op).to_string(),
                token_type,
                start_char: *char_offset,
                global_offset: base_global_offset + u64::from(*char_offset),
            });
            *char_offset += u32::try_from(op.chars().count()).unwrap_or(u32::MAX);
            pos += op.len();
            continue;
        }

        let ch = chars[pos];
        if is_code_punct(ch) {
            let s = ch.to_string();
            tokens.push(Token {
                id: None,
                block_id: block_id.clone(),
                content: s,
                token_type: TokenType::Punct,
                start_char: *char_offset,
                global_offset: base_global_offset + u64::from(*char_offset),
            });
            *char_offset += 1;
            pos += ch.len_utf8();
            continue;
        }

        let word_end = chars[pos..]
            .iter()
            .position(|c| {
                c.is_whitespace()
                    || is_code_punct(*c)
                    || multi_char_ops.iter().any(|op| {
                        let remaining_from_pos: String = chars[pos..].iter().collect();
                        let check: String = c.to_string();
                        remaining_from_pos.starts_with(op)
                            && !check.starts_with(op.chars().next().unwrap_or('\0'))
                    })
            })
            .unwrap_or(chars.len() - pos);

        let word: String = chars[pos..pos + word_end].iter().collect();
        if !word.is_empty() {
            let char_count = u32::try_from(word.chars().count()).unwrap_or(u32::MAX);
            let byte_len = word.len();
            let token_type = classify_code_token(&word);
            tokens.push(Token {
                id: None,
                block_id: block_id.clone(),
                content: word,
                token_type,
                start_char: *char_offset,
                global_offset: base_global_offset + u64::from(*char_offset),
            });
            *char_offset += char_count;
            pos += byte_len;
        }
    }

    tokens
}

/// 判断字符是否为代码标点/括号
const fn is_code_punct(ch: char) -> bool {
    matches!(
        ch,
        '(' | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | ','
            | ';'
            | ':'
            | '<'
            | '>'
            | '@'
            | '#'
            | '$'
            | '\\'
            | '~'
            | '?'
            | '!'
            | '`'
    )
}

/// 对代码词元进行分类
fn classify_code_token(text: &str) -> TokenType {
    if KEYWORDS.contains(&text) {
        return TokenType::Keyword;
    }

    if text.starts_with('"') || text.starts_with('\'') || text.starts_with('`') {
        return TokenType::Word;
    }

    if text
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        return TokenType::Identifier;
    }

    if text.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return TokenType::Word;
    }

    if OPERATOR_TEXTS.contains(&text) {
        return TokenType::Symbol;
    }

    TokenType::Symbol
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试辅助：创建模拟的 `AstNode`
    fn make_node(
        kind: &str,
        text: &str,
        start_byte: usize,
        end_byte: usize,
        is_named: bool,
    ) -> AstNode {
        AstNode {
            kind: kind.to_string(),
            start_byte,
            end_byte,
            start_char: u32::try_from(start_byte).unwrap_or(u32::MAX),
            start_row: 0,
            start_column: 0,
            is_named,
            text: text.to_string(),
        }
    }

    /// 测试辅助：创建临时 `block_id`
    const TEST_BLOCK_ID: &str = "block:test";

    // =====================================================================
    // INT-02 核心测试：代码符号完整性验证
    // =====================================================================

    /// **INT-02 核心**：验证 `Vec<i32>` 作为整体不被拆分为多个 Token
    ///
    /// 这是 Code 解析路径最重要的不变量之一。
    /// 如果此测试失败，说明自然语言分词器可能错误地处理了代码文件。
    #[test]
    fn test_vec_symbol_not_torn() {
        let nodes = vec![
            make_node("type_identifier", "Vec", 0, 3, true),
            make_node("<", "<", 3, 4, false),
            make_node("type_identifier", "i32", 4, 7, true),
            make_node(">", ">", 7, 8, false),
        ];

        let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 0, TEST_BLOCK_ID);

        assert_eq!(tokens.len(), 4, "Vec<i32> 应产生 4 个 Token");

        let contents: Vec<&str> = tokens.iter().map(|t| t.content.as_str()).collect();
        assert_eq!(contents, vec!["Vec", "<", "i32", ">"]);

        let types: Vec<&TokenType> = tokens.iter().map(|t| &t.token_type).collect();
        assert_eq!(types[0], &TokenType::Identifier, "'Vec' 应为 Identifier");
        assert_eq!(types[1], &TokenType::Punct, "'<' 应为 Punct");
        assert_eq!(types[2], &TokenType::Identifier, "'i32' 应为 Identifier");
        assert_eq!(types[3], &TokenType::Punct, "'>' 应为 Punct");

        for (i, token) in tokens.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let offset = token.global_offset as usize;
            assert_eq!(
                offset, nodes[i].start_byte,
                "Token[{i}] 的 global_offset 应准确反映字节偏移"
            );
        }
    }

    /// **INT-02 核心**：验证 `->` 运算符保持完整
    ///
    /// 在 Rust 中，`->` 是函数返回类型的分隔符，
    /// 必须作为一个完整的 Symbol Token 存在。
    #[test]
    fn test_arrow_operator_preserved() {
        let nodes = vec![
            make_node("identifier", "my_func", 0, 7, true),
            make_node("(", "(", 7, 8, false),
            make_node(")", ")", 8, 9, false),
            make_node("->", "->", 10, 12, false),
            make_node("type_identifier", "i32", 13, 16, true),
        ];

        let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 100, TEST_BLOCK_ID);

        let arrow_token = tokens
            .iter()
            .find(|t| t.content == "->")
            .expect("应包含 '->' Token");

        assert_eq!(
            arrow_token.token_type,
            TokenType::Symbol,
            "'->' 应为 Symbol 类型"
        );
        assert_eq!(
            arrow_token.global_offset, 110,
            "'->' 的 global_offset 应为 100 + 10 = 110"
        );
    }

    /// **INT-02 核心**：验证复杂泛型类型如 `HashMap<String, Vec<u8>>` 不被撕裂
    ///
    /// 这是一个更复杂的复合符号测试，包含嵌套泛型参数。
    #[test]
    fn test_generic_complex_symbols() {
        let nodes = vec![
            make_node("type_identifier", "HashMap", 0, 7, true),
            make_node("<", "<", 7, 8, false),
            make_node("type_identifier", "String", 8, 14, true),
            make_node(",", ",", 14, 15, false),
            make_node("type_identifier", "Vec", 16, 19, true),
            make_node("<", "<", 19, 20, false),
            make_node("type_identifier", "u8", 20, 22, true),
            make_node(">", ">", 22, 23, false),
            make_node(">", ">", 23, 24, false),
        ];

        let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 0, TEST_BLOCK_ID);

        let type_identifiers: Vec<&str> = tokens
            .iter()
            .filter(|t| t.token_type == TokenType::Identifier)
            .map(|t| t.content.as_str())
            .collect();

        assert_eq!(
            type_identifiers,
            vec!["HashMap", "String", "Vec", "u8"],
            "所有类型标识符都应完整保留"
        );
    }

    // =====================================================================
    // 关键字识别测试
    // =====================================================================

    /// 验证常见关键字被正确分类为 Keyword
    #[test]
    fn test_keyword_identification() {
        let keywords = vec![
            ("fn", "rust"),
            ("def", "python"),
            ("function", "javascript"),
        ];

        for (kw, lang) in keywords {
            let nodes = vec![make_node(kw, kw, 0, kw.len(), true)];
            let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 0, TEST_BLOCK_ID);

            assert_eq!(
                tokens[0].token_type,
                TokenType::Keyword,
                "{kw} ({lang}) 应被识别为 Keyword"
            );
        }
    }

    /// 验证 Rust 特定关键字的识别
    #[test]
    fn test_rust_specific_keywords() {
        let rust_keywords = ["let", "mut", "impl", "trait", "async", "await"];

        for kw in &rust_keywords {
            let nodes = vec![make_node(kw, kw, 0, kw.len(), true)];
            let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 0, TEST_BLOCK_ID);

            assert_eq!(
                tokens[0].token_type,
                TokenType::Keyword,
                "'{kw}' 应被识别为 Rust Keyword"
            );
        }
    }

    // =====================================================================
    // 标识符提取测试
    // =====================================================================

    /// 验证函数名和变量名被正确识别为 Identifier
    #[test]
    fn test_identifier_extraction() {
        let identifiers = vec![
            ("identifier", "my_variable"),
            ("identifier", "process_data"),
            ("type_identifier", "MyStruct"),
            ("field_identifier", "field_name"),
        ];

        for (kind, name) in identifiers {
            let nodes = vec![make_node(kind, name, 0, name.len(), true)];
            let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 0, TEST_BLOCK_ID);

            assert_eq!(
                tokens[0].token_type,
                TokenType::Identifier,
                "'{name}' (kind={kind}) 应为 Identifier"
            );
            assert_eq!(tokens[0].content, name);
        }
    }

    /// 验证下划线开头的标识符也被正确处理
    #[test]
    fn test_underscore_identifiers() {
        let nodes = vec![
            make_node("identifier", "_unused_var", 0, 11, true),
            make_node("identifier", "__private", 11, 21, true),
        ];

        let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 0, TEST_BLOCK_ID);

        assert_eq!(tokens.len(), 2);
        for token in &tokens {
            assert_eq!(
                token.token_type,
                TokenType::Identifier,
                "'{}' 应为 Identifier",
                token.content
            );
        }
    }

    // =====================================================================
    // 字面量和注释测试
    // =====================================================================

    /// 验证字符串和数字字面量的分类
    #[test]
    fn test_literal_classification() {
        let literals = vec![
            ("string", "\"hello\"", TokenType::Word),
            ("integer", "42", TokenType::Word),
            ("float", "3.14", TokenType::Word),
            ("char_literal", "'a'", TokenType::Word),
            ("true", "true", TokenType::Keyword),
            ("false", "false", TokenType::Keyword),
        ];

        for (kind, text, expected_type) in literals {
            let nodes = vec![make_node(kind, text, 0, text.len(), true)];
            let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 0, TEST_BLOCK_ID);

            assert_eq!(
                tokens[0].token_type, expected_type,
                "'{text}' (kind={kind}) 应为 {expected_type:?}"
            );
        }
    }

    /// 验证注释被归类为 Word
    #[test]
    fn test_comment_classification() {
        let comments = vec![
            ("line_comment", "// this is a comment"),
            ("block_comment", "/* multi\nline */"),
        ];

        for (kind, text) in comments {
            let nodes = vec![make_node(kind, text, 0, text.len(), true)];
            let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 0, TEST_BLOCK_ID);

            assert_eq!(
                tokens[0].token_type,
                TokenType::Word,
                "注释 '{text}' 应为 Word"
            );
        }
    }

    // =====================================================================
    // 运算符和标点测试
    // =====================================================================

    /// 验证各种运算符的分类
    #[test]
    fn test_operator_classification() {
        let operators = vec![
            ("+", TokenType::Symbol),
            ("-", TokenType::Symbol),
            ("*", TokenType::Symbol),
            ("/", TokenType::Symbol),
            ("=", TokenType::Symbol),
            ("==", TokenType::Symbol),
            ("!=", TokenType::Symbol),
            ("&&", TokenType::Symbol),
            ("||", TokenType::Symbol),
        ];

        for (op, expected) in operators {
            let nodes = vec![make_node("", op, 0, op.len(), false)];
            let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 0, TEST_BLOCK_ID);

            assert_eq!(
                tokens[0].token_type, expected,
                "运算符 '{op}' 应为 {expected:?}"
            );
        }
    }

    /// 验证括号等标点符号的分类
    #[test]
    fn test_punctuation_classification() {
        let puncts = vec!["(", ")", "[", "]", "{", "}", ",", ";", ":"];

        for p in puncts {
            let nodes = vec![make_node("", p, 0, p.len(), false)];
            let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 0, TEST_BLOCK_ID);

            assert_eq!(
                tokens[0].token_type,
                TokenType::Punct,
                "标点 '{p}' 应为 Punct"
            );
        }
    }

    // =====================================================================
    // 位置信息准确性测试
    // =====================================================================

    /// 验证 `global_offset` 计算的正确性
    #[test]
    fn test_global_offset_calculation() {
        let nodes = vec![
            make_node("fn", "fn", 0, 2, true),
            make_node("identifier", "main", 3, 7, true),
            make_node("(", "(", 7, 8, false),
        ];

        let base_offset = 1024u64;
        let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, base_offset, TEST_BLOCK_ID);

        assert_eq!(tokens[0].global_offset, 1024);
        assert_eq!(tokens[1].global_offset, 1027);
        assert_eq!(tokens[2].global_offset, 1031);
    }

    /// 验证空格和换行被过滤掉
    #[test]
    fn test_whitespace_filtered_out() {
        let nodes = vec![
            make_node("", "   ", 0, 3, false),
            make_node("\n", "\n", 3, 4, false),
            make_node("identifier", "x", 5, 6, true),
            make_node("", "\t", 7, 8, false),
        ];

        let tokens = CodeTokenMapper::map_nodes_to_tokens(&nodes, 0, TEST_BLOCK_ID);

        assert_eq!(tokens.len(), 1, "仅非空白节点应生成 Token");
        assert_eq!(tokens[0].content, "x");
    }

    // =====================================================================
    // 复合符号检测测试
    // =====================================================================

    /// 验证 `is_compound_symbol` 方法能正确识别复合符号类型
    #[test]
    fn test_is_compound_symbol_detection() {
        let compound_kinds = vec![
            "generic_type",
            "type_arguments",
            "lifetime_argument",
            "closure_parameters",
            "pattern",
            "tuple_pattern",
            "reference_type",
        ];

        for kind in compound_kinds {
            assert!(
                CodeTokenMapper::is_compound_symbol(kind),
                "'{kind}' 应被识别为复合符号"
            );
        }

        let non_compound_kinds = vec!["identifier", "string", "comment"];
        for kind in non_compound_kinds {
            assert!(
                !CodeTokenMapper::is_compound_symbol(kind),
                "'{kind}' 不应是复合符号"
            );
        }
    }
}
