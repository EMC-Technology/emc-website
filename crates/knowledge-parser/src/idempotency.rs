//! 幂等性键生成器（doc_id+start_line+end_line+content_hash 复合去重标识）
//!
//! # 设计目标
//!
//! - **防止重复入库**：同一文件的相同内容多次提交不会产生重复实体
//! - **支持增量更新**：内容变更时 key 变化，触发更新而非插入
//! - **极低碰撞概率**：使用 BLAKE3 哈希，碰撞概率 < 2^-128
//!
//! # 键格式
//!
//! ```text
//! blake3("{doc_id}:{start_line}:{end_line}:{content_hash}")
//! ```
//!
//! # 为什么选择 BLAKE3？
//!
//! | 特性 | BLAKE3 | SHA-256 | xxHash |
//! |------|--------|---------|--------|
//! | 安全性（抗碰撞）| ✅ 256-bit | ✅ 256-bit | ❌ 非密码学 |
//! | 速度 | ✅ 极快 | 🐌 较慢 | ✅ 最快 |
//! | 输出长度可配置 | ✅ | ❌ 固定 | ❌ 固定 |
//!
//! BLAKE3 在安全性和性能之间取得了最佳平衡：
//! - 比 SHA-256 快约 5-10 倍（单线程）
//! - 内置的密钥派生和 HKDF 支持
//! - 可扩展到多核并行计算

/// 幂等性键生成器（`doc_id`+`start_line`+`end_line`+`content_hash` 复合去重标识）
///
/// 用途：
/// - 防止同一文件的相同内容重复入库
/// - 支持增量更新检测（内容变更时 key 变化触发更新而非插入）
///
/// # 键格式
///
/// ```text
/// blake3("{doc_id}:{start_line}:{end_line}:{content_hash}")
/// ```
///
/// # 碰撞概率分析
///
/// BLAKE3 输出 256 位哈希值，生日攻击下的碰撞概率为：
/// - 2^64 个不同输入 → 碰撞概率 ≈ 2^-128（可忽略不计）
/// - 对于知识图谱场景（单个项目 < 2^40 个块），实际碰撞概率趋近于零
pub struct IdempotencyKeyGenerator;

impl IdempotencyKeyGenerator {
    /// 为单个 Block 生成幂等性键
    ///
    /// 组合文档 ID、行范围和内容哈希，生成全局唯一的去重标识。
    ///
    /// # 参数
    ///
    /// * `doc_id` - 所属文档的唯一标识符
    /// * `start_line` - 块起始行号（从 0 开始）
    /// * `end_line` - 块结束行号（包含）
    /// * `content` - 块的内容文本
    ///
    /// # Returns
    ///
    /// 64 字符十六进制编码的 BLAKE3 哈希字符串
    ///
    /// # Example
    ///
    /// ```ignore
    /// use knowledge_parser::IdempotencyKeyGenerator;
    ///
    /// let key = IdempotencyKeyGenerator::generate_for_block(
    ///     "document:abc123",
    ///     0,
    ///     25,
    ///     "这是第一段内容...",
    /// );
    /// assert_eq!(key.len(), 64); // BLAKE3 hex 编码固定 64 字符
    /// ```
    #[must_use]
    pub fn generate_for_block(
        doc_id: &str,
        start_line: u32,
        end_line: u32,
        content: &str,
    ) -> String {
        let content_hash = Self::hash_content(content);
        let input = format!("{doc_id}:{start_line}:{end_line}:{content_hash}");
        blake3::hash(input.as_bytes()).to_hex().to_string()
    }

    /// 为 Document 生成幂等性键（基于文件路径和内容哈希）
    ///
    /// 用于 Document 级别的去重：相同路径 + 相同内容 = 同一 Document。
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件系统路径
    /// * `content_hash` - 文件内容的 `BLAKE3` 哈希（来自 `FileIngester`）
    #[must_use]
    pub fn generate_for_document(file_path: &str, content_hash: &str) -> String {
        let input = format!("{file_path}:{content_hash}");
        blake3::hash(input.as_bytes()).to_hex().to_string()
    }

    /// 为 `Token` 生成幂等性键（基于 `block_id` + `content` + `offset`）
    ///
    /// Token 级别的去重粒度更细，用于防止重复 Token 入库。
    ///
    /// # 参数
    ///
    /// * `block_id` - 所属 Block 的 ID
    /// * `content` - Token 的文本内容
    /// * `global_offset` - 全局字符偏移量
    #[must_use]
    pub fn generate_for_token(
        block_id: &str,
        content: &str,
        global_offset: u64,
    ) -> String {
        let input = format!("{block_id}:{content}:{global_offset}");
        blake3::hash(input.as_bytes()).to_hex().to_string()
    }

