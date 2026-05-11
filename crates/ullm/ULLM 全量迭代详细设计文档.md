# ULLM 全量迭代详细设计文档

> **模块全称**: Universal LLM Access Lifecycle Management Module（通用 LLM 接入全生命周期管理模块）
> **Cargo 包名**: `ullm` | **Rust 标识符**: `ullm`
> **版本**: 0.1.0 → 0.2.0
> **设计日期**: 2026-04-25

---

## 〇、设计锚点：使命·愿景·铁律·哲学合规性

> 本节确保迭代设计 100% 锚定项目核心约束，任何偏离均视为设计缺陷。

### 0.1 项目使命

> 构建一款高性能、高可定制、完全可控的 AI IDE 系统，为开发者提供真正自由、高效的 AI 辅助编程体验。

**ULLM 模块使命映射**：作为 LLM 适配层 + 提供商实现层的核心载体，ULLM 承载"多模型统一接入"与"零费用 AI 调用"两大核心能力。

### 0.2 项目愿景

> 通过"乐高式拼装"实现功能最大化与自编码最小化的统一。

**ULLM 模块愿景映射**：所有新供应商基于 `OpenAiCompatibleProvider` 复用（减少 80% 自编码），仅 GLM 因 JWT 需求新增独立文件。

### 0.3 六条铁律合规性校验

| # | 铁律 | 本迭代合规性 | 证据 |
|---|------|-------------|------|
| 1 | **100% 基于 Rust** | ✅ 合规 | 所有新增代码均为 Rust，无外部语言依赖 |
| 2 | **所有依赖在 crates.io 存在** | ✅ 合规 | 新增 4 个依赖均已验证存在于 crates.io（见第十节验证表） |
| 3 | **License 为 MIT/Apache-2.0** | ✅ 合规 | `hmac`(MIT/Apache-2.0)、`sha2`(MIT/Apache-2.0)、`base64`(MIT/Apache-2.0)、`cookie`(MIT/Apache-2.0) |
| 4 | **零自编码底线** | ✅ 合规 | JWT 由 `hmac`+`sha2`+`base64` crate 组合生成；HTTP 由 `reqwest` 处理；SSE 由现有 `SseParser` 解析；Cookie 由 `cookie` crate 解析 |
| 5 | **零费用闭环** | ✅ 合规 | 三优先级零费用策略：P1=CodeGeeX 免费 API(已有) → P2=Ollama 本地推理(已有) → P3=Portal Auth 零成本访问(本迭代新增) |
| 6 | **防幻觉校验** | ✅ 合规 | 每个依赖均提供 crates.io 链接和版本号，API URL 均经 Web 搜索验证 |

### 0.4 设计哲学合规性

| 哲学 | 本迭代体现 |
|------|-----------|
| **乐高式拼装** | 8 个新供应商中 7 个复用 `OpenAiCompatibleProvider`，仅 GLM 因 JWT 特殊需求独立文件；Portal Auth 复用 `CredentialsProvider` trait |
| **分层解耦** | 新供应商位于提供商实现层（第五层），通过 `LanguageModel`/`LanguageModelProvider` 双 Trait 与核心调度层（第三层）解耦 |
| **零成本路径** | Portal Auth 框架实现第三优先级零费用路径，与现有 CodeGeeX(P1) + Ollama(P2) 形成完整零费用闭环 |
| **第三方代码复用** | JWT 逻辑使用 `hmac`+`sha2`+`base64` crate 组合（RustCrypto 生态标准组件）手动构造；SSE 解析复用现有 `SseParser`；限流复用现有 `RateLimiter` |

---

## 一、迭代总览

### 1.1 迭代目标

| # | 目标 | 优先级 | 状态 |
|---|------|--------|------|
| 1 | 模块重命名 `ide-llm` → `ullm` | P0 | 设计完成 |
| 2 | 新增 GLM 供应商 | P0 | 设计完成 |
| 3 | 新增 DeepSeek 供应商 | P0 | 设计完成 |
| 4 | 新增 Kimi 供应商 | P0 | 设计完成 |
| 5 | 新增 Qwen（千问）供应商 | P0 | 设计完成 |
| 6 | 新增 OpenRouter 供应商 | P1 | 设计完成 |
| 7 | 新增 Google Gemini 供应商 | P1 | 设计完成 |
| 8 | 新增 Mistral 供应商 | P1 | 设计完成 |
| 9 | 新增 Groq 供应商 | P1 | 设计完成 |
| 10 | Portal Auth 零成本访问框架 | P1 | 设计完成 |
| 11 | 完善 WebScraper Provider | P2 | 设计完成 |
| 12 | thinking 模型识别扩展 | P1 | 设计完成 |
| 13 | ProviderKind 扩展 | P0 | 设计完成 |

### 1.2 架构决策记录

#### ADR-001: 模块命名

- **状态**: 已接受
- **决策**: 使用 `ullm`（而非 `ullm-almm`）
- **理由**: Rust 生态偏好简洁命名（tokio/reqwest/hyper）；`ullm` 已隐含"通用 LLM 全生命周期管理"语义；双缩写不直观
- **影响**: 所有 `ide_llm` 引用改为 `ullm`，目录 `crates/ide-llm` 改为 `crates/ullm`

#### ADR-002: 新供应商实现策略

- **状态**: 已接受（修订）
- **决策**: DeepSeek/Kimi/Qwen/OpenRouter/Gemini/Mistral/Groq 基于 `OpenAiCompatibleProvider` 工厂函数扩展；GLM 因 JWT Token 生成需求创建独立文件 `glm.rs`
- **理由**: 七大供应商均提供 OpenAI 兼容 API，复用 `OpenAiCompatibleProvider` 可减少 80% 代码量；GLM 旧版 API Key（`{id}.{secret}` 格式）需 JWT 签名转换，无法直接使用 `OpenAiCompatibleProvider` 的标准 Bearer 认证流程
- **影响**: GLM 新增 `provider/glm.rs`（约 120 行），其余 7 个供应商仅需工厂函数（每个约 30 行）

#### ADR-003: Portal Auth 框架设计

- **状态**: 已接受
- **决策**: 新增 `portal_auth` 子模块，作为 `CredentialsProvider` 的扩展实现
- **理由**: 与现有凭证体系无缝集成；Portal Auth 本质是"另一种获取 API Key 的方式"
- **影响**: 新增 `portal_auth/mod.rs`，修改 `credential/mod.rs` 增加组合器

#### ADR-004: 凭证别名策略

- **状态**: 已接受
- **决策**: Qwen 和 Kimi 使用 `AliasCredentialProvider` 包装 `EnvCredentialProvider`，支持多个环境变量别名
- **理由**: `EnvCredentialProvider::env_var_names()` 根据 `provider_id` 自动生成环境变量名（如 `qwen` → `QWEN_API_KEY`），但 Qwen 实际使用 `DASHSCOPE_API_KEY`，Kimi 也接受 `MOONSHOT_API_KEY`。需在自动生成逻辑之外支持别名
- **影响**: 新增 `credential/alias.rs`（约 40 行），Qwen/Kimi 工厂函数使用 `AliasCredentialProvider` 包装

#### ADR-005: 依赖防幻觉校验

- **状态**: 已接受
- **决策**: 所有新增依赖必须在 crates.io 上验证存在且 License 为 MIT/Apache-2.0
- **理由**: 项目铁律 #2（所有依赖在 crates.io 存在）和铁律 #3（License 为 MIT/Apache-2.0）要求每个依赖提供可验证的证据
- **影响**: 第十节依赖清单包含 crates.io 链接和 License 信息

#### ADR-006: ProviderKind 序列化稳定性

- **状态**: 已接受（修订）
- **决策**: 新增 `ProviderKind` 变体的 `Serialize`/`Deserialize` 使用个别 `#[serde(rename = "...")]` 属性，名称与 `Display` 实现一致；`OpenAiCompatible(String)` 使用 `#[serde(untagged)]` 实现配置文件中的纯字符串表示
- **理由**: `#[serde(rename_all = "kebab-case")]` 会将 `OpenAi` 序列化为 `"open-ai"`（应为 `"openai"`）、`DeepSeek` 序列化为 `"deep-seek"`（应为 `"deepseek"`），导致配置文件断裂。个别 `rename` 属性可精确控制每个变体的序列化名称。`untagged` 使 `OpenAiCompatible("foo")` 序列化为 `"foo"` 而非 `{"OpenAiCompatible":"foo"}`，提升配置文件可读性
- **影响**: 配置文件中供应商名称使用与 `Display` 一致的小写字符串；`OpenAiCompatible` 变体在反序列化时作为最后匹配的兜底选项

---

## 二、模块重命名方案（ide-llm → ullm）

### 2.1 文件变更清单

#### A. 目录重命名

```
crates/ide-llm/ → crates/ullm/
```

#### B. Cargo.toml 修改

| 文件 | 修改前 | 修改后 |
|------|--------|--------|
| `crates/ullm/Cargo.toml` | `name = "ide-llm"` | `name = "ullm"` |
| `crates/ullm/Cargo.toml` | `description = "Universal LLM lifecycle management module for AI IDE"` | `description = "Universal LLM Access Lifecycle Management Module"` |
| `Cargo.toml`（根） | `"crates/ide-llm"` | `"crates/ullm"` |
| `Cargo.toml`（根） | `ide-llm = { path = "crates/ide-llm" }` | `ullm = { path = "crates/ullm" }` |
| `crates/ide-core/Cargo.toml` | `ide-llm = { path = "../ide-llm" }` | `ullm = { path = "../ullm" }` |

#### C. Rust 源码引用替换

**全局替换规则**: `ide_llm` → `ullm`，`ide-llm` → `ullm`

| 文件 | 修改内容 |
|------|----------|
| `src/lib.rs` | `pub use ide_llm;` → `pub use ullm;` |
| `crates/ide-core/src/lib.rs` | `pub use ide_llm;` → `pub use ullm;` |
| `crates/ide-core/src/llm/mod.rs` | 3 处 `pub use ide_llm::` → `pub use ullm::` + 注释更新 |
| `crates/ide-core/src/scheduler/actor.rs` | 8 处 `ide_llm::` → `ullm::` |
| `crates/ide-core/src/context/snapshot.rs` | 2 处 `ide_llm::` → `ullm::` |
| `crates/ide-core/src/context/monitor.rs` | 2 处 `ide_llm::` → `ullm::` |
| `crates/ide-core/src/llm/claude.rs` | 废弃消息 `"Use ide-llm crate directly"` → `"Use ullm crate directly"` |
| `crates/ide-core/src/llm/codegee.rs` | 废弃消息同上 |

#### D. 内部模块引用

`crates/ullm/src/lib.rs` 中所有 `crate::` 路径无需修改（crate 内部引用不变）。

### 2.2 执行顺序

1. 重命名目录 `crates/ide-llm/` → `crates/ullm/`
2. 修改 `crates/ullm/Cargo.toml`
3. 修改根 `Cargo.toml`
4. 修改 `crates/ide-core/Cargo.toml`
5. 全局替换 Rust 源码中的 `ide_llm` → `ullm`
6. 运行 `cargo build` 验证编译通过
7. 运行 `cargo test` 验证测试通过

---

## 三、新增供应商详细设计

### 3.1 供应商总览

