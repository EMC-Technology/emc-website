#![allow(
    clippy::result_large_err,
    clippy::empty_line_after_doc_comments,
    clippy::unreadable_literal,
    clippy::items_after_statements,
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    clippy::manual_async_fn
)]
#![warn(missing_docs, unused_imports)]
//! 知识系统核心数据模型与错误类型
//!
//! 本 crate 提供整个知识图谱系统的 foundational 类型定义：
//! - 知识节点（KnowledgeNode）、关系边（RelationEdge）等核心数据结构
//! - 使用 `error_core::ErrorObject` 构建的统一结构化错误体系
//! - 加密哈希（blake3）与对称加密（aes-gcm）工具
//! - 统一的 serde 序列化策略

#[cfg(feature = "db")]
/// 数据库客户端抽象与 SurrealDB 生产实现
pub mod database;

#[cfg(feature = "db")]
/// 数据库 Schema 版本管理与迁移
pub mod schema_manager;

#[cfg(feature = "db")]
/// 仓储层：对 Document/Block/Token/Reference 的 CRUD 封装
pub mod repository;

/// 审计日志系统（基础 + 不可篡改增强 + 合规报告）
#[cfg(feature = "db")]
pub mod audit;

/// 加密与哈希工具（blake3 + aes-gcm）
pub mod crypto;

/// 统一错误构造辅助（委托至 `error_core::helpers`）
pub mod error;

/// 向量数学工具函数（余弦相似度、欧氏距离、点积）
pub mod math;

/// 核心领域实体定义（三层节点 + 有向边）
pub mod model;

/// 搜索索引（BM25 关键词索引与倒排索引）
pub mod search;

/// 索引过期检测（blake3 哈希比较）
pub mod staleness;

/// 多仓库注册表（管理 ~/.knowledge-system/registry.json）
pub mod registry;

/// 密钥管理系统 (KMS)
#[cfg(any(feature = "kms-local", feature = "kms-vault"))]
pub mod kms;

/// PII 脱敏系统
#[cfg(feature = "pii")]
pub mod pii;

/// 事件驱动架构（Event-Driven Architecture）
#[cfg(feature = "event-driven")]
pub mod event;

/// 多级缓存架构 (L1 Moka + L2 Redis)
#[cfg(feature = "db")]
pub mod cache;

/// CQRS (Command Query Responsibility Segregation) + Event Sourcing 架构
#[cfg(feature = "db")]
pub mod cqrs;

/// 向量数据库抽象层
#[cfg(feature = "db")]
pub mod vector_store;

/// 多阶段重排序管道
#[cfg(feature = "db")]
pub mod reranker;

#[cfg(feature = "db")]
/// 安全的 SurrealDB RecordId 封装
pub mod record_id;

pub use crypto::{Decryptor, Encryptor, KeyManager, hash, hash_str};
pub use math::{cosine_similarity, dot_product, euclidean_distance};
pub use registry::{Registry, RegistryEntry};
#[cfg(feature = "db")]
pub use search::reciprocal_rank_fusion;
pub use search::{Bm25Index, HybridSearchResult, InvertedIndex, RRF_DEFAULT_K};
pub use staleness::{StalenessChecker, StalenessStatus};

#[cfg(feature = "kms-local")]
pub use kms::{
    envelope::EnvelopeEncryption, local::LocalKms, rotation::KeyRotationScheduler,
    traits::EncryptionAlgorithm, traits::KeyManagementService,
};

#[cfg(feature = "pii")]
pub use pii::PIIScanner;

#[cfg(feature = "event-driven")]
pub use event::{
    EventBus, EventBusConfig, EventError, HandleResult, Handler, KnowledgeEvent, SystemEvent,
    global_event_bus,
};

#[cfg(feature = "db")]
pub use database::{DatabaseClient, SurrealDbClient};

#[cfg(feature = "db")]
pub use schema_manager::SchemaManager;

#[cfg(feature = "db")]
pub use repository::{
    BlockRepository, CommunityRepository, DocumentRepository, KnowledgeRepository,
    ProcessRepository, ReferenceRepository, TokenRepository, deserialize_value,
    surreal_value_to_json,
};

#[cfg(feature = "db")]
pub use record_id::SafeRecordId;

#[cfg(feature = "db")]
pub use cqrs::{
    Aggregate, AggregateError, AggregateRepository, Command, CommandDispatcher, CommandHandler,
    EventStore, Query, QueryDispatcher, QueryHandler,
};

#[cfg(feature = "db")]
pub use audit::{AuditEvent, AuditLogger};

#[cfg(feature = "db")]
pub use cache::{CacheLookupResult, CacheManager, CacheStats, L1Cache, L1CacheConfig};

#[cfg(feature = "db")]
pub use vector_store::{CollectionInfo, CollectionStatus, SearchResult, VectorPoint, VectorStore};

#[cfg(feature = "db")]
pub use reranker::{
    CrossEncoderModel, Document, HybridRetriever, LLMJudger, PipelineStats, RerankedResults,
    RerankingPipeline, ScoredDocument,
};

/// 统一结果类型别名，委托至 [`error_core::Result`]
pub type Result<T> = error_core::Result<T>;

/// 形式化验证模块
///
/// 提供三种形式化验证方法的实现：
/// - **MIRI**: Rust MIR 解释器，检测未定义行为
/// - **Kani**: 模型检验工具，验证内存安全性和不变量
/// - **Proptest**: 属性测试，随机输入验证代码属性
///
/// # 启用方式
///
/// ```bash
/// # 属性测试
/// cargo test --features formal
///
/// # Kani 模型检验（需要 cargo-kani）
/// cargo kani
///
/// # MIRI（仅 Unix/Linux/macOS）
/// cargo +miri test
/// ```
#[cfg(feature = "formal")]
pub mod proofs;

/// 当前 crate 版本号，编译时从 `CARGO_PKG_VERSION` 环境变量注入
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
