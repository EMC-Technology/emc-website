//! RBAC 角色与权限类型定义
//!
//! 定义知识管理系统中所有资源类型的权限模型。
//! 每种资源类型支持 CRUD + 自定义动作，并可通过 `PermissionScope` 控制访问范围。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 角色
///
/// 角色是权限的集合，用户通过被分配角色来获得相应权限。
/// 支持角色继承（`parent_roles`），形成角色层次结构。
///
/// # 内置系统角色
///
/// | 角色名 | 说明 |
/// |--------|------|
/// | `super_admin` | 超级管理员，拥有所有权限（不可删除） |
/// | `admin` | 组织管理员，可管理组织内资源和用户 |
/// | `editor` | 编辑者，可创建和编辑文档/节点 |
/// | `viewer` | 查看者，仅可读取 |
/// | `agent_operator` | Agent 操作员，可执行 AI Agent 任务 |
/// | `mcp_user` | MCP 工具使用者，可调用已授权的工具 |
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    /// 角色唯一标识
    pub id: Uuid,
    /// 角色名称（系统唯一，如 "admin"、"editor"）
    pub name: String,
    /// 显示名称（人类可读）
    pub display_name: String,
    /// 角色描述
    pub description: String,
    /// 该角色关联的所有权限
    pub permissions: Vec<Permission>,
    /// 父角色 ID 列表（角色继承：自动获得父角色的所有权限）
    pub parent_roles: Vec<Uuid>,
    /// 是否为系统内置角色（系统角色不可删除或修改核心属性）
    pub is_system: bool,
    /// 创建时间
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: DateTime<Utc>,
}

impl Role {
    /// 获取角色的所有有效权限（含继承自父角色的权限）
    ///
    /// 需要配合 `RoleStore` 的 `get_role_with_parents` 方法使用，
    /// 以递归展开完整的角色继承链。
    #[must_use]
    pub fn effective_permissions(&self, inherited: &[Permission]) -> Vec<Permission> {
        let mut perms: std::collections::HashSet<Uuid> =
            self.permissions.iter().map(|p| p.id).collect();
        for p in inherited {
            perms.insert(p.id);
        }
        // 保留原始顺序（自身权限优先于继承权限）
        let mut result: Vec<Permission> = self
            .permissions
            .iter()
            .filter(|p| perms.contains(&p.id))
            .cloned()
            .collect();
        for p in inherited {
            if !result.iter().any(|existing| existing.id == p.id) {
                result.push(p.clone());
            }
        }
        result
    }

    /// 检查是否拥有对指定资源类型和动作的权限
    #[must_use]
    pub fn has_permission_for(
        &self,
        resource_type: &ResourceType,
        action: &str,
        inherited: &[Permission],
    ) -> bool {
        let effective = self.effective_permissions(inherited);
        effective.iter().any(|p| {
            &p.resource_type == resource_type && p.actions.iter().any(|a| a == action)
        })
    }
}

/// 权限
///
/// 定义对一个或多个资源类型执行特定操作的许可。
/// 可选携带 ABAC 条件表达式以实现更细粒度的控制。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    /// 权限唯一标识
    pub id: Uuid,
    /// 目标资源类型
    pub resource_type: ResourceType,
    /// 允许的动作列表（如 `["create", "read", "update", "delete"]`）
    pub actions: Vec<String>,
    /// ABAC 条件表达式（Cedar CEL 兼容语法，可选）
    ///
    /// 当条件存在时，仅当请求上下文满足该条件时才授予此权限。
    /// 示例: `"resource.owner_id == principal.id"`
    pub conditions: Option<String>,
    /// 权限范围（限制可操作的资源集合）
    pub scope: PermissionScope,
}