| 供应商 | Provider ID | API 协议 | API Base URL | 认证环境变量 | 凭证别名 | 实现方式 |
|--------|------------|----------|-------------|-------------|----------|----------|
| GLM（智谱） | `glm` | OpenAI 兼容 | `https://open.bigmodel.cn/api/paas/v4` | `GLM_API_KEY` | 无 | 独立文件（JWT） |
| DeepSeek | `deepseek` | OpenAI 兼容 | `https://api.deepseek.com` | `DEEPSEEK_API_KEY` | 无 | 工厂函数 |
| Kimi（月之暗面） | `kimi` | OpenAI 兼容 | `https://api.moonshot.cn/v1` | `KIMI_API_KEY` | `MOONSHOT_API_KEY` | 工厂函数 + AliasCredentialProvider |
| Qwen（千问） | `qwen` | OpenAI 兼容 | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `QWEN_API_KEY` | `DASHSCOPE_API_KEY` | 工厂函数 + AliasCredentialProvider |
| OpenRouter | `openrouter` | OpenAI 兼容 | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` | 无 | 工厂函数 |
| Google Gemini | `gemini` | OpenAI 兼容 | `https://generativelanguage.googleapis.com/v1beta/openai` | `GEMINI_API_KEY` | 无 | 工厂函数 |
| Mistral | `mistral` | OpenAI 兼容 | `https://api.mistral.ai/v1` | `MISTRAL_API_KEY` | 无 | 工厂函数 |
| Groq | `groq` | OpenAI 兼容 | `https://api.groq.com/openai/v1` | `GROQ_API_KEY` | 无 | 工厂函数 |

### 3.2 GLM（智谱 AI）供应商

#### 模型规格

| 模型 ID | 显示名 | 上下文窗口 | 最大输出 | 工具 | 图片 | 推理 |
|---------|--------|-----------|---------|------|------|------|
| `glm-4-plus` | GLM-4 Plus | 128,000 | 4,096 | ✅ | ✅ | ❌ |
| `glm-4-0520` | GLM-4 | 128,000 | 4,096 | ✅ | ✅ | ❌ |
| `glm-4-air` | GLM-4 Air | 128,000 | 4,096 | ✅ | ❌ | ❌ |
| `glm-4-airx` | GLM-4 AirX | 8,192 | 4,096 | ✅ | ❌ | ❌ |
| `glm-4-long` | GLM-4 Long | 1,000,000 | 4,096 | ✅ | ❌ | ❌ |
| `glm-4-flash` | GLM-4 Flash | 128,000 | 4,096 | ✅ | ❌ | ❌ |
| `glm-4v` | GLM-4V | 128,000 | 1,024 | ❌ | ✅ | ❌ |
| `glm-4.7-flash` | GLM-4.7 Flash（免费） | 200,000 | 4,096 | ✅ | ❌ | ❌ |
| `glm-z1-air` | GLM-Z1 Air | 128,000 | 4,096 | ✅ | ❌ | ✅ |
| `glm-z1-airx` | GLM-Z1 AirX | 8,192 | 4,096 | ✅ | ❌ | ✅ |
| `glm-z1-flash` | GLM-Z1 Flash | 128,000 | 4,096 | ✅ | ❌ | ✅ |
| `glm-5` | GLM-5 | 200,000 | 16,384 | ✅ | ✅ | ❌ |
| `glm-5.1` | GLM-5.1 | 200,000 | 16,384 | ✅ | ✅ | ✅ |
| `glm-5-turbo` | GLM-5 Turbo | 200,000 | 16,384 | ✅ | ✅ | ❌ |

#### API 特性

- 认证头: `Authorization: Bearer {api_key}`
- 请求体: 标准 OpenAI Chat Completions 格式
- SSE 流: 标准 OpenAI SSE 格式
- Thinking 模式: 通过 `extra_body.thinking.type = "enabled"` 开启
- Function Calling: 标准 OpenAI tools 格式
- 特殊处理: GLM API Key 格式为 `{id}.{secret}`，旧版需要生成 JWT Token；新版（2025+）支持直接使用 API Key

#### JWT Token 生成（旧版兼容）

GLM 旧版 API Key 格式为 `{id}.{secret}`，需要生成 JWT Token 作为 Bearer Token。

**⚠️ 关键约束**：GLM JWT Header 必须包含 `"sign_type": "SIGN"` 字段，而 `jsonwebtoken` crate 的 `Header` 结构体不支持自定义字段。因此必须使用 `hmac`+`sha2`+`base64` 手动构造 JWT，**不可使用 `jsonwebtoken` crate**。

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::Engine;

type HmacSha256 = Hmac<Sha256>;

fn generate_jwt_token(api_key: &str) -> Result<String, LlmError> {
    let parts: Vec<&str> = api_key.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Ok(api_key.to_string());
    }
    let id = parts[0];
    let secret = parts[1];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as usize;

    let header = serde_json::json!({
        "alg": "HS256",
        "sign_type": "SIGN"
    });
    let payload = serde_json::json!({
        "api_key": id,
        "exp": now + 3600,
        "timestamp": now
    });

    let header_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_string(&header).unwrap());
    let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_string(&payload).unwrap());

    let message = format!("{}.{}", header_b64, payload_b64);

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| LlmError::Other(format!("HMAC init failed: {}", e)))?;
    mac.update(message.as_bytes());
    let signature = mac.finalize().into_bytes();
    let signature_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signature);

    Ok(format!("{}.{}", message, signature_b64))
}
```

#### 实现文件

新增 `crates/ullm/src/provider/glm.rs`

```rust
use std::sync::Arc;
use async_trait::async_trait;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::Engine;

use crate::credential::CredentialsProvider;
use crate::error::LlmError;
use crate::provider::{LanguageModel, LanguageModelProvider};
use crate::provider::types::*;
use crate::stream::StreamFuture;

type HmacSha256 = Hmac<Sha256>;

pub struct GlmModel {
    inner: crate::provider::openai_compatible::OpenAiCompatibleModel,
}

impl GlmModel {
    fn needs_jwt(api_key: &str) -> bool {
        api_key.contains('.')
    }

    fn generate_jwt_token(api_key: &str) -> Result<String, LlmError> {
        let parts: Vec<&str> = api_key.splitn(2, '.').collect();
        if parts.len() != 2 {
            return Ok(api_key.to_string());
        }
        let id = parts[0];
        let secret = parts[1];
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as usize;
        let header = serde_json::json!({"alg": "HS256", "sign_type": "SIGN"});
        let payload = serde_json::json!({"api_key": id, "exp": now + 3600, "timestamp": now});
        let header_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_string(&header).unwrap());
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_string(&payload).unwrap());
        let message = format!("{}.{}", header_b64, payload_b64);
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| LlmError::Other(format!("HMAC init failed: {}", e)))?;
        mac.update(message.as_bytes());
        let sig_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(mac.finalize().into_bytes());
        Ok(format!("{}.{}", message, sig_b64))
    }
}

#[async_trait]
impl LanguageModel for GlmModel {
    fn id(&self) -> &ModelId { self.inner.id() }
    fn name(&self) -> &ModelName { self.inner.name() }
    fn provider_id(&self) -> &ProviderId { self.inner.provider_id() }
    fn provider_name(&self) -> &ProviderName { self.inner.provider_name() }
    fn supports_tools(&self) -> bool { self.inner.supports_tools() }
    fn supports_streaming_tools(&self) -> bool { self.inner.supports_streaming_tools() }
    fn supports_images(&self) -> bool { self.inner.supports_images() }
    fn supports_thinking(&self) -> bool { self.inner.supports_thinking() }
    fn max_token_count(&self) -> u64 { self.inner.max_token_count() }
    fn max_output_tokens(&self) -> Option<u64> { self.inner.max_output_tokens() }
    fn count_tokens(&self, request: &LanguageModelRequest) -> crate::token_count::TokenCountFuture<'_> {
        self.inner.count_tokens(request)
    }
    fn stream_completion(&self, request: LanguageModelRequest) -> StreamFuture<'_> {
        let credentials = self.inner.credentials().clone();
        let provider_id = self.inner.provider_id_str().to_string();
        Box::pin(async move {
            let raw_key = credentials.get_api_key(&provider_id).await?;
            let bearer_token = if Self::needs_jwt(&raw_key) {
                Self::generate_jwt_token(&raw_key)?
            } else {
                raw_key
            };
            self.inner.stream_completion_with_token(request, &bearer_token).await
        })
    }
}

pub struct GlmProvider {
    inner: crate::provider::openai_compatible::OpenAiCompatibleProvider,
}

fn glm_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        ("glm-4-plus".into(), "GLM-4 Plus".into(), 128000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 128000 }),
        ("glm-4-0520".into(), "GLM-4".into(), 128000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 128000 }),
        ("glm-4-air".into(), "GLM-4 Air".into(), 128000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 128000 }),
        ("glm-4-airx".into(), "GLM-4 AirX".into(), 8192, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 8192 }),
        ("glm-4-long".into(), "GLM-4 Long".into(), 1000000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 1000000 }),
        ("glm-4-flash".into(), "GLM-4 Flash".into(), 128000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 128000 }),
        ("glm-4v".into(), "GLM-4V".into(), 128000, Some(1024),
         ModelCapabilities { tools: false, streaming_tools: false, images: true, thinking: false, parallel_tool_calls: false, max_tokens: 128000 }),
        ("glm-4.7-flash".into(), "GLM-4.7 Flash".into(), 200000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 200000 }),
        ("glm-z1-air".into(), "GLM-Z1 Air".into(), 128000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: true, parallel_tool_calls: true, max_tokens: 128000 }),
        ("glm-z1-airx".into(), "GLM-Z1 AirX".into(), 8192, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: true, parallel_tool_calls: true, max_tokens: 8192 }),
        ("glm-z1-flash".into(), "GLM-Z1 Flash".into(), 128000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: true, parallel_tool_calls: true, max_tokens: 128000 }),
        ("glm-5".into(), "GLM-5".into(), 200000, Some(16384),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 200000 }),
        ("glm-5.1".into(), "GLM-5.1".into(), 200000, Some(16384),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: true, parallel_tool_calls: true, max_tokens: 200000 }),
        ("glm-5-turbo".into(), "GLM-5 Turbo".into(), 200000, Some(16384),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 200000 }),
    ]
}

impl GlmProvider {
    pub fn new() -> Self {
        Self {
            inner: crate::provider::openai_compatible::OpenAiCompatibleProvider::new(
                "glm",
                "GLM（智谱AI）",
                "https://open.bigmodel.cn/api/paas/v4",
                Some("GLM_API_KEY".to_string()),  // 显式指定，与自动生成结果一致；传 None 也可
                glm_models(),
            ),
        }
    }
}

