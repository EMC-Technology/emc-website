/// 向量存储抽象 trait
///
/// 定义统一的向量数据库操作接口，支持多种后端实现。

use crate::vector_store::types::{DistanceMetric, HybridQuery, SearchOptions, SearchResult};
use error_core::Result;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

/// 向量存储抽象 trait
///
/// 定义统一的向量数据库操作接口，支持多种后端实现。
/// 所有方法均为异步，确保在 tokio 运行时中安全使用。
#[async_trait::async_trait]
pub trait VectorStore: Send + Sync {
    /// 初始化向量集合（创建 collection）
    ///
    /// # Errors
    /// 集合已存在或后端连接失败时返回错误
    async fn init_collection(&self, name: &str, dimension: usize, distance: DistanceMetric) -> Result<()>;

    /// 插入或更新向量点
    ///
    /// # Errors
    /// 后端连接失败或数据格式不匹配时返回错误
    async fn upsert(&self, collection: &str, points: Vec<VectorPoint>) -> Result<Vec<Uuid>>;

    /// 向量相似度搜索
    ///
    /// # Errors
    /// 集合不存在或后端连接失败时返回错误
    async fn similarity_search(&self, collection: &str, query_vector: &[f32], options: SearchOptions) -> Result<Vec<SearchResult>>;

    /// 混合搜索（向量 + BM25 文本）
    ///
    /// # Errors
    /// 集合不存在或后端连接失败时返回错误
    async fn hybrid_search(&self, collection: &str, query: &HybridQuery) -> Result<Vec<SearchResult>>;

    /// 按 ID 批量获取向量点
    ///
    /// # Errors
    /// 后端连接失败时返回错误
    async fn get_by_ids(&self, collection: &str, ids: &[Uuid]) -> Result<Vec<SearchResult>>;

    /// 按 ID 批量删除向量点
    ///
    /// # Errors
    /// 后端连接失败时返回错误
    async fn delete(&self, collection: &str, ids: &[Uuid]) -> Result<()>;

    /// 获取集合元信息
    ///
    /// # Errors
    /// 集合不存在或后端连接失败时返回错误
    async fn collection_info(&self, collection: &str) -> Result<CollectionInfo>;

    /// 关闭连接并释放资源
    ///
    /// # Errors
    /// 后端连接关闭失败时返回错误
    async fn close(&self) -> Result<()>;
}

/// 向量点（向量数据库中的基本存储单元）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorPoint {
    /// 唯一标识符
    pub id: Uuid,
    /// 嵌入向量
    pub vector: Vec<f32>,
    /// 附加元数据
    pub payload: serde_json::Value,
}

impl VectorPoint {
    /// 创建新的向量点
    #[must_use]
    pub const fn new(id: Uuid, vector: Vec<f32>, payload: serde_json::Value) -> Self {
        Self { id, vector, payload }
    }

    /// 获取向量维度
    #[must_use]
    pub fn dimension(&self) -> usize { self.vector.len() }
}

/// 集合信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    /// 集合名称
    pub name: String,
    /// 向量数量
    pub vectors_count: u64,
    /// 向量维度
    pub dimension: usize,
    /// 集合运行状态
    pub status: CollectionStatus,
}

/// 集合状态枚举
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CollectionStatus {
    /// 健康
    Green,
    /// 降级
    Yellow,
    /// 不可用
    Red,
}

impl std::fmt::Display for CollectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Green => write!(f, "healthy"),
            Self::Yellow => write!(f, "degraded"),
            Self::Red => write!(f, "unavailable"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_point_creation() {
        let point = VectorPoint::new(Uuid::new_v4(), vec![0.1; 3], serde_json::json!({}));
        assert_eq!(point.dimension(), 3);
    }

    #[test]
    fn test_collection_status_display() {
        assert_eq!(CollectionStatus::Green.to_string(), "healthy");
        assert_eq!(CollectionStatus::Red.to_string(), "unavailable");
    }
}
