//! Cedar 策略定义与类型系统
//!
//! 将 Cedar Policy DSL 映射为 Rust 类型，支持：
//! - 策略的序列化/反序列化（持久化到数据库）
//! - 授权请求/响应的结构化表示
//! - ABAC 条件表达式的类型安全构建

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Cedar 访问控制策略
///
/// 每条策略定义了一个授权规则：在什么条件下（conditions），
/// 允许或拒绝某个主体（principal）对某个资源（resource）执行某个动作（action）。
///
/// # Example
///
/// ```ignore
/// let policy = Policy {
///     id: Uuid::new_v4(),
///     version: 1,
///     effect: PolicyEffect::Allow,
///     principal: PrincipalExpr::AnyAuthenticated,
///     action: ActionExpr::Specific("document::read".to_string()),
///     resource: ResourceExpr::Type(ResourceType::Document),
///     conditions: Some(ConditionExpr::Attribute {
///         attribute: "owner_id".to_string(),
///         operator: ConditionOperator::Equals,
///         value: ContextValue::PrincipalAttr("id".to_string()),
///     }),
///     description: "仅允许文档所有者读取自己的文档".to_string(),
///     created_at: Utc::now(),
///     updated_at: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    /// 策略唯一标识
    pub id: Uuid,
    /// 策略版本号（用于乐观并发控制）
    pub version: u32,
    /// 策略效果：允许或拒绝
    pub effect: PolicyEffect,
    /// 主体表达式：谁可以执行此操作
    pub principal: PrincipalExpr,
    /// 动作表达式：允许/拒绝的操作
    pub action: ActionExpr,
    /// 资源表达式：目标资源范围
    pub resource: ResourceExpr,
    /// 可选的条件约束（ABAC 核心）
    pub conditions: Option<ConditionExpr>,
    /// 人类可读的策略描述
    pub description: String,
    /// 创建时间
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: DateTime<Utc>,
}

/// 策略效果枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolicyEffect {
    /// 显式允许
    Allow,
    /// 显式拒绝（优先级高于 Allow）
    Deny,
}

use super::middleware::PrincipalEntityType;

/// 主体（Principal）表达式
///
/// 定义"谁"可以触发此策略。Cedar 中 principal 是一个 [`EntityUid`]。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum PrincipalExpr {
    /// 特定用户/服务
    Specific {
        /// 主体实体类型
        entity_type: PrincipalEntityType,
        /// 主体实体唯一标识
        entity_id: String,
    },
    /// 角色匹配（RBAC）
    Role {
        /// 角色名称，用于 RBAC 角色匹配
        role_name: String,
    },
    /// 所有已认证用户
    AnyAuthenticated,
    /// 任意主体（含匿名，应谨慎使用）
    Any,
}

/// 动作（Action）表达式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum ActionExpr {
    /// 特定动作，如 `"document::read"`
    Specific(String),
    /// 动作前缀匹配，如 `"document::*"`
    Prefix(String),
    /// 该资源类型的所有动作
    AllForResource(String),
    /// 任意动作
    Any,
}

/// 资源（Resource）表达式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum ResourceExpr {
    /// 特定资源实例
    Specific {
        /// 资源实体类型
        entity_type: ResourceType,
        /// 资源实体唯一标识
        entity_id: String,
    },
    /// 资源类型级别
    Type(ResourceType),
    /// 资源类型前缀匹配
    TypePrefix(String),
    /// 任意资源
    Any,
}

/// 条件表达式（ABAC 核心组件）
///
/// 支持嵌套布尔逻辑和属性比较运算。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ConditionExpr {
    /// 单一属性比较
    Attribute {
        /// 待比较的属性路径，如 `"owner_id"`、`"classification.level"`
        attribute: String,
        /// 比较操作符
        operator: ConditionOperator,
        /// 比较的右值，可为字面量或属性引用
        value: ContextValue,
    },
    /// 逻辑与：所有子条件必须满足
    And(Vec<Self>),
    /// 逻辑或：任一子条件满足即可
    Or(Vec<Self>),
    /// 逻辑非
    Not(Box<Self>),
}

/// 条件比较操作符
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConditionOperator {
    /// 等于比较
    Equals,
    /// 不等于比较
    NotEquals,
    /// 大于比较
    GreaterThan,
    /// 大于或等于比较
    GreaterThanOrEqual,
    /// 小于比较
    LessThan,
    /// 小于或等于比较
    LessThanOrEqual,
    /// 属于集合成员（左值在右值集合中）
    In,
    /// 不属于集合成员
    NotIn,
    /// 包含（左值集合包含右值元素）
    Contains,
    /// 不包含
    NotContains,
    /// 字符串前缀匹配
    StartsWith,
    /// 字符串后缀匹配
    EndsWith,
    /// 正则表达式匹配
    MatchesRegex,
}