#[async_trait]
impl LanguageModelProvider for GlmProvider {
    fn id(&self) -> &ProviderId { self.inner.id() }
    fn name(&self) -> &ProviderName { self.inner.name() }

    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
        self.inner.provided_models_raw().into_iter().map(|model| {
            Arc::new(GlmModel {
                inner: model,
            }) as Arc<dyn LanguageModel>
        }).collect()
    }

    fn is_authenticated(&self) -> bool { self.inner.is_authenticated() }
    async fn authenticate(&self) -> Result<(), LlmError> { self.inner.authenticate().await }
    async fn reset_credentials(&self) -> Result<(), LlmError> { self.inner.reset_credentials().await }
    fn configuration(&self) -> &ProviderConfig { self.inner.configuration() }
}
```

**关键实现细节**：

1. `GlmModel` 包装 `OpenAiCompatibleModel`，委托所有 `LanguageModel` 方法给内部实例
2. `stream_completion` 中拦截认证流程：先获取原始 API Key，检测是否包含 `.`，若包含则生成 JWT Token
3. 需要在 `OpenAiCompatibleModel` 上新增 `credentials()` 和 `provider_id_str()` 访问器方法（当前为私有字段）
4. 需要将 `OpenAiCompatibleModel::stream_completion` 重构为 `stream_completion_inner` + 两个公开入口（见下方详细规格）
5. `GlmProvider` 包装 `OpenAiCompatibleProvider`，通过 `provided_models_raw()` 获取原始模型实例包装为 `GlmModel`
6. `OpenAiCompatibleModel` 需新增 `extra_headers: HashMap<String, String>` 和 `max_request_body_bytes: Option<usize>` 字段，从 `ProviderConfig` 读取
7. `OpenAiCompatibleProvider` 需新增 `with_extra_headers()` 和 `with_max_request_body_bytes()` 构建器方法，供 OpenRouter/Qwen 等工厂函数使用

**⚠️ `OpenAiCompatibleProvider::new()` 当前内部行为锚点**（源码 `openai_compatible.rs` 第 260-311 行）：

| 行为 | 当前实现 | 对新供应商的影响 |
|------|---------|----------------|
| 参数类型 | `id: impl Into<String>`, `name: impl Into<String>`, `api_url: impl Into<String>` | 工厂函数可直接传 `&str` 字面量 |
| 默认凭证 | `Arc::new(EnvCredentialProvider::new())` | Kimi/Qwen 需 `.with_credentials(Arc::new(alias_provider))` 替换 |
| 默认限流 | `RateLimiter::new(5)`（最大 5 并发） | 所有新供应商共享此默认值 |
| `api_key_env` 为 `None` 时 | 自动生成 `format!("{}_API_KEY", id.to_uppercase().replace('-', "_"))` | Kimi/Qwen 传 `None` 即可，自动生成 `KIMI_API_KEY`/`QWEN_API_KEY` |
| `ProviderConfig.api_key_env` | 始终为 `Some(...)`（即使传入 `None`，也会填入自动生成的值） | 配置文件中始终有 `api_key_env` 字段 |
| `model_specs` vs `config.models` | `model_specs` 保留原始元组，`config.models` 转换为 `ModelConfig` | `provided_models()`/`provided_models_raw()` 遍历 `model_specs`，非 `config.models` |
| HTTP Client | `Client::builder().timeout(DEFAULT_TIMEOUT).build()`，`DEFAULT_TIMEOUT = 120s` | 所有新供应商共享此超时 |

**OpenAiCompatibleModel 需新增的访问器方法**：

```rust
impl OpenAiCompatibleModel {
    pub fn credentials(&self) -> &Arc<dyn CredentialsProvider> { &self.credentials }
    pub fn provider_id_str(&self) -> &str { &self.provider_id_str }
}
```

**OpenAiCompatibleModel 结构体新增字段**：

在现有 `OpenAiCompatibleModel` 结构体中新增两个字段（从 `ProviderConfig` 读取）：

```rust
pub struct OpenAiCompatibleModel {
    // ... 现有字段 ...
    credentials: Arc<dyn CredentialsProvider>,
    provider_id_str: String,
    rate_limiter: Arc<RateLimiter>,
    extra_headers: HashMap<String, String>,          // 新增：自定义 HTTP Headers（OpenRouter 等）
    max_request_body_bytes: Option<usize>,            // 新增：请求体大小限制（Qwen 6MB 等）
}
```

**`OpenAiCompatibleModel::new()` 签名更新**：

在现有 12 参数基础上新增 2 个参数：

```rust
pub fn new(
    model_id: &str,
    display_name: &str,
    provider_id: ProviderId,
    provider_name: ProviderName,
    max_tokens: u64,
    max_output: Option<u64>,
    capabilities: ModelCapabilities,
    client: Client,
    api_url: String,
    credentials: Arc<dyn CredentialsProvider>,
    provider_id_str: String,
    rate_limiter: Arc<RateLimiter>,
    extra_headers: HashMap<String, String>,           // 新增
    max_request_body_bytes: Option<usize>,             // 新增
) -> Self
```

**`provided_models()` 和 `provided_models_raw()` 传递新字段**：

两个方法在构造 `OpenAiCompatibleModel` 时必须传递 `extra_headers` 和 `max_request_body_bytes`：

```rust
fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
    self.model_specs.iter().map(|(mid, dname, max_t, max_o, caps)| {
        Arc::new(OpenAiCompatibleModel::new(
            mid, dname,
            self.config.id.clone(),
            self.config.name.clone(),
            *max_t, *max_o,
            caps.clone(),
            self.client.clone(),
            self.config.api_url.clone(),
            self.credentials.clone(),
            self.config.id.to_string(),
            self.rate_limiter.clone(),
            self.config.extra_headers.clone(),                    // 新增
            self.config.max_request_body_bytes,                   // 新增
        )) as Arc<dyn LanguageModel>
    }).collect()
}
```

**OpenAiCompatibleModel 流式补全重构方案**：

将现有 `stream_completion` 拆分为内部方法 `stream_completion_inner` 和两个公开入口，避免代码重复：

> **⚠️ 重构关键变更**（对比当前源码 `openai_compatible.rs` 第 206-248 行）：
> 1. **请求体发送方式**：当前 `.json(&body)` → 重构为 `.body(body_bytes)` + 手动 `.header("Content-Type", "application/json")`。原因：需先序列化 body 为字节以检查大小（`max_request_body_bytes`），`.json()` 会重复序列化
> 2. **请求体大小检查**：新增 `serde_json::to_vec(&body)` 序列化后检查 `body_bytes.len() > max_bytes`，超限返回 `LlmError::RequestBodySizeExceeded`
> 3. **额外 Headers**：新增 `for (key, value) in &self.extra_headers` 遍历附加到 request builder
> 4. **认证解耦**：当前 `stream_completion` 内部获取 API Key → 重构为 `stream_completion` 获取 Key 后传入 `stream_completion_inner`，`stream_completion_with_token` 接受外部 Token（供 `GlmModel` JWT 使用）

```rust
impl OpenAiCompatibleModel {
    // ⚠️ 以下方法依赖 openai_compatible.rs 已有导入：
    // use crate::stream::{ModelStream, StreamEvent, StreamFuture};
    // use crate::stream::openai_mapper::OpenAiSseMapper;
    // use crate::stream::sse::SseParser;
    // 无需额外新增导入
    fn stream_completion_inner(
        &self, request: LanguageModelRequest, bearer_token: &str,
    ) -> StreamFuture<'_> {
        Box::pin(async move {
            let _permit = self.rate_limiter.acquire_owned().await?;
            let body = self.build_request_body(&request);

            let body_bytes = serde_json::to_vec(&body)
                .map_err(|e| LlmError::Other(format!("Serialize request body failed: {}", e)))?;
            if let Some(max_bytes) = self.max_request_body_bytes {
                if body_bytes.len() > max_bytes {
                    return Err(LlmError::RequestBodySizeExceeded {
                        estimated_bytes: body_bytes.len(),
                        max_bytes,
                    });
                }
            }

            let url = format!("{}/chat/completions", self.api_url);
            let mut request_builder = self.client
                .post(&url)
                .header("Authorization", format!("Bearer {}", bearer_token))
                .header("Content-Type", "application/json");
            for (key, value) in &self.extra_headers {
                request_builder = request_builder.header(key.as_str(), value.as_str());
            }
            let response = request_builder
                .body(body_bytes)
                .send()
                .await
                .map_err(LlmError::from)?;
            let status = response.status();
            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                return Err(LlmError::from_http_status(status, &body_text));
            }
            let byte_stream = response.bytes_stream();
            let mapper = OpenAiSseMapper;
            let parser = SseParser::new();
            let stream: ModelStream = Box::pin(byte_stream.scan(
                parser,
                move |parser, chunk_result| {
                    let events = match chunk_result {
                        Ok(bytes) => parser.push(&bytes, &mapper),
                        Err(e) => Err(LlmError::StreamError(e.to_string())),
                    };
                    let events = events.unwrap_or_else(|e| vec![StreamEvent::Error(e.to_string())]);
                    std::future::ready(Some(events))
                },
            ).flat_map(|events| futures_util::stream::iter(events.into_iter().map(Ok))));
            Ok(stream)
        })
    }

    fn stream_completion(&self, request: LanguageModelRequest) -> StreamFuture<'_> {
        let credentials = self.credentials.clone();
        let provider_id = self.provider_id_str.to_string();
        Box::pin(async move {
            let api_key = credentials.get_api_key(&provider_id).await?;
            self.stream_completion_inner(request, &api_key).await
        })
    }

    pub fn stream_completion_with_token(
        &self, request: LanguageModelRequest, bearer_token: &str,
    ) -> StreamFuture<'_> {
        self.stream_completion_inner(request, bearer_token)
    }
}
```

**OpenAiCompatibleProvider 需新增的方法**：

`GlmProvider::provided_models()` 需要获取 `OpenAiCompatibleModel` 原始实例以包装为 `GlmModel`，但现有 `provided_models()` 返回 `Vec<Arc<dyn LanguageModel>>` 无法向下转型。需新增：

```rust
impl OpenAiCompatibleProvider {
    pub fn provided_models_raw(&self) -> Vec<Arc<OpenAiCompatibleModel>> {
        self.model_specs.iter().map(|(mid, dname, max_t, max_o, caps)| {
            Arc::new(OpenAiCompatibleModel::new(
                mid, dname,
                self.config.id.clone(),
                self.config.name.clone(),
                *max_t, *max_o,
                caps.clone(),
                self.client.clone(),
                self.config.api_url.clone(),
                self.credentials.clone(),
                self.config.id.to_string(),
                self.rate_limiter.clone(),
                self.config.extra_headers.clone(),                    // 新增
                self.config.max_request_body_bytes,                   // 新增
            ))
        }).collect()
    }

    pub fn with_extra_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.config.extra_headers = headers;
        self
    }

    pub fn with_max_request_body_bytes(mut self, max_bytes: usize) -> Self {
        self.config.max_request_body_bytes = Some(max_bytes);
        self
    }
}
```

**`OpenAiCompatibleProvider::new()` 中 `ProviderConfig` 初始化更新**：

在现有 `ProviderConfig` 构造处补充新字段：

```rust
let config = ProviderConfig {
    id: ProviderId::new(&id_str),
    name: ProviderName::new(&name_str),
    api_url: api_url_str.clone(),
    api_key_env: Some(env_key),
    models: model_configs,
    rate_limit: None,
    retry: None,
    extra_headers: HashMap::new(),               // 新增
    max_request_body_bytes: None,                 // 新增
};
```

### 3.3 DeepSeek 供应商

#### 模型规格

| 模型 ID | 显示名 | 上下文窗口 | 最大输出 | 工具 | 图片 | 推理 |
|---------|--------|-----------|---------|------|------|------|
| `deepseek-chat` | DeepSeek Chat | 64,000 | 8,192 | ✅ | ❌ | ❌ |
| `deepseek-reasoner` | DeepSeek Reasoner | 64,000 | 8,192 | ✅ | ❌ | ✅ |

#### API 特性

- 认证头: `Authorization: Bearer {api_key}`
- 请求体: 标准 OpenAI Chat Completions 格式
- SSE 流: 标准 OpenAI SSE 格式
- API Base URL: `https://api.deepseek.com`（也兼容 `https://api.deepseek.com/v1`）
- 推理模型 `deepseek-reasoner`: 返回 `reasoning_content` 字段（需扩展 StreamEvent）

#### 实现文件

新增 `crates/ullm/src/provider/deepseek.rs`

```rust
use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn deepseek_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        ("deepseek-chat".into(), "DeepSeek Chat".into(), 64000, Some(8192),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 64000 }),
        ("deepseek-reasoner".into(), "DeepSeek Reasoner".into(), 64000, Some(8192),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: true, parallel_tool_calls: true, max_tokens: 64000 }),
    ]
}

pub fn create_deepseek_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "deepseek",
        "DeepSeek",
        "https://api.deepseek.com",
        Some("DEEPSEEK_API_KEY".to_string()),
        deepseek_models(),
    )
}
```

**凭证环境变量映射**：`EnvCredentialProvider::env_var_names("deepseek")` 自动生成 `["DEEPSEEK_API_KEY", "DEEPSEEK_API_KEY", "DEEPSEEK"]`，与 `api_key_env = Some("DEEPSEEK_API_KEY")` 一致，无需别名。

**特殊处理**: `deepseek-reasoner` 的 SSE 响应中 `delta` 包含 `reasoning_content` 字段，需在 `OpenAiSseMapper` 中增加处理逻辑，映射为 `StreamEvent::Thinking`。

### 3.4 Kimi（月之暗面）供应商

#### 模型规格

