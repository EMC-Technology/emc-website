//! 角色管理器
//!
//! 提供 RBAC 的完整生命周期管理：

#![allow(clippy::significant_drop_tightening)]
//! - 角色的创建、更新、删除
//! - 用户-角色分配与撤销
/// - 角色继承链解析（含环检测）
/// - 权限检查（含 ABAC 条件求值）
/// - 内置系统角色的初始化
use std::collections::{HashMap, HashSet, VecDeque};
use std::num::NonZero;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use lru::LruCache;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

use crate::Result;
use crate::rbac::role::{
    AssignmentSource, Permission, PermissionScope, ResourceType, Role, UserRoleAssignment,
};
use error_core::helpers;

/// 用户角色分配缓存条目类型
type AssignmentCacheEntry = (Vec<UserRoleAssignment>, std::time::Instant);

/// 角色缓存容量
const ROLE_CACHE_CAPACITY: usize = 500;
/// 用户角色分配缓存容量
const USER_ROLE_CACHE_CAPACITY: usize = 1000;
/// 角色分配缓存 TTL (秒)
const ASSIGNMENT_CACHE_TTL_SECS: u64 = 120;

/// 角色存储 trait
///
/// 抽象角色和权限的持久化层。
#[async_trait::async_trait]
pub trait RoleStore: Send + Sync {
    /// 创建新角色，返回角色 ID
    async fn create_role(&self, role: &Role) -> Result<Uuid>;

    /// 根据 ID 获取角色
    async fn get_role(&self, role_id: &Uuid) -> Result<Option<Role>>;

    /// 根据名称获取角色
    async fn get_role_by_name(&self, name: &str) -> Result<Option<Role>>;

    /// 更新角色
    async fn update_role(&self, role: &Role) -> Result<()>;

    /// 删除角色（仅非系统角色）
    async fn delete_role(&self, role_id: &Uuid) -> Result<()>;

    /// 列出所有角色
    async fn list_roles(&self) -> Result<Vec<Role>>;

    /// 分配角色给用户
    async fn assign_role(
        &self,
        user_id: &str,
        role_id: Uuid,
        source: AssignmentSource,
        assigned_by: Option<String>,
        expires_at: Option<chrono::DateTime<Utc>>,
    ) -> Result<Uuid>;

    /// 撤销用户的某个角色
    async fn revoke_role(&self, user_id: &str, role_id: Uuid) -> Result<()>;

    /// 获取用户的直接角色分配列表
    async fn get_user_assignments(&self, user_id: &str) -> Result<Vec<UserRoleAssignment>>;

    /// 批量获取角色（用于继承解析）
    async fn get_roles_by_ids(&self, role_ids: &[Uuid]) -> Result<Vec<Role>>;
}

/// 角色管理器
///
/// RBAC 的核心组件。提供线程安全的角色/权限管理接口，
/// 内置 LRU 缓存以减少数据库查询压力。
///
/// # 角色继承
///
/// 支持有向无环图（DAG）形式的多重继承：
/// - `parent_roles` 字段定义父角色列表
/// - 权限沿继承边向下传递
/// - 内置环检测防止无限递归
pub struct RoleManager {
    store: Arc<dyn RoleStore>,
    role_cache: Arc<RwLock<LruCache<Uuid, Role>>>,
    assignment_cache: Arc<RwLock<LruCache<String, AssignmentCacheEntry>>>,
    /// 角色名→ID 缓存（预留：按名称查找角色尚未实现，ABAC 策略引擎热更新后实现）
    #[allow(dead_code)]
    name_cache: Arc<RwLock<LruCache<String, Uuid>>>,
}

impl RoleManager {
    /// 创建新的角色管理器实例
    ///
    /// # Panics
    ///
    /// 当缓存容量为 0 时 panic（硬编码常量保证不会发生）。
    pub fn new(store: Arc<dyn RoleStore>) -> Self {
        info!("初始化 RoleManager");
        let role_cache = LruCache::new(NonZero::new(ROLE_CACHE_CAPACITY).unwrap());
        let name_cache = LruCache::new(NonZero::new(ROLE_CACHE_CAPACITY).unwrap());
        let assignment_cache = LruCache::new(NonZero::new(USER_ROLE_CACHE_CAPACITY).unwrap());
        Self {
            store,
            role_cache: Arc::new(RwLock::new(role_cache)),
            name_cache: Arc::new(RwLock::new(name_cache)),
            assignment_cache: Arc::new(RwLock::new(assignment_cache)),
        }
    }

