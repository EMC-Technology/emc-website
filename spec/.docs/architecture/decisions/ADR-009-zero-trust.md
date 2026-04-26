# ADR-009: 零信任安全模型

## 状态
✅ **Accepted** (2026-04-13)

## Context (背景)

文本全结构化知识系统作为企业级平台，面临以下安全挑战：

1. **多租户环境**：不同团队/项目可能有隔离的数据访问需求
2. **AI Agent 接入**：MCP 协议意味着 AI Agent 将直接操作系统数据
3. **敏感信息处理**：技术文档可能包含 API Key、密码、内部架构细节
4. **合规要求**：企业环境通常有 GDPR、SOC2 等合规审计需求

传统的"边界安全"模型（信任内网、防御外网）已不足以应对现代威胁：
- 内部威胁（恶意或误操作的员工/Agent）
- 供应链攻击（ compromised 依赖库）
- 侧信道攻击（日志泄露、错误信息泄露）

## Decision (决定)

采用 **零信任 (Zero Trust)** 安全原则："永不信任，始终验证"。

### 零信任架构层次

```
┌─────────────────────────────────────────────────────────────┐
│                      零信任控制平面                           │
│                                                              │
│  Layer 7: 应用层                                            │
│  ┌────────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │ ABAC 授权      │  │ 输入验证      │  │ PII 脱敏       │  │
│  │ (细粒度权限)    │  │ (Schema约束)  │  │ (日志过滤)     │  │
│  └────────────────┘  └──────────────┘  └────────────────┘  │
│                                                              │
│  Layer 6: 表示层                                             │
│  ┌────────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │ JWT 认证       │  │ CORS 策略     │  │ 速率限制       │  │
│  │ (身份验证)     │  │ (来源控制)    │  │ (防滥用)       │  │
│  └────────────────┘  └──────────────┘  └────────────────┘  │
│                                                              │
│  Layer 4: 传输层                                             │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ TLS 1.3 强制加密 (Nginx/Caddy 终结)                     │ │
│  │ HSTS / Certificate Pinning                              │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  Layer 3/2/1: 网络/数据链路/物理                             │
│  ┌────────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │ 网络分段       │  │ 加密存储      │  │ 审计日志       │  │
│  │ (微隔离)       │  │ (AES-256-GCM)│  │ (完整追踪)     │  │
│  └────────────────┘  └──────────────┘  └────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 核心安全组件

#### 1. 身份认证 (Authentication)

```rust
// 位于 auth.rs
use jsonwebtoken::{encode, decode, Header, Validation};
use argon2::{Argon2, password_hash};

/// JWT Token 结构
pub struct Claims {
    pub sub: String,        // 用户 ID
    pub exp: usize,         // 过期时间
    pub iat: usize,         // 签发时间
    pub roles: Vec<String>, // 角色列表
    pub permissions: Vec<String>, // 细粒度权限
}

/// 密码哈希 — Argon2id (抗 GPU/ASIC 破解)
pub fn hash_password(password: &str) -> Result<String> {
    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    argon2.hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| error_core::Error::internal("密码哈希失败", e))
}
```

**选型理由**：
- **JWT**: 无状态认证，适合水平扩展的 API 服务
- **Argon2id**: 密码哈希竞赛 winner，抗 GPU/ASIC 暴力破解

#### 2. 授权控制 (Authorization) — ABAC

> **V4.0 修正**：原设计采用 RBAC，实际实现升级为 Cedar 风格自研 ABAC（Attribute-Based Access Control）策略引擎。ABAC 比 RBAC 更细粒度：权限判定基于主体属性（角色/部门/安全级别）、资源属性（分类/敏感度/所属项目）、环境属性（时间/IP/设备），而非仅角色。策略引擎支持 Deny-Override + LRU 缓存 + 批量评估 + 热更新 + 审计。

```rust
/// ABAC 策略引擎（Cedar 风格自研实现）
/// 权限判定基于：主体属性 × 资源属性 × 环境属性
/// 策略格式：permit(principal, action, resource) when { conditions }
pub struct AbacEngine {
    policies: Vec<Policy>,         // 策略集合
    cache: LruCache<PolicyKey, Decision>,  // LRU 缓存
    audit_logger: AuditLogger,     // 审计日志
}

/// 策略示例
/// permit(principal, action, resource) when {
///     principal.role == "editor" &&
///     resource.project == principal.project &&
///     resource.sensitivity != "restricted"
/// }
///
/// deny(principal, action, resource) when {
///     principal.role == "viewer" &&
///     action in [Write, Delete]
/// }
```

#### 3. 输入验证与 PII 脱敏

```rust
// 中间件层 — PII 字段自动脱敏
pub struct PiiSanitizeLayer {
    pii_patterns: RegexSet,  // 预编译的正则集合
}

// SurrealDB ASSERT 约束 — 数据库层验证
DEFINE FIELD content ON token TYPE string
    ASSERT $value != '' AND string::len($value) <= 1024;
```

#### 4. 审计日志

```rust
// 位于 audit.rs
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub user_id: String,
    pub action: AuditAction,
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub result: AuditResult,  // Success / Failure / Denied
}
```

#### 5. 速率限制

```rust
// 令牌桶算法 — 防止滥用
pub struct RateLimiter {
    buckets: DashMap<String, TokenBucket>,  // per-user/per-ip
    config: RateLimitConfig,
}

pub struct RateLimitConfig {
    pub api_requests: u32,      // API 请求: 1000/min
    pub search_queries: u32,    // 搜索: 200/min
    pub mcp_calls: u32,         // MCP 工具调用: 500/min
    pub ws_messages: u32,       // WebSocket: 60/sec
}
```

## Consequences (影响)

### 正面影响

- 🟢 **纵深防御**：多层安全控制，单点突破不会导致全面沦陷
- 🟢 **合规友好**：内置审计日志和细粒度权限满足 GDPR/SOC2 要求
- 🟢 **AI 时代就绪**：RBAC 和速率限制天然适合 AI Agent 接入场景
- 🟢 **最小权限原则**：每个请求只授予必要的最小权限集

### 负面影响

- 🔴 **性能开销**：每次请求都需要经过认证→授权→验证链
- 🔴 **复杂度增加**：安全逻辑渗透到每一层代码中
- 🔴 **开发体验**：本地开发也需要配置认证（可通过 dev mode 简化）

### 性能缓解

- JWT 验证使用本地密钥，无需数据库查询（无状态认证）
- 权限检查使用内存中的 Role-Permission 映射表
- 速率限制使用 DashMap 高并发 HashMap
- PII 正则表达式预编译并复用

## Alternatives (替代方案)

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| **边界安全模型** | 实现简单 | 内部信任假设过强 | ❌ 不满足企业安全标准 |
| **仅网络层安全** | 部署简单 | 应用层无防护 | ❌ 防御深度不足 |
| **OAuth2/OIDC 外部 IdP** | 认证委托专业服务 | 引入外部依赖 | ⚠️ 可作为企业版选项 |
| **零信任模型** ✅ | 最大安全保障 | 实现复杂度高 | ✅ **选定方案** |

## References

- [NIST Zero Trust Architecture (SP 800-207)](https://csrc.nist.gov/publications/detail/sp/800-207/final)
- [OWASP API Security Top 10](https://owasp.org/www-project-api-security/)
- [JWT Best Practices](https://auth0.com/docs/security/tokens/json-web-tokens/json-web-tokens-best-practices)
