//! Cedar 授权引擎
//!
//! 策略评估的核心组件，负责：

#![allow(clippy::significant_drop_tightening)]
//! 1. 从策略存储加载适用策略
//! 2. 构建 Cedar Request 并执行评估
//! 3. 决策缓存（LRU）以实现 < 10ms P99 延迟
//! 4. 批量授权检查（用于列表/批量操作）
//! 5. 策略热更新（不中断服务）
//! 6. 审计日志记录

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::num::NonZero;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

use crate::Result;
use crate::authz::policy::{
    Action, AuthorizationDecision, AuthorizationRequest, ConditionExpr, DecisionType, DenialReason,
    DiagnosticInfo, DiagnosticLevel, LiteralValue, Policy, PolicyEffect, Principal, Resource,
};
use error_core::helpers;

/// 默认决策缓存容量（应覆盖绝大多数热点请求模式）
const DEFAULT_CACHE_CAPACITY: usize = 10_000;
/// 决策缓存 TTL（秒）
const CACHE_TTL_SECS: u64 = 300;
/// 批量授权最大请求数
const MAX_BATCH_SIZE: usize = 1000;

/// 缓存键 —— 由请求的确定性字段哈希生成
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
#[allow(clippy::struct_field_names)]
struct CacheKey {
    principal: String,
    principal_entity_type: String,
    action: String,
    resource: String,
    resource_type: String,
    owner_id: Option<String>,
    scope_id: Option<String>,
    context_hash: u64,
}

impl CacheKey {
    fn from_request(req: &AuthorizationRequest) -> Self {
        let mut hasher = blake3::Hasher::new();
        let ctx = &req.context;
        hasher.update(ctx.request_time.timestamp_millis().to_le_bytes().as_slice());
        if let Some(ip) = &ctx.source_ip {
            hasher.update(ip.as_bytes());
        }
        if let Some(fp) = &ctx.device_fingerprint {
            hasher.update(fp.as_bytes());
        }
        for (k, v) in &ctx.extra {
            hasher.update(k.as_bytes());
            v.update_hasher(&mut hasher);
        }
        let hash = hasher.finalize();
        let context_hash = u64::from_le_bytes(
            hash.as_bytes()[..8]
                .try_into()
                .expect("blake3 输出至少 8 字节"),
        );
        Self {
            principal: req.principal.id.clone(),
            principal_entity_type: req.principal.entity_type.clone(),
            action: req.action.id.clone(),
            resource: req.resource.id.clone(),
            resource_type: req.resource.resource_type.clone(),
            owner_id: req.resource.owner_id.clone(),
            scope_id: req.resource.scope_id.clone(),
            context_hash,
        }
    }
}

/// 带过期时间的缓存条目
struct CacheEntry {
    decision: AuthorizationDecision,
    inserted_at: Instant,
}

/// 授权指标收集器
///
/// 记录授权引擎的关键性能和安全指标，导出至 Prometheus。
pub struct AuthzMetrics {
    total_evaluations: std::sync::atomic::AtomicU64,
    cache_hits: std::sync::atomic::AtomicU64,
    cache_misses: std::sync::atomic::AtomicU64,
    allow_decisions: std::sync::atomic::AtomicU64,
    deny_decisions: std::sync::atomic::AtomicU64,
    evaluation_errors: std::sync::atomic::AtomicU64,
    policy_reload_count: std::sync::atomic::AtomicU64,
}