    /// 快速哈希内容（用于复合键生成）
    ///
    /// 对原始内容做一次 BLAKE3 哈希，返回 hex 编码字符串。
    /// 这是 `generate_for_block` 和其他方法的内部构建块。
    ///
    /// # 参数
    ///
    /// * `content` - 要哈希的文本内容
    ///
    /// # Returns
    ///
    /// 64 字符十六进制编码的 BLAKE3 哈希值
    #[inline]
    fn hash_content(content: &str) -> String {
        blake3::hash(content.as_bytes()).to_hex().to_string()
    }

    /// 验证两个键是否可能由相同内容生成（仅用于测试目的）
    ///
    /// ⚠️ 此方法不应用于生产环境的去重判断，
    /// 仅在单元测试中验证键生成的一致性。
    #[cfg(test)]
    #[must_use]
    pub fn keys_equal(key_a: &str, key_b: &str) -> bool {
        key_a == key_b
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_for_block_produces_fixed_length() {
        let key = IdempotencyKeyGenerator::generate_for_block("doc:test", 0, 10, "hello");

        assert_eq!(
            key.len(),
            64,
            "BLAKE3 hex 编码应始终为 64 字符"
        );
        assert!(
            key.chars().all(|c| c.is_ascii_hexdigit()),
            "键应只包含十六进制字符"
        );
    }

    #[test]
    fn test_same_input_produces_same_key() {
        let key1 = IdempotencyKeyGenerator::generate_for_block("doc:a", 5, 15, "content");
        let key2 = IdempotencyKeyGenerator::generate_for_block("doc:a", 5, 15, "content");

        assert_eq!(
            key1, key2,
            "相同输入应产生相同的幂等键"
        );
    }

    #[test]
    fn test_different_content_produces_different_key() {
        let key1 =
            IdempotencyKeyGenerator::generate_for_block("doc:x", 0, 10, "version one");
        let key2 =
            IdempotencyKeyGenerator::generate_for_block("doc:x", 0, 10, "version two");

        assert_ne!(
            key1, key2,
            "不同内容应产生不同的幂等键"
        );
    }

    #[test]
    fn test_different_line_range_produces_different_key() {
        let key1 = IdempotencyKeyGenerator::generate_for_block("doc:y", 0, 10, "same");
        let key2 = IdempotencyKeyGenerator::generate_for_block("doc:y", 5, 15, "same");

        assert_ne!(
            key1, key2,
            "不同行范围应产生不同的幂等键"
        );
    }

    #[test]
    fn test_different_doc_id_produces_different_key() {
        let key1 = IdempotencyKeyGenerator::generate_for_block("doc:alpha", 0, 5, "text");
        let key2 = IdempotencyKeyGenerator::generate_for_block("doc:beta", 0, 5, "text");

        assert_ne!(
            key1, key2,
            "不同 doc_id 应产生不同的幂等键"
        );
    }

    #[test]
    fn test_empty_content_handling() {
        let key = IdempotencyKeyGenerator::generate_for_block("doc:e", 0, 0, "");

        assert_eq!(key.len(), 64, "空内容也应产生有效的 64 字符键");
    }

    #[test]
    fn test_large_content_handling() {
        let large_content = "x".repeat(100_000);
        let key = IdempotencyKeyGenerator::generate_for_block(
            "doc:large",
            0,
            1000,
            &large_content,
        );

        assert_eq!(key.len(), 64, "大内容不应影响输出长度");
    }

    #[test]
    fn test_unicode_content_stable() {
        let key1 = IdempotencyKeyGenerator::generate_for_block(
            "doc:utf8",
            0,
            1,
            "你好世界 🌍",
        );
        let key2 = IdempotencyKeyGenerator::generate_for_block(
            "doc:utf8",
            0,
            1,
            "你好世界 🌍",
        );

        assert_eq!(
            key1, key2,
            "Unicode 内容应稳定地产生相同键"
        );
    }

    #[test]
    fn test_generate_for_document() {
        let key =
            IdempotencyKeyGenerator::generate_for_document("/path/to/file.md", &"a".repeat(64));

        assert_eq!(key.len(), 64, "Document 键应为 64 字符");
    }

    #[test]
    fn test_generate_for_token() {
        let key = IdempotencyKeyGenerator::generate_for_token(
            "block:test",
            "identifier",
            42,
        );

        assert_eq!(key.len(), 64, "Token 键应为 64 字符");
    }

    #[test]
    fn test_determinism_across_multiple_calls() {
        let mut keys = Vec::new();

        for _ in 0..100 {
            let key = IdempotencyKeyGenerator::generate_for_block(
                "doc:deterministic",
                42,
                137,
                "repeated call test",
            );
            keys.push(key);
        }

        let first = &keys[0];
        for key in &keys[1..] {
            assert_eq!(
                key, first,
                "多次调用应产生确定性的相同结果"
            );
        }
    }
}