/// 资源类型枚举
///
/// 知识管理系统中的所有受保护资源类型。
/// 每种资源类型都有其独立的权限命名空间。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceType {
    /// 文档（Document）
    Document,
    /// 知识节点（Node）
    Node,
    /// 关系边（Edge）
    Edge,
    /// 用户账户（User）
    User,
    /// 角色（Role）
    Role,
    /// 系统配置（System）
    System,
    /// MCP 工具（Tool）
    Tool,
    /// AI Agent 任务（Agent）
    Agent,
    /// 工作流（Workflow）
    Workflow,
    /// RAG 引擎配置（RagConfig）
    RagConfig,
    /// 向量索引（VectorIndex）
    VectorIndex,
    /// 审计日志（AuditLog）
    AuditLog,
    /// API 密钥（ApiKey)
    ApiKey,
}

impl ResourceType {
    /// 获取资源类型的标准动作集
    #[must_use]
    pub const fn standard_actions(&self) -> &'static [&'static str] {
        match self {
            Self::Document => &["create", "read", "update", "delete", "export"],
            Self::Node => &["create", "read", "update", "delete", "move", "link"],
            Self::Edge => &["create", "read", "delete"],
            Self::User => &["create", "read", "update", "delete", "manage_roles"],
            Self::Role => &["create", "read", "update", "delete", "assign"],
            Self::System => &["read", "update", "configure"],
            Self::Tool => &["register", "deregister", "invoke", "configure"],
            Self::Agent => &["create", "read", "execute", "stop", "delete"],
            Self::Workflow => &["create", "read", "update", "delete", "execute", "pause"],
            Self::RagConfig => &["read", "update", "rebuild_index"],
            Self::VectorIndex => &["create", "read", "delete", "query"],
            Self::AuditLog => &["read", "export"],
            Self::ApiKey => &["create", "read", "revoke", "rotate"],
        }
    }

    /// 资源类型的显示名称
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Document => "文档",
            Self::Node => "知识节点",
            Self::Edge => "关系边",
            Self::User => "用户",
            Self::Role => "角色",
            Self::System => "系统",
            Self::Tool => "MCP 工具",
            Self::Agent => "AI Agent",
            Self::Workflow => "工作流",
            Self::RagConfig => "RAG 配置",
            Self::VectorIndex => "向量索引",
            Self::AuditLog => "审计日志",
            Self::ApiKey => "API 密钥",
        }
    }
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// 权限范围
///
/// 定义权限适用的资源边界，实现多租户场景下的数据隔离。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum PermissionScope {
    /// 仅自己的资源（最严格）
    Own,
    /// 同一团队内的资源
    Team,
    /// 同一组织内的资源
    Organization,
    /// 所有资源（全局，需谨慎分配）
    Global,
    /// 自定义范围（指定一组范围 ID）
    Custom(Vec<String>),
}

impl PermissionScope {
    /// 检查给定资源是否在权限范围内
    #[must_use]
    pub fn contains_resource(
        &self,
        owner_id: &str,
        principal_id: &str,
        team_ids: &[String],
        org_id: &str,
    ) -> bool {
        match self {
            Self::Own => owner_id == principal_id,
            Self::Team => team_ids.iter().any(|tid| {
                tid.as_str() == org_id
            }),
            Self::Organization | Self::Global => true,
            Self::Custom(scope_ids) => scope_ids.iter().any(|sid| {
                sid.as_str() == org_id || sid.as_str() == owner_id
            }),
        }
    }
}

/// 用户-角色分配关系
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRoleAssignment {
    /// 分配记录 ID
    pub id: Uuid,
    /// 用户 ID
    pub user_id: String,
    /// 角色 ID
    pub role_id: Uuid,
    /// 分配来源（手动 / 自动 / 继承）
    pub assignment_source: AssignmentSource,
    /// 分配人（手动分配时记录）
    pub assigned_by: Option<String>,
    /// 过期时间（可选，用于临时授权）
    pub expires_at: Option<DateTime<Utc>>,
    /// 分配时间
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
}

/// 角色分配来源
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssignmentSource {
    /// 管理员手动分配
    Manual,
    /// 基于规则自动分配（如部门自动获得对应角色）
    AutoRule,
    /// 从父角色继承
    Inherited,
    /// 通过 API 密钥获得
    ApiKey,
}
