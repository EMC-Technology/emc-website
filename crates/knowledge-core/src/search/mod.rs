/// 搜索索引模块
///
/// 提供 BM25 关键词索引、倒排索引与混合搜索融合（RRF），
/// 用于文档检索与相关性排序。
pub mod bm25;
/// 混合搜索模块
///
/// 结合关键词搜索与向量搜索的结果，通过倒数排名融合（RRF）
/// 算法合并为统一的排序结果。
pub mod hybrid;

pub use bm25::{Bm25Index, InvertedIndex};
#[cfg(feature = "db")]
pub use hybrid::reciprocal_rank_fusion;
pub use hybrid::{HybridSearchResult, RRF_DEFAULT_K};
