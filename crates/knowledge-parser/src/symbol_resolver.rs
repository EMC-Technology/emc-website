//! 符号解析器（遍历 Token 流 → 识别 Definition/Usage/LINK 等引用关系）

use knowledge_core::model::{Direction, RefType, Reference, Token, TokenType, RecordIdType};
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

            let token_id: RecordIdType = surrealdb::sql::Thing::from((
                "token".to_string(),
                format!("{document_id}_{idx}"),
            ));

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
        self.symbol_table.get(symbol_name).map_or_else(Vec::new, |entries| entries
            .iter()
            .flat_map(|entry| {
                self.references
                    .iter()
                    .filter(|r| r.to_id == entry.token_id)
            })
            .collect())
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

    fn is_definition_point(
        _token: &Token,
        idx: usize,
        tokens: &[Token],
        context: &str,
    ) -> bool {
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

    fn is_usage_point(
        &self,
        token: &Token,
        idx: usize,
        tokens: &[Token],
        context: &str,
    ) -> bool {
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
        if let Some(entries) = self.symbol_table.get(symbol_name) {
            if let Some(def_entry) = entries.last() {
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
    }

    fn create_link_reference(&mut self, from_id: &RecordIdType, link_text: &str) {
        let to_id: RecordIdType = surrealdb::sql::Thing::from(("link".to_string(), link_text.to_string()));
        let reference = Reference::new(
            RefType::Link,
            Direction::OneWay,
            from_id.clone(),
            to_id,
        );
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

        if context.contains("where")
            || (context.contains('<') && context.contains('>'))
        {
            return RefType::Constrain;
        }

        RefType::Usage
    }

    fn build_context_string(tokens: &[Token]) -> String {
        let filtered: Vec<&str> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Punct || matches!(t.content.as_str(), "(" | ")" | "[" | "]" | "{" | "}" | ":" | ";" | "," | "."))
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
    use knowledge_core::model::TokenType;

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
        }
    }

    fn make_ident(content: &str, start_char: u32) -> Token {
        make_token(content, TokenType::Identifier, start_char, u64::from(start_char))
    }

    fn make_keyword(content: &str, start_char: u32) -> Token {
        make_token(content, TokenType::Keyword, start_char, u64::from(start_char))
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
}
