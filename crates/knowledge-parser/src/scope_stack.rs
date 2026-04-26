//! 作用域栈（tree-sitter AST 作用域管理）
//!
//! # 设计参考
//!
//! 本模块的设计灵感来源于 GitHub 的 [`stack-graphs`] 项目，
//! 实现了一个轻量级的作用域解析栈，用于在遍历 tree-sitter AST 时
//! 追踪当前所在的作用域层级。
//!
//! [stack-graphs]: https://github.com/github/stack-graphs
//!
//! # 核心不变量
//!
//! - 遍历 tree-sitter AST 时，遇到 `{` push scope
//! - 遇到 `}` pop scope
//! - 解析符号时**仅向上回溯栈**（不向下查找）
//!
//! # 支持的语言作用域规则
//!
//! | 语言 | 作用域边界 | 说明 |
//! |------|-----------|------|
//! | Rust | fn/block/mod 块级作用域 | `{` 和 `}` 定界 |
//! | Python | 缩进级别决定作用域 | `:` + 缩进增加 |
//! | JavaScript | function/{}/块级作用域 | 同 Rust |
//!
//! # 为什么不使用完整的 stack-graphs？
//!
//! stack-graphs 提供了完整的作用域图分析能力（跨文件、路径敏感），
//! 但对于我们的场景（单文件内符号解析），一个简单的栈结构已经足够：
//! - 不需要跨文件跳转定义查找
//! - 不需要路径敏感的名称解析
//! - 栈结构 O(1) push/pop，性能优于图遍历

use crate::tree_sitter_parser::AstNode;

/// 作用域类型分类
///
/// 表示不同语义层次的作用域，影响符号可见性规则。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeType {
    /// 函数作用域（fn/def/function 内部）
    Function,
    /// 块作用域（{} 或缩进块内部）
    Block,
    /// 模块作用域（mod/package/file 级别）
    Module,
    /// 类作用域（class/struct/enum 内部）
    Class,
    /// 全局作用域（文件根级别）
    Global,
}

impl std::fmt::Display for ScopeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function => write!(f, "function"),
            Self::Block => write!(f, "block"),
            Self::Module => write!(f, "module"),
            Self::Class => write!(f, "class"),
            Self::Global => write!(f, "global"),
        }
    }
}

/// 作用域帧（栈中的单个元素）
///
/// 记录一个作用域的元信息，包括其类型、位置和内部定义的符号。
#[derive(Debug, Clone)]
pub struct ScopeFrame {
    pub scope_id: String,
    pub scope_type: ScopeType,
    pub start_line: u32,
    /// 该作用域内定义的符号名列表
    symbols: Vec<String>,
}

/// 作用域栈（tree-sitter AST 作用域管理）
///
/// 维护一个后进先出（LIFO）的作用域帧栈，
/// 在 AST 遍历过程中动态追踪当前所在的作用域上下文。
///
/// # 线程安全
///
/// `ScopeStack` 不实现 `Sync`/`Send`（包含 `Vec<String>` 等非原子类型），
/// 但由于 tree-sitter 解析本身是单线程的，这不构成实际问题。
///
/// # Example
///
/// ```ignore
/// use knowledge_parser::{ScopeStack, ScopeType};
///
/// let mut stack = ScopeStack::new();
///
/// // 进入全局作用域
/// stack.push_scope(ScopeType::Global, 0);
///
/// // 进入函数作用域
/// stack.push_scope(ScopeType::Function, 5);
/// stack.define_symbol("x");
///
/// // 查找符号（应找到 x 在 function 作用域中）
/// let found = stack.lookup_symbol("x");
/// assert!(found.is_some());
///
/// // 退出函数作用域
/// let frame = stack.pop_scope();
/// assert!(frame.is_some() && frame.unwrap().scope_type == ScopeType::Function);
/// ```
pub struct ScopeStack {
    /// 作用域帧栈（栈顶为当前作用域）
    stack: Vec<ScopeFrame>,
    /// 当前嵌套深度（用于生成唯一 `scope_id`）
    depth: usize,
}

impl Default for ScopeStack {
    fn default() -> Self {
        Self::new()
    }
}