impl AuthzMetrics {
    #[must_use]
    /// 创建新的授权指标计数器实例（所有计数器初始化为 0）
    pub const fn new() -> Self {
        Self {
            total_evaluations: std::sync::atomic::AtomicU64::new(0),
            cache_hits: std::sync::atomic::AtomicU64::new(0),
            cache_misses: std::sync::atomic::AtomicU64::new(0),
            allow_decisions: std::sync::atomic::AtomicU64::new(0),
            deny_decisions: std::sync::atomic::AtomicU64::new(0),
            evaluation_errors: std::sync::atomic::AtomicU64::new(0),
            policy_reload_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn record_evaluation(&self) {
        self.total_evaluations
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn record_cache_hit(&self) {
        self.cache_hits
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn record_cache_miss(&self) {
        self.cache_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn record_allow(&self) {
        self.allow_decisions
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn record_deny(&self) {
        self.deny_decisions
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn record_error(&self) {
        self.evaluation_errors
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn record_policy_reload(&self, count: u32) {
        self.policy_reload_count
            .fetch_add(u64::from(count), std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for AuthzMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// 策略存储 trait
///
/// 抽象策略的持久化层，支持不同的后端实现（`SurrealDB`、Redis 等）。
#[async_trait::async_trait]
pub trait PolicyStore: Send + Sync {
    /// 根据主体 ID 加载所有适用的策略
    async fn load_policies_for_principal(&self, principal_id: &str) -> Result<Vec<Policy>>;

    /// 根据资源类型和动作加载全局策略
    async fn load_policies_for_resource_action(
        &self,
        resource_type: &str,
        action: &str,
    ) -> Result<Vec<Policy>>;

    /// 加载所有活跃策略（用于缓存预热和批量评估）
    async fn load_all_active_policies(&self) -> Result<Vec<Policy>>;
}

/// 审计日志 trait
///
/// 所有授权决策必须记录审计日志以满足合规要求。
#[async_trait::async_trait]
pub trait AuditLogger: Send + Sync {
    /// 记录一次授权决策事件
    async fn log_authorization_decision(
        &self,
        request: &AuthorizationRequest,
        decision: &AuthorizationDecision,
    );
}

/// 空操作审计日志实现（用于测试/开发环境）
pub struct NoopAuditLogger;

#[async_trait::async_trait]
impl AuditLogger for NoopAuditLogger {
    async fn log_authorization_decision(
        &self,
        _request: &AuthorizationRequest,
        _decision: &AuthorizationDecision,
    ) {
    }
}

/// 基于 `SurrealDB` 的策略存储实现
///
/// 从数据库加载授权策略，支持热更新。
/// 当数据库不可用时，回退到内置默认策略（Deny-by-Default + RBAC 基础规则）。
pub struct DbPolicyStore {
    policies: Arc<RwLock<Vec<Policy>>>,
}

impl DbPolicyStore {
    /// 创建新的数据库策略存储
    ///
    /// 初始化时加载一组安全的默认策略：
    /// - Admin 角色拥有全部权限
    /// - Editor 角色拥有读写权限
    /// - Viewer 角色仅拥有只读权限
    /// - 匿名用户无任何权限
    #[must_use]
    pub fn new() -> Self {
        let default_policies = Self::default_policies();
        Self {
            policies: Arc::new(RwLock::new(default_policies)),
        }
    }

    fn default_policies() -> Vec<Policy> {
        use crate::authz::policy::{ActionExpr, PolicyEffect, PrincipalExpr, ResourceExpr};

        vec![
            Policy {
                id: uuid::Uuid::new_v4(),
                version: 1,
                effect: PolicyEffect::Allow,
                principal: PrincipalExpr::Role {
                    role_name: "admin".to_string(),
                },
                action: ActionExpr::Any,
                resource: ResourceExpr::Any,
                conditions: None,
                description: "管理员拥有全部权限".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            Policy {
                id: uuid::Uuid::new_v4(),
                version: 1,
                effect: PolicyEffect::Allow,
                principal: PrincipalExpr::Role {
                    role_name: "editor".to_string(),
                },
                action: ActionExpr::Prefix("document::read".to_string()),
                resource: ResourceExpr::Any,
                conditions: None,
                description: "编辑者拥有读权限".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            Policy {
                id: uuid::Uuid::new_v4(),
                version: 1,
                effect: PolicyEffect::Allow,
                principal: PrincipalExpr::Role {
                    role_name: "editor".to_string(),
                },
                action: ActionExpr::Prefix("document::write".to_string()),
                resource: ResourceExpr::Any,
                conditions: None,
                description: "编辑者拥有写权限".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            Policy {
                id: uuid::Uuid::new_v4(),
                version: 1,
                effect: PolicyEffect::Allow,
                principal: PrincipalExpr::Role {
                    role_name: "viewer".to_string(),
                },
                action: ActionExpr::Prefix("document::read".to_string()),
                resource: ResourceExpr::Any,
                conditions: None,
                description: "查看者仅拥有读权限".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        ]
    }

    /// 热更新策略（从数据库重新加载）
    pub async fn reload_policies(&self, new_policies: Vec<Policy>) {
        let mut policies = self.policies.write().await;
        *policies = new_policies;
        info!(count = policies.len(), "策略热更新完成");
    }
}

impl Default for DbPolicyStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl PolicyStore for DbPolicyStore {
    async fn load_policies_for_principal(&self, _principal_id: &str) -> Result<Vec<Policy>> {
        Ok(self.policies.read().await.clone())
    }

    async fn load_policies_for_resource_action(
        &self,
        _resource_type: &str,
        _action: &str,
    ) -> Result<Vec<Policy>> {
        Ok(self.policies.read().await.clone())
    }

    async fn load_all_active_policies(&self) -> Result<Vec<Policy>> {
        Ok(self.policies.read().await.clone())
    }
}

/// Cedar 授权引擎
///
/// 核心授权决策组件。采用 **Deny-by-Default** 策略：
/// - 无匹配策略 → Deny
/// - 显式 Allow 且条件满足 → Allow
/// - 显式 Deny → Deny（优先级最高）
///
/// # 性能保证
///
/// - LRU 缓存确保 P99 < 10ms
/// - 批量评估减少重复策略加载开销
/// - 异步策略加载不阻塞请求处理线程
pub struct AuthorizationEngine {
    policy_store: Arc<dyn PolicyStore>,
    cache: Arc<RwLock<LruCache<CacheKey, CacheEntry>>>,
    metrics: Arc<AuthzMetrics>,
    audit_logger: Arc<dyn AuditLogger>,
}

impl AuthorizationEngine {
    /// 创建新的授权引擎实例
    ///
    /// # Arguments
    ///
    /// * `policy_store` - 策略持久化后端
    /// * `cache_capacity` - LRU 缓存容量（默认 10,000）
    /// * `audit_logger` - 审计日志记录器
    ///
    /// # Panics
    ///
    /// `cache_capacity` 为 0 时 `NonZero::new` 会返回 `None`，此时 `unwrap` 会 panic。
    pub fn new(
        policy_store: Arc<dyn PolicyStore>,
        cache_capacity: Option<usize>,
        audit_logger: Arc<dyn AuditLogger>,
    ) -> Self {
        let capacity = cache_capacity.unwrap_or(DEFAULT_CACHE_CAPACITY);
        info!(cache_capacity = capacity, "初始化 AuthorizationEngine");
        let cache = LruCache::new(NonZero::new(capacity).unwrap());
        Self {
            policy_store,
            cache: Arc::new(RwLock::new(cache)),
            metrics: Arc::new(AuthzMetrics::new()),
            audit_logger,
        }
    }

    /// 执行单次授权检查
    ///
    /// 完整流程：缓存检查 → 策略加载 → 条件求值 → 结果缓存 → 审计日志。
    ///
    /// # Errors
    ///
    /// 当策略加载失败或条件表达式求值出错时返回错误。
    #[instrument(skip(self), fields(principal = %req.principal.id, action = %req.action.id, resource = %req.resource.id))]
    pub async fn authorize(&self, req: &AuthorizationRequest) -> Result<AuthorizationDecision> {
        self.metrics.record_evaluation();

        // 1. 检查缓存
        if let Some(cached) = self.get_cached_decision(req).await {
            self.metrics.record_cache_hit();
            match &cached.decision {
                DecisionType::Allowed => {
                    self.metrics.record_allow();
                }
                DecisionType::Denied { .. } => {
                    self.metrics.record_deny();
                }
            }
            debug!(decision = ?cached.decision, "命中授权缓存");
            self.audit_logger
                .log_authorization_decision(req, &cached)
                .await;
            return Ok(cached);
        }
        self.metrics.record_cache_miss();

        // 2. 加载适用策略
        let policies = self
            .policy_store
            .load_policies_for_principal(&req.principal.id)
            .await?;

        let resource_policies = self
            .policy_store
            .load_policies_for_resource_action(&req.resource.resource_type, &req.action.id)
            .await?;

        // 合并主体策略和资源-动作策略，去重
        let mut all_policies = policies;
        for p in resource_policies {
            if !all_policies.iter().any(|existing| existing.id == p.id) {
                all_policies.push(p);
            }
        }

        // 3. 执行策略评估
        let decision = self.evaluate_policies(req, &all_policies);

        // 4. 缓存决策结果
        self.cache_decision(req, &decision).await;

        // 5. 记录审计日志
        self.audit_logger
            .log_authorization_decision(req, &decision)
            .await;

        match &decision.decision {
            DecisionType::Allowed => {
                self.metrics.record_allow();
            }
            DecisionType::Denied { .. } => {
                self.metrics.record_deny();
            }
        }

        Ok(decision)
    }

    /// 批量授权检查
    ///
    /// 用于列表查询等需要一次性验证多个资源的场景。
    /// 内部会合并相同主体的策略加载请求以优化性能。
    ///
    /// # Arguments
    ///
    /// * `requests` - 授权请求列表（上限 [`MAX_BATCH_SIZE`]）
    ///
    /// # Errors
    ///
    /// 当请求数超过上限或策略加载失败时返回错误。
    pub async fn authorize_batch(
        &self,
        requests: Vec<AuthorizationRequest>,
    ) -> Result<Vec<AuthorizationDecision>> {
        if requests.len() > MAX_BATCH_SIZE {
            return Err(helpers::validation_error(
                &format!(
                    "批量授权请求数超过上限: {} > {}",
                    requests.len(),
                    MAX_BATCH_SIZE
                ),
                "authorize_batch",
            ));
        }

        let mut results = Vec::with_capacity(requests.len());

        // 先尝试从缓存获取
        let mut uncached_requests: Vec<(usize, AuthorizationRequest)> = Vec::new();
        for (idx, req) in requests.into_iter().enumerate() {
            if let Some(cached) = self.get_cached_decision(&req).await {
                self.metrics.record_cache_hit();
                results.push((idx, cached));
            } else {
                self.metrics.record_cache_miss();
                uncached_requests.push((idx, req));
            }
        }

        // 对未命中的请求执行批量评估
        if !uncached_requests.is_empty() {
            // 收集所有唯一主体 ID 以批量加载策略
            let principal_ids: Vec<&str> = uncached_requests
                .iter()
                .map(|(_, r)| r.principal.id.as_str())
                .collect();

            // 加载所有相关策略（主体策略 + 资源-动作策略）
            let mut all_policies_map: HashMap<String, Vec<Policy>> = HashMap::new();
            for pid in &principal_ids {
                if !all_policies_map.contains_key(*pid) {
                    let mut policies = self.policy_store.load_policies_for_principal(pid).await?;
                    let pid_string = pid.to_string();
                    let key = pid_string.clone();
                    for (_, req) in &uncached_requests {
                        if req.principal.id == pid_string {
                            let resource_policies = self
                                .policy_store
                                .load_policies_for_resource_action(&req.resource.id, &req.action.id)
                                .await?;
                            policies.extend(resource_policies);
                        }
                    }
                    all_policies_map.insert(key, policies);
                }
            }

            for (original_idx, req) in &uncached_requests {
                let policies = all_policies_map
                    .get(&req.principal.id)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let decision = self.evaluate_policies(req, policies);
                self.cache_decision(req, &decision).await;
                self.audit_logger
                    .log_authorization_decision(req, &decision)
                    .await;
                results.push((*original_idx, decision));
            }
        }

        // 按原始顺序排列结果
        results.sort_by_key(|(idx, _)| *idx);
        Ok(results.into_iter().map(|(_, d)| d).collect())
    }

    /// 热更新策略缓存
    ///
    /// 在不中断服务的情况下重新加载策略，适用于策略变更后的实时生效场景。
    /// 清空当前缓存以确保新策略立即生效。
    ///
    /// # Returns
    ///
    /// 返回重新加载的策略数量。
    ///
    /// # Errors
    ///
    /// 策略加载失败时返回错误。
    #[allow(clippy::cast_possible_truncation)]
    pub async fn reload_policies(&self) -> Result<u32> {
        info!("开始热更新策略缓存");

        let policies = self.policy_store.load_all_active_policies().await?;
        let count = policies.len() as u32;

        // 清空决策缓存，强制下次请求重新评估
        {
            let mut cache = self.cache.write().await;
            cache.clear();
        }

        self.metrics.record_policy_reload(count);

        info!(count, "策略热更新完成");
        Ok(count)
    }

    /// 获取当前性能指标快照
    #[must_use]
    pub fn metrics_snapshot(&self) -> AuthzMetricsSnapshot {
        AuthzMetricsSnapshot {
            total_evaluations: self
                .metrics
                .total_evaluations
                .load(std::sync::atomic::Ordering::Relaxed),
            cache_hits: self
                .metrics
                .cache_hits
                .load(std::sync::atomic::Ordering::Relaxed),
            cache_misses: self
                .metrics
                .cache_misses
                .load(std::sync::atomic::Ordering::Relaxed),
            allow_decisions: self
                .metrics
                .allow_decisions
                .load(std::sync::atomic::Ordering::Relaxed),
            deny_decisions: self
                .metrics
                .deny_decisions
                .load(std::sync::atomic::Ordering::Relaxed),
            evaluation_errors: self
                .metrics
                .evaluation_errors
                .load(std::sync::atomic::Ordering::Relaxed),
            policy_reload_count: self
                .metrics
                .policy_reload_count
                .load(std::sync::atomic::Ordering::Relaxed),
        }
    }

    // ========== 内部方法 ==========

    /// 从缓存中查找已缓存的决策
    async fn get_cached_decision(
        &self,
        req: &AuthorizationRequest,
    ) -> Option<AuthorizationDecision> {
        let key = CacheKey::from_request(req);
        let cache = self.cache.read().await;
        let entry = cache.peek(&key)?;
        if entry.inserted_at.elapsed() < Duration::from_secs(CACHE_TTL_SECS) {
            Some(entry.decision.clone())
        } else {
            None
        }
    }

    /// 将决策写入缓存
    async fn cache_decision(&self, req: &AuthorizationRequest, decision: &AuthorizationDecision) {
        let key = CacheKey::from_request(req);
        let mut cache = self.cache.write().await;
        cache.put(
            key,
            CacheEntry {
                decision: decision.clone(),
                inserted_at: Instant::now(),
            },
        );
    }

    /// 核心策略评估逻辑
    ///
    /// 采用 **Deny-Override** 模型：
    /// 1. 任何显式 Deny 匹配 → Deny（最高优先级）
    /// 2. 存在显式 Allow 且条件满足 → Allow
    /// 3. 否则 → Deny（默认拒绝）
    fn evaluate_policies(
        &self,
        req: &AuthorizationRequest,
        policies: &[Policy],
    ) -> AuthorizationDecision {
        let mut diagnostics: Vec<DiagnosticInfo> = Vec::new();
        let evaluated_at = Utc::now();

        // Phase 1: 检查显式 Deny 策略
        for policy in policies {
            if policy.effect != PolicyEffect::Deny {
                continue;
            }

            match Self::matches_policy(req, policy) {
                Ok(matches) => {
                    if matches {
                        diagnostics.push(DiagnosticInfo {
                            level: DiagnosticLevel::Info,
                            message: format!("命中显式拒绝策略: {}", policy.id),
                            policy_id: Some(policy.id),
                            metadata: HashMap::new(),
                        });

                        return AuthorizationDecision {
                            decision: DecisionType::Denied {
                                reason: DenialReason::ExplicitDeny,
                            },
                            policy_id: Some(policy.id),
                            reason: format!("被策略 {} 显式拒绝", policy.id),
                            diagnostics,
                            evaluated_at,
                        };
                    }
                }
                Err(e) => {
                    self.metrics.record_error();
                    diagnostics.push(DiagnosticInfo {
                        level: DiagnosticLevel::Error,
                        message: format!("策略评估错误: {e}"),
                        policy_id: Some(policy.id),
                        metadata: HashMap::new(),
                    });
                    warn!(error = %e, policy_id = %policy.id, "Deny 策略评估出错，安全起见拒绝访问");
                    return AuthorizationDecision {
                        decision: DecisionType::Denied {
                            reason: DenialReason::EvaluationError(format!(
                                "Deny 策略评估失败，安全策略要求拒绝: {e}"
                            )),
                        },
                        policy_id: Some(policy.id),
                        reason: format!("Deny 策略评估失败，安全策略要求拒绝: {e}"),
                        diagnostics,
                        evaluated_at,
                    };
                }
            }
        }

        // Phase 2: 检查显式 Allow 策略
        for policy in policies {
            if policy.effect != PolicyEffect::Allow {
                continue;
            }

            match Self::matches_policy(req, policy) {
                Ok(matches) => {
                    if matches {
                        diagnostics.push(DiagnosticInfo {
                            level: DiagnosticLevel::Info,
                            message: format!("命中允许策略: {}", policy.id),
                            policy_id: Some(policy.id),
                            metadata: HashMap::new(),
                        });

                        return AuthorizationDecision {
                            decision: DecisionType::Allowed,
                            policy_id: Some(policy.id),
                            reason: format!("被策略 {} 允许", policy.id),
                            diagnostics,
                            evaluated_at,
                        };
                    }
                }
                Err(e) => {
                    self.metrics.record_error();
                    diagnostics.push(DiagnosticInfo {
                        level: DiagnosticLevel::Error,
                        message: format!("策略评估错误: {e}"),
                        policy_id: Some(policy.id),
                        metadata: HashMap::new(),
                    });
                    warn!(error = %e, policy_id = %policy.id, "Allow 策略评估出错，跳过");
                }
            }
        }

        // Phase 3: 无匹配策略 → Deny by Default
        AuthorizationDecision {
            decision: DecisionType::Denied {
                reason: DenialReason::NoMatchingPolicy,
            },
            policy_id: None,
            reason: "无匹配的允许策略".to_string(),
            diagnostics,
            evaluated_at,
        }
    }

    /// 判断请求是否匹配给定策略
    ///
    /// 依次检查 principal、action、resource 是否匹配，最后评估 conditions。
    fn matches_policy(req: &AuthorizationRequest, policy: &Policy) -> Result<bool> {
        // Principal 匹配
        if !Self::match_principal(&req.principal, &policy.principal) {
            return Ok(false);
        }

        // Action 匹配
        if !Self::match_action(&req.action, &policy.action) {
            return Ok(false);
        }

        // Resource 匹配
        if !Self::match_resource(&req.resource, &policy.resource) {
            return Ok(false);
        }

        // Conditions 评估
        if let Some(ref conditions) = policy.conditions {
            if !Self::evaluate_conditions(conditions, req)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn match_principal(principal: &Principal, expr: &crate::authz::policy::PrincipalExpr) -> bool {
        use crate::authz::policy::PrincipalExpr;
        match expr {
            PrincipalExpr::Any => true,
            PrincipalExpr::AnyAuthenticated => !principal.roles.is_empty(),
            PrincipalExpr::Specific {
                entity_type,
                entity_id,
            } => principal.entity_type == *entity_type && principal.id == *entity_id,
            PrincipalExpr::Role { role_name } => principal.roles.contains(role_name),
        }
    }

    fn match_action(action: &Action, expr: &crate::authz::policy::ActionExpr) -> bool {
        use crate::authz::policy::ActionExpr;
        match expr {
            ActionExpr::Any => true,
            ActionExpr::Specific(pattern) => action.id == *pattern,
            ActionExpr::Prefix(prefix) => action.id.starts_with(prefix.as_str()),
            ActionExpr::AllForResource(rt) => action.resource_type == *rt,
        }
    }

    fn match_resource(resource: &Resource, expr: &crate::authz::policy::ResourceExpr) -> bool {
        use crate::authz::policy::ResourceExpr;
        match expr {
            ResourceExpr::Any => true,
            ResourceExpr::Type(rt) => resource.resource_type == *rt,
            ResourceExpr::Specific {
                entity_type,
                entity_id,
            } => resource.resource_type == *entity_type && resource.id == *entity_id,
            ResourceExpr::TypePrefix(prefix) => resource.resource_type.starts_with(prefix),
        }
    }

    /// 递归评估条件表达式树
    fn evaluate_conditions(condition: &ConditionExpr, req: &AuthorizationRequest) -> Result<bool> {
        use crate::authz::policy::ConditionExpr;
        match condition {
            ConditionExpr::Attribute {
                attribute,
                operator,
                value,
            } => {
                let left = Self::resolve_context_value(value, req)?;
                let right = Self::resolve_attr_from_request(attribute, req);
                let Some(right) = right else {
                    warn!(attribute = %attribute, "条件评估引用的属性不存在，安全策略要求拒绝");
                    return Ok(false);
                };

                Ok(Self::compare_values(operator, &left, &right))
            }
            ConditionExpr::And(children) => {
                for child in children {
                    if !Self::evaluate_conditions(child, req)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            ConditionExpr::Or(children) => {
                for child in children {
                    if Self::evaluate_conditions(child, req)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            ConditionExpr::Not(inner) => Ok(!Self::evaluate_conditions(inner, req)?),
        }
    }

    /// 解析 `ContextValue` 为实际字面量值
    fn resolve_context_value(
        value: &crate::authz::policy::ContextValue,
        req: &AuthorizationRequest,
    ) -> Result<LiteralValue> {
        use crate::authz::policy::ContextValue;
        match value {
            ContextValue::Literal(lit) => Ok(lit.clone()),
            ContextValue::PrincipalAttr(attr) => {
                if let Some(v) = req.principal.attrs.get(attr) {
                    return Ok(v.clone());
                }
                match attr.as_str() {
                    "id" => Ok(LiteralValue::String(req.principal.id.clone())),
                    "entity_type" => Ok(LiteralValue::String(req.principal.entity_type.clone())),
                    "roles" => Ok(LiteralValue::String(req.principal.roles.clone().join(","))),
                    _ => Err(helpers::not_found("PrincipalAttr", &format!("主体属性不存在: {attr}"))),
                }
            }
            ContextValue::ResourceAttr(attr) => {
                if let Some(v) = req.resource.attrs.get(attr) {
                    return Ok(v.clone());
                }
                match attr.as_str() {
                    "id" => Ok(LiteralValue::String(req.resource.id.clone())),
                    "resource_type" => Ok(LiteralValue::String(req.resource.resource_type.clone())),
                    "owner_id" => req
                        .resource
                        .owner_id
                        .as_ref()
                        .map(|v| LiteralValue::String(v.clone()))
                        .ok_or_else(|| helpers::not_found("ResourceAttr", &format!("资源属性不存在: {attr}"))),
                    "scope_id" => req
                        .resource
                        .scope_id
                        .as_ref()
                        .map(|v| LiteralValue::String(v.clone()))
                        .ok_or_else(|| helpers::not_found("ResourceAttr", &format!("资源属性不存在: {attr}"))),
                    _ => Err(helpers::not_found("ResourceAttr", &format!("资源属性不存在: {attr}"))),
                }
            }
            ContextValue::ActionAttr(attr) => match attr.as_str() {
                "id" => Ok(LiteralValue::String(req.action.id.clone())),
                "resource_type" => Ok(LiteralValue::String(req.action.resource_type.clone())),
                other => Err(helpers::validation_error(&format!(
                    "不支持的动作属性: {other}"
                ), "resolve_context_value")),
            },
            ContextValue::ContextAttr(attr) => req
                .context
                .extra
                .get(attr)
                .cloned()
                .ok_or_else(|| helpers::not_found("ContextAttr", &format!("上下文属性不存在: {attr}"))),
        }
    }

    /// 从请求中解析属性值
    fn resolve_attr_from_request(
        attribute: &str,
        req: &AuthorizationRequest,
    ) -> Option<LiteralValue> {
        if let Some(v) = req.principal.attrs.get(attribute) {
            return Some(v.clone());
        }
        match attribute {
            "owner_id" => {
                if let Some(ref owner_id) = req.resource.owner_id {
                    return Some(LiteralValue::String(owner_id.clone()));
                }
            }
            "scope_id" => {
                if let Some(ref scope_id) = req.resource.scope_id {
                    return Some(LiteralValue::String(scope_id.clone()));
                }
            }
            _ => {}
        }
        if let Some(v) = req.resource.attrs.get(attribute) {
            return Some(v.clone());
        }
        if let Some(v) = req.context.extra.get(attribute) {
            return Some(v.clone());
        }
        None
    }

    /// 执行两个字面量值的比较运算
    fn compare_values(
        op: &crate::authz::policy::ConditionOperator,
        left: &LiteralValue,
        right: &LiteralValue,
    ) -> bool {
        use crate::authz::policy::{ConditionOperator, LiteralValue};
        match (op, left, right) {
            (ConditionOperator::Equals, a, b) => a == b,
            (ConditionOperator::NotEquals, a, b) => a != b,
            (ConditionOperator::In, item, LiteralValue::Set(set)) => set.contains(item),
            (ConditionOperator::NotIn, item, LiteralValue::Set(set)) => !set.contains(item),
            (
                ConditionOperator::Contains,
                LiteralValue::String(haystack),
                LiteralValue::String(needle),
            ) => haystack.contains(needle.as_str()),
            (
                ConditionOperator::StartsWith,
                LiteralValue::String(s),
                LiteralValue::String(prefix),
            ) => s.starts_with(prefix.as_str()),
            (
                ConditionOperator::EndsWith,
                LiteralValue::String(s),
                LiteralValue::String(suffix),
            ) => s.ends_with(suffix.as_str()),
            (
                ConditionOperator::GreaterThan,
                LiteralValue::Integer(a),
                LiteralValue::Integer(b),
            ) => a > b,
            (
                ConditionOperator::GreaterThanOrEqual,
                LiteralValue::Integer(a),
                LiteralValue::Integer(b),
            ) => a >= b,
            (ConditionOperator::LessThan, LiteralValue::Integer(a), LiteralValue::Integer(b)) => {
                a < b
            }
            (
                ConditionOperator::LessThanOrEqual,
                LiteralValue::Integer(a),
                LiteralValue::Integer(b),
            ) => a <= b,
            (
                ConditionOperator::MatchesRegex,
                LiteralValue::String(s),
                LiteralValue::String(pattern),
            ) => regex::Regex::new(pattern)
                .is_ok_and(|re| re.is_match(s)),
            _ => false,
        }
    }
}

/// 授权引擎性能指标快照
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthzMetricsSnapshot {
    /// 累计策略评估总次数
    pub total_evaluations: u64,
    /// 策略缓存命中次数
    pub cache_hits: u64,
    /// 策略缓存未命中次数
    pub cache_misses: u64,
    /// 累计允许（Allow）决策次数
    pub allow_decisions: u64,
    /// 累计拒绝（Deny）决策次数
    pub deny_decisions: u64,
    /// 累计评估错误次数
    pub evaluation_errors: u64,
    /// 策略文件热重载次数
    pub policy_reload_count: u64,
}

impl AuthzMetricsSnapshot {
    /// 缓存命中率百分比
    #[must_use]
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        {
            (self.cache_hits as f64 / total as f64) * 100.0
        }
    }
}

// ========== 单元测试 ==========

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authz::policy::{ActionCategory, Context};
    use std::collections::HashMap;
    use std::sync::Arc;
    use uuid::Uuid;

    /// 内存策略存储（仅用于测试）
    struct InMemoryPolicyStore {
        policies: Arc<RwLock<Vec<Policy>>>,
    }

    impl InMemoryPolicyStore {
        fn new(policies: Vec<Policy>) -> Self {
            Self {
                policies: Arc::new(RwLock::new(policies)),
            }
        }
    }

    #[async_trait::async_trait]
    impl PolicyStore for InMemoryPolicyStore {
        async fn load_policies_for_principal(&self, _principal_id: &str) -> Result<Vec<Policy>> {
            Ok(self.policies.read().await.clone())
        }

        async fn load_policies_for_resource_action(
            &self,
            _resource_type: &str,
            _action: &str,
        ) -> Result<Vec<Policy>> {
            Ok(Vec::new())
        }

        async fn load_all_active_policies(&self) -> Result<Vec<Policy>> {
            Ok(self.policies.read().await.clone())
        }
    }

    fn make_test_policy(effect: PolicyEffect) -> Policy {
        Policy {
            id: Uuid::new_v4(),
            version: 1,
            effect,
            principal: crate::authz::policy::PrincipalExpr::AnyAuthenticated,
            action: crate::authz::policy::ActionExpr::Specific("document::read".to_string()),
            resource: crate::authz::policy::ResourceExpr::Type("Document".to_string()),
            conditions: None,
            description: "测试策略".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_test_request() -> AuthorizationRequest {
        let mut attrs = HashMap::new();
        attrs.insert(
            "department".to_string(),
            LiteralValue::String("engineering".to_string()),
        );

        AuthorizationRequest {
            principal: Principal {
                id: "user-001".to_string(),
                entity_type: "User".to_string(),
                roles: vec!["editor".to_string()],
                attrs: attrs.clone(),
            },
            action: Action {
                id: "document::read".to_string(),
                resource_type: "Document".to_string(),
                category: ActionCategory::Read,
            },
            resource: Resource {
                id: "doc-001".to_string(),
                resource_type: "Document".to_string(),
                owner_id: Some("user-001".to_string()),
                scope_id: None,
                attrs: HashMap::new(),
            },
            context: Context {
                request_time: Utc::now(),
                source_ip: Some("127.0.0.1".to_string()),
                user_agent: None,
                device_fingerprint: None,
                extra: HashMap::new(),
            },
        }
    }

    #[tokio::test]
    async fn test_allow_policy_grants_access() {
        let store = Arc::new(InMemoryPolicyStore::new(vec![make_test_policy(
            PolicyEffect::Allow,
        )]));
        let engine = AuthorizationEngine::new(store, Some(100), Arc::new(NoopAuditLogger));
        let req = make_test_request();
        let decision = engine.authorize(&req).await.unwrap();
        assert_eq!(decision.decision, DecisionType::Allowed);
    }

    #[tokio::test]
    async fn test_deny_policy_overrides_allow() {
        let store = Arc::new(InMemoryPolicyStore::new(vec![
            make_test_policy(PolicyEffect::Allow),
            make_test_policy(PolicyEffect::Deny),
        ]));
        let engine = AuthorizationEngine::new(store, Some(100), Arc::new(NoopAuditLogger));
        let req = make_test_request();
        let decision = engine.authorize(&req).await.unwrap();
        assert!(matches!(decision.decision, DecisionType::Denied { .. }));
    }

    #[tokio::test]
    async fn test_no_matching_policy_results_in_deny() {
        let store = Arc::new(InMemoryPolicyStore::new(Vec::new()));
        let engine = AuthorizationEngine::new(store, Some(100), Arc::new(NoopAuditLogger));
        let req = make_test_request();
        let decision = engine.authorize(&req).await.unwrap();
        assert!(matches!(
            decision.decision,
            DecisionType::Denied {
                reason: DenialReason::NoMatchingPolicy
            }
        ));
    }

    #[tokio::test]
    async fn test_authorize_batch() {
        let store = Arc::new(InMemoryPolicyStore::new(vec![make_test_policy(
            PolicyEffect::Allow,
        )]));
        let engine = AuthorizationEngine::new(store, Some(100), Arc::new(NoopAuditLogger));

        let requests = vec![make_test_request(); 5];
        let decisions = engine.authorize_batch(requests).await.unwrap();
        assert_eq!(decisions.len(), 5);
        for d in decisions {
            assert_eq!(d.decision, DecisionType::Allowed);
        }
    }

    #[tokio::test]
    async fn test_condition_evaluation_owner_check() {
        let policy = Policy {
            id: Uuid::new_v4(),
            version: 1,
            effect: PolicyEffect::Allow,
            principal: crate::authz::policy::PrincipalExpr::AnyAuthenticated,
            action: crate::authz::policy::ActionExpr::Specific("document::read".to_string()),
            resource: crate::authz::policy::ResourceExpr::Type("Document".to_string()),
            conditions: Some(ConditionExpr::Attribute {
                attribute: "owner_id".to_string(),
                operator: crate::authz::policy::ConditionOperator::Equals,
                value: crate::authz::policy::ContextValue::PrincipalAttr("id".to_string()),
            }),
            description: "仅允许文档所有者读取".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let store = Arc::new(InMemoryPolicyStore::new(vec![policy]));
        let engine = AuthorizationEngine::new(store, Some(100), Arc::new(NoopAuditLogger));

        // 所有者匹配
        let mut req = make_test_request();
        req.resource.owner_id = Some("user-001".to_string());
        let decision = engine.authorize(&req).await.unwrap();
        assert_eq!(decision.decision, DecisionType::Allowed);

        // 非所有者
        req.resource.owner_id = Some("user-other".to_string());
        let decision = engine.authorize(&req).await.unwrap();
        assert!(matches!(decision.decision, DecisionType::Denied { .. }));
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let store = Arc::new(InMemoryPolicyStore::new(vec![make_test_policy(
            PolicyEffect::Allow,
        )]));
        let engine = AuthorizationEngine::new(store, Some(100), Arc::new(NoopAuditLogger));
        let req = make_test_request();

        engine.authorize(&req).await.unwrap();
        engine.authorize(&req).await.unwrap();

        let snapshot = engine.metrics_snapshot();
        assert_eq!(snapshot.total_evaluations, 2);
        assert_eq!(snapshot.allow_decisions, 2);
    }

    #[tokio::test]
    async fn test_batch_size_limit_enforced() {
        let store = Arc::new(InMemoryPolicyStore::new(Vec::new()));
        let engine = AuthorizationEngine::new(store, Some(100), Arc::new(NoopAuditLogger));
        let requests: Vec<_> = (0..=MAX_BATCH_SIZE).map(|_| make_test_request()).collect();
        let result = engine.authorize_batch(requests).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = err.message();
        assert!(
            err_msg.contains("超过上限"),
            "错误消息应包含'超过上限'，实际: {err_msg}"
        );
    }
}