| 模型 ID | 显示名 | 上下文窗口 | 最大输出 | 工具 | 图片 | 推理 |
|---------|--------|-----------|---------|------|------|------|
| `moonshot-v1-8k` | Kimi V1 8K | 8,192 | 4,096 | ✅ | ❌ | ❌ |
| `moonshot-v1-32k` | Kimi V1 32K | 32,768 | 4,096 | ✅ | ❌ | ❌ |
| `moonshot-v1-128k` | Kimi V1 128K | 131,072 | 4,096 | ✅ | ❌ | ❌ |
| `kimi-k2-0711` | Kimi K2 | 256,000 | 16,384 | ✅ | ❌ | ✅ |
| `kimi-latest` | Kimi Latest | 131,072 | 8,192 | ✅ | ❌ | ❌ |

#### API 特性

- 认证头: `Authorization: Bearer {api_key}`
- 请求体: 标准 OpenAI Chat Completions 格式
- SSE 流: 标准 OpenAI SSE 格式
- 无特殊处理，完全兼容
- 环境变量: `KIMI_API_KEY` 或 `MOONSHOT_API_KEY`

#### 实现文件

新增 `crates/ullm/src/provider/kimi.rs`

```rust
use std::sync::Arc;
use crate::credential::{CredentialsProvider, AliasCredentialProvider, EnvCredentialProvider};
use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn kimi_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        ("moonshot-v1-8k".into(), "Kimi V1 8K".into(), 8192, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 8192 }),
        ("moonshot-v1-32k".into(), "Kimi V1 32K".into(), 32768, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 32768 }),
        ("moonshot-v1-128k".into(), "Kimi V1 128K".into(), 131072, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 131072 }),
        ("kimi-k2-0711".into(), "Kimi K2".into(), 256000, Some(16384),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: true, parallel_tool_calls: true, max_tokens: 256000 }),
        ("kimi-latest".into(), "Kimi Latest".into(), 131072, Some(8192),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 131072 }),
    ]
}

pub fn create_kimi_provider() -> OpenAiCompatibleProvider {
    let alias_provider = AliasCredentialProvider::new(
        "kimi",
        Arc::new(EnvCredentialProvider::new()),
        vec!["MOONSHOT_API_KEY".to_string()],
    );
    OpenAiCompatibleProvider::new(
        "kimi",
        "Kimi（月之暗面）",
        "https://api.moonshot.cn/v1",
        None,
        kimi_models(),
    )
    .with_credentials(Arc::new(alias_provider))
}
```

**凭证别名处理**：`EnvCredentialProvider::env_var_names("kimi")` 自动生成 `["KIMI_API_KEY", "KIMI_API_KEY", "KIMI"]`，但 Kimi 官方也使用 `MOONSHOT_API_KEY`。通过 `AliasCredentialProvider` 包装，查找顺序为：`KIMI_API_KEY` → `MOONSHOT_API_KEY` → 配置文件。

### 3.5 Qwen（千问）供应商

#### 模型规格

| 模型 ID | 显示名 | 上下文窗口 | 最大输出 | 工具 | 图片 | 推理 |
|---------|--------|-----------|---------|------|------|------|
| `qwen-max` | Qwen Max | 32,768 | 8,192 | ✅ | ✅ | ❌ |
| `qwen-plus` | Qwen Plus | 131,072 | 8,192 | ✅ | ✅ | ❌ |
| `qwen-turbo` | Qwen Turbo | 1,000,000 | 8,192 | ✅ | ✅ | ❌ |
| `qwen-long` | Qwen Long | 10,000,000 | 6,000 | ❌ | ❌ | ❌ |
| `qwq-32b` | QwQ 32B | 131,072 | 16,384 | ✅ | ❌ | ✅ |
| `qwen-vl-max` | Qwen VL Max | 32,768 | 2,048 | ❌ | ✅ | ❌ |
| `qwen-coder-plus` | Qwen Coder Plus | 131,072 | 8,192 | ✅ | ❌ | ❌ |

#### API 特性

- 认证头: `Authorization: Bearer {api_key}`
- 请求体: 标准 OpenAI Chat Completions 格式
- SSE 流: 标准 OpenAI SSE 格式
- DashScope 兼容端点: `https://dashscope.aliyuncs.com/compatible-mode/v1`
- 请求体大小限制: 6MB（需在 ProviderConfig 中配置）

#### 实现文件

新增 `crates/ullm/src/provider/qwen.rs`

```rust
use std::sync::Arc;
use crate::credential::{CredentialsProvider, AliasCredentialProvider, EnvCredentialProvider};
use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn qwen_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        ("qwen-max".into(), "Qwen Max".into(), 32768, Some(8192),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 32768 }),
        ("qwen-plus".into(), "Qwen Plus".into(), 131072, Some(8192),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 131072 }),
        ("qwen-turbo".into(), "Qwen Turbo".into(), 1000000, Some(8192),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 1000000 }),
        ("qwen-long".into(), "Qwen Long".into(), 10000000, Some(6000),
         ModelCapabilities { tools: false, streaming_tools: false, images: false, thinking: false, parallel_tool_calls: false, max_tokens: 10000000 }),
        ("qwq-32b".into(), "QwQ 32B".into(), 131072, Some(16384),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: true, parallel_tool_calls: true, max_tokens: 131072 }),
        ("qwen-vl-max".into(), "Qwen VL Max".into(), 32768, Some(2048),
         ModelCapabilities { tools: false, streaming_tools: false, images: true, thinking: false, parallel_tool_calls: false, max_tokens: 32768 }),
        ("qwen-coder-plus".into(), "Qwen Coder Plus".into(), 131072, Some(8192),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 131072 }),
    ]
}

pub fn create_qwen_provider() -> OpenAiCompatibleProvider {
    let alias_provider = AliasCredentialProvider::new(
        "qwen",
        Arc::new(EnvCredentialProvider::new()),
        vec!["DASHSCOPE_API_KEY".to_string()],
    );
    OpenAiCompatibleProvider::new(
        "qwen",
        "Qwen（千问）",
        "https://dashscope.aliyuncs.com/compatible-mode/v1",
        None,
        qwen_models(),
    )
    .with_credentials(Arc::new(alias_provider))
    .with_max_request_body_bytes(6 * 1024 * 1024)
}
```

**凭证别名处理（关键）**：`EnvCredentialProvider::env_var_names("qwen")` 自动生成 `["QWEN_API_KEY", "QWEN_API_KEY", "QWEN"]`，但阿里云 DashScope 官方使用 `DASHSCOPE_API_KEY`。通过 `AliasCredentialProvider` 包装，查找顺序为：`QWEN_API_KEY` → `DASHSCOPE_API_KEY` → 配置文件。

**请求体大小限制**：Qwen DashScope 端点限制请求体 6MB，需在 `ProviderConfig.rate_limit` 中配置或在 `build_request_body` 中增加大小检查。

### 3.6 OpenRouter 供应商

| 模型 ID | 显示名 | 上下文窗口 | 最大输出 |
|---------|--------|-----------|---------|
| `openai/gpt-4o` | GPT-4o (via OpenRouter) | 128,000 | 16,384 |
| `anthropic/claude-sonnet-4` | Claude Sonnet 4 (via OpenRouter) | 200,000 | 64,000 |
| `google/gemini-2.5-pro` | Gemini 2.5 Pro (via OpenRouter) | 1,000,000 | 65,536 |
| `deepseek/deepseek-chat` | DeepSeek Chat (via OpenRouter) | 64,000 | 8,192 |
| `meta-llama/llama-3.3-70b-instruct` | Llama 3.3 70B (via OpenRouter) | 128,000 | 4,096 |

- API Base: `https://openrouter.ai/api/v1`
- 认证: `Authorization: Bearer {api_key}`
- 额外 Headers: `HTTP-Referer: https://github.com/ai-ide`, `X-Title: AI-IDE`（通过 `ProviderConfig.extra_headers` 配置，工厂函数使用 `.with_extra_headers()` 设置）
- 完全 OpenAI 兼容

#### 实现文件

新增 `crates/ullm/src/provider/openrouter.rs`

```rust
use std::collections::HashMap;
use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn openrouter_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        ("openai/gpt-4o".into(), "GPT-4o (via OpenRouter)".into(), 128000, Some(16384),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 128000 }),
        ("anthropic/claude-sonnet-4".into(), "Claude Sonnet 4 (via OpenRouter)".into(), 200000, Some(64000),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 200000 }),
        ("google/gemini-2.5-pro".into(), "Gemini 2.5 Pro (via OpenRouter)".into(), 1000000, Some(65536),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 1000000 }),
        ("deepseek/deepseek-chat".into(), "DeepSeek Chat (via OpenRouter)".into(), 64000, Some(8192),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 64000 }),
        ("meta-llama/llama-3.3-70b-instruct".into(), "Llama 3.3 70B (via OpenRouter)".into(), 128000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 128000 }),
    ]
}

pub fn create_openrouter_provider() -> OpenAiCompatibleProvider {
    let mut extra_headers = HashMap::new();
    extra_headers.insert("HTTP-Referer".to_string(), "https://github.com/ai-ide".to_string());
    extra_headers.insert("X-Title".to_string(), "AI-IDE".to_string());
    OpenAiCompatibleProvider::new(
        "openrouter",
        "OpenRouter",
        "https://openrouter.ai/api/v1",
        Some("OPENROUTER_API_KEY".to_string()),
        openrouter_models(),
    )
    .with_extra_headers(extra_headers)
}
```

### 3.7 Google Gemini 供应商

| 模型 ID | 显示名 | 上下文窗口 | 最大输出 |
|---------|--------|-----------|---------|
| `gemini-2.5-pro` | Gemini 2.5 Pro | 1,000,000 | 65,536 |
| `gemini-2.5-flash` | Gemini 2.5 Flash | 1,000,000 | 65,536 |
| `gemini-2.0-flash` | Gemini 2.0 Flash | 1,000,000 | 8,192 |

- API Base: `https://generativelanguage.googleapis.com/v1beta/openai`
- 认证: `Authorization: Bearer {api_key}`
- OpenAI 兼容模式（Google 已提供兼容端点）

#### 实现文件

新增 `crates/ullm/src/provider/gemini.rs`

```rust
use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn gemini_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        ("gemini-2.5-pro".into(), "Gemini 2.5 Pro".into(), 1000000, Some(65536),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 1000000 }),
        ("gemini-2.5-flash".into(), "Gemini 2.5 Flash".into(), 1000000, Some(65536),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 1000000 }),
        ("gemini-2.0-flash".into(), "Gemini 2.0 Flash".into(), 1000000, Some(8192),
         ModelCapabilities { tools: true, streaming_tools: true, images: true, thinking: false, parallel_tool_calls: true, max_tokens: 1000000 }),
    ]
}

pub fn create_gemini_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "gemini",
        "Google Gemini",
        "https://generativelanguage.googleapis.com/v1beta/openai",
        Some("GEMINI_API_KEY".to_string()),
        gemini_models(),
    )
}
```

### 3.8 Mistral 供应商

| 模型 ID | 显示名 | 上下文窗口 | 最大输出 |
|---------|--------|-----------|---------|
| `mistral-large-latest` | Mistral Large | 128,000 | 4,096 |
| `mistral-medium-latest` | Mistral Medium | 32,000 | 4,096 |
| `mistral-small-latest` | Mistral Small | 32,000 | 4,096 |
| `codestral-latest` | Codestral | 256,000 | 4,096 |

- API Base: `https://api.mistral.ai/v1`
- 认证: `Authorization: Bearer {api_key}`

#### 实现文件

新增 `crates/ullm/src/provider/mistral.rs`

```rust
use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn mistral_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        ("mistral-large-latest".into(), "Mistral Large".into(), 128000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 128000 }),
        ("mistral-medium-latest".into(), "Mistral Medium".into(), 32000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 32000 }),
        ("mistral-small-latest".into(), "Mistral Small".into(), 32000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 32000 }),
        ("codestral-latest".into(), "Codestral".into(), 256000, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 256000 }),
    ]
}

pub fn create_mistral_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "mistral",
        "Mistral",
        "https://api.mistral.ai/v1",
        Some("MISTRAL_API_KEY".to_string()),
        mistral_models(),
    )
}
```

