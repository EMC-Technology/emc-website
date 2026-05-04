#![allow(
    clippy::result_large_err,
    clippy::empty_line_after_doc_comments,
    clippy::unreadable_literal,
    clippy::items_after_statements,
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    clippy::manual_async_fn,
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

pub use crypto::{Encryptor, Decryptor, KeyManager, hash, hash_str};
pub use search::{Bm25Index, InvertedIndex, HybridSearchResult, RRF_DEFAULT_K};
#[cfg(feature = "db")]
pub use search::reciprocal_rank_fusion;
pub use staleness::{StalenessChecker, StalenessStatus};
pub use registry::{Registry, RegistryEntry};

#[cfg(feature = "kms-local")]
pub use kms::{
    traits::KeyManagementService,
    traits::EncryptionAlgorithm,
    local::LocalKms,
    envelope::EnvelopeEncryption,
    rotation::KeyRotationScheduler,
};

#[cfg(feature = "pii")]
pub use pii::PIIScanner;

#[cfg(feature = "event-driven")]
pub use event::{
    SystemEvent, KnowledgeEvent, EventBus, EventBusConfig, EventError, global_event_bus,
    Handler, HandleResult,
};

#[cfg(feature = "db")]
pub use database::{DatabaseClient, SurrealDbClient};

#[cfg(feature = "db")]
pub use schema_manager::SchemaManager;

#[cfg(feature = "db")]
pub use repository::{
    DocumentRepository,
    BlockRepository,
    TokenRepository,
    ReferenceRepository,
    CommunityRepository,
    ProcessRepository,
    KnowledgeRepository,
    surreal_value_to_json,
    deserialize_value,
};

#[cfg(feature = "db")]
pub use record_id::SafeRecordId;

#[cfg(feature = "db")]
pub use cqrs::{
    Command, Query,
    CommandHandler, QueryHandler, CommandDispatcher, QueryDispatcher,
    EventStore, Aggregate, AggregateRepository, AggregateError,
};

#[cfg(feature = "db")]
pub use audit::{AuditLogger, AuditEvent};

#[cfg(feature = "db")]
pub use cache::{L1Cache, L1CacheConfig, CacheStats, CacheManager, CacheLookupResult};

#[cfg(feature = "db")]
pub use vector_store::{VectorStore, SearchResult, VectorPoint, CollectionInfo, CollectionStatus};

#[cfg(feature = "db")]
pub use reranker::{CrossEncoderModel, RerankingPipeline, ScoredDocument, Document, RerankedResults, PipelineStats, HybridRetriever, LLMJudger};

/// 统一结果类型别名，委托至 [`error_core::Result`]
pub type Result<T> = error_core::Result<T>;

/// 当前 crate 版本号，编译时从 `CARGO_PKG_VERSION` 环境变量注入
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
