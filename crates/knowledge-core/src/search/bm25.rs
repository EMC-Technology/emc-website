//! BM25 评分算法与倒排索引实现
//!
//! 本模块实现经典的 BM25（Best Matching 25）相关性评分算法，
//! 用于基于词频的文档检索排序。同时提供倒排索引（Inverted Index），
//! 用于快速定位包含特定词项的文档。
//!
//! # BM25 公式
//!
//! ```text
//! score(D, Q) = Σ IDF(qi) * (f(qi, D) * (k1 + 1)) / (f(qi, D) + k1 * (1 - b + b * |D| / avgdl))
//! ```
//!
//! 其中：
//! - `k1 = 1.2`：词频饱和参数
//! - `b = 0.75`：文档长度归一化参数
//! - `IDF(qi) = ln(1 + (N - n(qi) + 0.5) / (n(qi) + 0.5))  [BM25+ 变体，保证 IDF 非负]`
//! - `N`：文档总数
//! - `n(qi)`：包含词项 qi 的文档数
//! - `f(qi, D)`：词项 qi 在文档 D 中的词频
//! - `|D|`：文档 D 的长度（词项总数）
//! - `avgdl`：所有文档的平均长度

use std::collections::{HashMap, HashSet};

/// BM25 算法常量：词频饱和参数
const K1: f64 = 1.2;

/// BM25 算法常量：文档长度归一化参数
const B: f64 = 0.75;

/// BM25 关键词索引
///
/// 基于 BM25 算法对文档集合建立索引，支持对查询词项计算文档相关性评分。
/// 索引构建后为不可变结构，适用于读多写少的检索场景。
///
/// # 示例
///
/// ```rust
/// use knowledge_core::search::Bm25Index;
///
/// let tokens = vec![
///     ("doc1".to_string(), "rust".to_string()),
///     ("doc1".to_string(), "programming".to_string()),
///     ("doc2".to_string(), "python".to_string()),
///     ("doc2".to_string(), "programming".to_string()),
/// ];
///
/// let index = Bm25Index::build_from_tokens(&tokens);
/// let score = index.score(&["programming".to_string()], "doc1");
/// assert!(score > 0.0);
/// ```
#[derive(Debug, Clone)]
pub struct Bm25Index {
    /// 词项 → 包含该词项的文档数量
    term_doc_freqs: HashMap<String, usize>,
    /// 文档 ID → 文档长度（词项总数）
    doc_lengths: HashMap<String, usize>,
    /// 文档总数
    total_docs: usize,
    /// 所有文档的平均长度
    avg_doc_length: f64,
    /// 词项 → (文档 ID → 词频) 的内部映射，用于评分计算
    term_doc_tf: HashMap<String, HashMap<String, usize>>,
}

