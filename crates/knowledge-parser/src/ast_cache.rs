//! AST LRU 缓存（缓存 tree-sitter 语法树，避免重复解析）
//!
//! # 设计目标
//!
//! - 缓存已解析的 `tree_sitter::Tree` 实例，避免对同一源码重复调用解析器
//! - LRU（Least Recently Used）淘汰策略：容量满时自动移除最久未访问的条目
//! - 使用 `Vec<String>` 追踪访问顺序（前端为最近访问，后端为最久未访问）
//!
//! # 所有权语义
//!
//! `tree_sitter::Tree` 不实现 `Clone`，因此缓存采用所有权语义：
//! - `insert` 接受 `Tree` 的所有权
//! - `get` 返回 `Tree` 的不可变引用（不转移所有权）
//!
//! # 线程安全
//!
//! `LruAstCache` 本身不是线程安全的。跨 DAG 阶段共享时，
//! 应使用 `AstCache` 类型别名（`Arc<Mutex<LruAstCache>>`）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// AST LRU 缓存（缓存 tree-sitter 语法树，避免重复解析）
///
/// 使用 `HashMap` 存储键值对，`Vec` 追踪访问顺序实现 LRU 淘汰。
/// 当缓存容量达到上限时，自动移除最久未访问的条目。
///
/// # Example
///
/// ```ignore
/// let mut cache = LruAstCache::new(64);
/// cache.insert("file.rs".to_string(), tree);
/// if let Some(ast) = cache.get("file.rs") {
///     println!("缓存命中，根节点类型: {}", ast.root_node().kind());
/// }
/// ```
pub struct LruAstCache {
    cache: HashMap<String, tree_sitter::Tree>,
    capacity: usize,
    access_order: Vec<String>,
}

impl LruAstCache {
    /// 创建指定容量的 LRU 缓存
    ///
    /// # 参数
    ///
    /// * `capacity` - 缓存最大条目数。若为 0，则缓存不存储任何条目（所有插入立即丢弃）
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: HashMap::new(),
            capacity,
            access_order: Vec::new(),
        }
    }

    /// 获取缓存中的语法树（同时更新访问顺序）
    ///
    /// 若键存在，将该键移至访问顺序前端（标记为最近使用），
    /// 并返回对应语法树的不可变引用。
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键（通常为文件路径或内容哈希）
    pub fn get(&mut self, key: &str) -> Option<&tree_sitter::Tree> {
        if self.cache.contains_key(key) {
            self.touch(key);
        }
        self.cache.get(key)
    }

    /// 插入语法树到缓存
    ///
    /// 若键已存在，替换旧值并将键移至访问顺序前端。
    /// 若键不存在且缓存已满，先淘汰最久未访问的条目再插入。
    /// 若容量为 0，直接丢弃（不存储）。
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键（通常为文件路径或内容哈希）
    /// * `tree` - tree-sitter 语法树（转移所有权）
    pub fn insert(&mut self, key: String, tree: tree_sitter::Tree) {
        if self.capacity == 0 {
            return;
        }

        if self.cache.contains_key(&key) {
            self.cache.insert(key.clone(), tree);
            self.touch(&key);
            return;
        }

        if self.cache.len() >= self.capacity {
            self.evict_lru();
        }

        self.access_order.insert(0, key.clone());
        self.cache.insert(key, tree);
    }

    /// 检查缓存中是否包含指定键（不更新访问顺序）
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.cache.contains_key(key)
    }

    /// 返回缓存中的条目数量
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// 检查缓存是否为空
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// 清空缓存（移除所有条目和访问记录）
    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_order.clear();
    }

    fn touch(&mut self, key: &str) {
        self.access_order.retain(|k| k != key);
        self.access_order.insert(0, key.to_string());
    }

    fn evict_lru(&mut self) {
        if let Some(lru_key) = self.access_order.pop() {
            self.cache.remove(&lru_key);
        }
    }
}