### 3.9 Groq 供应商

| 模型 ID | 显示名 | 上下文窗口 | 最大输出 |
|---------|--------|-----------|---------|
| `llama-3.3-70b-versatile` | Llama 3.3 70B (Groq) | 128,000 | 32,768 |
| `llama-3.1-8b-instant` | Llama 3.1 8B (Groq) | 128,000 | 8,192 |
| `mixtral-8x7b-32768` | Mixtral 8x7B (Groq) | 32,768 | 4,096 |

- API Base: `https://api.groq.com/openai/v1`
- 认证: `Authorization: Bearer {api_key}`

#### 实现文件

新增 `crates/ullm/src/provider/groq.rs`

```rust
use crate::provider::openai_compatible::OpenAiCompatibleProvider;
use crate::provider::types::ModelCapabilities;

fn groq_models() -> Vec<(String, String, u64, Option<u64>, ModelCapabilities)> {
    vec![
        ("llama-3.3-70b-versatile".into(), "Llama 3.3 70B (Groq)".into(), 128000, Some(32768),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 128000 }),
        ("llama-3.1-8b-instant".into(), "Llama 3.1 8B (Groq)".into(), 128000, Some(8192),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 128000 }),
        ("mixtral-8x7b-32768".into(), "Mixtral 8x7B (Groq)".into(), 32768, Some(4096),
         ModelCapabilities { tools: true, streaming_tools: true, images: false, thinking: false, parallel_tool_calls: true, max_tokens: 32768 }),
    ]
}

pub fn create_groq_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "groq",
        "Groq",
        "https://api.groq.com/openai/v1",
        Some("GROQ_API_KEY".to_string()),
        groq_models(),
    )
}
```

### 3.10 供应商工厂函数设计

在 `crates/ullm/src/provider/factory.rs` 中新增工厂模块：

```rust
use std::sync::Arc;
use crate::provider::{LanguageModelProvider, ProviderKind};
use crate::provider::glm::GlmProvider;

pub fn create_glm_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(GlmProvider::new())
}

pub fn create_deepseek_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::deepseek::create_deepseek_provider())
}

pub fn create_kimi_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::kimi::create_kimi_provider())
}

pub fn create_qwen_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::qwen::create_qwen_provider())
}

pub fn create_openrouter_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::openrouter::create_openrouter_provider())
}

pub fn create_gemini_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::gemini::create_gemini_provider())
}

pub fn create_mistral_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::mistral::create_mistral_provider())
}

pub fn create_groq_provider() -> Arc<dyn LanguageModelProvider> {
    Arc::new(crate::provider::groq::create_groq_provider())
}

pub fn create_provider_by_kind(kind: &ProviderKind) -> Option<Arc<dyn LanguageModelProvider>> {
    match kind {
        ProviderKind::Glm => Some(create_glm_provider()),
        ProviderKind::DeepSeek => Some(create_deepseek_provider()),
        ProviderKind::Kimi => Some(create_kimi_provider()),
        ProviderKind::Qwen => Some(create_qwen_provider()),
        ProviderKind::OpenRouter => Some(create_openrouter_provider()),
        ProviderKind::Gemini => Some(create_gemini_provider()),
        ProviderKind::Mistral => Some(create_mistral_provider()),
        ProviderKind::Groq => Some(create_groq_provider()),
        _ => None,
    }
}
```

**OpenRouter 额外 Headers**：OpenRouter 需要在请求中添加 `HTTP-Referer` 和 `X-Title` 头。已在 `provider/openrouter.rs` 中通过 `ProviderConfig.extra_headers` + `.with_extra_headers()` 构建器设置，`stream_completion_inner` 中自动附加（见 §3.2 重构方案）。

---

## 四、ProviderKind 扩展

### 4.1 当前定义

```rust
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Ollama,
    XAI,
    CodeGeeX,
    WebScraper,
    OpenAiCompatible(String),
}
```

### 4.2 扩展后定义

> **注意**：以下为逻辑扩展视图（新增变体一览），最终实现见下方含 `#[serde(untagged)]` 和 `#[serde(rename)]` 属性的完整定义。

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Ollama,
    XAI,
    CodeGeeX,
    WebScraper,
    Glm,
    DeepSeek,
    Kimi,
    Qwen,
    OpenRouter,
    Gemini,
    Mistral,
    Groq,
    OpenAiCompatible(String),
}
```

**序列化名称约定**（ADR-006 修订）：`Serialize`/`Deserialize` 使用与 `Display` 实现一致的名称。**不可使用 `#[serde(rename_all = "kebab-case")]`**，因为 `kebab-case` 会将 `OpenAi` 序列化为 `"open-ai"`（应为 `"openai"`）、`DeepSeek` 序列化为 `"deep-seek"`（应为 `"deepseek"`），导致配置文件断裂。必须使用个别 `#[serde(rename = "...")]` 属性。

> **⚠️ 序列化迁移影响**：当前 `ProviderKind`（源码 `provider/mod.rs` 第 39-48 行）无任何 `#[serde(rename)]` 属性，serde 默认使用变体名的 PascalCase 形式（如 `OpenAi` → `"OpenAi"`）。新增 `#[serde(rename = "...")]` 后，现有配置文件中的 `"OpenAi"` 将无法反序列化为 `ProviderKind::OpenAi`（期望 `"openai"`）。**迁移策略**：在 `ProviderKind` 的 `Deserialize` 实现中，对旧名称（`"OpenAi"`/`"Anthropic"` 等）添加兼容性映射，或在配置加载层做一次性迁移。

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProviderKind {
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "xai")]
    XAI,
    #[serde(rename = "codegeex")]
    CodeGeeX,
    #[serde(rename = "web-scraper")]
    WebScraper,
    #[serde(rename = "glm")]
    Glm,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "kimi")]
    Kimi,
    #[serde(rename = "qwen")]
    Qwen,
    #[serde(rename = "openrouter")]
    OpenRouter,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "mistral")]
    Mistral,
    #[serde(rename = "groq")]
    Groq,
    OpenAiCompatible(String),
}
```

> **⚠️ `#[serde(untagged)]` 必须为枚举级属性**：serde 的 `untagged` 作为变体级属性仅适用于内部标记（`#[serde(tag = "...")]`）或相邻标记（`#[serde(tag = "...", content = "...")]`）枚举。当前 `ProviderKind` 使用默认的外部标记表示，`#[serde(untagged)]` 必须放在枚举定义上方。效果：所有变体序列化为纯字符串（`OpenAi` → `"openai"`，`OpenAiCompatible("foo")` → `"foo"`），反序列化按变体声明顺序依次尝试匹配，`OpenAiCompatible(String)` 作为最后兜底匹配任意字符串。

> **⚠️ `#[serde(untagged)]` 反序列化歧义风险**：`untagged` 枚举的反序列化按变体声明顺序依次尝试，**第一个成功匹配的变体胜出**。由于所有单元变体（`OpenAi`/`Anthropic` 等）都 rename 为字符串字面量，而 `OpenAiCompatible(String)` 匹配任意字符串，因此**必须将 `OpenAiCompatible(String)` 放在最后**，否则它会抢先匹配所有输入。当前定义已正确放置。但需注意：如果未来新增的 `rename` 值与某个已有 `rename` 值相同（如两个变体都 rename 为 `"glm"`），serde 不会报错，而是始终匹配先声明的变体——这是静默逻辑错误，需人工审查确保所有 `rename` 值唯一。

### 4.3 Display 实现

```rust
impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderKind::OpenAi => write!(f, "openai"),
            ProviderKind::Anthropic => write!(f, "anthropic"),
            ProviderKind::Ollama => write!(f, "ollama"),
            ProviderKind::XAI => write!(f, "xai"),
            ProviderKind::CodeGeeX => write!(f, "codegeex"),
            ProviderKind::WebScraper => write!(f, "web-scraper"),
            ProviderKind::Glm => write!(f, "glm"),
            ProviderKind::DeepSeek => write!(f, "deepseek"),
            ProviderKind::Kimi => write!(f, "kimi"),
            ProviderKind::Qwen => write!(f, "qwen"),
            ProviderKind::OpenRouter => write!(f, "openrouter"),
            ProviderKind::Gemini => write!(f, "gemini"),
            ProviderKind::Mistral => write!(f, "mistral"),
            ProviderKind::Groq => write!(f, "groq"),
            ProviderKind::OpenAiCompatible(name) => write!(f, "openai-compatible:{}", name),
        }
    }
}
```

---

## 五、Portal Auth 零成本访问框架

### 5.1 架构设计

Portal Auth 本质是一种**替代凭证获取方式**，与现有的 `EnvCredentialProvider`、`ConfigCredentialProvider`、`OAuthCredentialProvider` 并列。

```
CredentialsProvider (trait)
├── EnvCredentialProvider      ← 环境变量 API Key
├── ConfigCredentialProvider   ← 配置文件 API Key
├── OAuthCredentialProvider    ← OAuth Token
└── PortalAuthCredentialProvider ← Portal Auth 零成本凭证
    ├── QwenPortalAuth         ← 千问网页版
    ├── MiniMaxPortalAuth      ← 海螺AI 网页版
    ├── GeminiCliAuth          ← Gemini CLI 认证
    └── CopilotProxyAuth       ← Copilot 代理
```

### 5.2 PortalAuthCredentialProvider 设计

新增文件: `crates/ullm/src/credential/portal_auth.rs`

```rust
pub struct PortalAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub token_type: PortalAuthType,
}

pub enum PortalAuthType {
    Cookie { domain: String },
    Bearer { header_name: String },
    Custom { header_name: String, prefix: String },
}

#[async_trait]
pub trait PortalAuthStrategy: Send + Sync {
    fn name(&self) -> &str;
    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError>;
    async fn refresh(&self, refresh_token: &str) -> Result<PortalAuthToken, LlmError>;
    fn auth_type(&self) -> PortalAuthType;
}

pub struct PortalAuthCredentialProvider {
    strategy: Arc<dyn PortalAuthStrategy>,
    token: tokio::sync::RwLock<Option<PortalAuthToken>>,  // 异步 RwLock（因 get_api_key 是 async）
    env_fallback: EnvCredentialProvider,
}

impl PortalAuthCredentialProvider {
    pub fn new(strategy: Arc<dyn PortalAuthStrategy>) -> Self {
        Self {
            strategy,
            token: tokio::sync::RwLock::new(None),
            env_fallback: EnvCredentialProvider::new(),
        }
    }
}

> **⚠️ 为何使用 `tokio::sync::RwLock` 而非 `parking_lot::RwLock`**：现有 `credential/mod.rs` 中的 `EnvCredentialProvider`/`ConfigCredentialProvider`/`OAuthCredentialProvider` 均使用 `parking_lot::RwLock`（同步锁），因为它们的 `get_api_key()` 虽然是 `async fn`，但锁持有期间无 `.await` 点（仅内存 HashMap 读写）。而 `PortalAuthCredentialProvider::get_api_key()` 需要在持有读锁时调用 `self.strategy.refresh().await`，若使用 `parking_lot::RwLock`（非 Send 守卫），跨 `.await` 点会编译失败。`tokio::sync::RwLock` 的守卫是 `Send`，可安全跨 `.await`。但为避免死锁，**读锁范围必须最小化**：先获取读锁 → clone 需要的数据 → 释放读锁 → 执行 async 操作 → 获取写锁 → 写入结果。

#[async_trait]
impl CredentialsProvider for PortalAuthCredentialProvider {
    async fn get_api_key(&self, provider_id: &str) -> Result<String, LlmError> {
        // 1. 检查缓存的 Portal Auth Token（读锁范围最小化，避免死锁）
        let cached = {
            let guard = self.token.read().await;
            guard.as_ref().and_then(|t| {
                if let Some(expires) = t.expires_at {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    if now < expires {
                        Some(t.access_token.clone())
                    } else {
                        None
                    }
                } else {
                    Some(t.access_token.clone())
                }
            })
        };
        if let Some(access) = cached {
            return Ok(access);
        }

        // 2. 检查是否有过期 Token 可刷新（读锁 → 释放 → 异步刷新 → 写锁）
        let refresh_token = {
            let guard = self.token.read().await;
            guard.as_ref().and_then(|t| t.refresh_token.clone())
        };
        if let Some(refresh) = refresh_token {
            match self.strategy.refresh(&refresh).await {
                Ok(new_token) => {
                    let access = new_token.access_token.clone();
                    *self.token.write().await = Some(new_token);
                    return Ok(access);
                }
                Err(_) => {
                    *self.token.write().await = None;
                }
            }
        }

        // 3. 尝试 env_fallback
        if let Ok(key) = self.env_fallback.get_api_key(provider_id).await {
            return Ok(key);
        }

        // 4. 触发 Portal Auth 流程
        let new_token = self.strategy.authenticate().await?;
        let access = new_token.access_token.clone();
        *self.token.write().await = Some(new_token);
        Ok(access)
    }

    async fn state(&self, provider_id: &str) -> ApiKeyState {
        match self.get_api_key(provider_id).await {
            Ok(_) => ApiKeyState::Valid,
            Err(LlmError::ExpiredToken(_)) => ApiKeyState::NeedsRefresh,
            Err(LlmError::MissingCredentials { .. }) => ApiKeyState::Missing,
            _ => ApiKeyState::Invalid,
        }
    }

    async fn invalidate(&self, provider_id: &str) {
        *self.token.write().await = None;
        self.env_fallback.invalidate(provider_id).await;
    }
}
```

