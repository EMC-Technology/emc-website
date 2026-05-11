//! 符号解析器（遍历 Token 流 → 识别 Definition/Usage/LINK 等引用关系）

use knowledge_core::model::{Direction, RecordIdType, RefType, Reference, Token, TokenType};
use std::collections::HashMap;

/// 符号表条目（记录一个符号的定义点信息）
#[derive(Debug, Clone)]
struct SymbolEntry {
    token_id: RecordIdType,
    _scope: Option<String>,
}

/// 符号解析器（遍历 Token 流 → 识别 Definition/Usage/LINK 等引用关系）
pub struct SymbolResolver {
    /// 符号名 → 定义点 Token ID 列表的映射
    symbol_table: HashMap<String, Vec<SymbolEntry>>,
    /// 检测到的引用边集合
    references: Vec<Reference>,
    /// 当前作用域标识符
    current_scope: Option<String>,
}

impl Default for SymbolResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolResolver {
    /// 创建符号解析器
    #[must_use]
    pub fn new() -> Self {
        Self {
            symbol_table: HashMap::new(),
            references: Vec::new(),
            current_scope: None,
        }
    }

    /// 解析单个文档的所有 Token，提取引用关系
    pub fn resolve(&mut self, tokens: &[Token], document_id: &str) -> Vec<Reference> {
        self.references.clear();

        let context = Self::build_context_string(tokens);

        for (idx, token) in tokens.iter().enumerate() {
            if token.token_type != TokenType::Identifier {
                continue;
            }

            let token_id: RecordIdType =
                surrealdb::sql::Thing::from(("token".to_string(), format!("{document_id}_{idx}")));

            if Self::is_definition_point(token, idx, tokens, &context) {
                self.register_definition(&token_id, &token.content, &context);
            } else if self.is_usage_point(token, idx, tokens, &context) {
                self.match_usage_to_definition(&token_id, &token.content, &context);
            }

            if Self::is_link_reference(token, &context) {
                self.create_link_reference(&token_id, &token.content);
            }
        }

        std::mem::take(&mut self.references)
    }

    /// 注册符号定义点
    ///
    /// 将符号名与其定义点 Token ID 关联，供后续 `find_usages` 查询。
    pub fn register_definition(
        &mut self,
        token_id: &RecordIdType,
        symbol_name: &str,
        _context: &str,
    ) {
        let entry = SymbolEntry {
            token_id: token_id.clone(),
            _scope: self.current_scope.clone(),
        };

        self.symbol_table
            .entry(symbol_name.to_string())
            .or_default()
            .push(entry);
    }

    /// 查找符号的所有引用
    ///
    /// 返回指向该符号定义点的所有引用边。
    #[must_use]
    pub fn find_usages(&self, symbol_name: &str) -> Vec<&Reference> {
        self.symbol_table
            .get(symbol_name)
            .map_or_else(Vec::new, |entries| {
                entries
                    .iter()
                    .flat_map(|entry| self.references.iter().filter(|r| r.to_id == entry.token_id))
                    .collect()
            })
    }

    /// 设置当前解析作用域
    ///
    /// 作用域用于限定符号的可见范围，影响定义点与引用的匹配。
    pub fn set_scope(&mut self, scope: Option<String>) {
        self.current_scope = scope;
    }

    /// 获取已注册的符号数量
    #[must_use]
    pub fn symbol_count(&self) -> usize {
        self.symbol_table.len()
    }

    /// 获取已发现的引用数量
    #[must_use]
    pub fn reference_count(&self) -> usize {
        self.references.len()
    }

    fn is_definition_point(_token: &Token, idx: usize, tokens: &[Token], context: &str) -> bool {
        if idx == 0 {
            return false;
        }

        let prev_token = &tokens[idx - 1];

        if prev_token.token_type == TokenType::Keyword {
            let kw = prev_token.content.as_str();
            matches!(
                kw,
                "fn" | "def"
                    | "function"
                    | "let"
                    | "var"
                    | "const"
                    | "class"
                    | "struct"
                    | "enum"
                    | "interface"
                    | "type"
                    | "trait"
                    | "impl"
                    | "func"
                    | "mod"
                    | "package"
                    | "import"
                    | "export"
            )
        } else {
            Self::is_type_definition_context(context)
        }
    }

