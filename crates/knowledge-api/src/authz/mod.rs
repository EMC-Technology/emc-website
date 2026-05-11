//! 零信任授权（Authorization）模块
//!
//! 基于 Cedar 策略引擎实现企业级细粒度访问控制：
//! - **策略定义** (`policy`): Cedar Policy DSL 的 Rust 类型映射与序列化
//! - **策略引擎** (`engine`): 授权决策核心，含缓存、批量评估、热更新
//! - **授权中间件** (`middleware`): Axum 集成层，自动提取身份并执行授权检查
//!
//! # 架构设计
//!
//! ```text
//! HTTP Request → mTLS 认证 → JWT 解析 → AuthZ 中间件 → Cedar 引擎评估 → Handler
//!                                         ↓
//!                                   缓存命中检查 (LRU)
//!                                         ↓
//!                              RBAC 角色解析 + ABAC 条件求值
//! ```
//!
//! # 性能目标
//! - P99 延迟 < 10ms
//! - 缓存命中率 > 95%
//! - 吞吐量 > 10K QPS

pub mod engine;
pub mod middleware;
pub mod policy;

pub use engine::AuthorizationEngine;
pub use middleware::{AuthorizationExtractor, authorization_middleware};
pub use policy::*;