### 5.3 具体策略实现

#### QwenPortalAuth

```rust
pub struct QwenPortalAuth {
    client: Client,
    cookie_jar: Arc<cookie::Jar>,  // cookie crate 的 Jar（Cookie 存储容器）
}

> **`cookie::Jar` 类型说明**：`cookie` crate（v0.18）的 `Jar` 类型是线程安全的 Cookie 存储容器，实现 `Send + Sync`。用法：`jar.add(&url, "key=value")` 添加 Cookie，`jar.get(&url, "key")` 读取。与 `reqwest::cookie::Jar` 不同，`cookie::Jar` 是独立的 Cookie 解析/存储类型，不依赖 reqwest。

impl QwenPortalAuth {
    pub fn new() -> Self { ... }
}

#[async_trait]
impl PortalAuthStrategy for QwenPortalAuth {
    fn name(&self) -> &str { "qwen-portal" }

    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError> {
        // 1. 访问 tongyi.aliyun.com 获取 CSRF Token
        // 2. 使用浏览器 Cookie 进行认证
        // 3. 调用 /api/chat 接口获取 Access Token
        // 4. 返回 PortalAuthToken
    }

    async fn refresh(&self, refresh_token: &str) -> Result<PortalAuthToken, LlmError> {
        // 使用 refresh_token 刷新
    }

    fn auth_type(&self) -> PortalAuthType {
        PortalAuthType::Custom {
            header_name: "X-Access-Token".into(),
            prefix: "".into(),
        }
    }
}
```

#### MiniMaxPortalAuth

```rust
pub struct MiniMaxPortalAuth {
    client: Client,
}

#[async_trait]
impl PortalAuthStrategy for MiniMaxPortalAuth {
    fn name(&self) -> &str { "minimax-portal" }
    // 类似 QwenPortalAuth，目标网站 hailuoai.com
}
```

#### GeminiCliAuth

```rust
pub struct GeminiCliAuth {
    client: Client,
}

#[async_trait]
impl PortalAuthStrategy for GeminiCliAuth {
    fn name(&self) -> &str { "gemini-cli" }

    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError> {
        // 1. 检查本地 Gemini CLI 配置 (~/.gemini/)
        // 2. 读取 OAuth Token 缓存
        // 3. 若过期，执行 gemini auth 流程
        // 4. 返回 Token
    }
}
```

#### CopilotProxyAuth

```rust
pub struct CopilotProxyAuth {
    client: Client,
}

#[async_trait]
impl PortalAuthStrategy for CopilotProxyAuth {
    fn name(&self) -> &str { "copilot-proxy" }

    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError> {
        // 1. 读取 VS Code Copilot 扩展的 OAuth Token
        // 2. 路径: ~/.config/github-copilot/
        // 3. 验证 Token 有效性
        // 4. 返回 Token
    }
}
```

### 5.4 新增依赖

```toml
# Cargo.toml [dependencies] 新增
hmac = { version = "0.12", default-features = false }  # HMAC-SHA256 签名（GLM JWT）
sha2 = { version = "0.10", default-features = false }  # SHA-256 哈希（GLM JWT）
base64 = { version = "0.22", default-features = false, features = ["std"] }  # Base64 URL-SAFE 编解码（GLM JWT + Portal Auth）
cookie = { version = "0.18", default-features = false }  # Cookie 解析（Portal Auth）
```

> **⚠️ 不可使用 `jsonwebtoken` crate**：GLM JWT Header 必须包含 `"sign_type": "SIGN"` 自定义字段，而 `jsonwebtoken` 的 `Header` 结构体仅支持 RFC 7515 标准字段（`alg`/`cty`/`kid`/`typ`），不支持自定义字段。使用 `hmac`+`sha2`+`base64` 手动构造 JWT 可完全控制 Header 内容。

### 5.5 AliasCredentialProvider 实现

新增文件: `crates/ullm/src/credential/alias.rs`

解决 Qwen（`DASHSCOPE_API_KEY`）和 Kimi（`MOONSHOT_API_KEY`）的凭证别名问题。

```rust
use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use parking_lot::RwLock;

use crate::credential::{CredentialsProvider, ApiKeyState};
use crate::error::LlmError;

pub struct AliasCredentialProvider {
    provider_id: String,
    inner: Arc<dyn CredentialsProvider>,
    aliases: Vec<String>,
    alias_cache: RwLock<HashMap<String, String>>,
}

impl AliasCredentialProvider {
    pub fn new(
        provider_id: impl Into<String>,
        inner: Arc<dyn CredentialsProvider>,
        aliases: Vec<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            inner,
            aliases,
            alias_cache: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl CredentialsProvider for AliasCredentialProvider {
    async fn get_api_key(&self, provider_id: &str) -> Result<String, LlmError> {
        if let Some(key) = self.alias_cache.read().get(provider_id).cloned() {
            return Ok(key);
        }
        if let Ok(key) = self.inner.get_api_key(&self.provider_id).await {
            self.alias_cache.write().insert(provider_id.to_string(), key.clone());
            return Ok(key);
        }
        for alias in &self.aliases {
            if let Ok(val) = std::env::var(alias) {
                if !val.is_empty() {
                    self.alias_cache.write().insert(provider_id.to_string(), val.clone());
                    return Ok(val);
                }
            }
        }
        Err(LlmError::MissingCredentials {
            provider: provider_id.to_string(),
            env_vars: self.aliases.clone(),
        })
    }

    async fn state(&self, provider_id: &str) -> ApiKeyState {
        match self.get_api_key(provider_id).await {
            Ok(_) => ApiKeyState::Valid,
            Err(LlmError::MissingCredentials { .. }) => ApiKeyState::Missing,
            _ => ApiKeyState::Invalid,
        }
    }

    async fn invalidate(&self, provider_id: &str) {
        self.alias_cache.write().remove(provider_id);
        self.inner.invalidate(provider_id).await;
    }
}
```

---

## 六、WebScraper Provider 完善

### 6.1 当前状态

`web_scraper.rs` 中的 `stream_completion()` 直接返回错误，未实现实际功能。

### 6.2 重设计

将 WebScraper 改为基于 Portal Auth 的 Provider，而非独立的浏览器自动化：

```rust
pub struct WebScraperProvider {
    config: ProviderConfig,
    target: WebTarget,
    portal_auth: Option<Arc<PortalAuthCredentialProvider>>,
    client: Client,
}

impl WebScraperProvider {
    pub fn new(target: WebTarget) -> Self { ... }

    pub fn with_portal_auth(mut self, auth: Arc<PortalAuthCredentialProvider>) -> Self {
        self.portal_auth = Some(auth);
        self
    }
}

// stream_completion 实现:
// 1. 若有 portal_auth，使用 Portal Auth Token 调用网页版 API
// 2. 解析网页版 SSE 响应
// 3. 转换为标准 StreamEvent
```

### 6.3 WebTarget 扩展

```rust
pub enum WebTarget {
    ClaudeWeb,
    CopilotWeb,
    QwenWeb,       // 新增
    MiniMaxWeb,     // 新增
    GeminiWeb,      // 新增
    Custom { url: String },
}
```

---

## 七、thinking 模型识别扩展

### 7.1 当前识别规则

```rust
pub fn is_reasoning_model(model: &str) -> bool {
    let lower = model.to_lowercase();
    lower.starts_with("o1")
        || lower.starts_with("o3")
        || lower.starts_with("o4")
        || lower.contains("grok-3-mini")
        || lower.contains("qwq")
        || lower.contains("qwen-qwq")
        || lower.contains("thinking")
}
```

### 7.2 扩展后规则

```rust
pub fn is_reasoning_model(model: &str) -> bool {
    let lower = model.to_lowercase();
    lower.starts_with("o1")
        || lower.starts_with("o3")
        || lower.starts_with("o4")
        || lower.contains("grok-3-mini")
        || lower.contains("qwq")
        || lower.contains("qwen-qwq")
        || lower.contains("thinking")
        || lower.contains("deepseek-reasoner")
        || lower.contains("glm-z1")              // GLM-Z1 推理模型
        || lower.contains("glm-5.1")              // GLM-5.1 支持 thinking
        || lower.contains("kimi-k2")              // Kimi K2 推理增强
}
```

---

## 八、OpenAiSseMapper 扩展

### 8.1 DeepSeek reasoning_content 支持

DeepSeek Reasoner 的 SSE 响应中 `delta` 包含 `reasoning_content` 字段：

```json
{
  "choices": [{
    "delta": {
      "reasoning_content": "思考过程...",
      "content": null
    }
  }]
}
```

### 8.2 修改方案

在 `OpenAiDelta` 结构体中新增字段：

```rust
#[derive(Debug, Deserialize, Default)]
struct OpenAiDelta {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiToolCallDelta>>,
    role: Option<String>,
    reasoning_content: Option<String>,  // 新增：DeepSeek 推理内容
}
```

> **⚠️ 不可添加 `#[serde(deny_unknown_fields)]`**：当前 `OpenAiDelta` 无 `deny_unknown_fields` 属性，这是正确的。不同供应商的 SSE delta 可能包含额外字段（如 DeepSeek 的 `reasoning_content`、OpenAI 的 `refusal` 等），`deny_unknown_fields` 会导致未知字段反序列化失败。serde 默认忽略未知字段，新增 `reasoning_content` 不影响现有供应商的兼容性。

在 `OpenAiSseMapper::map_frame` 中增加处理（**插入位置：在 `finish_reason` 处理之后、`content` 处理之前**）：

> **精确插入锚点**（当前源码 `openai_mapper.rs`）：在第 93 行 `return Ok(Some(StreamEvent::Stop(stop_reason)));` 之后，第 95 行 `if let Some(content) = &choice.delta.content {` 之前插入。即 `finish_reason` 分支的 `return` 语句与 `content` 分支的 `if let` 之间。

```rust
// 在 finish_reason 处理之后（约第 93 行 return Ok(Some(StreamEvent::Stop(...)) 之后）
// 在 content 处理之前（约第 95 行 if let Some(content) = &choice.delta.content 之前）
if let Some(reasoning) = &choice.delta.reasoning_content {
    if !reasoning.is_empty() {
        return Ok(Some(StreamEvent::Thinking {
            text: reasoning.clone(),
            signature: None,
        }));
    }
}
```

---

## 九、文件变更总览

### 9.0 ProviderConfig 扩展

当前 `ProviderConfig` 结构体（位于 `provider/types.rs`）需新增字段以支持新供应商特性：