/// 上下文值 —— 条件表达式的操作数
///
/// 可以引用请求中的任意属性：principal、action、resource 或 context。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "camelCase")]
pub enum ContextValue {
    /// 字面量值
    Literal(LiteralValue),
    /// 引用主体的属性
    PrincipalAttr(String),
    /// 引用资源的属性
    ResourceAttr(String),
    /// 引用动作的属性
    ActionAttr(String),
    /// 引用请求上下文的属性
    ContextAttr(String),
}

/// 字面量值类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "PascalCase")]
pub enum LiteralValue {
    /// 字符串字面量
    String(String),
    /// 布尔字面量
    Bool(bool),
    /// 64 位有符号整数字面量
    Integer(i64),
    /// 64 位浮点数字面量
    Decimal(f64),
    /// 嵌套集合字面量
    Set(Vec<LiteralValue>),
}

impl std::hash::Hash for LiteralValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::String(s) => s.hash(state),
            Self::Bool(b) => b.hash(state),
            Self::Integer(i) => i.hash(state),
            Self::Decimal(d) => d.to_bits().hash(state),
            Self::Set(v) => v.hash(state),
        }
    }
}

impl LiteralValue {
    /// 将枚举值以确定性字节序列写入 BLAKE3 哈希器
    ///
    /// 每个变体前缀类型标签（`S`/`B`/`I`/`D`/`V`），确保不同类型的相同底层值
    /// 产生不同的哈希（如 `Integer(1)` 与 `Bool(true)` 不会碰撞）。
    /// 浮点数使用 `to_bits()` 避免NaN语义问题，符合"0 随机性"设计哲学。
    pub fn update_hasher(&self, hasher: &mut blake3::Hasher) {
        match self {
            Self::String(s) => {
                hasher.update(b"S");
                hasher.update(s.as_bytes());
            }
            Self::Bool(b) => {
                hasher.update(b"B");
                hasher.update(&[u8::from(*b)]);
            }
            Self::Integer(i) => {
                hasher.update(b"I");
                hasher.update(&i.to_le_bytes());
            }
            Self::Decimal(d) => {
                hasher.update(b"D");
                hasher.update(&d.to_bits().to_le_bytes());
            }
            Self::Set(v) => {
                hasher.update(b"V");
                for item in v {
                    item.update_hasher(hasher);
                }
            }
        }
    }
}

// ========== 授权请求 / 响应 ==========

/// 授权决策请求
///
/// 封装一次完整的授权检查所需的全部上下文信息。
/// 对应 Cedar 的 `Request { principal, action, resource, context }`。
#[derive(Debug, Clone)]
pub struct AuthorizationRequest {
    /// 发起请求的主体（用户或服务）
    pub principal: Principal,
    /// 请求执行的动作
    pub action: Action,
    /// 目标资源
    pub resource: Resource,
    /// 请求上下文（时间、IP、环境等）
    pub context: Context,
}

/// 主体信息
#[derive(Debug, Clone)]
pub struct Principal {
    /// 主体唯一 ID（对应 [`EntityUid`]）
    pub id: String,
    /// 主体类型
    pub entity_type: PrincipalEntityType,
    /// 主体拥有的角色列表（已展开继承关系）
    pub roles: Vec<String>,
    /// 主体属性（用于 ABAC 条件求值）
    pub attrs: HashMap<String, LiteralValue>,
}

/// 资源类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ResourceType {
    /// 文档资源
    Document,
    /// 知识节点资源
    Node,
    /// 用户资源
    User,
    /// 系统资源
    System,
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Document => write!(f, "Document"),
            Self::Node => write!(f, "Node"),
            Self::User => write!(f, "User"),
            Self::System => write!(f, "System"),
        }
    }
}

impl ResourceType {
    /// 返回资源类型的字符串表示
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Document => "Document",
            Self::Node => "Node",
            Self::User => "User",
            Self::System => "System",
        }
    }
}

impl std::str::FromStr for ResourceType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "document" | "documents" => Ok(Self::Document),
            "node" | "nodes" => Ok(Self::Node),
            "user" | "users" => Ok(Self::User),
            _ => Ok(Self::System),
        }
    }
}

/// 动作信息
#[derive(Debug, Clone)]
pub struct Action {
    /// 动作标识符，如 `"document::read"`、`"user::manage"`
    pub id: String,
    /// 动作所属的资源类型
    pub resource_type: ResourceType,
    /// 动作类别：读 / 写 / 管理
    pub category: ActionCategory,
}

/// 动作类别
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionCategory {
    /// 读取类操作（如查看、检索）
    Read,
    /// 写入类操作（如创建、更新）
    Write,
    /// 删除类操作
    Delete,
    /// 管理类操作（如权限配置、系统设置）
    Admin,
    /// 执行类操作（如运行脚本、触发工作流）
    Execute,
}

