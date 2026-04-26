//! 基于角色的访问控制（RBAC）+ 基于属性的访问控制（ABAC）
//!
//! 实现企业级细粒度权限管理：
//! - **角色模型** (`role`): Role / Permission / ResourceType / PermissionScope 定义
//! - **角色管理器** (`manager`): 角色的 CRUD、分配、继承、权限检查
//!
//! # RBAC 模型设计
//!
//! ```text
//! User ──1:N──> UserRoleAssignment ──N:1──> Role
//!   │                                    │
//!   │                              1:N ──┘
//!   │                            RoleInheritance (parent_roles)
//!   │                                    │
//!   │                              1:N ──┘
//!   │                            Permission
//! ```
//!
//! # ABAC 集成
//!
//! 每个 Permission 可携带可选的条件表达式（Cedar CEL 兼容），
//! 在运行时结合请求上下文动态求值，实现"最小权限原则"。

pub mod role;
pub mod manager;

pub use role::*;
pub use manager::RoleManager;