> **⚠️ 编译阻断警告**：`provider/types.rs` 当前无 `HashMap` 导入，必须新增 `use std::collections::HashMap;`（在文件顶部 `use` 区域），否则 `extra_headers: HashMap<String, String>` 字段声明将编译失败。

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: ProviderId,
    pub name: ProviderName,
    pub api_url: String,
    pub api_key_env: Option<String>,
    pub models: Vec<ModelConfig>,
    pub rate_limit: Option<RateLimitConfig>,
    pub retry: Option<RetryConfig>,
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,
    #[serde(default)]
    pub max_request_body_bytes: Option<usize>,
}
```

**`#[serde(default)]` 必要性**：新增字段必须标注 `#[serde(default)]`，否则反序列化旧版配置文件（不含这两个字段）时会报错。`HashMap` 的 default 为空 map，`Option<usize>` 的 default 为 `None`。

**迁移影响**：所有现有 `ProviderConfig` 构造处必须补充新字段初始化：

| 文件 | 修改 |
|------|------|
| `openai_compatible.rs` → `OpenAiCompatibleProvider::new()` | 构造 `ProviderConfig` 时新增 `extra_headers: HashMap::new(), max_request_body_bytes: None` |
| `web_scraper.rs` → `WebScraperProvider::new()` | 构造 `ProviderConfig` 时新增 `extra_headers: HashMap::new(), max_request_body_bytes: None` |
| `openai.rs` → `OpenAiProvider::new()` | 构造 `ProviderConfig` 时新增 `extra_headers: HashMap::new(), max_request_body_bytes: None` |
| `anthropic.rs` → `AnthropicProvider::new()` | 构造 `ProviderConfig` 时新增 `extra_headers: HashMap::new(), max_request_body_bytes: None` |
| `ollama.rs` → `OllamaProvider::new()` | 构造 `ProviderConfig` 时新增 `extra_headers: HashMap::new(), max_request_body_bytes: None` |
| `codegeex.rs` → `CodeGeeXProvider::new()` | 构造 `ProviderConfig` 时新增 `extra_headers: HashMap::new(), max_request_body_bytes: None` |
| `xai.rs` → `XaiProvider::new()` | 构造 `ProviderConfig` 时新增 `extra_headers: HashMap::new(), max_request_body_bytes: None` |

> **⚠️ 编译阻断**：以上 7 个文件均需在文件顶部新增 `use std::collections::HashMap;` 导入（`openai.rs`/`anthropic.rs`/`ollama.rs`/`codegeex.rs`/`xai.rs` 当前均无 `HashMap` 导入），否则 `HashMap::new()` 编译失败。

**通用修改模板**（适用于 `openai.rs`/`anthropic.rs`/`ollama.rs`/`codegeex.rs`/`xai.rs`）：

```rust
// 1. 文件顶部新增导入
use std::collections::HashMap;

// 2. ProviderConfig 构造处补充最后两个字段
let config = ProviderConfig {
    // ... 现有字段不变 ...
    rate_limit: None,          // 或 Some(...)
    retry: None,               // 或 Some(...)
    extra_headers: HashMap::new(),       // 新增
    max_request_body_bytes: None,        // 新增
};
```

**`web_scraper.rs` 具体修改**（当前代码位于 `WebScraperProvider::new()` 第 104-125 行）：

> **⚠️ 编译阻断**：`web_scraper.rs` 当前无 `HashMap` 导入，必须在文件顶部新增 `use std::collections::HashMap;`，否则 `extra_headers: HashMap::new()` 编译失败。

```rust
let config = ProviderConfig {
    id: ProviderId::new(id_str),
    name: ProviderName::new(name_str),
    api_url: String::new(),
    api_key_env: None,
    models: vec![ModelConfig { ... }],
    rate_limit: None,
    retry: None,
    extra_headers: HashMap::new(),               // 新增
    max_request_body_bytes: None,                 // 新增
};
```

**`extra_headers` 使用场景**：OpenRouter 需要附加 `HTTP-Referer` 和 `X-Title` 头。已在 §3.2 `stream_completion_inner` 中集成，发送请求前遍历 `self.extra_headers` 附加到 request builder。

**`max_request_body_bytes` 使用场景**：Qwen DashScope 限制请求体 6MB。已在 §3.2 `stream_completion_inner` 中集成，序列化 body 后检查大小，超限返回 `LlmError::RequestBodySizeExceeded`。

### 9.1 新增文件

| 文件路径 | 用途 |
|----------|------|
| `crates/ullm/src/provider/glm.rs` | GLM 供应商（含 JWT 手动构造，使用 hmac+sha2+base64） |
| `crates/ullm/src/provider/deepseek.rs` | DeepSeek 供应商 |
| `crates/ullm/src/provider/kimi.rs` | Kimi 供应商 |
| `crates/ullm/src/provider/qwen.rs` | Qwen 供应商 |
| `crates/ullm/src/provider/openrouter.rs` | OpenRouter 供应商 |
| `crates/ullm/src/provider/gemini.rs` | Gemini 供应商 |
| `crates/ullm/src/provider/mistral.rs` | Mistral 供应商 |
| `crates/ullm/src/provider/groq.rs` | Groq 供应商 |
| `crates/ullm/src/provider/factory.rs` | 供应商工厂 |
| `crates/ullm/src/credential/portal_auth.rs` | Portal Auth 框架 |
| `crates/ullm/src/credential/alias.rs` | 凭证别名提供者 |
| `crates/ullm/src/credential/portal/qwen.rs` | 千问 Portal Auth |
| `crates/ullm/src/credential/portal/minimax.rs` | 海螺AI Portal Auth |
| `crates/ullm/src/credential/portal/gemini_cli.rs` | Gemini CLI Auth |
| `crates/ullm/src/credential/portal/copilot.rs` | Copilot Proxy Auth |
| `crates/ullm/src/credential/portal/mod.rs` | Portal Auth 策略注册 |

### 9.2 修改文件

| 文件路径 | 修改内容 |
|----------|----------|
| `crates/ullm/Cargo.toml` | 包名、描述、新增依赖 |
| `crates/ullm/src/lib.rs` | 新增模块导出（见下方详细规格） |
| `crates/ullm/src/provider/mod.rs` | ProviderKind 扩展 + 新增模块声明（见下方详细规格） |
| `crates/ullm/src/provider/web_scraper.rs` | 集成 Portal Auth + ProviderConfig 新字段初始化 |
| `crates/ullm/src/stream/openai_mapper.rs` | reasoning_content 支持 |
| `crates/ullm/src/thinking/mod.rs` | 推理模型识别扩展 |
| `crates/ullm/src/credential/mod.rs` | 新增 portal_auth/alias 模块导出（见下方详细规格） |
| `crates/ullm/src/provider/types.rs` | ProviderConfig 新增 `extra_headers: HashMap<String, String>`/`max_request_body_bytes: Option<usize>` 字段 + `#[serde(default)]` + 新增 `use std::collections::HashMap;` 导入 |
| `crates/ullm/src/provider/openai_compatible.rs` | 7 项修改（见下方详细操作步骤） |
| `Cargo.toml`（根） | workspace members + dependencies |
| `crates/ide-core/Cargo.toml` | dependencies |
| `src/lib.rs` | `pub use ullm;` |
| `crates/ide-core/src/lib.rs` | `pub use ullm;` |
| `crates/ide-core/src/llm/mod.rs` | 引用替换 |
| `crates/ide-core/src/scheduler/actor.rs` | 引用替换 |
| `crates/ide-core/src/context/snapshot.rs` | 引用替换 |
| `crates/ide-core/src/context/monitor.rs` | 引用替换 |
| `crates/ide-core/src/llm/claude.rs` | 废弃消息 |
| `crates/ide-core/src/llm/codegee.rs` | 废弃消息 |

### 9.3 模块声明详细规格

#### `provider/mod.rs` 新增模块声明

```rust
pub mod glm;
pub mod deepseek;
pub mod kimi;
pub mod qwen;
pub mod openrouter;
pub mod gemini;
pub mod mistral;
pub mod groq;
pub mod factory;
```

#### `credential/mod.rs` 新增模块声明

```rust
pub mod alias;
pub mod portal_auth;
pub mod portal;
```

#### `lib.rs` 新增导出

```rust
pub use provider::glm::{GlmModel, GlmProvider};
pub use provider::factory::create_provider_by_kind;
pub use credential::alias::AliasCredentialProvider;
pub use credential::portal_auth::PortalAuthCredentialProvider;
```

#### `provider/openai_compatible.rs` 7 项精确修改步骤

> **⚠️ 此文件是本次迭代修改量最大的文件，必须严格按以下顺序操作，否则编译失败。**

| 步骤 | 操作 | 当前源码锚点 | 修改内容 |
|------|------|-------------|----------|
| 1 | 新增导入 | 文件顶部 `use` 区域（第 1-16 行） | 新增 `use std::collections::HashMap;` |
| 2 | 结构体新增字段 | `OpenAiCompatibleModel` 结构体（第 71-86 行），在 `rate_limiter` 字段之后 | 新增 `extra_headers: HashMap<String, String>` 和 `max_request_body_bytes: Option<usize>` |
| 3 | `new()` 签名更新 | `OpenAiCompatibleModel::new()`（第 90-103 行），在 `rate_limiter` 参数之后 | 新增 `extra_headers: HashMap<String, String>` 和 `max_request_body_bytes: Option<usize>` 两个参数，并在 Self 构造中赋值 |
| 4 | 新增访问器方法 | `OpenAiCompatibleModel` impl 块（第 88 行之后） | 新增 `pub fn credentials(&self) -> &Arc<dyn CredentialsProvider>` 和 `pub fn provider_id_str(&self) -> &str` |
| 5 | 重构 `stream_completion` | 当前 `stream_completion()` 实现（第 206-248 行） | 拆分为 `stream_completion_inner`（含 extra_headers 遍历 + body 大小检查 + `.body(body_bytes)` 发送）、`stream_completion`（获取 API Key 后调 inner）、`stream_completion_with_token`（接受外部 Token 调 inner）。完整代码见 §3.2 |
| 6 | `provided_models()` 传递新字段 | `LanguageModelProvider::provided_models()` 实现（第 324-341 行），`OpenAiCompatibleModel::new()` 调用处 | 在 12 个参数后追加 `self.config.extra_headers.clone()` 和 `self.config.max_request_body_bytes` |
| 7 | 新增 `provided_models_raw()` + 构建器方法 | `OpenAiCompatibleProvider` impl 块（第 317 行 `with_credentials` 之后） | 新增 `provided_models_raw()` 返回 `Vec<Arc<OpenAiCompatibleModel>>`、`with_extra_headers()`、`with_max_request_body_bytes()`。同时更新 `new()` 中 `ProviderConfig` 构造（第 294-302 行）追加 `extra_headers: HashMap::new(), max_request_body_bytes: None` |

---

## 十、新增依赖清单（含防幻觉校验）

### 10.1 依赖验证表

> 铁律 #2（所有依赖在 crates.io 存在）和铁律 #3（License 为 MIT/Apache-2.0）合规性证据

| 依赖 | 版本 | crates.io 链接 | License | 用途 | 验证状态 |
|------|------|---------------|---------|------|----------|
| `hmac` | `0.12` | https://crates.io/crates/hmac | MIT/Apache-2.0 | HMAC-SHA256 签名（GLM JWT） | ✅ 已验证 |
| `sha2` | `0.10` | https://crates.io/crates/sha2 | MIT/Apache-2.0 | SHA-256 哈希（GLM JWT） | ✅ 已验证 |
| `base64` | `0.22` | https://crates.io/crates/base64 | MIT/Apache-2.0 | Base64 URL-SAFE 编解码（GLM JWT + Portal Auth） | ✅ 已验证 |
| `cookie` | `0.18` | https://crates.io/crates/cookie | MIT/Apache-2.0 | Cookie 解析（Portal Auth） | ✅ 已验证 |