impl ScopeStack {
    /// 创建新的空作用域栈
    ///
    /// 初始状态：空栈（无任何作用域帧）。
    /// 使用前应先调用 `push_scope` 推入至少一个帧（通常是 Global）。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            stack: Vec::new(),
            depth: 0,
        }
    }

    /// 进入新作用域（遇到开括号或缩进增加时调用）
    ///
    /// 在栈顶推入一个新的作用域帧。
    ///
    /// # 参数
    ///
    /// * `scope_type` - 新作用域的类型
    /// * `line` - 新作用域起始行号
    pub fn push_scope(&mut self, scope_type: ScopeType, line: u32) {
        let scope_id = format!("scope_{}_{}_{}", scope_type, self.depth, line);

        self.stack.push(ScopeFrame {
            scope_id,
            scope_type,
            start_line: line,
            symbols: Vec::new(),
        });

        self.depth += 1;
    }

    /// 退出当前作用域（遇到闭括号或缩进减少时调用）
    ///
    /// 弹出并返回栈顶的作用域帧。
    ///
    /// # Returns
    ///
    /// - `Some(frame)` - 被弹出的作用域帧
    /// - `None` - 栈为空（可能存在不匹配的 pop）
    pub fn pop_scope(&mut self) -> Option<ScopeFrame> {
        if let Some(frame) = self.stack.pop() {
            self.depth = self.depth.saturating_sub(1);
            Some(frame)
        } else {
            None
        }
    }

    /// 在当前作用域注册符号定义
    ///
    /// 将符号名添加到**当前栈顶**作用域的符号列表中。
    /// 如果栈为空则静默忽略（防止 panic）。
    ///
    /// # 参数
    ///
    /// * `name` - 被定义的符号名
    pub fn define_symbol(&mut self, name: &str) {
        if let Some(frame) = self.stack.last_mut() {
            frame.symbols.push(name.to_string());
        }
    }

    /// 向上回溯查找符号定义（从内到外）
    ///
    /// 从当前作用域开始，逐层向外搜索符号定义。
    /// 这实现了编程语言常见的"变量遮蔽"（shadowing）语义：
    /// **内层定义优先于外层同名定义**。
    ///
    /// # 参数
    ///
    /// * `name` - 要查找的符号名
    ///
    /// # Returns
    ///
    /// - `Some((level, &str))` - 找到的信息：
    ///   - `level`: 作用域层级（0 = 最内层，越大越外层）
    ///   - `&str`: 符号名（与输入相同，用于链式调用）
    /// - `None` - 所有层均未找到该符号
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut stack = ScopeStack::new();
    /// stack.push_scope(ScopeType::Global, 0);
    /// stack.define_symbol("global_var");
    /// stack.push_scope(ScopeType::Function, 10);
    /// stack.define_symbol("local_var");
    ///
    /// // local_var 在第 0 层（最内层）找到
    /// assert_eq!(stack.lookup_symbol("local_var"), Some((0, "local_var")));
    ///
    /// // global_var 在第 1 层（外层）找到
    /// assert_eq!(stack.lookup_symbol("global_var"), Some((1, "global_var")));
    ///
    /// // undefined 未在任何层找到
    /// assert_eq!(stack.lookup_symbol("undefined"), None);
    /// ```
    #[must_use]
    pub fn lookup_symbol<'a>(&self, name: &'a str) -> Option<(usize, &'a str)> {
        for (level, frame) in self.stack.iter().rev().enumerate() {
            if frame.symbols.iter().any(|s| s == name) {
                return Some((level, name));
            }
        }
        None
    }

    /// 获取当前作用域深度（栈中帧的数量）
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// 判断是否在指定类型的作用域内
    ///
    /// 从栈顶向栈底检查是否存在匹配类型的作用域帧。
    #[must_use]
    pub fn is_within(&self, scope_type: &ScopeType) -> bool {
        self.stack.iter().any(|f| &f.scope_type == scope_type)
    }

    /// 获取当前（最内层）作用域 ID
    ///
    /// # Returns
    ///
    /// - `Some(id)` - 当前作用域的唯一标识符
    /// - `None` - 栈为空
    #[must_use]
    pub fn current_scope_id(&self) -> Option<&str> {
        self.stack.last().map(|f| f.scope_id.as_str())
    }

    /// 获取当前作用域类型
    ///
    /// # Returns
    ///
    /// - `Some(type)` - 当前作用域的类型
    /// - `None` - 栈为空
    #[must_use]
    pub fn current_scope_type(&self) -> Option<&ScopeType> {
        self.stack.last().map(|f| &f.scope_type)
    }

    /// 根据 AST 节点自动推断并执行 push/pop 操作
    ///
    /// 这是一个便捷方法，根据 tree-sitter AST 节点的 kind 自动判断：
    /// - 函数定义节点 → push Function scope
    /// - 块节点 → push Block scope
    /// - 类/结构体节点 → push Class scope
    ///
    /// # 参数
    ///
    /// * `node` - tree-sitter AST 节点
    /// * `is_enter` - true 表示进入节点（push），false 表示离开节点（pop）
    pub fn handle_ast_node(&mut self, node: &AstNode, is_enter: bool) {
        if is_enter {
            let scope_type = Self::infer_scope_type_from_node_kind(&node.kind);
            self.push_scope(scope_type, node.start_row.try_into().unwrap_or(u32::MAX));
        } else {
            self.pop_scope();
        }
    }

    /// 从 AST 节点类型推断对应的作用域类型
    fn infer_scope_type_from_node_kind(kind: &str) -> ScopeType {
        match kind {
            k if k.contains("function") || k.contains("method") || k.contains("procedure") => {
                ScopeType::Function
            }
            k if k.contains("class") || k.contains("struct") || k.contains("enum") => {
                ScopeType::Class
            }
            k if k.contains("module") || k.contains("package") || k.contains("namespace") => {
                ScopeType::Module
            }
            _ => ScopeType::Block,
        }
    }

    /// 清空栈（重置为初始状态）
    pub fn clear(&mut self) {
        self.stack.clear();
        self.depth = 0;
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_empty_stack() {
        let stack = ScopeStack::new();

        assert_eq!(stack.depth(), 0, "新创建的栈应为空");
        assert!(stack.current_scope_id().is_none());
        assert!(stack.current_scope_type().is_none());
    }

    #[test]
    fn test_push_pop_scope_basic() {
        let mut stack = ScopeStack::new();

        stack.push_scope(ScopeType::Function, 5);
        assert_eq!(stack.depth(), 1, "push 后深度应为 1");

        let frame = stack.pop_scope();
        assert!(frame.is_some(), "pop 应返回帧");
        assert_eq!(
            frame.unwrap().scope_type,
            ScopeType::Function,
            "pop 出来的帧应为 Function 类型"
        );
        assert_eq!(stack.depth(), 0, "pop 后深度应为 0");
    }

    #[test]
    fn test_pop_empty_stack_returns_none() {
        let mut stack = ScopeStack::new();

        let result = stack.pop_scope();
        assert!(result.is_none(), "对空栈 pop 应返回 None");
    }

    #[test]
    fn test_nested_scopes() {
        let mut stack = ScopeStack::new();

        stack.push_scope(ScopeType::Global, 0);
        stack.push_scope(ScopeType::Function, 10);
        stack.push_scope(ScopeType::Block, 15);

        assert_eq!(stack.depth(), 3, "三层嵌套后深度应为 3");

        assert_eq!(
            stack.current_scope_type(),
            Some(&ScopeType::Block),
            "当前作用域应为 Block"
        );

        let block_frame = stack.pop_scope();
        assert_eq!(
            block_frame.unwrap().scope_type,
            ScopeType::Block,
            "第一次 pop 应返回 Block"
        );

        assert_eq!(
            stack.current_scope_type(),
            Some(&ScopeType::Function),
            "pop Block 后当前应为 Function"
        );

        assert_eq!(stack.depth(), 2, "pop 一次后深度应为 2");
    }

    #[test]
    fn test_define_and_lookup_symbol() {
        let mut stack = ScopeStack::new();

        stack.push_scope(ScopeType::Global, 0);
        stack.define_symbol("global_x");

        stack.push_scope(ScopeType::Function, 10);
        stack.define_symbol("local_y");

        let found_local = stack.lookup_symbol("local_y");
        assert_eq!(
            found_local,
            Some((0, "local_y")),
            "local_y 应在第 0 层（最内层）找到"
        );

        let found_global = stack.lookup_symbol("global_x");
        assert_eq!(
            found_global,
            Some((1, "global_x")),
            "global_x 应在第 1 层（外层）找到"
        );
    }

    #[test]
    fn test_lookup_symbol_upward() {
        let mut stack = ScopeStack::new();

        stack.push_scope(ScopeType::Global, 0);
        stack.define_symbol("outer");

        stack.push_scope(ScopeType::Function, 10);
        stack.define_symbol("inner");

        stack.push_scope(ScopeType::Block, 20);
        stack.define_symbol("block_var");

        // 最内层符号应最先找到（level=0）
        assert_eq!(
            stack.lookup_symbol("block_var"),
            Some((0, "block_var")),
            "block_var 应在最内层找到"
        );

        // 中间层符号
        assert_eq!(
            stack.lookup_symbol("inner"),
            Some((1, "inner")),
            "inner 应在中间层找到"
        );

        // 最外层符号
        assert_eq!(
            stack.lookup_symbol("outer"),
            Some((2, "outer")),
            "outer 应在最外层找到"
        );
    }

    #[test]
    fn test_lookup_nonexistent_symbol() {
        let mut stack = ScopeStack::new();
        stack.push_scope(ScopeType::Global, 0);

        let result = stack.lookup_symbol("nonexistent");
        assert!(result.is_none(), "查找不存在的符号应返回 None");
    }

    #[test]
    fn test_is_within_scope_type() {
        let mut stack = ScopeStack::new();

        stack.push_scope(ScopeType::Global, 0);
        assert!(stack.is_within(&ScopeType::Global), "应在 Global 作用域内");

        stack.push_scope(ScopeType::Function, 10);
        assert!(
            stack.is_within(&ScopeType::Function),
            "应在 Function 作用域内"
        );
        assert!(
            stack.is_within(&ScopeType::Global),
            "同时也在 Global 作用域内（嵌套）"
        );

        assert!(!stack.is_within(&ScopeType::Class), "不应在 Class 作用域内");
    }

    #[test]
    fn test_define_symbol_on_empty_stack_no_panic() {
        let mut stack = ScopeStack::new();

        // 空栈上 define_symbol 不应 panic
        stack.define_symbol("should_not_panic");
        assert_eq!(stack.depth(), 0, "空栈上 define 不应改变深度");
    }

    #[test]
    fn test_handle_ast_node_auto_inference() {
        let mut stack = ScopeStack::new();

        let func_node = AstNode {
            kind: "function_definition".to_string(),
            start_byte: 0,
            end_byte: 50,
            start_char: 0,
            start_row: 5,
            start_column: 0,
            is_named: true,
            text: "fn test() {}".to_string(),
        };

        stack.handle_ast_node(&func_node, true);
        assert_eq!(
            stack.current_scope_type(),
            Some(&ScopeType::Function),
            "function_definition 节点应推入 Function 作用域"
        );

        stack.handle_ast_node(&func_node, false);
        assert_eq!(stack.depth(), 0, "离开节点应弹出作用域");
    }

    #[test]
    fn test_class_node_inference() {
        let mut stack = ScopeStack::new();

        let class_node = AstNode {
            kind: "class_declaration".to_string(),
            start_byte: 0,
            end_byte: 100,
            start_char: 0,
            start_row: 0,
            start_column: 0,
            is_named: true,
            text: "class MyClass {}".to_string(),
        };

        stack.handle_ast_node(&class_node, true);
        assert_eq!(
            stack.current_scope_type(),
            Some(&ScopeType::Class),
            "class_declaration 节点应推入 Class 作用域"
        );
    }

    #[test]
    fn test_clear_resets_state() {
        let mut stack = ScopeStack::new();
        stack.push_scope(ScopeType::Global, 0);
        stack.push_scope(ScopeType::Function, 10);
        stack.define_symbol("test");

        stack.clear();

        assert_eq!(stack.depth(), 0, "clear 后深度应为 0");
        assert!(stack.current_scope_id().is_none());
    }

    #[test]
    fn test_scope_id_uniqueness() {
        let mut stack = ScopeStack::new();

        stack.push_scope(ScopeType::Function, 10);
        let id1 = stack.current_scope_id().unwrap().to_string();

        stack.push_scope(ScopeType::Block, 20);
        let id2 = stack.current_scope_id().unwrap().to_string();

        assert_ne!(id1, id2, "不同深度的作用域应有不同的 ID");
    }

    #[test]
    fn test_multiple_symbols_same_scope() {
        let mut stack = ScopeStack::new();
        stack.push_scope(ScopeType::Function, 0);

        stack.define_symbol("a");
        stack.define_symbol("b");
        stack.define_symbol("c");

        assert_eq!(stack.lookup_symbol("a"), Some((0, "a")));
        assert_eq!(stack.lookup_symbol("b"), Some((0, "b")));
        assert_eq!(stack.lookup_symbol("c"), Some((0, "c")));
    }

    #[test]
    fn test_shadowing_inner_hides_outer() {
        let mut stack = ScopeStack::new();

        stack.push_scope(ScopeType::Global, 0);
        stack.define_symbol("name"); // 外层定义

        stack.push_scope(ScopeType::Function, 10);
        stack.define_symbol("name"); // 内层遮蔽

        // lookup_symbol 返回最先找到的（最内层）
        let (level, _) = stack.lookup_symbol("name").expect("应找到 name");
        assert_eq!(level, 0, "内层定义应在 level 0 找到（遮蔽外层）");
    }
}