impl Bm25Index {
    /// 创建空的 BM25 索引
    ///
    /// # 示例
    ///
    /// ```rust
    /// use knowledge_core::search::Bm25Index;
    ///
    /// let index = Bm25Index::new();
    /// assert_eq!(index.total_docs(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            term_doc_freqs: HashMap::new(),
            doc_lengths: HashMap::new(),
            total_docs: 0,
            avg_doc_length: 0.0,
            term_doc_tf: HashMap::new(),
        }
    }

    /// 从词项序列构建 BM25 索引
    ///
    /// 每个元组为 `(doc_id, term)`，表示文档 `doc_id` 中出现了一个词项 `term`。
    /// 同一文档中同一词项出现多次时，词频自动累加。
    ///
    /// # 参数
    ///
    /// * `tokens` - 词项序列，每个元素为 `(文档 ID, 词项)` 元组
    ///
    /// # 示例
    ///
    /// ```rust
    /// use knowledge_core::search::Bm25Index;
    ///
    /// let tokens = vec![
    ///     ("doc1".to_string(), "hello".to_string()),
    ///     ("doc1".to_string(), "world".to_string()),
    ///     ("doc2".to_string(), "hello".to_string()),
    /// ];
    ///
    /// let index = Bm25Index::build_from_tokens(&tokens);
    /// assert_eq!(index.total_docs(), 2);
    /// ```
    pub fn build_from_tokens(tokens: &[(String, String)]) -> Self {
        let mut term_doc_freqs: HashMap<String, usize> = HashMap::new();
        let mut doc_lengths: HashMap<String, usize> = HashMap::new();
        let mut term_doc_tf: HashMap<String, HashMap<String, usize>> = HashMap::new();

        let mut docs_with_term: HashMap<String, HashSet<String>> = HashMap::new();

        for (doc_id, term) in tokens {
            *doc_lengths.entry(doc_id.clone()).or_insert(0) += 1;

            let tf_entry = term_doc_tf
                .entry(term.clone())
                .or_default()
                .entry(doc_id.clone())
                .or_insert(0);
            *tf_entry += 1;

            docs_with_term
                .entry(term.clone())
                .or_default()
                .insert(doc_id.clone());
        }

        for (term, doc_set) in &docs_with_term {
            term_doc_freqs.insert(term.clone(), doc_set.len());
        }

        let total_docs = doc_lengths.len();
        #[allow(clippy::cast_precision_loss)]
        let avg_doc_length = if total_docs > 0 {
            doc_lengths.values().sum::<usize>() as f64 / total_docs as f64
        } else {
            0.0
        };

        Self {
            term_doc_freqs,
            doc_lengths,
            total_docs,
            avg_doc_length,
            term_doc_tf,
        }
    }

    /// 计算给定查询词项对指定文档的 BM25 评分
    ///
    /// 对查询中的每个词项，计算其 IDF 与 TF 加权得分并求和。
    /// 若文档不在索引中，返回 0.0。
    ///
    /// # 参数
    ///
    /// * `query_terms` - 查询词项列表
    /// * `doc_id` - 目标文档 ID
    ///
    /// # 返回
    ///
    /// BM25 评分值，评分越高表示文档与查询越相关
    #[allow(clippy::cast_precision_loss)]
    pub fn score(&self, query_terms: &[String], doc_id: &str) -> f64 {
        let doc_length = match self.doc_lengths.get(doc_id) {
            Some(&len) => len as f64,
            None => return 0.0,
        };

        let mut total_score = 0.0;

        for term in query_terms {
            let n_qi = self.term_doc_freqs.get(term).copied().unwrap_or(0);
            if n_qi == 0 {
                continue;
            }

            let idf = ((self.total_docs as f64 - n_qi as f64 + 0.5) / (n_qi as f64 + 0.5)).ln_1p();

            let f_qi_d = self
                .term_doc_tf
                .get(term)
                .and_then(|m| m.get(doc_id))
                .copied()
                .unwrap_or(0) as f64;

            if f_qi_d == 0.0 {
                continue;
            }

            let tf_component = (f_qi_d * (K1 + 1.0))
                / K1.mul_add(1.0 - B + B * doc_length / self.avg_doc_length, f_qi_d);

            total_score += idf * tf_component;
        }

        total_score
    }

    /// 返回索引中的文档总数
    pub const fn total_docs(&self) -> usize {
        self.total_docs
    }

    /// 返回所有文档的平均长度
    pub const fn avg_doc_length(&self) -> f64 {
        self.avg_doc_length
    }

    /// 返回指定词项的文档频率（包含该词项的文档数量）
    pub fn doc_freq(&self, term: &str) -> usize {
        self.term_doc_freqs.get(term).copied().unwrap_or(0)
    }

    /// 返回指定文档的长度（词项总数）
    pub fn doc_length(&self, doc_id: &str) -> usize {
        self.doc_lengths.get(doc_id).copied().unwrap_or(0)
    }

    /// 返回所有文档长度映射的引用
    ///
    /// 键为文档 ID，值为该文档的词项总数。
    /// 可用于遍历索引中的所有文档。
    pub const fn doc_lengths(&self) -> &HashMap<String, usize> {
        &self.doc_lengths
    }
}

impl Default for Bm25Index {
    fn default() -> Self {
        Self::new()
    }
}

