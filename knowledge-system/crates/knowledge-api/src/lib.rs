//! HTTP/WebSocket/MCP API — GEMMA4 推理、Agent、ABAC

pub mod agent;
pub mod authz;
pub mod mcp;
pub mod middleware;
pub mod observability;
pub mod rag;
pub mod rbac;

pub mod application;
pub mod auth;
pub mod candle_loader;
pub mod config;
pub mod context_generator;
pub mod dto;
pub mod embedding_factory;
pub mod embedding_model;
pub mod embedding_service;
pub mod embedding_worker;
pub mod error_handler;
pub mod event_subscriber;
pub mod gemma4_model;
pub mod gemma_embedding;
pub mod handler;
pub mod hash_embedding;
pub mod hf_downloader;
pub mod knowledge_vm;
pub mod logging_middleware;
pub mod model_factory;
pub mod model_loader;
pub mod observability_endpoints;
pub mod query_types;
pub mod router;
pub mod ws;

/// API库版本
pub const VERSION: &str = "0.1.0";

/// API库名称
pub const NAME: &str = "knowledge-api";
