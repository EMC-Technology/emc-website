//! 核心领域逻辑 — 检索、重排序、安全、缓存、CQRS

pub mod audit;
pub mod cache;
pub mod cqrs;
pub mod event;
pub mod kms;
pub mod pii;
pub mod reranker;
pub mod search;
pub mod vector_store;

pub mod crypto;
pub mod database;
pub mod error;
pub mod model;
pub mod record_id;
pub mod registry;
pub mod repository;
pub mod schema_manager;
pub mod staleness;

/// 核心领域逻辑库版本
pub const VERSION: &str = "0.1.0";

/// 核心领域逻辑库名称
pub const NAME: &str = "knowledge-core";