### 10.2 Cargo.toml 配置

```toml
# crates/ullm/Cargo.toml [dependencies] 新增
hmac = { version = "0.12", default-features = false }
sha2 = { version = "0.10", default-features = false }
base64 = { version = "0.22", default-features = false, features = ["std"] }
cookie = { version = "0.18", default-features = false }
```

### 10.3 依赖选择决策

- **JWT 方案选择 `hmac`+`sha2`+`base64` 手动构造而非 `jsonwebtoken` crate**：GLM JWT Header 必须包含 `"sign_type": "SIGN"` 字段，而 `jsonwebtoken` crate 的 `Header` 结构体不支持自定义字段（仅支持 RFC 7515 标准字段 `alg`/`cty`/`kid`/`typ` 等）。使用 `hmac`+`sha2` 手动签名 + `base64` URL-SAFE 编码可完全控制 JWT Header 内容，且这三个 crate 均为 RustCrypto 生态核心组件，质量可靠
- **`cookie` 仅 Portal Auth 使用**：通过 feature gate 控制编译，非 Portal Auth 场景不引入

---

## 十一、实现优先级与执行顺序

### Phase 1: 模块重命名（P0，无功能变更）

1. 重命名目录 `crates/ide-llm/` → `crates/ullm/`
2. 修改所有 Cargo.toml
3. 全局替换 `ide_llm` → `ullm`
4. `cargo build` + `cargo test` 验证

### Phase 2: 四大必须供应商（P0）

1. 新增 `provider/glm.rs` + JWT Token 生成（使用 `hmac`+`sha2`+`base64`，**不可使用 `jsonwebtoken`**）
2. 新增 `provider/deepseek.rs`
3. 新增 `provider/kimi.rs`
4. 新增 `provider/qwen.rs`
5. 新增 `credential/alias.rs`（AliasCredentialProvider）
6. 扩展 `ProviderKind` 枚举 + 个别 `#[serde(rename = "...")]` 属性（**不可用 `rename_all`**）
7. 新增 `provider/factory.rs`
8. 修改 `provider/types.rs`：ProviderConfig 新增 `extra_headers`/`max_request_body_bytes` 字段 + `#[serde(default)]`
9. 修改 `provider/openai_compatible.rs`：新增访问器方法 + `stream_completion_inner` 重构 + `provided_models_raw()` + extra_headers/max_request_body_bytes 支持 + ProviderConfig 新字段初始化
10. 修改 `provider/web_scraper.rs`：ProviderConfig 新字段初始化
11. 修改 `lib.rs` 导出
12. 修改 `thinking/mod.rs` 推理模型识别
13. 修改 `stream/openai_mapper.rs` reasoning_content 支持
14. 修改 `credential/mod.rs` 导出 alias 模块
15. 新增 `hmac`/`sha2`/`base64` 依赖
16. 全局搜索 `ModelCapabilities {` 补充 `max_tokens` 字段
17. `cargo build` + `cargo test` 验证

### Phase 3: 扩展供应商（P1）

1. 新增 `provider/openrouter.rs`
2. 新增 `provider/gemini.rs`
3. 新增 `provider/mistral.rs`
4. 新增 `provider/groq.rs`
5. 扩展 `ProviderKind` 枚举
6. 更新 `factory.rs`
7. `cargo build` + `cargo test` 验证

### Phase 4: Portal Auth 框架（P1）

1. 新增 `credential/portal_auth.rs`
2. 新增 `credential/portal/` 目录及策略实现
3. 修改 `credential/mod.rs` 导出
4. 完善 `web_scraper.rs` 集成 Portal Auth
5. 新增 `cookie`/`base64` 依赖
6. `cargo build` + `cargo test` 验证

### Phase 5: 集成测试与文档（P2）

1. 为每个新供应商编写单元测试（见第十二节测试用例表）
2. 编写集成测试（使用 mock 服务器）
3. 编写 `ProviderKind` 序列化往返测试
4. 编写 `AliasCredentialProvider` 凭证别名测试
5. 更新 `lib.rs` 文档注释
6. `cargo clippy` + `cargo test` 最终验证

---

## 十二、测试策略

### 12.1 单元测试

每个新供应商文件中包含 `#[cfg(test)] mod tests`：

- 模型规格验证
- 请求体构建验证
- JWT Token 生成验证（GLM）
- 推理模型识别验证
- SSE 解析验证（reasoning_content）
- 凭证别名验证（Qwen/Kimi）

### 12.2 供应商级测试用例

#### GLM 测试

| 测试函数 | 场景 | 预期结果 |
|----------|------|----------|
| `test_glm_needs_jwt_dot_format` | API Key = `"abc.def"` | `needs_jwt()` 返回 `true` |
| `test_glm_no_jwt_new_format` | API Key = `"abcdef123456"` | `needs_jwt()` 返回 `false` |
| `test_glm_generate_jwt_valid` | API Key = `"id.secret"` | JWT Token 非空且可解码 |
| `test_glm_generate_jwt_no_dot` | API Key = `"directkey"` | 直接返回原 Key |
| `test_glm_provider_models_count` | `create_glm_provider().provided_models()` | 返回 14 个模型 |
| `test_glm_provider_id` | `GlmProvider.id()` | 返回 `"glm"` |

#### DeepSeek 测试

| 测试函数 | 场景 | 预期结果 |
|----------|------|----------|
| `test_deepseek_provider_models` | `create_deepseek_provider().provided_models()` | 返回 2 个模型 |
| `test_deepseek_reasoner_is_reasoning` | `is_reasoning_model("deepseek-reasoner")` | 返回 `true` |
| `test_deepseek_chat_not_reasoning` | `is_reasoning_model("deepseek-chat")` | 返回 `false` |
| `test_deepseek_api_url` | Provider config api_url | `"https://api.deepseek.com"` |

#### Kimi 测试

| 测试函数 | 场景 | 预期结果 |
|----------|------|----------|
| `test_kimi_provider_models` | `create_kimi_provider().provided_models()` | 返回 5 个模型 |
| `test_kimi_alias_moonshot_key` | 设置 `MOONSHOT_API_KEY` | `AliasCredentialProvider` 能获取 |
| `test_kimi_alias_kimi_key` | 设置 `KIMI_API_KEY` | `AliasCredentialProvider` 能获取 |
| `test_kimi_alias_priority` | 同时设置两个 Key | `KIMI_API_KEY` 优先 |

#### Qwen 测试

| 测试函数 | 场景 | 预期结果 |
|----------|------|----------|
| `test_qwen_provider_models` | `create_qwen_provider().provided_models()` | 返回 7 个模型 |
| `test_qwen_alias_dashscope_key` | 设置 `DASHSCOPE_API_KEY` | `AliasCredentialProvider` 能获取 |
| `test_qwq_is_reasoning` | `is_reasoning_model("qwq-32b")` | 返回 `true` |
| `test_qwen_api_url` | Provider config api_url | `"https://dashscope.aliyuncs.com/compatible-mode/v1"` |

#### SSE Mapper 测试

| 测试函数 | 场景 | 预期结果 |
|----------|------|----------|
| `test_openai_mapper_reasoning_content` | delta 包含 `reasoning_content` | 映射为 `StreamEvent::Thinking` |
| `test_openai_mapper_reasoning_empty` | delta `reasoning_content` 为空 | 不产生 Thinking 事件 |
| `test_openai_mapper_reasoning_with_content` | delta 同时有 `reasoning_content` 和 `content` | 先 Thinking 后 Text |

#### 凭证别名测试

| 测试函数 | 场景 | 预期结果 |
|----------|------|----------|
| `test_alias_provider_primary_key` | 主环境变量存在 | 返回主 Key |
| `test_alias_provider_fallback` | 主不存在，别名存在 | 返回别名 Key |
| `test_alias_provider_both_missing` | 都不存在 | 返回 `MissingCredentials` 错误 |
| `test_alias_provider_invalidate` | 清除缓存后 | 重新查找环境变量 |

### 12.3 集成测试

`tests/` 目录下新增：

- `test_providers.rs`: 验证所有 Provider 的 `provided_models()` 返回非空
- `test_factory.rs`: 验证工厂函数创建正确的 Provider
- `test_portal_auth.rs`: 验证 Portal Auth 流程（mock）
- `test_provider_kind_roundtrip.rs`: 验证 `ProviderKind` 序列化/反序列化往返一致性

### 12.4 测试命名规范

```
test_<module>_<function>_<scenario>_<expected_result>
```

示例:
- `test_glm_generate_jwt_token_valid_key_returns_token`
- `test_deepseek_is_reasoning_model_deepseek_reasoner_returns_true`
- `test_kimi_provider_provided_models_returns_non_empty`
- `test_qwen_provider_new_creates_valid_config`
- `test_alias_credential_provider_dashscope_key_returns_valid`
- `test_openai_mapper_reasoning_content_returns_thinking_event`

---

## 十三、风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| GLM JWT Header 需要 `sign_type: "SIGN"` 自定义字段 | `jsonwebtoken` crate 不支持自定义 Header 字段，使用该 crate 生成的 JWT 会被 GLM API 拒绝 | 改用 `hmac`+`sha2`+`base64` 手动构造 JWT，完全控制 Header 内容 |
| DeepSeek reasoning_content 字段可能变化 | SSE 解析失败 | 优雅降级：未知字段忽略，不 panic；`OpenAiDelta` 使用 `Option<String>` 包装 |
| Portal Auth 供应商随时可能封堵 | 零成本访问失效 | 保留 `env_fallback` 回退到 API Key 模式；`AliasCredentialProvider` 支持多源查找 |
| 模块重命名可能遗漏引用 | 编译失败 | 全局搜索 `ide_llm` 和 `ide-llm` 确认无遗漏；`cargo build` 验证 |
| 新增 Provider 数量多导致代码膨胀 | 维护成本 | 所有 OpenAI 兼容供应商复用 `OpenAiCompatibleProvider`；7/8 供应商仅需工厂函数 |
| Qwen 环境变量 `DASHSCOPE_API_KEY` 与自动生成 `QWEN_API_KEY` 不匹配 | 凭证查找失败 | `AliasCredentialProvider` 包装，先查 `QWEN_API_KEY` 再查 `DASHSCOPE_API_KEY` |
| Kimi 环境变量 `MOONSHOT_API_KEY` 与自动生成 `KIMI_API_KEY` 不匹配 | 凭证查找失败 | 同上，`AliasCredentialProvider` 包装 |
| OpenRouter 需要额外 HTTP Headers | 请求被拒绝 | `ProviderConfig` 新增 `extra_headers` 字段，`stream_completion_inner` 自动附加 |
| Qwen 请求体 6MB 限制 | 大上下文请求失败 | `ProviderConfig` 新增 `max_request_body_bytes` 字段，发送前检查 |
| `OpenAiCompatibleModel` 私有字段无法被 GLM 访问 | GLM JWT 无法注入 | 新增 `credentials()`/`provider_id_str()` 访问器 + `stream_completion_inner` 重构 + `stream_completion_with_token()` |
| `ProviderKind` 序列化名称与 `Display` 不一致 | 配置文件兼容性破坏 | 使用个别 `#[serde(rename = "...")]` 属性（与 `Display` 一致），**不可用 `rename_all`** |
| `ProviderConfig` 新增字段导致旧配置反序列化失败 | 运行时配置加载报错 | 新增字段标注 `#[serde(default)]`，确保旧配置文件兼容 |
| `GlmProvider::provided_models()` 无法从 `Vec<Arc<dyn LanguageModel>>` 向下转型为 `OpenAiCompatibleModel` | GLM 模型无法包装为 `GlmModel` | 新增 `OpenAiCompatibleProvider::provided_models_raw()` 返回 `Vec<Arc<OpenAiCompatibleModel>>` |
| `ModelCapabilities` 新增 `max_tokens` 字段 | 所有现有 `ModelCapabilities` 构造处编译失败 | 全局搜索 `ModelCapabilities {` 并补充 `max_tokens` 字段 |