/// 倒排索引
///
/// 将词项映射到包含该词项的文档列表及其词频（Term Frequency），
/// 用于快速定位包含特定词项的文档。
///
/// # 示例
///
/// ```rust
/// use knowledge_core::search::InvertedIndex;
///
/// let tokens = vec![
///     ("doc1".to_string(), "rust".to_string()),
///     ("doc1".to_string(), "rust".to_string()),
///     ("doc2".to_string(), "rust".to_string()),
/// ];
///
/// let index = InvertedIndex::build_from_tokens(&tokens);
/// let results = index.search("rust");
/// assert_eq!(results.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct InvertedIndex {
    /// 词项 → (文档 ID, 词频) 列表
    index: HashMap<String, Vec<(String, f64)>>,
}

impl InvertedIndex {
    /// 创建空的倒排索引
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
        }
    }

    /// 从词项序列构建倒排索引
    ///
    /// 每个元组为 `(doc_id, term)`，同一文档中同一词项出现多次时词频累加。
    ///
    /// # 参数
    ///
    /// * `tokens` - 词项序列，每个元素为 `(文档 ID, 词项)` 元组
    #[allow(clippy::cast_precision_loss)]
    pub fn build_from_tokens(tokens: &[(String, String)]) -> Self {
        let mut raw: HashMap<String, HashMap<String, usize>> = HashMap::new();
        for (doc_id, term) in tokens {
            *raw.entry(term.clone())
                .or_default()
                .entry(doc_id.clone())
                .or_insert(0) += 1;
        }

        let index: HashMap<String, Vec<(String, f64)>> = raw
            .into_iter()
            .map(|(term, doc_map)| {
                let postings: Vec<(String, f64)> = doc_map
                    .into_iter()
                    .map(|(doc_id, tf)| (doc_id, tf as f64))
                    .collect();
                (term, postings)
            })
            .collect();

        Self { index }
    }

    /// 搜索包含指定词项的文档
    ///
    /// 返回 (文档 ID, 词频) 对列表，按词频降序排列。
    /// 若词项不在索引中，返回空列表。
    ///
    /// # 参数
    ///
    /// * `term` - 待搜索的词项
    ///
    /// # 返回
    ///
    /// 按词频降序排列的 (文档 ID, 词频) 列表
    pub fn search(&self, term: &str) -> Vec<(String, f64)> {
        let mut results: Vec<(String, f64)> = self
            .index
            .get(term)
            .map(|v| v.iter().map(|(k, f)| (k.clone(), *f)).collect())
            .unwrap_or_default();
        results.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        results
    }
}

impl Default for InvertedIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bm25_index_build_from_tokens() {
        let tokens = vec![
            ("doc1".to_string(), "rust".to_string()),
            ("doc1".to_string(), "programming".to_string()),
            ("doc1".to_string(), "language".to_string()),
            ("doc2".to_string(), "python".to_string()),
            ("doc2".to_string(), "programming".to_string()),
            ("doc2".to_string(), "language".to_string()),
            ("doc3".to_string(), "rust".to_string()),
            ("doc3".to_string(), "rust".to_string()),
            ("doc3".to_string(), "systems".to_string()),
        ];

        let index = Bm25Index::build_from_tokens(&tokens);