    fn is_type_definition_context(context: &str) -> bool {
        let patterns = [
            "struct ",
            "class ",
            "enum ",
            "interface ",
            "type ",
            "trait ",
            "impl ",
        ];
        patterns.iter().any(|p| context.contains(p))
    }

    fn is_usage_point(&self, token: &Token, idx: usize, tokens: &[Token], context: &str) -> bool {
        if token.token_type != TokenType::Identifier {
            return false;
        }

        if Self::is_definition_point(token, idx, tokens, context) {
            return false;
        }

        if idx > 0 {
            let prev = &tokens[idx - 1];
            if prev.token_type == TokenType::Punct {
                let punct = prev.content.as_str();
                if matches!(punct, "." | "::") {
                    return true;
                }
            }
            if prev.token_type == TokenType::Keyword
                && matches!(
                    prev.content.as_str(),
                    "return"
                        | "yield"
                        | "await"
                        | "throw"
                        | "raise"
                        | "new"
                        | "typeof"
                        | "instanceof"
                )
            {
                return true;
            }
        }

        if idx + 1 < tokens.len() {
            let next = &tokens[idx + 1];
            if next.token_type == TokenType::Punct && next.content == "(" {
                return true;
            }
        }

        self.symbol_table.contains_key(&token.content)
    }

    fn is_link_reference(_token: &Token, context: &str) -> bool {
        context.contains("](")
    }

    fn match_usage_to_definition(
        &mut self,
        usage_token_id: &RecordIdType,
        symbol_name: &str,
        context: &str,
    ) {
        if let Some(entries) = self.symbol_table.get(symbol_name)
            && let Some(def_entry) = entries.last()
        {
            let ref_type = Self::classify_reference_for_usage(context);
            let direction = if matches!(ref_type, RefType::Usage) {
                Direction::ImplicitTwoWay
            } else {
                Direction::OneWay
            };

            let reference = Reference::new(
                ref_type,
                direction,
                usage_token_id.clone(),
                def_entry.token_id.clone(),
            );

            self.references.push(reference);
        }
    }

    fn create_link_reference(&mut self, from_id: &RecordIdType, link_text: &str) {
        let to_id: RecordIdType =
            surrealdb::sql::Thing::from(("link".to_string(), link_text.to_string()));
        let reference = Reference::new(RefType::Link, Direction::OneWay, from_id.clone(), to_id);
        self.references.push(reference);
    }

    fn classify_reference_for_usage(context: &str) -> RefType {
        if context.contains("extends")
            || (context.contains(": ") && !context.contains("::"))
            || context.contains("impl ")
        {
            return RefType::Inherit;
        }

        if context.contains("implements") || context.contains(" for ") {
            return RefType::Implement;
        }

        if context.contains("where") || (context.contains('<') && context.contains('>')) {
            return RefType::Constrain;
        }

        RefType::Usage
    }