/// 跨 DAG 阶段共享的 AST 缓存类型别名
///
/// 使用 `Arc<Mutex<>>` 包装 `LruAstCache`，使其可在线程间安全共享。
/// `tree_sitter::Tree` 是 `Send` 但非 `Sync`，
/// `Mutex` 保证同一时刻只有一个线程可以访问缓存。
pub type AstCache = Arc<Mutex<LruAstCache>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_rust_tree(source: &str) -> Option<tree_sitter::Tree> {
        let lang = crate::grammar_cache::GrammarCachePool::get_language("rust").ok()?;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(lang).ok()?;
        parser.parse(source, None)
    }

    #[test]
    fn test_ast_cache_insert_and_get() {
        let Some(tree) = parse_rust_tree("fn main() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };

        let mut cache = LruAstCache::new(10);
        assert!(cache.is_empty());
        assert!(!cache.contains("file.rs"));

        cache.insert("file.rs".to_string(), tree);
        assert_eq!(cache.len(), 1);
        assert!(cache.contains("file.rs"));

        let result = cache.get("file.rs");
        assert!(result.is_some());
        assert_eq!(result.unwrap().root_node().kind(), "source_file");
    }

    #[test]
    fn test_ast_cache_lru_eviction() {
        let Some(tree1) = parse_rust_tree("fn a() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };
        let Some(tree2) = parse_rust_tree("fn b() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };
        let Some(tree3) = parse_rust_tree("fn c() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };

        let mut cache = LruAstCache::new(2);

        cache.insert("a.rs".to_string(), tree1);
        cache.insert("b.rs".to_string(), tree2);
        assert_eq!(cache.len(), 2);
        assert!(cache.contains("a.rs"));
        assert!(cache.contains("b.rs"));

        cache.insert("c.rs".to_string(), tree3);
        assert_eq!(cache.len(), 2);
        assert!(!cache.contains("a.rs"), "a.rs 应作为 LRU 被淘汰");
        assert!(cache.contains("b.rs"));
        assert!(cache.contains("c.rs"));
    }

    #[test]
    fn test_ast_cache_capacity_zero() {
        let Some(tree) = parse_rust_tree("fn main() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };

        let mut cache = LruAstCache::new(0);
        cache.insert("file.rs".to_string(), tree);
        assert_eq!(cache.len(), 0, "容量为 0 时不应存储任何条目");
        assert!(!cache.contains("file.rs"));
        assert!(cache.is_empty());
    }

    #[test]
    fn test_ast_cache_update_existing_key() {
        let Some(tree_v1) = parse_rust_tree("fn old() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };
        let Some(tree_v2) = parse_rust_tree("fn new() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };

        let mut cache = LruAstCache::new(10);

        cache.insert("file.rs".to_string(), tree_v1);
        assert_eq!(cache.len(), 1);

        cache.insert("file.rs".to_string(), tree_v2);
        assert_eq!(cache.len(), 1, "更新已存在的键不应增加条目数");

        let result = cache.get("file.rs").unwrap();
        let root_text = result.root_node().utf8_text(b"fn new() {}").unwrap_or("");
        assert!(root_text.contains("new"), "应返回更新后的语法树");
    }

    #[test]
    fn test_clear_empties_cache() {
        let Some(tree) = parse_rust_tree("fn main() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };
        let mut cache = LruAstCache::new(10);
        cache.insert("file.rs".to_string(), tree);
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty(), "clear 后缓存应为空");
        assert_eq!(cache.len(), 0, "clear 后 len 应为 0");
        assert!(!cache.contains("file.rs"), "clear 后不应包含任何键");
    }

    #[test]
    fn test_get_updates_access_order_lru_eviction() {
        let Some(tree1) = parse_rust_tree("fn a() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };
        let Some(tree2) = parse_rust_tree("fn b() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };
        let Some(tree3) = parse_rust_tree("fn c() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };

        let mut cache = LruAstCache::new(2);
        cache.insert("a.rs".to_string(), tree1);
        cache.insert("b.rs".to_string(), tree2);

        let _ = cache.get("a.rs");

        cache.insert("c.rs".to_string(), tree3);

        assert!(cache.contains("a.rs"), "a.rs 应被保留（最近访问）");
        assert!(!cache.contains("b.rs"), "b.rs 应被淘汰（最久未访问）");
        assert!(cache.contains("c.rs"), "c.rs 应被插入");
    }

    #[test]
    fn test_get_nonexistent_key_returns_none() {
        let mut cache = LruAstCache::new(10);
        let result = cache.get("nonexistent");
        assert!(result.is_none(), "不存在的键应返回 None");
    }

    #[test]
    fn test_contains_does_not_update_access_order() {
        let Some(tree1) = parse_rust_tree("fn a() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };
        let Some(tree2) = parse_rust_tree("fn b() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };
        let Some(tree3) = parse_rust_tree("fn c() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };

        let mut cache = LruAstCache::new(2);
        cache.insert("a.rs".to_string(), tree1);
        cache.insert("b.rs".to_string(), tree2);

        assert!(cache.contains("a.rs"));

        cache.insert("c.rs".to_string(), tree3);

        assert!(
            !cache.contains("a.rs"),
            "a.rs 应被淘汰（contains 不更新访问顺序）"
        );
        assert!(cache.contains("b.rs"), "b.rs 应被保留");
    }

    #[test]
    fn test_ast_cache_thread_safe_access() {
        use std::sync::{Arc, Mutex};

        let Some(tree) = parse_rust_tree("fn main() {}") else {
            eprintln!("Rust grammar 未安装，跳过测试");
            return;
        };

        let cache: AstCache = Arc::new(Mutex::new(LruAstCache::new(10)));
        cache.lock().unwrap().insert("file.rs".to_string(), tree);

        let cached = cache.lock().unwrap();
        assert!(cached.contains("file.rs"), "通过 Arc<Mutex> 访问应正常工作");
    }
}