        assert_eq!(index.total_docs(), 3);
        assert_eq!(index.doc_freq("rust"), 2);
        assert_eq!(index.doc_freq("programming"), 2);
        assert_eq!(index.doc_freq("python"), 1);
        assert_eq!(index.doc_freq("systems"), 1);
        assert_eq!(index.doc_length("doc1"), 3);
        assert_eq!(index.doc_length("doc3"), 3);
        assert!((index.avg_doc_length() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bm25_score_relevant_doc_higher() {
        let tokens = vec![
            ("doc1".to_string(), "rust".to_string()),
            ("doc1".to_string(), "rust".to_string()),
            ("doc1".to_string(), "rust".to_string()),
            ("doc1".to_string(), "programming".to_string()),
            ("doc2".to_string(), "python".to_string()),
            ("doc2".to_string(), "programming".to_string()),
        ];

        let index = Bm25Index::build_from_tokens(&tokens);

        let query: Vec<String> = vec!["rust".to_string()];
        let score_doc1 = index.score(&query, "doc1");
        let score_doc2 = index.score(&query, "doc2");

        assert!(
            score_doc1 > score_doc2,
            "doc1 包含 'rust' 三次，评分应高于不包含 'rust' 的 doc2: doc1={score_doc1}, doc2={score_doc2}"
        );
        assert!(score_doc1 > 0.0);
        assert!((score_doc2 - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bm25_idf_rare_term_higher() {
        let tokens = vec![
            ("doc1".to_string(), "common".to_string()),
            ("doc1".to_string(), "rare".to_string()),
            ("doc2".to_string(), "common".to_string()),
            ("doc3".to_string(), "common".to_string()),
        ];

        let index = Bm25Index::build_from_tokens(&tokens);

        let rare_query: Vec<String> = vec!["rare".to_string()];
        let common_query: Vec<String> = vec!["common".to_string()];

        let rare_score = index.score(&rare_query, "doc1");
        let common_score = index.score(&common_query, "doc1");

        assert!(
            rare_score > common_score,
            "稀有词 'rare' 的 IDF 应高于常见词 'common'，评分应更高: rare={rare_score}, common={common_score}"
        );
    }

    #[test]
    fn test_inverted_index_search() {
        let tokens = vec![
            ("doc1".to_string(), "rust".to_string()),
            ("doc1".to_string(), "rust".to_string()),
            ("doc1".to_string(), "rust".to_string()),
            ("doc2".to_string(), "rust".to_string()),
            ("doc2".to_string(), "python".to_string()),
            ("doc3".to_string(), "python".to_string()),
            ("doc3".to_string(), "python".to_string()),
        ];

        let index = InvertedIndex::build_from_tokens(&tokens);

        let rust_results = index.search("rust");
        assert_eq!(rust_results.len(), 2);
        assert_eq!(rust_results[0].0, "doc1");
        assert!((rust_results[0].1 - 3.0).abs() < f64::EPSILON);
        assert_eq!(rust_results[1].0, "doc2");
        assert!((rust_results[1].1 - 1.0).abs() < f64::EPSILON);

        let python_results = index.search("python");
        assert_eq!(python_results.len(), 2);
        assert_eq!(python_results[0].0, "doc3");
        assert!((python_results[0].1 - 2.0).abs() < f64::EPSILON);

        let empty_results = index.search("java");
        assert!(empty_results.is_empty());
    }

    #[test]
    fn test_bm25_index_new_is_empty() {
        let index = Bm25Index::new();
        assert_eq!(index.total_docs(), 0);
        assert!((index.avg_doc_length() - 0.0).abs() < f64::EPSILON);
        assert_eq!(index.doc_freq("any"), 0);
        assert_eq!(index.doc_length("any"), 0);
    }

    #[test]
    fn test_bm25_index_default() {
        let index = Bm25Index::default();
        assert_eq!(index.total_docs(), 0);
    }

    #[test]
    fn test_bm25_score_unknown_doc() {
        let tokens = vec![("doc1".to_string(), "rust".to_string())];
        let index = Bm25Index::build_from_tokens(&tokens);
        let score = index.score(&["rust".to_string()], "nonexistent");
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bm25_score_unknown_term() {
        let tokens = vec![("doc1".to_string(), "rust".to_string())];
        let index = Bm25Index::build_from_tokens(&tokens);
        let score = index.score(&["python".to_string()], "doc1");
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bm25_build_from_empty_tokens() {
        let index = Bm25Index::build_from_tokens(&[]);
        assert_eq!(index.total_docs(), 0);
        assert!((index.avg_doc_length() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_inverted_index_new_is_empty() {
        let index = InvertedIndex::new();
        assert!(index.search("any").is_empty());
    }

    #[test]
    fn test_inverted_index_default() {
        let index = InvertedIndex::default();
        assert!(index.search("any").is_empty());
    }

    #[test]
    fn test_bm25_doc_lengths() {
        let tokens = vec![
            ("doc1".to_string(), "a".to_string()),
            ("doc1".to_string(), "b".to_string()),
            ("doc2".to_string(), "c".to_string()),
        ];
        let index = Bm25Index::build_from_tokens(&tokens);
        let lengths = index.doc_lengths();
        assert_eq!(lengths.len(), 2);
        assert_eq!(lengths.get("doc1"), Some(&2));
        assert_eq!(lengths.get("doc2"), Some(&1));
    }
}