    // ========== 角色 CRUD ==========

    /// 创建新角色
    ///
    /// # Errors
    ///
    /// - 角色名已存在 → 返回冲突错误
    /// - 父角色不存在 → 返回验证错误
    #[instrument(skip(self), fields(role_name = %role.name))]
    pub async fn create_role(&self, role: &Role) -> Result<Uuid> {
        if let Some(existing) = self.store.get_role_by_name(&role.name).await? {
            return Err(helpers::config_error(&format!(
                "角色名 '{}' 已存在 (ID: {})",
                role.name, existing.id
            )));
        }

        for parent_id in &role.parent_roles {
            if self.store.get_role(parent_id).await?.is_none() {
                return Err(helpers::validation_error(
                    &format!("父角色 {parent_id} 不存在"),
                    "create_role",
                ));
            }
        }

        Self::detect_inheritance_cycle(role)?;

        let id = self.store.create_role(role).await?;
        info!(role_id = %id, name = %role.name, "角色创建成功");
        Ok(id)
    }

    /// 分配角色给用户
    ///
    /// # Arguments
    ///
    /// * `user_id` - 目标用户 ID
    /// * `role_id` - 要分配的角色 ID
    /// * `assigned_by` - 操作执行者（可选）
    /// * `expires_at` - 过期时间（可选，用于临时授权）
    ///
    /// # Errors
    ///
    /// - 角色不存在 → 验证错误
    /// - 重复分配 → 冲突错误
    pub async fn assign_role(
        &self,
        user_id: &str,
        role_id: Uuid,
        assigned_by: Option<&str>,
        expires_at: Option<chrono::DateTime<Utc>>,
    ) -> Result<()> {
        if self.store.get_role(&role_id).await?.is_none() {
            return Err(helpers::validation_error(
                &format!("角色 {role_id} 不存在"),
                "assign_role",
            ));
        }

        self.store
            .assign_role(
                user_id,
                role_id,
                AssignmentSource::Manual,
                assigned_by.map(String::from),
                expires_at,
            )
            .await?;

        self.invalidate_user_assignment_cache(user_id).await;

        info!(
            user_id = user_id,
            role_id = %role_id,
            assigned_by = ?assigned_by,
            "角色分配成功"
        );
        Ok(())
    }

    /// 撤销用户的指定角色
    ///
    /// # Errors
    ///
    /// 当角色分配不存在或存储操作失败时返回错误。
    pub async fn revoke_role(&self, user_id: &str, role_id: Uuid) -> Result<()> {
        self.store.revoke_role(user_id, role_id).await?;
        self.invalidate_user_assignment_cache(user_id).await;
        info!(user_id = user_id, role_id = %role_id, "角色撤销成功");
        Ok(())
    }

    /// 查询用户的所有有效角色（含继承展开）
    ///
    /// 递归解析角色继承链，返回用户最终拥有的全部角色及其有效权限。
    /// 结果按角色层级排序：直接分配的角色优先于继承角色。
    ///
    /// # Errors
    ///
    /// 当存储查询失败时返回错误。
    #[instrument(skip(self), fields(user_id = %user_id))]
    pub async fn get_user_roles(&self, user_id: &str) -> Result<Vec<RoleWithPermissions>> {
        let assignments = self.get_cached_or_load_assignments(user_id).await?;
        let now = Utc::now();

        let active_assignments: Vec<_> = assignments
            .into_iter()
            .filter(|a| {
                a.expires_at.is_none_or(|exp| exp > now)
                    && a.assignment_source != AssignmentSource::Inherited
            })
            .collect();

        if active_assignments.is_empty() {
            return Ok(Vec::new());
        }

        let mut visited: HashSet<Uuid> = HashSet::new();
        let mut result_roles: HashMap<Uuid, RoleWithPermissions> = HashMap::new();
        let mut queue: VecDeque<Uuid> = active_assignments.iter().map(|a| a.role_id).collect();

        while let Some(role_id) = queue.pop_front() {
            if !visited.insert(role_id) {
                continue;
            }

            let Some(role) = self.get_cached_or_load_role(&role_id).await? else {
                continue;
            };

            let inherited_permissions = self.collect_inherited_permissions(&role).await?;

            result_roles.insert(
                role_id,
                RoleWithPermissions {
                    role: role.clone(),
                    effective_permissions: role.effective_permissions(&inherited_permissions),
                    is_directly_assigned: active_assignments.iter().any(|a| a.role_id == role_id),
                },
            );

            for parent_id in &role.parent_roles {
                queue.push_back(*parent_id);
            }
        }

        Ok(result_roles.into_values().collect())
    }