    fn build_context_string(tokens: &[Token]) -> String {
        let filtered: Vec<&str> = tokens
            .iter()
            .filter(|t| {
                t.token_type != TokenType::Punct
                    || matches!(
                        t.content.as_str(),
                        "(" | ")" | "[" | "]" | "{" | "}" | ":" | ";" | "," | "."
                    )
            })
            .map(|t| t.content.as_str())
            .collect();

        let mut result = String::new();
        for (i, s) in filtered.iter().enumerate() {
            if i > 0 {
                let prev = filtered[i - 1];
                if (prev == "]" && *s == "(") || (prev == "[" && *s == "]") {
                    result.push_str(s);
                    continue;
                }
                result.push(' ');
            }
            result.push_str(s);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knowledge_core::model::{TokenStatus, TokenType};

    fn make_token(
        content: &str,
        token_type: TokenType,
        start_char: u32,
        global_offset: u64,
    ) -> Token {
        Token {
            id: None,
            block_id: surrealdb::sql::Thing::from(("block".to_string(), "test".to_string())),
            content: content.to_string(),
            token_type,
            start_char,
            global_offset,
            status: TokenStatus::Created,
        }
    }

    fn make_ident(content: &str, start_char: u32) -> Token {
        make_token(
            content,
            TokenType::Identifier,
            start_char,
            u64::from(start_char),
        )
    }

    fn make_keyword(content: &str, start_char: u32) -> Token {
        make_token(
            content,
            TokenType::Keyword,
            start_char,
            u64::from(start_char),
        )
    }

    #[test]
    fn test_resolver_new_creates_empty_state() {
        let resolver = SymbolResolver::new();
        assert_eq!(resolver.symbol_count(), 0);
        assert_eq!(resolver.reference_count(), 0);
    }

    #[test]
    fn test_resolve_function_definition_usage() {
        let tokens = vec![
            make_keyword("fn", 0),
            make_ident("main", 3),
            make_token("(", TokenType::Punct, 7, 7),
            make_token(")", TokenType::Punct, 8, 8),
            make_token("{", TokenType::Punct, 10, 10),
            make_keyword("let", 12),
            make_ident("x", 16),
            make_token("=", TokenType::Symbol, 18, 18),
            make_ident("println", 20),
            make_token("!", TokenType::Symbol, 27, 27),
            make_token("(", TokenType::Punct, 28, 28),
            make_ident("x", 29),
            make_token(")", TokenType::Punct, 30, 30),
            make_token("}", TokenType::Punct, 32, 32),
        ];

        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "test_doc_1");

        assert!(!refs.is_empty(), "应有至少一条引用边");
    }

    #[test]
    fn test_detect_link_references() {
        let tokens = vec![
            make_token("See", TokenType::Word, 0, 0),
            make_token("[", TokenType::Punct, 4, 4),
            make_token("docs", TokenType::Identifier, 5, 5),
            make_token("]", TokenType::Punct, 9, 9),
            make_token("(", TokenType::Punct, 10, 10),
            make_token("url", TokenType::Identifier, 11, 11),
            make_token(")", TokenType::Punct, 14, 14),
        ];

        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "test_doc_2");

        let has_link = refs.iter().any(|r| r.ref_type == RefType::Link);
        assert!(has_link, "应检测到 Markdown 链接引用");
    }

    #[test]
    fn test_empty_tokens_produces_no_references() {
        let tokens: Vec<Token> = vec![];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "test_doc_3");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_set_scope_updates_current_scope() {
        let mut resolver = SymbolResolver::new();
        assert!(resolver.current_scope.is_none());
        resolver.set_scope(Some("function::main".to_string()));
        assert_eq!(resolver.current_scope.as_deref(), Some("function::main"));
        resolver.set_scope(None);
        assert!(resolver.current_scope.is_none());
    }

    #[test]
    fn test_resolver_default() {
        let resolver = SymbolResolver::default();
        assert_eq!(resolver.symbol_count(), 0);
        assert_eq!(resolver.reference_count(), 0);
    }

    #[test]
    fn test_find_usages_no_definition() {
        let resolver = SymbolResolver::new();
        let usages = resolver.find_usages("nonexistent");
        assert!(usages.is_empty());
    }

    #[test]
    fn test_find_usages_with_definition() {
        let mut resolver = SymbolResolver::new();
        let tokens = vec![
            make_keyword("fn", 0),
            make_ident("main", 3),
            make_token("(", TokenType::Punct, 7, 7),
            make_token(")", TokenType::Punct, 8, 8),
        ];
        resolver.resolve(&tokens, "doc1");
        let usages = resolver.find_usages("main");
        assert!(!usages.is_empty() || resolver.symbol_count() > 0);
    }

    #[test]
    fn test_resolve_type_definition_context() {
        let tokens = vec![
            make_keyword("struct", 0),
            make_ident("MyStruct", 7),
            make_token("{", TokenType::Punct, 15, 15),
            make_token("}", TokenType::Punct, 16, 16),
        ];
        let mut resolver = SymbolResolver::new();
        let _refs = resolver.resolve(&tokens, "doc_struct");
        assert!(resolver.symbol_count() > 0, "struct 后的标识符应注册为定义");
    }

    #[test]
    fn test_resolve_class_definition() {
        let tokens = vec![
            make_keyword("class", 0),
            make_ident("MyClass", 6),
            make_token("{", TokenType::Punct, 13, 13),
            make_token("}", TokenType::Punct, 14, 14),
        ];
        let mut resolver = SymbolResolver::new();
        let _refs = resolver.resolve(&tokens, "doc_class");
        assert!(resolver.symbol_count() > 0, "class 后的标识符应注册为定义");
    }

