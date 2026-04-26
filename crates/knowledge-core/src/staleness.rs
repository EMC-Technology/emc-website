//! 索引过期检测模块
//!
//! 通过 blake3 哈希比较检测文档索引是否过期。
//! 当存储的哈希与当前文件哈希不一致时，标记索引为 Stale（过期）。
use serde::{Deserialize, Serialize};

/// 索引新鲜度状态
///
/// 表示文档索引与当前文件内容的一致性状态。
///
/// # 变体
///
/// - `Fresh`：索引与文件内容一致，无需重新索引
/// - `Stale`：索引与文件内容不一致，需要重新索引
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StalenessStatus {
    /// 索引与文件内容一致
    Fresh,
    /// 索引与文件内容不一致，需要重新索引
    Stale,
}

/// 索引过期检测器
///
/// 通过比较存储的 blake3 哈希与当前文件哈希来判断文档索引是否过期。
pub struct StalenessChecker;

impl StalenessChecker {
    /// 检测单个文档的索引是否过期
    ///
    /// 通过比较存储的哈希值与当前文件哈希值来判断索引状态。
    ///
    /// # 参数
    ///
    /// * `stored_hash` - 数据库中存储的 blake3 哈希值（64 字符 hex）
    /// * `current_hash` - 当前文件计算得到的 blake3 哈希值（64 字符 hex）
    ///
    /// # 返回值
    ///
    /// - `StalenessStatus::Fresh`：哈希一致，索引未过期
    /// - `StalenessStatus::Stale`：哈希不一致，索引已过期
    pub fn check_document(stored_hash: &str, current_hash: &str) -> StalenessStatus {
        if stored_hash == current_hash {
            StalenessStatus::Fresh
        } else {
            StalenessStatus::Stale
        }
    }

    /// 批量检测多个文档的索引过期状态
    ///
    /// # 参数
    ///
    /// * `docs` - 文档列表，每个元素为 `(doc_id, stored_hash, current_hash)` 三元组
    ///
    /// # 返回值
    ///
    /// 返回与输入顺序一致的 `(doc_id, StalenessStatus)` 列表
    pub fn check_documents(
        docs: &[(String, String, String)],
    ) -> Vec<(String, StalenessStatus)> {
        docs.iter()
            .map(|(doc_id, stored_hash, current_hash)| {
                (doc_id.clone(), Self::check_document(stored_hash, current_hash))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_staleness_checker_fresh_document() {
        let hash = "a".repeat(64);
        let status = StalenessChecker::check_document(&hash, &hash);
        assert_eq!(status, StalenessStatus::Fresh);
    }

    #[test]
    fn test_staleness_checker_stale_document() {
        let stored = "a".repeat(64);
        let current = "b".repeat(64);
        let status = StalenessChecker::check_document(&stored, &current);
        assert_eq!(status, StalenessStatus::Stale);
    }

    #[test]
    fn test_staleness_checker_multiple_documents() {
        let hash_a = "a".repeat(64);
        let hash_b = "b".repeat(64);
        let hash_c = "c".repeat(64);

        let docs = vec![
            ("doc1".to_string(), hash_a.clone(), hash_a.clone()),
            ("doc2".to_string(), hash_b.clone(), hash_c),
            ("doc3".to_string(), hash_a, hash_b),
        ];

        let results = StalenessChecker::check_documents(&docs);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, "doc1");
        assert_eq!(results[0].1, StalenessStatus::Fresh);
        assert_eq!(results[1].0, "doc2");
        assert_eq!(results[1].1, StalenessStatus::Stale);
        assert_eq!(results[2].0, "doc3");
        assert_eq!(results[2].1, StalenessStatus::Stale);
    }
}