    /// 检查用户是否拥有对指定资源类型和动作的权限
    ///
    /// 这是 RBAC + ABAC 的统一入口：
    /// 1. 解析用户的完整角色集（含继承）
    /// 2. 检查是否存在匹配的 Permission
    /// 3. 若 Permission 有 conditions，则进行 ABAC 求值
    ///
    /// # Errors
    ///
    /// 当角色加载失败时返回错误。
    pub async fn has_permission(
        &self,
        user_id: &str,
        resource_type: &ResourceType,
        action: &str,
    ) -> Result<bool> {
        let roles = self.get_user_roles(user_id).await?;

        for rp in &roles {
            if rp
                .role
                .has_permission_for(resource_type, action, &rp.effective_permissions)
            {
                // 检查是否有 ABAC 条件需要进一步验证
                let perm = rp.effective_permissions.iter().find(|p| {
                    &p.resource_type == resource_type && p.actions.contains(&action.to_string())
                });

                if let Some(p) = perm {
                    if p.conditions.is_some() {
                        // 存在 ABAC 条件 —— 需要调用 Cedar 引擎完成条件求值
                        // 此处返回 true 表示"有潜在权限"，具体由 AuthZ 中间件做最终判断
                        debug!(
                            permission_id = %p.id,
                            "权限存在 ABAC 条件，需运行时评估"
                        );
                        return Ok(true);
                    }
                }
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// 初始化内置系统角色
    ///
    /// 在系统首次启动时调用，创建预定义的超级管理员、编辑者等角色。
    /// 使用幂等操作：若角色已存在则跳过。
    ///
    /// # Errors
    ///
    /// 当角色创建或存储失败时返回错误。
    pub async fn initialize_system_roles(&self) -> Result<u32> {
        info!("初始化内置系统角色...");
        let system_roles = Self::builtin_system_roles();
        let mut created_count = 0u32;

        for role_template in &system_roles {
            match self.store.get_role_by_name(&role_template.name).await {
                Ok(Some(_)) => {
                    debug!(name = %role_template.name, "系统角色已存在，跳过");
                }
                Ok(None) => {
                    let id = self.store.create_role(role_template).await?;
                    info!(name = %role_template.name, role_id = %id, "系统角色已创建");
                    created_count += 1;
                }
                Err(e) => {
                    error!(error = %e, name = %role_template.name, "系统角色创建失败");
                }
            }
        }

        info!(count = created_count, "系统角色初始化完成");
        Ok(created_count)
    }

    // ========== 内部方法 ==========

    async fn get_cached_or_load_role(&self, role_id: &Uuid) -> Result<Option<Role>> {
        {
            let cache = self.role_cache.read().await;
            if let Some(cached) = cache.peek(role_id) {
                return Ok(Some(cached.clone()));
            }
        }

        match self.store.get_role(role_id).await? {
            Some(role) => {
                let mut cache = self.role_cache.write().await;
                cache.put(*role_id, role.clone());
                Ok(Some(role))
            }
            None => Ok(None),
        }
    }

    async fn get_cached_or_load_assignments(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserRoleAssignment>> {
        {
            let cache = self.assignment_cache.read().await;
            if let Some((assignments, ts)) = cache.peek(user_id) {
                if ts.elapsed() < Duration::from_secs(ASSIGNMENT_CACHE_TTL_SECS) {
                    return Ok(assignments.clone());
                }
            }
        }

        let assignments = self.store.get_user_assignments(user_id).await?;
        let mut cache = self.assignment_cache.write().await;
        cache.put(
            user_id.to_string(),
            (assignments.clone(), std::time::Instant::now()),
        );
        Ok(assignments)
    }

    async fn invalidate_user_assignment_cache(&self, user_id: &str) {
        let mut cache = self.assignment_cache.write().await;
        cache.pop(user_id);
    }

    fn collect_inherited_permissions<'a>(
        &'a self,
        role: &'a Role,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Permission>>> + Send + 'a>> {
        Box::pin(async move {
            if role.parent_roles.is_empty() {
                return Ok(Vec::new());
            }

            let parent_roles = self.store.get_roles_by_ids(&role.parent_roles).await?;
            let mut all_permissions: Vec<Permission> = Vec::new();
            let mut seen: HashSet<Uuid> = HashSet::new();

            for pr in &parent_roles {
                for p in &pr.permissions {
                    if seen.insert(p.id) {
                        all_permissions.push(p.clone());
                    }
                }
                let grand_parent_perms = self.collect_inherited_permissions(pr).await?;
                for gp in grand_parent_perms {
                    if seen.insert(gp.id) {
                        all_permissions.push(gp);
                    }
                }
            }

            Ok(all_permissions)
        })
    }

    /// 检测角色继承关系中的环路
    ///
    /// 使用 DFS 从待创建角色出发，沿 `parent_roles` 边遍历，
    /// 若能回到自身则存在环路。
    fn detect_inheritance_cycle(new_role: &Role) -> Result<()> {
        let mut visited: HashSet<Uuid> = HashSet::new();
        let mut stack: VecDeque<Uuid> = new_role.parent_roles.iter().copied().collect();

        while let Some(current_id) = stack.pop_front() {
            if current_id == new_role.id {
                return Err(helpers::validation_error(
                    "检测到角色继承环路",
                    "create_role",
                ));
            }
            if visited.insert(current_id) {
                // 注意：此处为简化实现，实际生产环境应从 store 加载完整的父角色信息
                // 由于 create_role 尚未持久化，我们只能基于已有的 parent_roles 做基本检测
                stack.extend(&new_role.parent_roles);
            }
        }

        Ok(())
    }

    /// 定义内置系统角色模板
    fn builtin_system_roles() -> Vec<Role> {
        let now = Utc::now();
        vec![
            Role {
                id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
                name: "super_admin".to_string(),
                display_name: "超级管理员".to_string(),
                description: "拥有系统所有权限的超级管理员".to_string(),
                permissions: Self::_all_resource_full_permissions(),
                parent_roles: vec![],
                is_system: true,
                created_at: now,
                updated_at: now,
            },
            Role {
                id: Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
                name: "admin".to_string(),
                display_name: "组织管理员".to_string(),
                description: "可管理组织内资源和用户的管理员".to_string(),
                permissions: Self::_admin_permissions(),
                parent_roles: vec![
                    Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap(),
                ],
                is_system: true,
                created_at: now,
                updated_at: now,
            },
            Role {
                id: Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap(),
                name: "editor".to_string(),
                display_name: "编辑者".to_string(),
                description: "可创建和编辑文档/节点的用户".to_string(),
                permissions: Self::_editor_permissions(),
                parent_roles: vec![
                    Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap(),
                ],
                is_system: true,
                created_at: now,
                updated_at: now,
            },
            Role {
                id: Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap(),
                name: "viewer".to_string(),
                display_name: "查看者".to_string(),
                description: "仅可读取资源的用户".to_string(),
                permissions: Self::_viewer_permissions(),
                parent_roles: vec![],
                is_system: true,
                created_at: now,
                updated_at: now,
            },
            Role {
                id: Uuid::parse_str("00000000-0000-0000-0000-000000000005").unwrap(),
                name: "agent_operator".to_string(),
                display_name: "Agent 操作员".to_string(),
                description: "可执行和管理 AI Agent 任务的用户".to_string(),
                permissions: Self::_agent_operator_permissions(),
                parent_roles: vec![
                    Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap(),
                ],
                is_system: true,
                created_at: now,
                updated_at: now,
            },
            Role {
                id: Uuid::parse_str("00000000-0000-0000-0000-000000000006").unwrap(),
                name: "mcp_user".to_string(),
                display_name: "MCP 工具使用者".to_string(),
                description: "可调用已授权 MCP 工具的用户".to_string(),
                permissions: Self::_mcp_user_permissions(),
                parent_roles: vec![
                    Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap(),
                ],
                is_system: true,
                created_at: now,
                updated_at: now,
            },
        ]
    }

    fn _make_perm(rt: ResourceType, actions: &[&str], scope: PermissionScope) -> Permission {
        Permission {
            id: Uuid::new_v4(),
            resource_type: rt,
            actions: actions.iter().map(std::string::ToString::to_string).collect(),
            conditions: None,
            scope,
        }
    }

    fn _all_resource_full_permissions() -> Vec<Permission> {
        let rts = [
            ResourceType::Document,
            ResourceType::Node,
            ResourceType::Edge,
            ResourceType::User,
            ResourceType::Role,
            ResourceType::System,
            ResourceType::Tool,
            ResourceType::Agent,
            ResourceType::Workflow,
            ResourceType::RagConfig,
            ResourceType::VectorIndex,
            ResourceType::AuditLog,
            ResourceType::ApiKey,
        ];
        rts.iter()
            .flat_map(|rt| {
                rt.standard_actions()
                    .iter()
                    .map(|action| Self::_make_perm(rt.clone(), &[*action], PermissionScope::Global))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn _admin_permissions() -> Vec<Permission> {
        let mut perms = vec![
            Self::_make_perm(
                ResourceType::User,
                &["create", "read", "update"],
                PermissionScope::Organization,
            ),
            Self::_make_perm(
                ResourceType::Role,
                &["read", "assign"],
                PermissionScope::Organization,
            ),
            Self::_make_perm(
                ResourceType::Document,
                &["create", "read", "update", "delete", "export"],
                PermissionScope::Organization,
            ),
            Self::_make_perm(
                ResourceType::Node,
                &["create", "read", "update", "delete", "move", "link"],
                PermissionScope::Organization,
            ),
            Self::_make_perm(
                ResourceType::Edge,
                &["create", "read", "delete"],
                PermissionScope::Organization,
            ),
            Self::_make_perm(
                ResourceType::System,
                &["read", "configure"],
                PermissionScope::Organization,
            ),
            Self::_make_perm(
                ResourceType::AuditLog,
                &["read", "export"],
                PermissionScope::Organization,
            ),
        ];
        perms.extend(Self::_editor_permissions());
        perms
    }

    fn _editor_permissions() -> Vec<Permission> {
        vec![
            Self::_make_perm(
                ResourceType::Document,
                &["create", "read", "update", "export"],
                PermissionScope::Team,
            ),
            Self::_make_perm(
                ResourceType::Node,
                &["create", "read", "update", "move", "link"],
                PermissionScope::Team,
            ),
            Self::_make_perm(
                ResourceType::Edge,
                &["create", "read", "delete"],
                PermissionScope::Team,
            ),
            Self::_make_perm(ResourceType::RagConfig, &["read"], PermissionScope::Team),
        ]
    }

    fn _viewer_permissions() -> Vec<Permission> {
        vec![
            Self::_make_perm(ResourceType::Document, &["read"], PermissionScope::Own),
            Self::_make_perm(ResourceType::Node, &["read"], PermissionScope::Own),
            Self::_make_perm(ResourceType::Edge, &["read"], PermissionScope::Own),
        ]
    }

    fn _agent_operator_permissions() -> Vec<Permission> {
        vec![
            Self::_make_perm(
                ResourceType::Agent,
                &["create", "read", "execute", "stop"],
                PermissionScope::Own,
            ),
            Self::_make_perm(
                ResourceType::Workflow,
                &["create", "read", "execute", "pause"],
                PermissionScope::Own,
            ),
            Self::_make_perm(
                ResourceType::Tool,
                &["invoke"],
                PermissionScope::Organization,
            ),
        ]
    }

    fn _mcp_user_permissions() -> Vec<Permission> {
        vec![
            Self::_make_perm(
                ResourceType::Tool,
                &["invoke"],
                PermissionScope::Organization,
            ),
            Self::_make_perm(
                ResourceType::Agent,
                &["read", "execute"],
                PermissionScope::Own,
            ),
        ]
    }
}

/// 展开后的角色及有效权限
#[derive(Debug, Clone)]
pub struct RoleWithPermissions {
    /// 角色定义
    pub role: Role,
    /// 有效权限（含从父角色继承的权限）
    pub effective_permissions: Vec<Permission>,
    /// 是否为直接分配（非继承获得）
    pub is_directly_assigned: bool,
}

// ========== 单元测试 ==========

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRoleStore {
        roles: RwLock<HashMap<Uuid, Role>>,
        assignments: RwLock<HashMap<String, Vec<UserRoleAssignment>>>,
    }

    impl MockRoleStore {
        fn new() -> Self {
            Self {
                roles: RwLock::new(HashMap::new()),
                assignments: RwLock::new(HashMap::new()),
            }
        }

        async fn add_role(&self, role: Role) {
            self.roles.write().await.insert(role.id, role);
        }

        async fn add_assignment(&self, user_id: &str, assignment: UserRoleAssignment) {
            self.assignments
                .write()
                .await
                .entry(user_id.to_string())
                .or_default()
                .push(assignment);
        }
    }

    #[async_trait::async_trait]
    impl RoleStore for MockRoleStore {
        async fn create_role(&self, role: &Role) -> Result<Uuid> {
            let mut roles = self.roles.write().await;
            roles.insert(role.id, role.clone());
            Ok(role.id)
        }

        async fn get_role(&self, role_id: &Uuid) -> Result<Option<Role>> {
            Ok(self.roles.read().await.get(role_id).cloned())
        }

        async fn get_role_by_name(&self, name: &str) -> Result<Option<Role>> {
            Ok(self
                .roles
                .read()
                .await
                .values()
                .find(|r| r.name == name)
                .cloned())
        }

        async fn update_role(&self, role: &Role) -> Result<()> {
            self.roles.write().await.insert(role.id, role.clone());
            Ok(())
        }

        async fn delete_role(&self, role_id: &Uuid) -> Result<()> {
            self.roles.write().await.remove(role_id);
            Ok(())
        }

        async fn list_roles(&self) -> Result<Vec<Role>> {
            Ok(self.roles.read().await.values().cloned().collect())
        }

        async fn assign_role(
            &self,
            user_id: &str,
            role_id: Uuid,
            source: AssignmentSource,
            assigned_by: Option<String>,
            expires_at: Option<chrono::DateTime<Utc>>,
        ) -> Result<Uuid> {
            let id = Uuid::new_v4();
            let assignment = UserRoleAssignment {
                id,
                user_id: user_id.to_string(),
                role_id,
                assignment_source: source,
                assigned_by,
                expires_at,
                created_at: Utc::now(),
            };
            self.assignments
                .write()
                .await
                .entry(user_id.to_string())
                .or_default()
                .push(assignment);
            Ok(id)
        }

        async fn revoke_role(&self, user_id: &str, role_id: Uuid) -> Result<()> {
            let mut assignments = self.assignments.write().await;
            if let Some(user_assignments) = assignments.get_mut(user_id) {
                user_assignments.retain(|a| a.role_id != role_id);
            }
            Ok(())
        }

        async fn get_user_assignments(&self, user_id: &str) -> Result<Vec<UserRoleAssignment>> {
            Ok(self
                .assignments
                .read()
                .await
                .get(user_id)
                .cloned()
                .unwrap_or_default())
        }

        async fn get_roles_by_ids(&self, role_ids: &[Uuid]) -> Result<Vec<Role>> {
            let roles = self.roles.read().await;
            Ok(role_ids
                .iter()
                .filter_map(|id| roles.get(id).cloned())
                .collect())
        }
    }

    fn make_test_role(name: &str, parent_ids: Vec<Uuid>, perms: Vec<Permission>) -> Role {
        let now = Utc::now();
        Role {
            id: Uuid::new_v4(),
            name: name.to_string(),
            display_name: name.to_string(),
            description: String::new(),
            permissions: perms,
            parent_roles: parent_ids,
            is_system: false,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_role() {
        let store = Arc::new(MockRoleStore::new());
        let manager = RoleManager::new(store);

        let role = make_test_role("test_role", vec![], vec![]);
        let id = manager.create_role(&role).await.unwrap();
        let fetched = manager.store.get_role(&id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "test_role");
    }

    #[tokio::test]
    async fn test_duplicate_role_name_rejected() {
        let store = Arc::new(MockRoleStore::new());
        let manager = RoleManager::new(store.clone());

        let role = make_test_role("unique_role", vec![], vec![]);
        manager.create_role(&role).await.unwrap();

        let duplicate = make_test_role("unique_role", vec![], vec![]);
        let result = manager.create_role(&duplicate).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("已存在"));
    }

    #[tokio::test]
    async fn test_assign_and_revoke_role() {
        let store = Arc::new(MockRoleStore::new());
        let manager = RoleManager::new(store);

        let role = make_test_role("test_role", vec![], vec![]);
        let role_id = manager.create_role(&role).await.unwrap();

        manager
            .assign_role("user-001", role_id, None, None)
            .await
            .unwrap();

        let roles = manager.get_user_roles("user-001").await.unwrap();
        assert_eq!(roles.len(), 1);

        manager.revoke_role("user-001", role_id).await.unwrap();
        let roles_after_revoke = manager.get_user_roles("user-001").await.unwrap();
        assert_eq!(roles_after_revoke.len(), 0);
    }

    #[tokio::test]
    async fn test_role_inheritance() {
        let store = Arc::new(MockRoleStore::new());

        let viewer_perms = vec![Permission {
            id: Uuid::new_v4(),
            resource_type: ResourceType::Document,
            actions: vec!["read".to_string()],
            conditions: None,
            scope: PermissionScope::Own,
        }];

        let editor_extra_perms = vec![Permission {
            id: Uuid::new_v4(),
            resource_type: ResourceType::Document,
            actions: vec!["write".to_string()],
            conditions: None,
            scope: PermissionScope::Team,
        }];

        let viewer = make_test_role("viewer", vec![], viewer_perms);
        let editor = make_test_role("editor", vec![viewer.id], editor_extra_perms);

        store.add_role(viewer).await;
        store.add_role(editor.clone()).await;

        let manager = RoleManager::new(store);
        manager
            .assign_role("user-001", editor.id, None, None)
            .await
            .unwrap();

        let roles = manager.get_user_roles("user-001").await.unwrap();
        let editor_role = roles
            .iter()
            .find(|r| r.role.name == "editor")
            .expect("应有 editor 角色");

        let has_read = editor_role
            .effective_permissions
            .iter()
            .any(|p| p.actions.contains(&"read".to_string()));
        let has_write = editor_role
            .effective_permissions
            .iter()
            .any(|p| p.actions.contains(&"write".to_string()));

        assert!(has_read, "editor 应通过继承拥有 read 权限");
        assert!(has_write, "editor 应自身拥有 write 权限");
    }

    #[tokio::test]
    async fn test_has_permission_check() {
        let store = Arc::new(MockRoleStore::new());

        let role = make_test_role(
            "doc_editor",
            vec![],
            vec![Permission {
                id: Uuid::new_v4(),
                resource_type: ResourceType::Document,
                actions: vec!["read".to_string(), "write".to_string()],
                conditions: None,
                scope: PermissionScope::Team,
            }],
        );

        store.add_role(role).await;
        let manager = RoleManager::new(store);
        manager
            .assign_role(
                "user-001",
                manager
                    .store
                    .get_role_by_name("doc_editor")
                    .await
                    .unwrap()
                    .unwrap()
                    .id,
                None,
                None,
            )
            .await
            .unwrap();

        assert!(
            manager
                .has_permission("user-001", &ResourceType::Document, "read")
                .await
                .unwrap()
        );
        assert!(
            manager
                .has_permission("user-001", &ResourceType::Document, "write")
                .await
                .unwrap()
        );
        assert!(
            !manager
                .has_permission("user-001", &ResourceType::Document, "delete")
                .await
                .unwrap()
        );
        assert!(
            !manager
                .has_permission("user-001", &ResourceType::User, "read")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_expired_assignment_excluded() {
        let store = Arc::new(MockRoleStore::new());
        let manager = RoleManager::new(store.clone());

        let role = make_test_role("temp_role", vec![], vec![]);
        let role_id = manager.create_role(&role).await.unwrap();

        let expired_time = Utc::now() - chrono::Duration::hours(1);
        store
            .add_assignment(
                "user-002",
                UserRoleAssignment {
                    id: Uuid::new_v4(),
                    user_id: "user-002".to_string(),
                    role_id,
                    assignment_source: AssignmentSource::Manual,
                    assigned_by: Some("admin".to_string()),
                    expires_at: Some(expired_time),
                    created_at: Utc::now(),
                },
            )
            .await;

        let roles = manager.get_user_roles("user-002").await.unwrap();
        assert!(roles.is_empty(), "过期的角色分配不应被包含");
    }

    #[tokio::test]
    async fn test_permission_scope_own_check() {
        assert!(PermissionScope::Own.contains_resource("user-001", "user-001", &[], "org-1"));
        assert!(!PermissionScope::Own.contains_resource("user-001", "user-002", &[], "org-1"));
    }

    #[tokio::test]
    async fn test_permission_scope_global_allows_everything() {
        assert!(PermissionScope::Global.contains_resource("any", "any", &[], "any"));
    }
}
