//! 向量存储抽象层
//!
//! 提供统一的向量数据库接口，支持多种后端：
//! - **Qdrant** (生产环境)：高性能向量数据库，支持 gRPC、过滤、Payload 索引
//! - **HNSWLIB** (开发/测试)：内存索引，无外部依赖，适合单元测试

#[cfg(feature = "hnswlib")]
pub mod hnswlib_adapter;
#[cfg(feature = "qdrant")]
pub mod qdrant_adapter;
/// 向量存储核心特征与数据类型
pub mod store;
/// 向量搜索选项与过滤条件定义
pub mod types;

pub use store::{CollectionInfo, CollectionStatus, VectorPoint, VectorStore};
pub use types::{
    Condition, DistanceMetric, Filter, HybridQuery, ResultMetadata, SearchOptions, SearchResult,
    ValueRange,
};