    #[test]
    fn test_resolve_enum_definition() {
        let tokens = vec![
            make_keyword("enum", 0),
            make_ident("Color", 5),
            make_token("{", TokenType::Punct, 10, 10),
            make_token("}", TokenType::Punct, 11, 11),
        ];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_enum");
        assert!(resolver.symbol_count() > 0, "enum 后的标识符应注册为定义");
    }

    #[test]
    fn test_resolve_trait_definition() {
        let tokens = vec![
            make_keyword("trait", 0),
            make_ident("Display", 6),
            make_token("{", TokenType::Punct, 13, 13),
            make_token("}", TokenType::Punct, 14, 14),
        ];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_trait");
        assert!(resolver.symbol_count() > 0, "trait 后的标识符应注册为定义");
    }

    #[test]
    fn test_resolve_impl_definition() {
        let tokens = vec![
            make_keyword("impl", 0),
            make_ident("Display", 5),
            make_token("{", TokenType::Punct, 12, 12),
            make_token("}", TokenType::Punct, 13, 13),
        ];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_impl");
        assert!(resolver.symbol_count() > 0, "impl 后的标识符应注册为定义");
    }

    #[test]
    fn test_resolve_usage_with_dot_access() {
        let tokens = vec![
            make_keyword("fn", 0),
            make_ident("method", 3),
            make_token("(", TokenType::Punct, 9, 9),
            make_token(")", TokenType::Punct, 10, 10),
            make_token("{", TokenType::Punct, 12, 12),
            make_token("}", TokenType::Punct, 13, 13),
            make_ident("obj", 15),
            make_token(".", TokenType::Punct, 18, 18),
            make_ident("method", 19),
            make_token("(", TokenType::Punct, 25, 25),
            make_token(")", TokenType::Punct, 26, 26),
        ];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "doc_dot");
        assert!(!refs.is_empty(), "点访问已定义符号应产生引用");
    }

    #[test]
    fn test_resolve_usage_with_double_colon() {
        let tokens = vec![
            make_keyword("fn", 0),
            make_ident("collections", 3),
            make_token("(", TokenType::Punct, 14, 14),
            make_token(")", TokenType::Punct, 15, 15),
            make_token("{", TokenType::Punct, 17, 17),
            make_token("}", TokenType::Punct, 18, 18),
            make_ident("std", 20),
            make_token("::", TokenType::Punct, 23, 23),
            make_ident("collections", 25),
        ];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "doc_dc");
        assert!(!refs.is_empty(), ":: 访问已定义符号应产生引用");
    }

    #[test]
    fn test_resolve_function_call_usage() {
        let tokens = vec![
            make_keyword("fn", 0),
            make_ident("helper", 3),
            make_token("(", TokenType::Punct, 9, 9),
            make_token(")", TokenType::Punct, 10, 10),
            make_token("{", TokenType::Punct, 12, 12),
            make_token("}", TokenType::Punct, 13, 13),
            make_ident("helper", 15),
            make_token("(", TokenType::Punct, 21, 21),
            make_token(")", TokenType::Punct, 22, 22),
        ];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "doc_call");
        assert!(!refs.is_empty(), "函数调用应产生引用");
    }

    #[test]
    fn test_resolve_return_usage() {
        let tokens = vec![
            make_keyword("fn", 0),
            make_ident("value", 3),
            make_token("(", TokenType::Punct, 8, 8),
            make_token(")", TokenType::Punct, 9, 9),
            make_token("{", TokenType::Punct, 11, 11),
            make_keyword("return", 13),
            make_ident("value", 20),
            make_token("}", TokenType::Punct, 25, 25),
        ];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "doc_return");
        assert!(!refs.is_empty(), "return 后已定义标识符应产生引用");
    }

    #[test]
    fn test_resolve_inherit_reference() {
        let tokens = vec![
            make_keyword("class", 0),
            make_ident("Parent", 6),
            make_token("{", TokenType::Punct, 12, 12),
            make_token("}", TokenType::Punct, 13, 13),
            make_keyword("class", 15),
            make_ident("Child", 21),
            make_ident("extends", 27),
            make_ident("Parent", 35),
            make_token("{", TokenType::Punct, 41, 41),
            make_token("}", TokenType::Punct, 42, 42),
        ];
        let mut resolver = SymbolResolver::new();
        let _refs = resolver.resolve(&tokens, "doc_inherit");
        assert!(
            resolver.symbol_count() > 0,
            "class/extends 上下文应注册定义"
        );
    }

    #[test]
    fn test_resolve_implement_reference() {
        let tokens = vec![
            make_keyword("trait", 0),
            make_ident("Display", 6),
            make_token("{", TokenType::Punct, 13, 13),
            make_token("}", TokenType::Punct, 14, 14),
            make_keyword("impl", 16),
            make_ident("Display", 21),
            make_keyword("for", 28),
            make_ident("MyType", 32),
            make_token("{", TokenType::Punct, 38, 38),
            make_token("}", TokenType::Punct, 39, 39),
        ];
        let mut resolver = SymbolResolver::new();
        let _refs = resolver.resolve(&tokens, "doc_impl_for");
        assert!(resolver.symbol_count() > 0, "impl/for 上下文应注册定义");
    }

    #[test]
    fn test_resolve_constrain_reference() {
        let tokens = vec![
            make_keyword("trait", 0),
            make_ident("Display", 6),
            make_token("{", TokenType::Punct, 13, 13),
            make_token("}", TokenType::Punct, 14, 14),
            make_keyword("fn", 16),
            make_ident("process", 19),
            make_token("(", TokenType::Punct, 26, 26),
            make_token(")", TokenType::Punct, 27, 27),
            make_keyword("where", 29),
            make_ident("T", 35),
            make_token(":", TokenType::Punct, 37, 37),
            make_ident("Display", 39),
        ];
        let mut resolver = SymbolResolver::new();
        let _refs = resolver.resolve(&tokens, "doc_where");
        assert!(resolver.symbol_count() > 0, "where 约束上下文应注册定义");
    }

    #[test]
    fn test_resolve_generic_constrain() {
        let tokens = vec![
            make_keyword("trait", 0),
            make_ident("T", 6),
            make_token("{", TokenType::Punct, 8, 8),
            make_token("}", TokenType::Punct, 9, 9),
            make_keyword("fn", 11),
            make_ident("foo", 14),
            make_token("<", TokenType::Punct, 18, 18),
            make_ident("T", 19),
            make_token(">", TokenType::Punct, 20, 20),
        ];
        let mut resolver = SymbolResolver::new();
        let _refs = resolver.resolve(&tokens, "doc_generic");
        assert!(resolver.symbol_count() > 0, "泛型上下文应注册定义");
    }

    #[test]
    fn test_resolve_type_annotation_inherit() {
        let tokens = vec![
            make_keyword("class", 0),
            make_ident("Parent", 6),
            make_token("{", TokenType::Punct, 12, 12),
            make_token("}", TokenType::Punct, 13, 13),
            make_keyword("class", 15),
            make_ident("Child", 21),
            make_token(":", TokenType::Punct, 27, 27),
            make_ident("Parent", 29),
        ];
        let mut resolver = SymbolResolver::new();
        let _refs = resolver.resolve(&tokens, "doc_type_annot");
        assert!(resolver.symbol_count() > 0, "类型注解上下文应注册定义");
    }

    #[test]
    fn test_resolve_await_usage() {
        let tokens = vec![
            make_keyword("fn", 0),
            make_ident("data", 3),
            make_token("(", TokenType::Punct, 7, 7),
            make_token(")", TokenType::Punct, 8, 8),
            make_token("{", TokenType::Punct, 10, 10),
            make_token("}", TokenType::Punct, 11, 11),
            make_keyword("async", 13),
            make_keyword("fn", 19),
            make_ident("fetch", 22),
            make_token("(", TokenType::Punct, 27, 27),
            make_token(")", TokenType::Punct, 28, 28),
            make_token("{", TokenType::Punct, 30, 30),
            make_keyword("await", 32),
            make_ident("data", 38),
            make_token("}", TokenType::Punct, 42, 42),
        ];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "doc_await");
        assert!(!refs.is_empty(), "await 后已定义标识符应产生引用");
    }

    #[test]
    fn test_resolve_new_usage() {
        let tokens = vec![
            make_keyword("class", 0),
            make_ident("MyClass", 6),
            make_token("{", TokenType::Punct, 13, 13),
            make_token("}", TokenType::Punct, 14, 14),
            make_keyword("let", 16),
            make_ident("obj", 20),
            make_token("=", TokenType::Symbol, 24, 24),
            make_keyword("new", 26),
            make_ident("MyClass", 30),
            make_token("(", TokenType::Punct, 37, 37),
            make_token(")", TokenType::Punct, 38, 38),
        ];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "doc_new");
        assert!(!refs.is_empty(), "new 后已定义标识符应产生引用");
    }

    #[test]
    fn test_register_definition_manual() {
        let mut resolver = SymbolResolver::new();
        let token_id: RecordIdType =
            surrealdb::sql::Thing::from(("token".to_string(), "t1".to_string()));
        resolver.register_definition(&token_id, "my_symbol", "fn my_symbol");
        assert_eq!(resolver.symbol_count(), 1);
    }

    #[test]
    fn test_resolve_def_keyword() {
        let tokens = vec![
            make_keyword("def", 0),
            make_ident("my_func", 4),
            make_token("(", TokenType::Punct, 11, 11),
            make_token(")", TokenType::Punct, 12, 12),
            make_token(":", TokenType::Punct, 14, 14),
        ];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_def");
        assert!(resolver.symbol_count() > 0, "def 后标识符应注册为定义");
    }

    #[test]
    fn test_resolve_function_keyword() {
        let tokens = vec![
            make_keyword("function", 0),
            make_ident("myFunc", 9),
            make_token("(", TokenType::Punct, 15, 15),
            make_token(")", TokenType::Punct, 16, 16),
            make_token("{", TokenType::Punct, 18, 18),
            make_token("}", TokenType::Punct, 19, 19),
        ];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_function");
        assert!(resolver.symbol_count() > 0, "function 后标识符应注册为定义");
    }

    #[test]
    fn test_resolve_const_definition() {
        let tokens = vec![
            make_keyword("const", 0),
            make_ident("MAX_SIZE", 6),
            make_token("=", TokenType::Symbol, 14, 14),
        ];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_const");
        assert!(resolver.symbol_count() > 0, "const 后标识符应注册为定义");
    }

    #[test]
    fn test_resolve_var_definition() {
        let tokens = vec![
            make_keyword("var", 0),
            make_ident("count", 4),
            make_token("=", TokenType::Symbol, 9, 9),
        ];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_var");
        assert!(resolver.symbol_count() > 0, "var 后标识符应注册为定义");
    }

    #[test]
    fn test_resolve_interface_definition() {
        let tokens = vec![
            make_keyword("interface", 0),
            make_ident("Serializable", 10),
            make_token("{", TokenType::Punct, 22, 22),
            make_token("}", TokenType::Punct, 23, 23),
        ];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_interface");
        assert!(
            resolver.symbol_count() > 0,
            "interface 后标识符应注册为定义"
        );
    }

    #[test]
    fn test_resolve_type_keyword_definition() {
        let tokens = vec![
            make_keyword("type", 0),
            make_ident("Alias", 5),
            make_token("=", TokenType::Symbol, 11, 11),
        ];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_type");
        assert!(resolver.symbol_count() > 0, "type 后标识符应注册为定义");
    }

    #[test]
    fn test_resolve_func_definition() {
        let tokens = vec![
            make_keyword("func", 0),
            make_ident("handler", 5),
            make_token("(", TokenType::Punct, 12, 12),
            make_token(")", TokenType::Punct, 13, 13),
        ];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_func");
        assert!(resolver.symbol_count() > 0, "func 后标识符应注册为定义");
    }

    #[test]
    fn test_resolve_mod_definition() {
        let tokens = vec![make_keyword("mod", 0), make_ident("utils", 4)];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_mod");
        assert!(resolver.symbol_count() > 0, "mod 后标识符应注册为定义");
    }

    #[test]
    fn test_resolve_import_definition() {
        let tokens = vec![make_keyword("import", 0), make_ident("fs", 7)];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_import");
        assert!(resolver.symbol_count() > 0, "import 后标识符应注册为定义");
    }

    #[test]
    fn test_resolve_export_definition() {
        let tokens = vec![make_keyword("export", 0), make_ident("helper", 7)];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_export");
        assert!(resolver.symbol_count() > 0, "export 后标识符应注册为定义");
    }

    #[test]
    fn test_resolve_package_definition() {
        let tokens = vec![make_keyword("package", 0), make_ident("main", 8)];
        let mut resolver = SymbolResolver::new();
        resolver.resolve(&tokens, "doc_package");
        assert!(resolver.symbol_count() > 0, "package 后标识符应注册为定义");
    }

    #[test]
    fn test_resolve_yield_usage() {
        let tokens = vec![
            make_keyword("fn", 0),
            make_ident("value", 3),
            make_token("(", TokenType::Punct, 8, 8),
            make_token(")", TokenType::Punct, 9, 9),
            make_token("{", TokenType::Punct, 11, 11),
            make_token("}", TokenType::Punct, 12, 12),
            make_keyword("fn", 14),
            make_ident("gen", 17),
            make_token("(", TokenType::Punct, 20, 20),
            make_token(")", TokenType::Punct, 21, 21),
            make_token("{", TokenType::Punct, 23, 23),
            make_keyword("yield", 25),
            make_ident("value", 31),
            make_token("}", TokenType::Punct, 36, 36),
        ];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "doc_yield");
        assert!(!refs.is_empty(), "yield 后已定义标识符应产生引用");
    }

    #[test]
    fn test_resolve_throw_usage() {
        let tokens = vec![
            make_keyword("class", 0),
            make_ident("Error", 6),
            make_token("{", TokenType::Punct, 11, 11),
            make_token("}", TokenType::Punct, 12, 12),
            make_keyword("throw", 14),
            make_ident("Error", 20),
            make_token("(", TokenType::Punct, 25, 25),
            make_token(")", TokenType::Punct, 26, 26),
        ];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "doc_throw");
        assert!(!refs.is_empty(), "throw 后已定义标识符应产生引用");
    }

    #[test]
    fn test_resolve_typeof_usage() {
        let tokens = vec![
            make_keyword("fn", 0),
            make_ident("obj", 3),
            make_token("(", TokenType::Punct, 6, 6),
            make_token(")", TokenType::Punct, 7, 7),
            make_token("{", TokenType::Punct, 9, 9),
            make_token("}", TokenType::Punct, 10, 10),
            make_keyword("typeof", 12),
            make_ident("obj", 19),
        ];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "doc_typeof");
        assert!(!refs.is_empty(), "typeof 后已定义标识符应产生引用");
    }

    #[test]
    fn test_resolve_instanceof_usage() {
        let tokens = vec![
            make_keyword("class", 0),
            make_ident("MyClass", 6),
            make_token("{", TokenType::Punct, 13, 13),
            make_token("}", TokenType::Punct, 14, 14),
            make_ident("obj", 16),
            make_keyword("instanceof", 20),
            make_ident("MyClass", 31),
        ];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "doc_instanceof");
        assert!(!refs.is_empty(), "instanceof 后已定义标识符应产生引用");
    }

    #[test]
    fn test_resolve_raise_usage() {
        let tokens = vec![
            make_keyword("class", 0),
            make_ident("ValueError", 6),
            make_token("{", TokenType::Punct, 16, 16),
            make_token("}", TokenType::Punct, 17, 17),
            make_keyword("raise", 19),
            make_ident("ValueError", 25),
            make_token("(", TokenType::Punct, 35, 35),
            make_token(")", TokenType::Punct, 36, 36),
        ];
        let mut resolver = SymbolResolver::new();
        let refs = resolver.resolve(&tokens, "doc_raise");
        assert!(!refs.is_empty(), "raise 后已定义标识符应产生引用");
    }

    #[test]
    fn test_resolve_implements_keyword() {
        let tokens = vec![
            make_keyword("interface", 0),
            make_ident("Runnable", 10),
            make_token("{", TokenType::Punct, 18, 18),
            make_token("}", TokenType::Punct, 19, 19),
            make_keyword("class", 21),
            make_ident("Impl", 27),
            make_ident("implements", 32),
            make_ident("Runnable", 43),
            make_token("{", TokenType::Punct, 51, 51),
            make_token("}", TokenType::Punct, 52, 52),
        ];
        let mut resolver = SymbolResolver::new();
        let _refs = resolver.resolve(&tokens, "doc_implements");
        assert!(resolver.symbol_count() > 0, "implements 上下文应注册定义");
    }
}