/// 资源信息
#[derive(Debug, Clone)]
pub struct Resource {
    /// 资源唯一 ID
    pub id: String,
    /// 资源类型
    pub resource_type: ResourceType,
    /// 资源拥有者 ID
    pub owner_id: Option<String>,
    /// 资源所属组织/团队 ID
    pub scope_id: Option<String>,
    /// 资源属性（用于 ABAC 条件求值）
    pub attrs: HashMap<String, LiteralValue>,
}

/// 请求上下文
///
/// 包含运行时环境信息，不依赖于特定主体、动作或资源。
#[derive(Debug, Clone)]
pub struct Context {
    /// 请求发起时间
    pub request_time: DateTime<Utc>,
    /// 客户端 IP 地址
    pub source_ip: Option<String>,
    /// 用户代理字符串
    pub user_agent: Option<String>,
    /// 设备指纹（可选）
    pub device_fingerprint: Option<String>,
    /// 自定义上下文键值对
    pub extra: HashMap<String, LiteralValue>,
}

/// 授权决策结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationDecision {
    /// 决策类型
    pub decision: DecisionType,
    /// 命中的策略 ID（如有）
    pub policy_id: Option<Uuid>,
    /// 决策原因（人类可读）
    pub reason: String,
    /// 诊断信息（调试用）
    pub diagnostics: Vec<DiagnosticInfo>,
    /// 决策生成时间
    #[serde(with = "chrono::serde::ts_seconds")]
    pub evaluated_at: DateTime<Utc>,
}

/// 决策类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type", content = "reason")]
pub enum DecisionType {
    /// 允许访问
    Allowed,
    /// 拒绝访问
    Denied {
        /// 拒绝原因分类
        reason: DenialReason,
    },
}

/// 拒绝原因分类
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "code", content = "message")]
pub enum DenialReason {
    /// 无匹配的策略
    NoMatchingPolicy,
    /// 显式拒绝策略命中
    ExplicitDeny,
    /// 缺少所需权限
    MissingPermission {
        /// 所需的权限标识
        required: String,
        /// 主体当前已持有的权限列表
        held: Vec<String>,
    },
    /// 条件不满足
    ConditionNotMet {
        /// 未满足的条件表达式描述
        condition: String,
        /// 条件未满足的详细信息
        detail: String,
    },
    /// 资源不存在
    ResourceNotFound,
    /// 主体未认证
    Unauthenticated,
    /// 账户被禁用
    AccountDisabled,
    /// 策略评估错误
    EvaluationError(String),
}

/// 诊断信息条目
///
/// 用于记录策略评估过程中的中间状态，辅助调试和安全审计。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticInfo {
    /// 诊断级别
    pub level: DiagnosticLevel,
    /// 诊断消息
    pub message: String,
    /// 关联的策略 ID（如有）
    pub policy_id: Option<Uuid>,
    /// 额外元数据
    pub metadata: HashMap<String, String>,
}

/// 诊断级别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticLevel {
    /// 信息级别，用于记录策略评估的正常流程
    Info,
    /// 警告级别，用于记录潜在问题
    Warning,
    /// 错误级别，用于记录策略评估中的异常
    Error,
}

#[cfg(test)]
mod tests {
    use super::ResourceType;
    use std::str::FromStr;

    #[test]
    fn test_resource_type_from_str_document() {
        assert_eq!(
            ResourceType::from_str("document").unwrap(),
            ResourceType::Document
        );
        assert_eq!(
            ResourceType::from_str("documents").unwrap(),
            ResourceType::Document
        );
        assert_eq!(
            ResourceType::from_str("Document").unwrap(),
            ResourceType::Document
        );
    }

    #[test]
    fn test_resource_type_from_str_node() {
        assert_eq!(ResourceType::from_str("node").unwrap(), ResourceType::Node);
        assert_eq!(ResourceType::from_str("nodes").unwrap(), ResourceType::Node);
    }

    #[test]
    fn test_resource_type_from_str_user() {
        assert_eq!(ResourceType::from_str("user").unwrap(), ResourceType::User);
        assert_eq!(ResourceType::from_str("users").unwrap(), ResourceType::User);
    }

    #[test]
    fn test_resource_type_from_str_system() {
        assert_eq!(
            ResourceType::from_str("system").unwrap(),
            ResourceType::System
        );
    }

    #[test]
    fn test_resource_type_from_str_unknown_falls_back_to_system() {
        assert_eq!(
            ResourceType::from_str("unknown").unwrap(),
            ResourceType::System
        );
        assert_eq!(ResourceType::from_str("").unwrap(), ResourceType::System);
        assert_eq!(
            ResourceType::from_str("anything").unwrap(),
            ResourceType::System
        );
    }

    #[test]
    fn test_resource_type_as_str_roundtrip() {
        for rt in [
            ResourceType::Document,
            ResourceType::Node,
            ResourceType::User,
            ResourceType::System,
        ] {
            assert_eq!(ResourceType::from_str(rt.as_str()).unwrap(), rt);
        }
    }
}
