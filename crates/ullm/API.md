# ullm — 全量 API 设计规范

> **版本**: v2.4
> **定位**: Universal LLM Access Lifecycle Management — 世界级 Rust 异步 LLM SDK  
> **对标**: OpenAI Python SDK v1.x / Anthropic Python SDK / Vercel AI SDK / Zed Language Model Core  
> **原则**: 类型驱动 · 流式优先 · 零成本抽象 · 渐进式复杂度

---

## §0. 项目锚定 — 使命·愿景·铁律·哲学

> **本节是整个 API 规范的宪法性约束。任何 API 设计或实现偏离本节，均视为设计缺陷。**

### 0.1 使命

**构建一款高性能、高可定制、完全可控的 AI IDE 系统，为开发者提供真正自由、高效的 AI 辅助编程体验。**

### 0.2 愿景

**通过"乐高式拼装"实现功能最大化与自编码最小化的统一。**

- 所有新 Provider 基于 `OpenAiCompatibleProvider` 复用（减少 80% 自编码）
- 仅 GLM 因 JWT 需求新增独立文件
- Portal Auth 复用 `CredentialsProvider` trait

### 0.3 六条铁律（不可逾越）

| # | 铁律 | 含义 | 合规校验 |
|---|------|------|----------|
| 1 | **100% 基于 Rust** | 所有代码均为 Rust，无外部语言依赖 | 全项目 Rust 源码 |
| 2 | **所有依赖在 crates.io 存在** | 禁止引入不在 crates.io 上的依赖 | 每个依赖必须提供 crates.io 链接和版本号 |
| 3 | **License 为 MIT/Apache-2.0** | 所有依赖必须使用 MIT 或 Apache-2.0 许可证 | Cargo.toml 中每个依赖需验证 License |
| 4 | **零自编码底线** | 最小化自定义代码，最大化 crate 复用 | JWT 由 `hmac`+`sha2`+`base64` 组合；HTTP 由 `reqwest` 处理；SSE 由 `SseParser` 解析 |
| 5 | **零费用闭环** | 三优先级零费用策略（见 §0.4） | P1→P2→P3 必须形成完整零费用路径 |
| 6 | **防幻觉校验** | 每个依赖均需提供可验证的存在性证据 | crates.io 链接 + 版本号 + License 信息 |

### 0.4 零费用闭环策略

| 优先级 | 路径 | Provider | 说明 |
|--------|------|----------|------|
| **P1** | 免费 API | CodeGeeX | 无需 API Key，无需本地资源 |
| **P2** | 本地推理 | Ollama | 无需 API Key，需本地运行 Ollama |
| **P3** | Portal Auth | Qwen/Copilot/Gemini/MiniMax | 逆向网页接口，零 API 费用 |

### 0.5 设计哲学映射

| 哲学 | 在 ullm 中的体现 |
|------|-----------------|
| **乐高式拼装** | 8 个新 Provider 中 7 个复用 `OpenAiCompatibleProvider`；Portal Auth 复用 `CredentialsProvider` trait |
| **分层解耦** | `LanguageModel`/`LanguageModelProvider` 双 Trait 与具体 Provider 实现解耦 |
| **零成本路径** | Portal Auth 框架实现 P3 零费用路径，与 CodeGeeX(P1) + Ollama(P2) 形成完整闭环 |
| **第三方代码复用** | JWT 使用 `hmac`+`sha2`+`base64`（RustCrypto 生态标准组件）；SSE 复用 `SseParser`；限流复用 `RateLimiter` |

### 0.6 实现状态标记规范

本文档中所有 API 声明使用以下状态标记：

| 标记 | 含义 | AI 编码行为 |
|------|------|------------|
| `[已实现]` | 代码已存在于当前代码库 | **禁止修改**，仅可添加测试 |
| `[待实现]` | API 已设计但代码尚未编写 | **按规范实现**，优先级 Phase 1 |
| `[扩展]` | 在已实现基础上新增方法/字段 | **在现有文件中扩展**，不破坏已有 API |
| `[Phase-2]` | 第二阶段实现，非紧急 | 可延后实现 |

---

## 目录

0. [项目锚定 — 使命·愿景·铁律·哲学](#0-项目锚定--使命愿景铁律哲学)
1. [架构总览](#1-架构总览)
2. [核心类型系统](#2-核心类型系统)
3. [Provider 抽象层](#3-provider-抽象层)
4. [消息与内容模型](#4-消息与内容模型)
5. [请求构建器](#5-请求构建器)
6. [响应模型](#6-响应模型)
7. [流式系统](#7-流式系统)
8. [凭据管理](#8-凭据管理)
9. [Portal 认证](#9-portal-认证)
10. [工具调用](#10-工具调用)
11. [思维链/推理模型](#11-思维链推理模型)
12. [错误处理](#12-错误处理)
13. [重试策略](#13-重试策略)
14. [限流器](#14-限流器)
15. [Token 计数](#15-token-计数)
16. [模型注册表](#16-模型注册表)
17. [Provider 工厂](#17-provider-工厂)
18. [中间件/拦截器](#18-中间件拦截器)
19. [可观测性](#19-可观测性)
20. [取消与超时](#20-取消与超时)
21. [配置系统](#21-配置系统)
22. [Provider 实现](#22-provider-实现)
23. [公开 API 导出表](#23-公开-api-导出表)
24. [使用示例](#24-使用示例)
25. [设计决策记录](#25-设计决策记录)
26. [源文件映射与依赖约束](#26-源文件映射与依赖约束)

---

## 1. 架构总览

### 1.1 分层架构

```
┌──────────────────────────────────────────────────────┐
│                   应用层 (Application)                │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │  complete()  │  │ stream_text()│  │ stream_full()│ │
│  └──────┬──────┘  └──────┬───────┘  └──────┬──────┘ │
│         │                │                  │         │
│  ┌──────▼────────────────▼──────────────────▼──────┐ │
│  │           高级 API (High-level API)              │ │
│  │  · 自动重试 · 流式聚合 · 工具调用循环            │ │
│  │  · 思维链处理 · 中间件管道                        │ │
│  └──────────────────┬──────────────────────────────┘ │
│                     │                                 │
│  ┌──────────────────▼──────────────────────────────┐ │
│  │           核心 API (Core API)                     │ │
│  │  LanguageModel::stream_completion()               │ │
│  │  LanguageModelProvider trait                      │ │
│  └──────────────────┬──────────────────────────────┘ │
│                     │                                 │
│  ┌──────────────────▼──────────────────────────────┐ │
│  │           基础设施层 (Infrastructure)             │ │
│  │  SSE 解析 · HTTP 客户端 · 凭据管理               │ │
│  │  限流器 · 重试策略 · Token 计数                   │ │
│  └─────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

### 1.2 核心设计原则

| 原则 | 说明 |
|------|------|
| **类型驱动** | 所有 API 通过 Rust 类型系统在编译期保证正确性，非法状态不可表达 |
| **流式优先** | 底层唯一 I/O 路径是 `stream_completion()`，非流式 API 在其上聚合构建 |
| **零成本抽象** | Provider 差异通过 trait + 泛型在编译期单态化，运行时无虚表开销（可选 `dyn` 分发） |
| **渐进式复杂度** | 3 行代码可完成最简调用；高级用户可精细控制每个环节 |
| **组合优于继承** | 中间件、凭据、限流器等通过组合模式而非继承层次实现 |
| **显式优于隐式** | 错误处理、超时、重试均为显式配置，无全局隐式状态 |

---

## 2. 核心类型系统

> **源文件**: `src/provider/types.rs` [已实现] · `src/provider/mod.rs` [已实现]

### 2.1 标识符 Newtype `[已实现]`

> **当前实现**: 4 个 newtype 结构体 + `new()`, `as_str()`, `From`, `AsRef<str>`, `Display` [已实现]

```rust
/// 模型唯一标识符 (e.g. "gpt-4o", "claude-sonnet-4-5-20250514")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId(Arc<str>);

impl ModelId {
    pub fn new(id: impl AsRef<str>) -> Self;
    pub fn as_str(&self) -> &str;
}

impl From<String> for ModelId { ... }
impl From<&str> for ModelId { ... }
impl AsRef<str> for ModelId { ... }
impl Display for ModelId { ... }

/// 模型显示名称 (e.g. "GPT-4o", "Claude Sonnet 4.5")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelName(Arc<str>);

impl ModelName {
    pub fn new(name: impl AsRef<str>) -> Self;
    pub fn as_str(&self) -> &str;
}
// From, AsRef, Display 同 ModelId

/// Provider 唯一标识符 (e.g. "openai", "anthropic", "glm")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderId(Arc<str>);

impl ProviderId {
    pub fn new(id: impl AsRef<str>) -> Self;
    pub fn as_str(&self) -> &str;
}
// From, AsRef, Display 同上

/// Provider 显示名称 (e.g. "OpenAI", "Anthropic", "智谱 GLM")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderName(Arc<str>);

impl ProviderName {
    pub fn new(name: impl AsRef<str>) -> Self;
    pub fn as_str(&self) -> &str;
}
// From, AsRef, Display 同上
```

### 2.2 ProviderKind 枚举 `[已实现]`

```rust
/// Provider 类型标识，支持 14 种已知 Provider + 1 种通用兼容模式
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

// 手动实现 Serialize/Deserialize（非 #[serde(untagged)]）
// 原因：untagged 导致单元变体序列化为 null
impl Serialize for ProviderKind { ... }
impl<'de> Deserialize<'de> for ProviderKind { ... }
impl Display for ProviderKind { ... }
```

---

## 3. Provider 抽象层

> **源文件**: `src/provider/mod.rs` [已实现] · 新增方法见下方标记

### 3.1 LanguageModel Trait `[已实现]`

```rust
/// 单个语言模型的能力接口
///
/// 职责：模型元数据查询、Token 计数、流式补全
/// 生命周期：'static 确保可存储于 Arc
#[async_trait]
pub trait LanguageModel: Send + Sync + 'static {
    // ── 元数据 ──
    fn id(&self) -> &ModelId;
    fn name(&self) -> &ModelName;
    fn provider_id(&self) -> &ProviderId;
    fn provider_name(&self) -> &ProviderName;

    // ── 能力查询 ──
    fn supports_tools(&self) -> bool;
    fn supports_streaming_tools(&self) -> bool;
    fn supports_images(&self) -> bool;
    fn supports_thinking(&self) -> bool;
    fn max_token_count(&self) -> u64;
    fn max_output_tokens(&self) -> Option<u64>;

    // ── Token 计数 ──
    fn count_tokens(&self, request: &LanguageModelRequest) -> TokenCountFuture<'_>;

    // ── 核心 I/O ──
    /// 流式补全 — 底层唯一 I/O 路径
    fn stream_completion(&self, request: LanguageModelRequest) -> StreamFuture<'_>;
}
```

### 3.2 LanguageModelProvider Trait `[已实现]`

```rust
/// Provider 管理接口
///
/// 职责：认证管理、模型列表、配置查询
#[async_trait]
pub trait LanguageModelProvider: Send + Sync + 'static {
    fn id(&self) -> &ProviderId;
    fn name(&self) -> &ProviderName;
    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>>;
    fn is_authenticated(&self) -> bool;
    async fn authenticate(&self) -> Result<(), LlmError>;
    async fn reset_credentials(&self) -> Result<(), LlmError>;
    fn configuration(&self) -> &ProviderConfig;
}
```

### 3.3 高级 API 扩展 `[已实现]`

> **实现约束**: 这些方法在 `src/provider/ext.rs` 文件中实现。

```rust
// 文件: src/provider/ext.rs
use super::{LanguageModel, LanguageModelRequest};
use crate::stream::{ModelStream, StreamEvent, StreamFuture};
use crate::error::LlmError;
use crate::response::CompletionResponse;
use crate::tool::ToolCallLoop;
use futures_util::StreamExt;

impl dyn LanguageModel {
    /// 非流式补全 — 聚合流式结果为完整响应
    pub async fn complete(
        &self,
        request: LanguageModelRequest,
    ) -> Result<CompletionResponse, LlmError> {
        let stream = self.stream_completion(request).await?;
        CompletionResponse::from_stream(stream).await
    }

    /// 纯文本流 — 仅产出文本增量，过滤 Thinking/Usage 等事件
    pub async fn stream_text(
        &self,
        request: LanguageModelRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LlmError>> + Send>>, LlmError> {
        let stream = self.stream_completion(request).await?;
        Ok(Box::pin(stream.filter_map(|event| async move {
            match event {
                Ok(StreamEvent::Text(text)) => Some(Ok(text)),
                Ok(StreamEvent::Stop(_)) => None,
                Err(e) => Some(Err(e)),
                _ => None,
            }
        })))
    }

    /// 带工具调用循环的流式补全
    pub async fn stream_with_tools(
        &self,
        request: LanguageModelRequest,
        _tool_loop: &ToolCallLoop,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>, LlmError> {
        // 自动处理 ToolUse 事件：执行工具 → 回注结果 → 继续流
        // 注意：当前为 passthrough 实现，工具循环逻辑待完善
    }
}
```

---

## 4. 消息与内容模型

> **源文件**: `src/provider/types.rs`  
> **当前状态**: `Message`/`Role`/`ContentBlock`/`MessageContent` 全部 [已实现]（content 字段已升级为 `MessageContent`）  
> **兼容性**: `MessageContent::Text(String)` 变体确保 `Message::user("hello")` 等现有调用无需修改

### 4.1 内容块（Content Block）`[已实现]`

> **实现约束**: `src/provider/content_block.rs` 文件，在 `types.rs` 中 `pub use`。

```rust
/// 多态内容块 — 支持文本、图像、工具调用、思维链混合内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "image")]
    Image {
        source: ImageSource,
        #[serde(default)]
        detail: ImageDetail,
    },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },

    #[serde(rename = "thinking")]
    Thinking {
        text: String,
        signature: Option<String>,
    },

    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: String },
}

/// 图像来源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageSource {
    Base64 { media_type: String, data: String },
    Url { url: String },
}

/// 图像细节级别
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ImageDetail {
    #[default]
    Auto,
    Low,
    High,
}

impl ContentBlock {
    // ── 便捷构造器 ──
    pub fn text(content: impl Into<String>) -> Self;
    pub fn image_base64(media_type: impl Into<String>, data: impl Into<String>) -> Self;
    pub fn image_url(url: impl Into<String>) -> Self;
    pub fn tool_use(id: impl Into<String>, name: impl Into<String>, input: serde_json::Value) -> Self;
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self;
    pub fn tool_result_error(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self;
    pub fn thinking(text: impl Into<String>) -> Self;
    pub fn thinking_with_signature(text: impl Into<String>, signature: impl Into<String>) -> Self;
    pub fn redacted_thinking(data: impl Into<String>) -> Self;

    // ── 类型查询 ──
    pub fn is_text(&self) -> bool;
    pub fn is_image(&self) -> bool;
    pub fn is_tool_use(&self) -> bool;
    pub fn is_tool_result(&self) -> bool;
    pub fn is_thinking(&self) -> bool;

    // ── 提取 ──
    pub fn as_text(&self) -> Option<&str>;
    pub fn as_tool_use(&self) -> Option<(&str, &str, &serde_json::Value)>;
}
```

### 4.2 Message — 升级 `[已实现]`

> **当前实现**: `Message { role: Role, content: MessageContent, name: Option<String>, tool_call_id: Option<String> }`  
> **兼容性**: `MessageContent::Text(String)` 变体确保 `Message::user("hello")` 等现有调用无需修改

```rust
/// 聊天消息 — 支持多态内容块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
}

/// 消息内容 — 兼容纯文本与多态内容块
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// 纯文本（向后兼容）
    Text(String),
    /// 多态内容块数组
    Blocks(Vec<ContentBlock>),
}

impl MessageContent {
    pub fn text(content: impl Into<String>) -> Self;
    pub fn blocks(blocks: Vec<ContentBlock>) -> Self;
    pub fn is_empty(&self) -> bool;
    pub fn as_text(&self) -> Option<&str>;
    pub fn to_text_lossy(&self) -> String;
    pub fn blocks(&self) -> Vec<&ContentBlock>;  // 实际方法名为 content_blocks()，避免与构造器同名
}

impl Message {
    // ── 便捷构造器 ──
    pub fn system(content: impl Into<String>) -> Self;
    pub fn user(content: impl Into<String>) -> Self;
    pub fn user_blocks(blocks: Vec<ContentBlock>) -> Self;
    pub fn assistant(content: impl Into<String>) -> Self;
    pub fn assistant_blocks(blocks: Vec<ContentBlock>) -> Self;
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self;
    pub fn tool_result_error(tool_call_id: impl Into<String>, error: impl Into<String>) -> Self;

    // ── Builder 方法 ──
    pub fn with_name(mut self, name: impl Into<String>) -> Self;
    pub fn with_tool_call_id(mut self, id: impl Into<String>) -> Self;
}
```

### 4.3 Role 枚举 `[已实现]`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub fn as_str(&self) -> &'static str;
}
impl Display for Role { ... }
```

---

## 5. 请求构建器

> **源文件**: `src/provider/types.rs`

### 5.1 LanguageModelRequest `[已实现]`

> **当前实现**: `model`, `messages`, `temperature`, `max_tokens`, `top_p`, `frequency_penalty`, `presence_penalty`, `stop`, `tools`, `thinking`, `stream`, `metadata`, `timeout`, `cancel` 字段 [已实现]  
> **当前实现 Builder**: `new()`, `with_temperature()`, `with_max_tokens()`, `with_top_p()`, `with_frequency_penalty()`, `with_presence_penalty()`, `with_stop()`, `with_tools()`, `with_thinking()`, `with_timeout()`, `with_cancel_token()`, `with_metadata()`, `stream()`, `add_message()`, `add_system()`, `add_user()`, `add_assistant()`, `validate()` [已实现]

```rust
/// 语言模型请求 — Builder 模式
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LanguageModelRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub thinking: Option<ThinkingConfig>,
    pub stream: bool,
    pub metadata: Option<RequestMetadata>,
    #[serde(skip)]
    pub timeout: Option<Duration>,
    #[serde(skip)]
    pub cancel: Option<CancellationToken>,
}

impl LanguageModelRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self;

    // ── Builder 方法 ──
    pub fn with_temperature(self, temp: f32) -> Self;
    pub fn with_max_tokens(self, tokens: u32) -> Self;
    pub fn with_top_p(self, p: f32) -> Self;
    pub fn with_frequency_penalty(self, penalty: f32) -> Self;
    pub fn with_presence_penalty(self, penalty: f32) -> Self;
    pub fn with_stop(self, stop: Vec<String>) -> Self;
    pub fn with_tools(self, tools: Vec<ToolDefinition>) -> Self;
    pub fn with_thinking(self, config: ThinkingConfig) -> Self;
    pub fn with_timeout(self, timeout: Duration) -> Self;
    pub fn with_cancel_token(self, token: CancellationToken) -> Self;
    pub fn with_metadata(self, metadata: RequestMetadata) -> Self;
    pub fn stream(self) -> Self;

    // ── 便捷方法 ──
    pub fn add_message(mut self, message: Message) -> Self;
    pub fn add_system(self, content: impl Into<String>) -> Self;
    pub fn add_user(self, content: impl Into<String>) -> Self;
    pub fn add_assistant(self, content: impl Into<String>) -> Self;

    // ── 验证 ──
    pub fn validate(&self) -> Result<(), LlmError>;
}
```

### 5.2 RequestMetadata `[已实现]`

```rust
/// 请求元数据 — 用于追踪、审计、调试
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestMetadata {
    pub request_id: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub tags: HashMap<String, String>,
}

impl RequestMetadata {
    pub fn new() -> Self;
    pub fn with_request_id(mut self, id: impl Into<String>) -> Self;
    pub fn with_user_id(mut self, id: impl Into<String>) -> Self;
    pub fn with_session_id(mut self, id: impl Into<String>) -> Self;
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self;
}
```

---

## 6. 响应模型

> **源文件**: 新增 `src/response.rs`

### 6.1 CompletionResponse `[已实现]`

```rust
/// 非流式补全响应 — 由流式事件聚合而来
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub model: String,
    pub content: MessageContent,
    pub stop_reason: StopReason,
    pub usage: TokenUsage,
    pub tool_calls: Vec<ToolCall>,
    pub thinking: Option<ThinkingContent>,
}

/// 思维链内容聚合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingContent {
    pub text: String,
    pub signature: Option<String>,
}

impl CompletionResponse {
    /// 从流式事件聚合为完整响应
    pub async fn from_stream(
        stream: ModelStream,
    ) -> Result<Self, LlmError>;

    /// 提取纯文本内容
    pub fn text(&self) -> Option<&str>;

    /// 是否包含工具调用
    pub fn has_tool_calls(&self) -> bool;

    /// 是否包含思维链
    pub fn has_thinking(&self) -> bool;
}
```

### 6.2 StopReason `[已实现]`

> **当前实现**: 5 个变体 + `as_str()`, `is_tool_call()` 方法 [已实现]

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StopReason {
    EndTurn,
    ToolCall,
    MaxTokens,
    StopSequence,
    Cancelled,
}

impl StopReason {
    pub fn as_str(&self) -> &'static str;
    pub fn is_tool_call(&self) -> bool;
}
```

### 6.3 TokenUsage `[已实现]`

> **当前实现**: 4 个字段 + `total_tokens()`, `is_empty()`, `merge()` [已实现]

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
}

impl TokenUsage {
    pub fn total_tokens(&self) -> u32;
    pub fn is_empty(&self) -> bool;
    pub fn merge(&mut self, other: &TokenUsage);
}
```

---

## 7. 流式系统

> **源文件**: `src/stream/mod.rs` · `src/stream/sse.rs` · `src/stream/openai_mapper.rs` · `src/stream/anthropic_mapper.rs` · `src/stream/tool_call_state.rs` · `src/stream/json_fix.rs`

### 7.1 StreamEvent `[已实现]`

> **当前实现**: 10 个变体 + `is_text()`, `is_thinking()`, `is_tool_use()`, `is_stop()`, `is_error()`, `as_text()` 辅助方法 [已实现]

```rust
/// 统一流式事件枚举 — 覆盖所有 Provider 的事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEvent {
    /// 排队中（部分 Provider 支持）
    Queued { position: usize },

    /// 流开始
    Started,

    /// 文本增量
    Text(String),

    /// 思维链增量
    Thinking { text: String, signature: Option<String> },

    /// 脱敏思维链增量
    RedactedThinking { data: String },

    /// 工具调用增量（完整参数）
    ToolUse(ToolCall),

    /// 工具调用 JSON 解析错误
    ToolUseJsonParseError {
        id: String,
        tool_name: String,
        raw_input: String,
        json_parse_error: String,
    },

    /// Token 用量更新
    UsageUpdate(TokenUsage),

    /// 流结束
    Stop(StopReason),

    /// 流错误
    Error(String),
}

impl StreamEvent {
    pub fn is_text(&self) -> bool;
    pub fn is_thinking(&self) -> bool;
    pub fn is_tool_use(&self) -> bool;
    pub fn is_stop(&self) -> bool;
    pub fn is_error(&self) -> bool;
    pub fn as_text(&self) -> Option<&str>;
}
```

### 7.2 类型别名 `[已实现]`

```rust
/// 模型流 — 异步事件流
pub type ModelStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>;

/// 流式 Future — 返回 ModelStream 的异步操作
pub type StreamFuture<'a> = Pin<Box<dyn Future<Output = Result<ModelStream, LlmError>> + Send + 'a>>;
```

### 7.3 SseParser `[已实现]`

```rust
/// SSE 帧解析器 — 增量缓冲区 + 可插拔映射器
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    provider: Option<String>,
    model: Option<String>,
}

impl SseParser {
    pub fn new() -> Self;
    pub fn with_context(self, provider: impl Into<String>, model: impl Into<String>) -> Self;

    /// 推入字节块，返回解析出的事件
    pub fn push(&mut self, chunk: &[u8], mapper: &dyn SseEventMapper) -> Result<Vec<StreamEvent>, LlmError>;

    /// 刷新缓冲区尾部
    pub fn finish(&mut self, mapper: &dyn SseEventMapper) -> Result<Vec<StreamEvent>, LlmError>;
}
```

### 7.4 SseEventMapper Trait `[已实现]`

```rust
/// SSE 事件映射器 — 将原始 SSE 帧映射为统一 StreamEvent
pub trait SseEventMapper: Send + Sync {
    fn map_frame(&self, frame: &str) -> Result<Option<StreamEvent>, LlmError>;
}

/// 从 SSE 帧中提取 data 行
pub fn extract_data_lines(frame: &str) -> Option<String>;
```

### 7.5 内置映射器 `[已实现]`

```rust
/// OpenAI 格式 SSE 映射器（覆盖 OpenAI/DeepSeek/Qwen/Kimi/Groq/Mistral/Gemini/OpenRouter）
pub struct OpenAiSseMapper;
impl SseEventMapper for OpenAiSseMapper { ... }

/// Anthropic 格式 SSE 映射器
pub struct AnthropicSseMapper;
impl SseEventMapper for AnthropicSseMapper { ... }
```

### 7.6 工具调用状态累积器 `[已实现]`

```rust
/// 工具调用增量累积器 — 处理流式工具调用的参数拼接
#[derive(Debug, Default)]
pub struct ToolCallAccumulator {
    tool_calls: BTreeMap<u32, ToolCallState>,
}

impl ToolCallAccumulator {
    pub fn ingest(&mut self, index: u32, id: Option<String>, name: Option<String>, arguments_delta: Option<String>);
    pub fn finalize_all(&mut self) -> Vec<ToolCall>;
}

pub struct ToolCallState {
    openai_index: u32,
    id: Option<String>,
    name: Option<String>,
    arguments: String,
    emitted_len: usize,
    started: bool,
}

impl ToolCallState {
    pub fn ingest_delta(&mut self, index: u32, id: Option<String>, name: Option<String>, arguments_delta: Option<String>) -> Option<StreamEvent>;
    pub fn finalize(&self) -> Option<ToolCall>;
}
```

### 7.7 JSON 修复工具 `[已实现]`

```rust
/// 修复流式传输中不完整的 JSON 参数
pub fn fix_streamed_json(json: &str) -> String;

/// 解析工具调用参数 JSON
pub fn parse_tool_arguments(raw: &str) -> Result<serde_json::Value, serde_json::Error>;
```

---

## 8. 凭据管理

> **源文件**: `src/credential/mod.rs` · `src/credential/alias.rs` · `src/credential/portal.rs` · `src/credential/portal_auth.rs`

### 8.1 CredentialsProvider Trait `[已实现]`

```rust
/// 凭据提供者接口 — 统一 API Key / OAuth / Portal Auth 获取方式
#[async_trait]
pub trait CredentialsProvider: Send + Sync {
    /// 获取 API Key（可能是静态 Key、OAuth Token 或 Portal Token）
    async fn get_api_key(&self, provider_id: &str) -> Result<String, LlmError>;

    /// 查询凭据状态
    async fn state(&self, provider_id: &str) -> ApiKeyState;

    /// 使缓存失效，强制下次重新获取
    async fn invalidate(&self, provider_id: &str);
}
```

### 8.2 ApiKeyState `[已实现]`

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeyState {
    Valid,
    Missing,
    Invalid,
    NeedsRefresh,
}
```

### 8.3 内置实现 `[已实现]`

> **环境变量命名规范**（AI 编码必须遵循）:
>
> | Provider | 主环境变量 | 别名环境变量 |
> |----------|-----------|-------------|
> | OpenAI | `OPENAI_API_KEY` | — |
> | Anthropic | `ANTHROPIC_API_KEY` | — |
> | GLM | `GLM_API_KEY` | — |
> | DeepSeek | `DEEPSEEK_API_KEY` | — |
> | Qwen | `QWEN_API_KEY` | `DASHSCOPE_API_KEY` |
> | Kimi | `KIMI_API_KEY` | `MOONSHOT_API_KEY` |
> | OpenRouter | `OPENROUTER_API_KEY` | — |
> | Gemini | `GEMINI_API_KEY` | — |
> | Mistral | `MISTRAL_API_KEY` | — |
> | Groq | `GROQ_API_KEY` | — |
> | XAI | `XAI_API_KEY` | — |
> | Ollama | （无需认证） | — |
> | CodeGeeX | （无需认证） | — |
>
> **AliasCredentialProvider 优先级**: 主环境变量 > 别名环境变量 > 环境变量回退

```rust
/// 环境变量凭据提供者
/// 查找顺序: {PROVIDER}_API_KEY → {PROVIDER_WITHOUT_UNDERSCORE}_API_KEY → {PROVIDER}
pub struct EnvCredentialProvider {
    cache: RwLock<HashMap<String, String>>,
}
impl EnvCredentialProvider {
    pub fn new() -> Self;
    fn env_var_names(provider_id: &str) -> Vec<String>;
}
impl Default for EnvCredentialProvider { ... }
impl CredentialsProvider for EnvCredentialProvider { ... }

/// 配置文件凭据提供者 — 从 HashMap 读取
pub struct ConfigCredentialProvider {
    keys: RwLock<HashMap<String, String>>,
}
impl ConfigCredentialProvider {
    pub fn new(keys: HashMap<String, String>) -> Self;
}
impl CredentialsProvider for ConfigCredentialProvider { ... }

/// OAuth 凭据提供者 — 支持 Token 过期检测 + 环境变量回退
pub struct OAuthCredentialProvider {
    tokens: RwLock<HashMap<String, OAuthTokenSet>>,
    env_fallback: EnvCredentialProvider,
}
impl OAuthCredentialProvider {
    pub fn new() -> Self;
}
impl Default for OAuthCredentialProvider { ... }
impl CredentialsProvider for OAuthCredentialProvider { ... }

/// 别名凭据提供者 — 支持主 Key + 多个别名环境变量回退
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
    ) -> Self;
}
impl CredentialsProvider for AliasCredentialProvider { ... }
```

### 8.4 OAuthTokenSet `[已实现]`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
}
```

---

## 9. Portal 认证

> **源文件**: `src/credential/portal_auth.rs` · `src/credential/portal/mod.rs` · `src/credential/portal/qwen.rs` · `src/credential/portal/copilot.rs` · `src/credential/portal/gemini_cli.rs` · `src/credential/portal/minimax.rs`

### 9.1 PortalAuthStrategy Trait `[已实现]`

```rust
/// Portal 认证策略 — 浏览器 Cookie/OAuth 等零 API Key 认证
#[async_trait]
pub trait PortalAuthStrategy: Send + Sync {
    fn name(&self) -> &str;
    async fn authenticate(&self) -> Result<PortalAuthToken, LlmError>;
    async fn refresh(&self, refresh_token: &str) -> Result<PortalAuthToken, LlmError>;
    fn auth_type(&self) -> PortalAuthType;
}
```

### 9.2 PortalAuthType `[已实现]`

```rust
#[derive(Debug, Clone)]
pub enum PortalAuthType {
    Cookie { domain: String },
    Bearer { header_name: String },
    Custom { header_name: String, prefix: String },
}
```

### 9.3 PortalAuthToken `[已实现]`

```rust
#[derive(Debug, Clone)]
pub struct PortalAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub token_type: PortalAuthType,
}

impl PortalAuthToken {
    pub fn is_expired(&self) -> bool;  // [已实现]
    pub fn remaining_secs(&self) -> Option<u64>;  // [已实现]
}
```

### 9.4 PortalAuthCredentialProvider `[已实现]`

```rust
/// Portal 认证凭据提供者 — 自动刷新 + 环境变量回退
pub struct PortalAuthCredentialProvider {
    strategy: Arc<dyn PortalAuthStrategy>,
    token: tokio::sync::RwLock<Option<PortalAuthToken>>,
    env_fallback: EnvCredentialProvider,
}

impl PortalAuthCredentialProvider {
    pub fn new(strategy: Arc<dyn PortalAuthStrategy>) -> Self;
    pub fn with_env_fallback(self, fallback: EnvCredentialProvider) -> Self;
    pub fn strategy_name(&self) -> &str;
}
impl CredentialsProvider for PortalAuthCredentialProvider { ... }
```

### 9.5 内置 Portal 策略 `[已实现]`

```rust
/// 智谱 Qwen Portal 认证 — X-Access-Token Header
pub struct QwenPortalAuth { ... }
impl PortalAuthStrategy for QwenPortalAuth { ... }

/// GitHub Copilot Portal 认证 — OAuth Token 交换
pub struct CopilotProxyAuth { ... }
impl PortalAuthStrategy for CopilotProxyAuth { ... }

/// Gemini CLI 认证 — ~/.gemini/access_token.json
pub struct GeminiCliAuth { ... }
impl PortalAuthStrategy for GeminiCliAuth { ... }

/// MiniMax/Hailuo Portal 认证 — Cookie-based
pub struct MiniMaxPortalAuth { ... }
impl PortalAuthStrategy for MiniMaxPortalAuth { ... }
```

### 9.6 PortalAuthProvider & PortalAuthConfig `[已实现]`

```rust
/// Portal 认证配置 — 支持预设和自定义
#[derive(Debug, Clone)]
pub struct PortalAuthConfig {
    pub portal_url: String,
    pub login_url: String,
    pub token_endpoint: String,
    pub cookie_names: Vec<String>,
    pub header_name: String,
    pub refresh_interval_secs: u64,
}

impl PortalAuthConfig {
    pub fn claude_web() -> Self;
    pub fn copilot_web() -> Self;
    pub fn custom(
        portal_url: impl Into<String>,
        login_url: impl Into<String>,
        token_endpoint: impl Into<String>,
    ) -> Self;
}

/// Portal 认证 Provider — 组合配置、HTTP 客户端、缓存与环境变量回退
pub struct PortalAuthProvider {
    config: PortalAuthConfig,
    client: reqwest::Client,
    cache: parking_lot::RwLock<HashMap<String, CachedToken>>,
    env_fallback: EnvCredentialProvider,
}

/// 内部缓存令牌（私有）
struct CachedToken {
    token: String,
    fetched_at: std::time::Instant,
    expires_in_secs: u64,
}

impl PortalAuthProvider {
    pub fn new(config: PortalAuthConfig) -> Self;
    pub fn with_env_fallback(self, fallback: EnvCredentialProvider) -> Self;
    pub fn config(&self) -> &PortalAuthConfig;
}
impl CredentialsProvider for PortalAuthProvider { ... }
```

---

## 10. 工具调用

> **源文件**: `src/tool/mod.rs`

### 10.1 核心类型 `[已实现]`

> **当前实现**: `ToolDefinition`/`ToolCall`/`ToolResult` 结构体字段 + `ToolResult::success()`, `ToolResult::error()`, `ToolDefinition::new()`, `ToolDefinition::from_json_schema()`, `ToolCall::parse_arguments()` [已实现]

```rust
/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self;

    pub fn from_json_schema(
        name: impl Into<String> + AsRef<str>,
        description: impl Into<String>,
        schema: &str,
    ) -> Result<Self, LlmError>;
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl ToolCall {
    pub fn parse_arguments(&self) -> Result<serde_json::Value, serde_json::Error>;
}

/// 工具结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn success(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self;  // [已实现]
    pub fn error(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self;  // [已实现]
}
```

### 10.2 ToolExecutor Trait `[已实现]`

```rust
/// 工具执行器接口
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, LlmError>;
    fn list_tools(&self) -> Vec<ToolDefinition>;
}
```

### 10.3 ToolCallLoop `[已实现]`

```rust
/// 工具调用循环 — 管理多执行器调度与最大轮次
pub struct ToolCallLoop {
    executors: Vec<Box<dyn ToolExecutor>>,
    max_rounds: u32,
}

impl ToolCallLoop {
    pub fn new(executors: Vec<Box<dyn ToolExecutor>>, max_rounds: u32) -> Self;

    pub fn find_executor(&self, tool_name: &str) -> Option<&dyn ToolExecutor>;
    pub async fn execute_tool(&self, call: &ToolCall) -> Result<ToolResult, LlmError>;
    pub fn max_rounds(&self) -> u32;
    pub fn all_tool_definitions(&self) -> Vec<ToolDefinition>;
}
```

---

## 11. 思维链/推理模型

> **源文件**: `src/thinking/mod.rs`

### 11.1 ThinkingConfig `[已实现]`

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThinkingConfig {
    pub enabled: bool,
    pub budget_tokens: Option<usize>,
}

impl ThinkingConfig {
    pub fn with_budget(budget_tokens: usize) -> Self;
    pub fn disabled() -> Self;
}
```

### 11.2 推理模型检测 `[已实现]`

```rust
/// 判断模型是否为推理模型（需要特殊参数处理）
pub fn is_reasoning_model(model: &str) -> bool;

/// 对推理模型剥离不兼容参数（temperature/top_p/frequency_penalty/presence_penalty）
pub fn strip_reasoning_params(
    temperature: &mut Option<f32>,
    top_p: &mut Option<f32>,
    frequency_penalty: &mut Option<f32>,
    presence_penalty: &mut Option<f32>,
    model: &str,
);
```

---

## 12. 错误处理

> **源文件**: `src/error.rs`

### 12.1 LlmError `[已实现]`

> **当前实现**: 所有 20 个变体（含 `Cancelled`、`RequestBodySizeExceeded`、`Other`）+ `is_retryable()`, `suggested_action()`, `from_http_status()`, `is_context_window_failure()`, `safe_failure_class()` [已实现]

```rust
#[derive(Error, Debug)]
pub enum LlmError {
    #[error("network error: {0}")]
    Network(String),

    #[error("request timed out after {0}ms")]
    Timeout(u64),

    #[error("rate limit exceeded{}", match retry_after_ms {
        Some(ms) => format!(", retry after {ms}ms"),
        None => String::new(),
    })]
    RateLimitExceeded { retry_after_ms: Option<u64> },

    #[error("authentication failed: {message}")]
    AuthenticationError { message: String },

    #[error("permission denied: {message}")]
    PermissionError { message: String },

    #[error("context window exceeded: estimated {estimated_tokens} tokens, limit {limit_tokens}")]
    ContextWindowExceeded { estimated_tokens: u64, limit_tokens: u64 },

    #[error("prompt too large: {0} tokens")]
    PromptTooLarge(u64),

    #[error("model unavailable: {0}")]
    ModelUnavailable(String),

    #[error("server overloaded, try again later")]
    ServerOverloaded,

    #[error("invalid request: {message}")]
    InvalidRequest { message: String },

    #[error("stream error: {0}")]
    StreamError(String),

    #[error("tool call error: {message}")]
    ToolCallError { message: String, tool_name: Option<String> },

    #[error("missing credentials for provider '{provider}', check environment variables: {}", .env_vars.join(", "))]
    MissingCredentials { provider: String, env_vars: Vec<String> },

    #[error("expired OAuth token for provider '{0}'")]
    ExpiredToken(String),

    #[error("JSON parse error from {provider}/{model}: {source}")]
    JsonParse {
        provider: String,
        model: String,
        body_snippet: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("retries exhausted after {attempts} attempts")]
    RetriesExhausted { attempts: u32 },

    #[error("configuration error: {0}")]
    Config(String),

    #[error("request body size exceeded: {estimated_bytes} bytes > {max_bytes} bytes limit")]
    RequestBodySizeExceeded { estimated_bytes: usize, max_bytes: usize },

    #[error("request cancelled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

impl LlmError {
    /// 是否可重试
    pub fn is_retryable(&self) -> bool;

    /// 建议操作
    pub fn suggested_action(&self) -> &'static str;

    /// 从 HTTP 状态码构建
    pub fn from_http_status(status: StatusCode, body: &str) -> Self;

    /// 是否为上下文窗口溢出
    pub fn is_context_window_failure(&self) -> bool;

    /// 安全失败分类（用于遥测/指标）
    pub fn safe_failure_class(&self) -> &'static str;
}

// From 转换
impl From<reqwest::Error> for LlmError { ... }
impl From<serde_json::Error> for LlmError { ... }
```

---

## 13. 重试策略

> **源文件**: `src/retry/mod.rs`

### 13.1 RetryPolicy `[已实现]`

```rust
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub retry_on: RetryOn,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            retry_on: RetryOn::all(),
        }
    }
}

impl RetryPolicy {
    pub fn new(max_attempts: u32, base_delay: Duration) -> Self;
    pub fn should_retry(&self, error: &LlmError, attempt: u32) -> bool;
}
```

### 13.2 RetryOn `[已实现]`

```rust
#[derive(Debug, Clone)]
pub struct RetryOn {
    pub rate_limit: bool,
    pub server_error: bool,
    pub network_error: bool,
    pub timeout: bool,
}

impl RetryOn {
    pub fn all() -> Self;
    pub fn none() -> Self;
}
```

### 13.3 执行函数 `[已实现]`

```rust
/// 指数退避 + 抖动
pub fn backoff(base: Duration, attempt: u32) -> Duration;

/// 带重试策略执行异步操作
pub async fn run_with_retry<T, F, Fut>(
    policy: &RetryPolicy,
    operation: F,
) -> Result<T, LlmError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, LlmError>>;
```

---

## 14. 限流器

> **源文件**: `src/rate_limit/mod.rs`

### 14.1 RateLimiter `[已实现]`

```rust
#[derive(Clone)]
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    info: Arc<RwLock<RateLimitInfo>>,
}

impl RateLimiter {
    pub fn new(max_concurrent: usize) -> Self;
    pub async fn acquire(&self) -> Result<SemaphorePermit<'_>, LlmError>;
    pub async fn acquire_owned(&self) -> Result<OwnedSemaphorePermit, LlmError>;
    pub async fn wrap_stream<T: Stream>(&self, stream: T) -> Result<RateLimitGuard<T>, LlmError>;
    pub fn update_from_headers(&self, remaining_requests: Option<u32>, remaining_tokens: Option<u64>);
    pub fn info(&self) -> RateLimitInfo;
}
```

### 14.2 RateLimitGuard `[已实现]`

```rust
/// RAII 限流守卫 — 持有信号量许可直到流结束
#[pin_project]
pub struct RateLimitGuard<T> {
    #[pin]
    inner: T,
    _permit: OwnedSemaphorePermit,
}

impl<T: Stream> Stream for RateLimitGuard<T> {
    type Item = T::Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;
}
```

### 14.3 RateLimitInfo `[已实现]`

```rust
#[derive(Debug, Clone, Default)]
pub struct RateLimitInfo {
    pub requests_remaining: Option<u32>,
    pub tokens_remaining: Option<u64>,
    pub reset_at: Option<Instant>,
}
```

---

## 15. Token 计数

> **源文件**: `src/token_count/mod.rs`

### 15.1 TokenCounter Trait `[已实现]`

```rust
pub type TokenCountFuture<'a> = Pin<Box<dyn Future<Output = Result<u64, LlmError>> + Send + 'a>>;

pub trait TokenCounter: Send + Sync {
    fn count_tokens(&self, text: &str, model: &str) -> u64;
    fn count_messages_tokens(&self, messages: &[(&str, &str)], model: &str) -> u64;
    fn count_image_tokens(&self, width: u32, height: u32) -> u64 {
        ((width as u64 * height as u64) / 750).max(1)
    }
}
```

### 15.2 内置实现 `[已实现]`

```rust
/// 字节估算器 — 4 bytes ≈ 1 token
pub struct ByteEstimator;
impl TokenCounter for ByteEstimator { ... }

/// Tiktoken 计数器 — 精确计数 + 字节回退
pub struct TiktokenCounter;
impl TokenCounter for TiktokenCounter { ... }

/// 远程计数器 — 调用远程 API（当前回退到 Tiktoken）
pub struct RemoteTokenCounter { ... }
impl RemoteTokenCounter {
    pub fn new(api_url: impl Into<String>, api_key: impl Into<String>) -> Self;
}
impl TokenCounter for RemoteTokenCounter { ... }

/// 回退计数器 — 主/备/兜底三级链
pub struct FallbackTokenCounter { ... }
impl FallbackTokenCounter {
    pub fn new(primary: Box<dyn TokenCounter>, secondary: Box<dyn TokenCounter>) -> Self;
    pub fn default_for_model(model: &str) -> Self;
}
impl TokenCounter for FallbackTokenCounter { ... }
```

---

## 16. 模型注册表

> **源文件**: `src/registry/mod.rs`

### 16.1 ModelRegistry `[已实现]`

> **当前实现**: `new()`, `register()`, `unregister()`, `find_model()`, `provider_by_id()`, `all_models()`, `models_by_provider()`, `providers()`, `add_alias()`, `resolve_alias()`, `set_default_model()`, `default_model()`, `set_role_model()`, `role_model()` [已实现]  
> **当前实现字段**: `aliases: HashMap<String, String>`, `default_model_id: Option<String>`, `role_models: HashMap<ModelRole, String>` [已实现]

```rust
/// 模型注册表 — Provider 注册 + 模型查找 + 别名解析
pub struct ModelRegistry {
    providers: Vec<Arc<dyn LanguageModelProvider>>,
    aliases: HashMap<String, String>,  // [已实现]
    default_model_id: Option<String>,  // [已实现]
    role_models: HashMap<ModelRole, String>,  // [已实现]
}

impl ModelRegistry {
    pub fn new() -> Self;  // [已实现]

    // ── Provider 管理 ──
    pub fn register(&mut self, provider: Arc<dyn LanguageModelProvider>);  // [已实现]
    pub fn unregister(&mut self, provider_id: &str);  // [已实现]
    pub fn provider_by_id(&self, provider_id: &str) -> Option<Arc<dyn LanguageModelProvider>>;  // [已实现]
    pub fn providers(&self) -> &[Arc<dyn LanguageModelProvider>];  // [已实现]

    // ── 模型查找 ──
    pub fn find_model(&self, model_id: &str) -> Option<Arc<dyn LanguageModel>>;  // [已实现]
    pub fn all_models(&self) -> Vec<Arc<dyn LanguageModel>>;  // [已实现]
    pub fn models_by_provider(&self, provider_id: &str) -> Vec<Arc<dyn LanguageModel>>;  // [已实现]

    // ── 别名解析 ──
    pub fn add_alias(&mut self, alias: impl Into<String>, model_id: impl Into<String>);  // [已实现]
    pub fn resolve_alias(&self, name: &str) -> Option<&str>;  // [已实现]

    // ── 角色模型 ──
    pub fn set_default_model(&mut self, model_id: impl Into<String>);  // [已实现]
    pub fn default_model(&self) -> Option<Arc<dyn LanguageModel>>;  // [已实现]
    pub fn set_role_model(&mut self, role: ModelRole, model_id: impl Into<String>);  // [已实现]
    pub fn role_model(&self, role: ModelRole) -> Option<Arc<dyn LanguageModel>>;  // [已实现]
}

impl Default for ModelRegistry { ... }
```

### 16.2 ModelRole `[已实现]`

```rust
/// 模型角色 — 用于不同场景自动选择模型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelRole {
    Default,
    Fast,
    Smart,
    Summary,
    Embedding,
}
```

---

## 17. Provider 工厂

> **源文件**: `src/provider/factory.rs`

### 17.1 工厂函数 `[已实现]`

```rust
/// 按 ProviderKind 创建 Provider 实例
pub fn create_provider_by_kind(kind: &ProviderKind) -> Option<Arc<dyn LanguageModelProvider>>;

/// 各 Provider 独立工厂函数
pub fn create_glm_provider() -> Arc<dyn LanguageModelProvider>;
pub fn create_deepseek_provider() -> Arc<dyn LanguageModelProvider>;
pub fn create_kimi_provider() -> Arc<dyn LanguageModelProvider>;
pub fn create_qwen_provider() -> Arc<dyn LanguageModelProvider>;
pub fn create_openrouter_provider() -> Arc<dyn LanguageModelProvider>;
pub fn create_gemini_provider() -> Arc<dyn LanguageModelProvider>;
pub fn create_mistral_provider() -> Arc<dyn LanguageModelProvider>;
pub fn create_groq_provider() -> Arc<dyn LanguageModelProvider>;
```

---

## 18. 中间件/拦截器

> **源文件**: `src/middleware.rs` [已实现]

### 18.1 Middleware Trait `[已实现]`

```rust
/// 请求/响应中间件接口
///
/// 用途：日志记录、指标收集、请求修改、缓存、审计
#[async_trait]
pub trait Middleware: Send + Sync + 'static {
    /// 请求拦截 — 在发送前修改请求
    async fn on_request(&self, request: &mut LanguageModelRequest) -> Result<(), LlmError> {
        Ok(())
    }

    /// 响应拦截 — 在收到流后处理
    async fn on_response(&self, request: &LanguageModelRequest, response: &CompletionResponse) {
    }

    /// 错误拦截
    async fn on_error(&self, request: &LanguageModelRequest, error: &LlmError) {
    }

    /// 流事件拦截
    async fn on_stream_event(&self, event: &StreamEvent) {
    }

    /// 中间件名称
    fn name(&self) -> &str;
}
```

### 18.2 MiddlewarePipeline `[已实现]`

```rust
/// 中间件管道 — 按注册顺序执行
pub struct MiddlewarePipeline {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewarePipeline {
    pub fn new() -> Self;
    pub fn add(&mut self, middleware: Arc<dyn Middleware>);
    pub async fn on_request(&self, request: &mut LanguageModelRequest) -> Result<(), LlmError>;
    pub async fn on_response(&self, request: &LanguageModelRequest, response: &CompletionResponse);
    pub async fn on_error(&self, request: &LanguageModelRequest, error: &LlmError);
    pub async fn on_stream_event(&self, event: &StreamEvent);
}

impl Default for MiddlewarePipeline { ... }
```

### 18.3 内置中间件 `[已实现]`

```rust
/// 日志中间件 — 记录请求/响应/错误
pub struct LoggingMiddleware {
    level: LogLevel,
}
impl LoggingMiddleware {
    pub fn new(level: LogLevel) -> Self;
}

/// 指标中间件 — 收集延迟/Token 用量/错误率
pub struct MetricsMiddleware {
    metrics: Arc<MetricsCollector>,
}
impl MetricsMiddleware {
    pub fn new(metrics: Arc<MetricsCollector>) -> Self;
}

/// 重试中间件 — 自动重试可重试错误
pub struct RetryMiddleware {
    policy: RetryPolicy,
}
impl RetryMiddleware {
    pub fn new(policy: RetryPolicy) -> Self;
}
```

---

## 19. 可观测性

> **源文件**: `src/observability.rs` [已实现]

### 19.1 MetricsCollector `[已实现]`

```rust
/// 指标收集器接口
pub trait MetricsCollector: Send + Sync {
    fn record_request(&self, provider: &str, model: &str);
    fn record_response(&self, provider: &str, model: &str, latency_ms: u64, usage: &TokenUsage);
    fn record_error(&self, provider: &str, model: &str, error_class: &str);
    fn record_stream_event(&self, provider: &str, model: &str, event_type: &str);
}

/// 空操作指标收集器
pub struct NoopMetricsCollector;
impl MetricsCollector for NoopMetricsCollector { ... }
```

### 19.2 日志 `[已实现]`

```rust
/// 日志级别
#[derive(Debug, Clone, Copy, Default)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}
```

---

## 20. 取消与超时

> **源文件**: `src/cancel.rs` [已实现]

### 20.1 CancellationToken `[已实现]`

```rust
/// 协作式取消令牌 — 基于 AtomicBool + Notify
#[derive(Clone)]
pub struct CancellationToken {
    inner: Arc<AtomicBool>,
    waker: Arc<Notify>,
}

impl CancellationToken {
    pub fn new() -> Self;
    pub fn cancel(&self);
    pub fn is_cancelled(&self) -> bool;
    pub async fn cancelled(&self);

    /// 创建子令牌 — 父令牌取消时子令牌也取消
    pub fn child_token(&self) -> Self;
}

impl Default for CancellationToken { ... }
```

### 20.2 超时集成 `[已实现]`

```rust
impl LanguageModelRequest {
    /// 设置请求超时
    pub fn with_timeout(self, timeout: Duration) -> Self;

    /// 设置取消令牌
    pub fn with_cancel_token(self, token: CancellationToken) -> Self;
}
```

---

## 21. 配置系统

> **源文件**: `src/provider/types.rs` (ProviderConfig/ModelConfig/ModelCapabilities/RateLimitConfig/RetryConfig)

### 21.1 ProviderConfig `[已实现]`

> **当前实现**: 结构体字段 + `from_json_file()`, `from_json()`, `validate()` 方法 [已实现]

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

impl ProviderConfig {
    /// 从 JSON 文件加载
    pub fn from_json_file(path: &Path) -> Result<Self, LlmError>;

    /// 从 JSON 字符串加载
    pub fn from_json(json: &str) -> Result<Self, LlmError>;

    /// 验证配置完整性
    pub fn validate(&self) -> Result<(), LlmError>;
}
```

### 21.2 ModelConfig `[已实现]`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub display_name: String,
    pub max_tokens: u64,
    pub max_output_tokens: Option<u64>,
    pub capabilities: ModelCapabilities,
}
```

### 21.3 ModelCapabilities `[已实现]`

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub tools: bool,
    pub images: bool,
    pub streaming_tools: bool,
    pub parallel_tool_calls: bool,
    pub thinking: bool,
    pub max_tokens: u64,
}
```

### 21.4 RateLimitConfig & RetryConfig `[已实现]`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub max_concurrent_requests: usize,
    pub requests_per_minute: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub retry_on_rate_limit: bool,
    pub retry_on_server_error: bool,
}
```

---

## 22. Provider 实现

### 22.1 OpenAiCompatibleProvider / OpenAiCompatibleModel `[已实现]`

```rust
/// OpenAI 兼容 Provider — 7 个 Provider 复用
pub struct OpenAiCompatibleProvider {
    client: Client,
    credentials: Arc<dyn CredentialsProvider>,
    rate_limiter: Arc<RateLimiter>,
    config: ProviderConfig,
    model_specs: Vec<(String, String, u64, Option<u64>, ModelCapabilities)>,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        api_url: impl Into<String>,
        api_key_env: Option<String>,
        models: Vec<(String, String, u64, Option<u64>, ModelCapabilities)>,
    ) -> Self;

    pub fn with_credentials(self, credentials: Arc<dyn CredentialsProvider>) -> Self;
    pub fn with_extra_headers(self, headers: HashMap<String, String>) -> Self;
    pub fn with_max_request_body_bytes(self, max_bytes: usize) -> Self;
    pub fn provided_models_raw(&self) -> Vec<Arc<OpenAiCompatibleModel>>;
}

impl LanguageModelProvider for OpenAiCompatibleProvider { ... }

/// OpenAI 兼容模型
pub struct OpenAiCompatibleModel { ... }

impl OpenAiCompatibleModel {
    pub fn new(...) -> Self;
    pub fn credentials(&self) -> &Arc<dyn CredentialsProvider>;
    pub fn provider_id_str(&self) -> &str;
    pub fn stream_completion_with_token<'a>(
        &'a self, request: LanguageModelRequest, bearer_token: &'a str,
    ) -> StreamFuture<'a>;
}

impl LanguageModel for OpenAiCompatibleModel { ... }
```

### 22.2 各 Provider 专用实现

| Provider | 类型 | 模型数 | 特殊处理 |
|----------|------|--------|----------|
| **OpenAi** | 独立实现 | 8 | 推理模型用 `max_completion_tokens` 替代 `max_tokens` |
| **Anthropic** | 独立实现 | 3 | System prompt 提取为顶级参数；`x-api-key` + `anthropic-version` Header；Thinking 支持 |
| **Glm** | 包装 OpenAiCompatible | 16 | JWT 认证（HMAC-SHA256）；`needs_jwt()` 检测点分隔 Key |
| **DeepSeek** | OpenAiCompatible | 2 | — |
| **Qwen** | OpenAiCompatible | 9 | AliasCredentialProvider（DASHSCOPE_API_KEY）；6MB 请求体限制 |
| **Kimi** | OpenAiCompatible | 5 | AliasCredentialProvider（MOONSHOT_API_KEY） |
| **OpenRouter** | OpenAiCompatible | 5 | extra_headers（HTTP-Referer, X-Title） |
| **Gemini** | OpenAiCompatible | 3 | — |
| **Mistral** | OpenAiCompatible | 4 | — |
| **Groq** | OpenAiCompatible | 3 | — |
| **XAI** | 独立实现 | 2 | 独立 HTTP 处理 |
| **Ollama** | 独立实现 | 动态 | `/api/tags` 动态模型发现；无需认证 |
| **CodeGeeX** | 独立实现 | 1 | 非流式；自定义请求/响应格式 |
| **WebScraper** | 实验性 | 0 | Portal Auth 集成 |

#### 精确模型清单（AI 编码必须使用以下 model id）

| Provider | Model ID | Display Name | Max Tokens | Max Output | Thinking |
|----------|----------|-------------|------------|------------|----------|
| **OpenAi** | `gpt-4` | GPT-4 | 8192 | — | ✗ |
| | `gpt-4-turbo` | GPT-4 Turbo | 128000 | 4096 | ✗ |
| | `gpt-4o` | GPT-4o | 128000 | 4096 | ✗ |
| | `gpt-4o-mini` | GPT-4o Mini | 128000 | 16384 | ✗ |
| | `gpt-3.5-turbo` | GPT-3.5 Turbo | 16385 | 4096 | ✗ |
| | `o1` | o1 | 200000 | 100000 | ✓ |
| | `o1-mini` | o1 Mini | 128000 | 65536 | ✓ |
| | `o3-mini` | o3 Mini | 200000 | 100000 | ✓ |
| **Anthropic** | `claude-opus-4-6` | Claude Opus 4.6 | 200000 | 16384 | ✗ |
| | `claude-sonnet-4-5-20250514` | Claude Sonnet 4.5 | 200000 | 16384 | ✓ |
| | `claude-haiku-3-5-20241022` | Claude Haiku 3.5 | 200000 | 8192 | ✗ |
| **Glm** | `glm-4-plus` | GLM-4 Plus | 128000 | 4096 | ✗ |
| | `glm-4-0520` | GLM-4 | 128000 | 4096 | ✗ |
| | `glm-4-air` | GLM-4 Air | 128000 | 4096 | ✗ |
| | `glm-4-airx` | GLM-4 AirX | 8192 | 4096 | ✗ |
| | `glm-4-long` | GLM-4 Long | 1000000 | 4096 | ✗ |
| | `glm-4-flash` | GLM-4 Flash | 128000 | 4096 | ✗ |
| | `glm-4-flashx` | GLM-4 FlashX | 128000 | 4096 | ✗ |
| | `glm-4v` | GLM-4V | 128000 | 1024 | ✗ |
| | `glm-4v-plus` | GLM-4V Plus | 8192 | 4096 | ✗ |
| | `glm-4.7-flash` | GLM-4.7 Flash | 200000 | 4096 | ✗ |
| | `glm-z1-air` | GLM-Z1 Air | 128000 | 4096 | ✓ |
| | `glm-z1-airx` | GLM-Z1 AirX | 8192 | 4096 | ✓ |
| | `glm-z1-flash` | GLM-Z1 Flash | 128000 | 4096 | ✓ |
| | `glm-5` | GLM-5 | 200000 | 16384 | ✓ |
| | `glm-5.1` | GLM-5.1 | 200000 | 16384 | ✓ |
| | `glm-5-turbo` | GLM-5 Turbo | 200000 | 16384 | ✓ |
| **DeepSeek** | `deepseek-chat` | DeepSeek Chat | 64000 | 8192 | ✗ |
| | `deepseek-reasoner` | DeepSeek Reasoner | 64000 | 8192 | ✓ |
| **Qwen** | `qwen-max` | Qwen Max | 32768 | 8192 | ✗ |
| | `qwen-plus` | Qwen Plus | 131072 | 8192 | ✗ |
| | `qwen-turbo` | Qwen Turbo | 1000000 | 8192 | ✗ |
| | `qwen-long` | Qwen Long | 10000000 | 6000 | ✗ |
| | `qwq-32b` | QwQ 32B | 131072 | 16384 | ✓ |
| | `qwen-qwq-32b` | Qwen QwQ 32B | 131072 | 8192 | ✓ |
| | `qwen3-235b-a22b` | Qwen3 235B | 131072 | 8192 | ✓ |
| | `qwen-vl-max` | Qwen VL Max | 32768 | 2048 | ✗ |
| | `qwen-coder-plus` | Qwen Coder Plus | 131072 | 8192 | ✗ |
| **Kimi** | `moonshot-v1-8k` | Moonshot V1 8K | 8192 | 4096 | ✗ |
| | `moonshot-v1-32k` | Moonshot V1 32K | 32768 | 4096 | ✗ |
| | `moonshot-v1-128k` | Moonshot V1 128K | 131072 | 4096 | ✗ |
| | `kimi-k2-0711` | Kimi K2 | 256000 | 16384 | ✓ |
| | `kimi-latest` | Kimi Latest | 131072 | 8192 | ✗ |
| **OpenRouter** | `openai/gpt-4o` | GPT-4o (via OpenRouter) | 128000 | 16384 | ✗ |
| | `anthropic/claude-sonnet-4` | Claude Sonnet 4 (via OpenRouter) | 200000 | 64000 | ✗ |
| | `google/gemini-2.5-pro` | Gemini 2.5 Pro (via OpenRouter) | 1000000 | 65536 | ✗ |
| | `deepseek/deepseek-chat` | DeepSeek Chat (via OpenRouter) | 64000 | 8192 | ✗ |
| | `meta-llama/llama-3.3-70b-instruct` | Llama 3.3 70B (via OpenRouter) | 128000 | 4096 | ✗ |
| **Gemini** | `gemini-2.5-pro` | Gemini 2.5 Pro | 1000000 | 65536 | ✗ |
| | `gemini-2.5-flash` | Gemini 2.5 Flash | 1000000 | 65536 | ✗ |
| | `gemini-2.0-flash` | Gemini 2.0 Flash | 1000000 | 8192 | ✗ |
| **Mistral** | `mistral-large-latest` | Mistral Large | 128000 | 4096 | ✗ |
| | `mistral-medium-latest` | Mistral Medium | 32000 | 4096 | ✗ |
| | `mistral-small-latest` | Mistral Small | 32000 | 4096 | ✗ |
| | `codestral-latest` | Codestral | 256000 | 4096 | ✗ |
| **Groq** | `llama-3.3-70b-versatile` | Llama 3.3 70B (Groq) | 128000 | 32768 | ✗ |
| | `llama-3.1-8b-instant` | Llama 3.1 8B (Groq) | 128000 | 8192 | ✗ |
| | `mixtral-8x7b-32768` | Mixtral 8x7B (Groq) | 32768 | 4096 | ✗ |
| **XAI** | `grok-3` | Grok 3 | 131072 | 8192 | ✗ |
| | `grok-3-mini` | Grok 3 Mini | 131072 | 8192 | ✓ |

### 22.3 GLM JWT 认证 `[已实现]`

```rust
pub struct GlmProvider { ... }
impl GlmProvider {
    pub fn new() -> Self;
}
impl Default for GlmProvider { ... }
impl LanguageModelProvider for GlmProvider { ... }

pub struct GlmModel { ... }
impl GlmModel {
    /// 判断 API Key 是否需要 JWT 转换（点分隔格式）
    fn needs_jwt(api_key: &str) -> bool;

    /// 生成 JWT Token（HMAC-SHA256，自定义 sign_type: "SIGN"，默认 3600 秒过期）
    fn generate_jwt_token(api_key: &str) -> Result<String, LlmError>;
}
impl LanguageModel for GlmModel { ... }
```

---

## 23. 公开 API 导出表

> **源文件**: `src/lib.rs`

### 23.1 lib.rs 导出 `[已实现]`

> **当前实现**: 所有 `pub mod` 和 `pub use` 声明 [已实现]

```rust
pub mod error;
pub mod provider;
pub mod registry;
pub mod stream;
pub mod retry;
pub mod credential;
pub mod tool;
pub mod thinking;
pub mod rate_limit;
pub mod token_count;
pub mod response;
pub mod cancel;
pub mod middleware;
pub mod observability;

// ── 核心类型 ──
pub use error::LlmError;
pub use provider::{
    LanguageModel, LanguageModelProvider, ProviderKind,
    types::{
        ModelId, ModelName, ProviderId, ProviderName,
        Role, Message, MessageContent, ContentBlock, ImageSource, ImageDetail,
        StopReason, LanguageModelRequest, RequestMetadata,
        TokenUsage, ModelCapabilities, ProviderConfig, ModelConfig,
        RateLimitConfig, RetryConfig,
    },
};
pub use registry::{ModelRegistry, ModelRole};
pub use stream::{StreamEvent, ModelStream, StreamFuture};

// ── 重试 ──
pub use retry::{RetryPolicy, RetryOn, run_with_retry, backoff};

// ── 凭据 ──
pub use credential::{
    CredentialsProvider, ApiKeyState,
    EnvCredentialProvider, ConfigCredentialProvider,
    OAuthCredentialProvider, OAuthTokenSet,
    alias::AliasCredentialProvider,
    portal::{PortalAuthProvider, PortalAuthConfig},
    portal_auth::{PortalAuthCredentialProvider, PortalAuthStrategy, PortalAuthToken, PortalAuthType},
};

// ── Provider ──
pub use provider::glm::{GlmModel, GlmProvider};
pub use provider::factory::create_provider_by_kind;
pub use provider::openai_compatible::{OpenAiCompatibleModel, OpenAiCompatibleProvider};

// ── 工具 ──
pub use tool::{ToolDefinition, ToolCall, ToolResult, ToolExecutor, ToolCallLoop};

// ── 思维链 ──
pub use thinking::{ThinkingConfig, is_reasoning_model, strip_reasoning_params};

// ── 限流 ──
pub use rate_limit::{RateLimiter, RateLimitInfo, RateLimitGuard};

// ── Token 计数 ──
pub use token_count::{
    TokenCounter, TokenCountFuture,
    ByteEstimator, TiktokenCounter, FallbackTokenCounter,
};

// ── 响应 ──
pub use response::{CompletionResponse, ThinkingContent};

// ── 中间件 ──
pub use middleware::{Middleware, MiddlewarePipeline, LoggingMiddleware, MetricsMiddleware, RetryMiddleware};

// ── 可观测性 ──
pub use observability::{MetricsCollector, NoopMetricsCollector, LogLevel};

// ── 取消 ──
pub use cancel::CancellationToken;
```

---

## 24. 使用示例

> **注意**: 以下示例展示当前已实现的 API 形态。

### 24.1 最简调用（3 行）`[已实现]`

```rust
use ullm::{create_provider_by_kind, ProviderKind, LanguageModel, LanguageModelRequest, Message};

let provider = create_provider_by_kind(&ProviderKind::DeepSeek).unwrap();
let model = provider.provided_models().into_iter().next().unwrap();
let stream = model.stream_completion(
    LanguageModelRequest::new("deepseek-chat", vec![Message::user("Hello!")]).stream()
).await?;

// 消费流
pin_mut!(stream);
while let Some(event) = stream.next().await {
    if let Ok(StreamEvent::Text(text)) = event {
        print!("{}", text);
    }
}
```

### 24.2 非流式补全 `[已实现]`

```rust
let response = model.complete(
    LanguageModelRequest::new("deepseek-chat", vec![Message::user("Explain Rust")])
).await?;

println!("{}", response.text().unwrap_or(""));
println!("Tokens: {}", response.usage.total_tokens());
```

### 24.3 纯文本流 `[已实现]`

```rust
let text_stream = model.stream_text(
    LanguageModelRequest::new("gpt-4o", vec![Message::user("Tell me a story")]).stream()
).await?;

pin_mut!(text_stream);
while let Some(text) = text_stream.next().await {
    print!("{}", text?);
}
```

### 24.4 带工具调用 `[已实现]`

```rust
let tools = vec![
    ToolDefinition::new(
        "get_weather",
        "Get weather for a location",
        serde_json::json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            },
            "required": ["location"]
        }),
    ),
];

let request = LanguageModelRequest::new("gpt-4o", vec![Message::user("Weather in SF?")])
    .with_tools(tools)
    .stream();

let stream = model.stream_completion(request).await?;
// 处理 ToolUse 事件...
```

### 24.5 多模态消息 `[已实现]`

```rust
let request = LanguageModelRequest::new("gpt-4o", vec![
    Message::user_blocks(vec![
        ContentBlock::text("What's in this image?"),
        ContentBlock::image_url("https://example.com/photo.jpg"),
    ]),
]).stream();
```

### 24.6 自定义凭据 `[已实现]`

```rust
let provider = OpenAiCompatibleProvider::new(
    "my-provider", "My Provider",
    "https://api.my-provider.com/v1",
    None,
    vec![("my-model", "My Model", 128000, None, ModelCapabilities::default())],
)
.with_credentials(Arc::new(ConfigCredentialProvider::new(HashMap::from([
    ("my-provider".to_string(), "sk-my-key".to_string()),
]))))
.with_extra_headers(HashMap::from([
    ("X-Custom-Header".to_string(), "value".to_string()),
]));
```

### 24.7 中间件管道 `[已实现]`

```rust
let mut pipeline = MiddlewarePipeline::new();
pipeline.add(Arc::new(LoggingMiddleware::new(LogLevel::Info)));
pipeline.add(Arc::new(MetricsMiddleware::new(Arc::new(NoopMetricsCollector))));

// 在请求前执行
pipeline.on_request(&mut request).await?;
// 在响应后执行
pipeline.on_response(&request, &response).await;
```

### 24.8 模型注册表 + 别名 `[已实现]`

```rust
let mut registry = ModelRegistry::new();
registry.register(create_provider_by_kind(&ProviderKind::OpenAi).unwrap());
registry.register(create_provider_by_kind(&ProviderKind::Anthropic).unwrap());

registry.add_alias("smart", "gpt-4o");
registry.add_alias("fast", "gpt-4o-mini");

let model = registry.resolve_alias("smart")
    .and_then(|id| registry.find_model(id))
    .unwrap();
```

### 24.9 取消与超时 `[已实现]`

```rust
let token = CancellationToken::new();

tokio::spawn(async move {
    tokio::time::sleep(Duration::from_secs(5)).await;
    token.cancel();
});

let request = LanguageModelRequest::new("gpt-4o", vec![Message::user("Long task")])
    .with_timeout(Duration::from_secs(10))
    .with_cancel_token(token.clone())
    .stream();
```

---

## 25. 设计决策记录

> **本节与 §0.5 设计哲学映射表互为参照。每个 ADR 都映射到至少一条铁律或哲学。**

### ADR-001: 流式优先架构

**决策**: 底层唯一 I/O 路径为 `stream_completion()`，非流式 `complete()` 在其上聚合。

**铁律映射**: 零自编码底线（#4）— 非流式 API 复用流式基础设施，无需额外 I/O 路径

**原因**:
- 所有主流 Provider 均支持 SSE 流式
- 非流式响应可从流式事件完美聚合，反之不可
- 流式场景（IDE 补全、聊天界面）是主要用例
- 参考 Zed、Vercel AI SDK 均采用此模式

### ADR-002: 双 Trait 分离 (LanguageModel + LanguageModelProvider)

**决策**: 模型能力与 Provider 管理分为两个独立 trait。

**铁律映射**: 乐高式拼装 — 一个 Provider 可提供多个模型，复用 Provider 级凭据/限流

**原因**:
- 单一职责：模型关注调用，Provider 关注认证/配置
- 一个 Provider 可提供多个模型
- 参考 Zed 的 `LanguageModel` + `LanguageModelProvider` 设计

### ADR-003: MessageContent 双模式 (Text + Blocks)

**决策**: `Message.content` 使用 `MessageContent` 枚举，支持纯文本和多态内容块。

**原因**:
- 向后兼容：现有 `Message::user("hello")` 代码无需修改
- 前向兼容：支持图像、工具调用结果、思维链混合内容
- 参考 Anthropic 的 Content Block 模式和 Zed 的 `MessageContent` 枚举

### ADR-004: ProviderKind 手动 Serialize/Deserialize

**决策**: 不使用 `#[serde(untagged)]`，手动实现序列化。

**原因**: `untagged` 导致单元变体序列化为 `null`，无法正确区分已知 Provider 和 `OpenAiCompatible`。

### ADR-005: OpenAiCompatibleProvider 复用模式

**决策**: 7 个 Provider 复用 `OpenAiCompatibleProvider`/`OpenAiCompatibleModel`。

**铁律映射**: 零自编码底线（#4）+ 乐高式拼装 — 复用减少 80% 自编码

**原因**: OpenAI API 已成为事实标准，DeepSeek/Qwen/Kimi/Gemini/Mistral/Groq/OpenRouter 均兼容。

### ADR-006: GLM JWT 自定义实现

**决策**: 使用 `hmac` + `sha2` + `base64` 手动实现 JWT，而非 `jsonwebtoken` crate。

**铁律映射**: 零自编码底线（#4）+ 所有依赖在 crates.io 存在（#2）— 用 RustCrypto 生态标准组件组合替代 jsonwebtoken

**原因**: GLM JWT 需要 `sign_type: "SIGN"` 自定义 Header 字段，`jsonwebtoken` 不支持。

### ADR-007: PortalAuthCredentialProvider 使用 tokio::sync::RwLock

**决策**: Portal 认证使用 `tokio::sync::RwLock` 而非 `parking_lot::RwLock`。

**原因**: Token 刷新可能跨 await 点，需要 Send-safe 的锁。`parking_lot::RwLock` 在 async 上下文中持有会导致编译错误。

### ADR-008: SSE 映射器 reasoning_content 优先检查

**决策**: OpenAI SSE 映射器中，`reasoning_content` 在 `content` 之前检查。

**原因**: 部分 Provider（DeepSeek）在同一 chunk 中同时发送 `reasoning_content` 和 `content`，优先检查确保思维链内容不丢失。

### ADR-009: 中间件管道模式

**决策**: 引入 `Middleware` trait + `MiddlewarePipeline` 管道。

**原因**:
- 日志、指标、缓存等横切关注点需要统一管理
- 参考 OpenAI Python SDK 的 `http_client` 注入和 Vercel AI SDK 的 `onChunk`/`onFinish` 回调
- 管道模式比回调更灵活，支持有序组合

### ADR-010: CancellationToken 协作式取消

**决策**: 使用协作式取消令牌，而非强制中止。

**原因**:
- 强制中止（`tokio::task::abort`）可能导致资源泄漏
- 协作式取消允许流式传输优雅关闭
- 参考 C# 的 `CancellationToken` 和 Go 的 `context.Context` 模式

---

## 26. 源文件映射与依赖约束

> **本节是 AI 编码的关键参考。违反本节约束将导致编译失败或铁律违规。**

### 26.1 源文件 → API 章节映射

| 源文件路径 | 对应章节 | 状态 |
|-----------|---------|------|
| `src/lib.rs` | §23 公开 API 导出表 | [已实现] |
| `src/error.rs` | §12 错误处理 | [已实现] |
| `src/provider/mod.rs` | §3 Provider 抽象层 · §2.2 ProviderKind | [已实现] |
| `src/provider/types.rs` | §2.1 标识符 · §4.2 Message · §4.3 Role · §5 请求构建器 · §5.2 RequestMetadata · §6.2 StopReason · §6.3 TokenUsage · §21 配置系统 | [已实现] |
| `src/provider/content_block.rs` | §4.1 ContentBlock · §4.2 MessageContent | [已实现] |
| `src/provider/ext.rs` | §3.3 高级 API 扩展 | [已实现] |
| `src/provider/openai_compatible.rs` | §22.1 OpenAiCompatibleProvider/Model | [已实现] |
| `src/provider/openai.rs` | §22.2 OpenAi Provider | [已实现] |
| `src/provider/anthropic.rs` | §22.2 Anthropic Provider | [已实现] |
| `src/provider/glm.rs` | §22.3 GLM JWT 认证 | [已实现] |
| `src/provider/deepseek.rs` | §22.2 DeepSeek Provider | [已实现] |
| `src/provider/qwen.rs` | §22.2 Qwen Provider | [已实现] |
| `src/provider/kimi.rs` | §22.2 Kimi Provider | [已实现] |
| `src/provider/openrouter.rs` | §22.2 OpenRouter Provider | [已实现] |
| `src/provider/gemini.rs` | §22.2 Gemini Provider | [已实现] |
| `src/provider/mistral.rs` | §22.2 Mistral Provider | [已实现] |
| `src/provider/groq.rs` | §22.2 Groq Provider | [已实现] |
| `src/provider/xai.rs` | §22.2 XAI Provider | [已实现] |
| `src/provider/ollama.rs` | §22.2 Ollama Provider | [已实现] |
| `src/provider/codegeex.rs` | §22.2 CodeGeeX Provider | [已实现] |
| `src/provider/web_scraper.rs` | §22.2 WebScraper Provider | [已实现] |
| `src/provider/factory.rs` | §17 Provider 工厂 | [已实现] |
| `src/stream/mod.rs` | §7.1 StreamEvent · §7.2 类型别名 | [已实现] |
| `src/stream/sse.rs` | §7.3 SseParser · §7.4 SseEventMapper | [已实现] |
| `src/stream/openai_mapper.rs` | §7.5 OpenAiSseMapper | [已实现] |
| `src/stream/anthropic_mapper.rs` | §7.5 AnthropicSseMapper | [已实现] |
| `src/stream/tool_call_state.rs` | §7.6 ToolCallAccumulator | [已实现] |
| `src/stream/json_fix.rs` | §7.7 JSON 修复工具 | [已实现] |
| `src/credential/mod.rs` | §8.1-8.4 凭据管理 | [已实现] |
| `src/credential/alias.rs` | §8.3 AliasCredentialProvider | [已实现] |
| `src/credential/portal_auth.rs` | §9.1-9.4 PortalAuth | [已实现] |
| `src/credential/portal/mod.rs` | §9.6 PortalAuthProvider/Config | [已实现] |
| `src/credential/portal/qwen.rs` | §9.5 QwenPortalAuth | [已实现] |
| `src/credential/portal/copilot.rs` | §9.5 CopilotProxyAuth | [已实现] |
| `src/credential/portal/gemini_cli.rs` | §9.5 GeminiCliAuth | [已实现] |
| `src/credential/portal/minimax.rs` | §9.5 MiniMaxPortalAuth | [已实现] |
| `src/tool/mod.rs` | §10 工具调用 | [已实现] |
| `src/thinking/mod.rs` | §11 思维链/推理模型 | [已实现] |
| `src/retry/mod.rs` | §13 重试策略 | [已实现] |
| `src/rate_limit/mod.rs` | §14 限流器 | [已实现] |
| `src/token_count/mod.rs` | §15 Token 计数 | [已实现] |
| `src/registry/mod.rs` | §16 模型注册表 · §16.2 ModelRole | [已实现] |
| `src/response.rs` | §6.1 CompletionResponse | [已实现] |
| `src/cancel.rs` | §20 取消与超时 | [已实现] |
| `src/middleware.rs` | §18 中间件/拦截器 | [已实现] |
| `src/observability.rs` | §19 可观测性 | [已实现] |

### 26.2 Cargo.toml 依赖约束

> **铁律 #2/#3/#4 合规**: 以下所有依赖均在 crates.io 存在，License 为 MIT/Apache-2.0。

```toml
[dependencies]
tokio = { workspace = true }                                          # 实际使用 workspace 继承
async-trait = { workspace = true }                                    # 实际使用 workspace 继承
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "stream", "cookies"] }  # 注意：必须包含 "json" feature
serde = { workspace = true, features = ["rc"] }                      # 注意：features 必须包含 "rc"（Arc<str> 序列化需要），derive 由 workspace 提供
serde_json = { workspace = true }                                     # 实际使用 workspace 继承
thiserror = { workspace = true }                                      # 实际使用 workspace 继承
anyhow = { workspace = true }                                         # 实际使用 workspace 继承
futures-util = { version = "0.3", default-features = false }
dashmap = { version = "5.5", default-features = false }              # 注意：版本是 5.5 而非 6
parking_lot = { version = "0.12", default-features = false }
tracing = { workspace = true }                                        # 实际使用 workspace 继承；API.md 前版遗漏此依赖
tiktoken-rs = { version = "0.6", default-features = false }
hmac = { version = "0.12", default-features = false }                # RustCrypto 生态 — JWT 签名（铁律 #4：复用 crate 而非自编码）
sha2 = { version = "0.10", default-features = false }                # RustCrypto 生态 — JWT 哈希
base64 = { version = "0.22", default-features = false, features = ["std"] }  # 注意：features 必须是 ["std"] 而非 ["alloc"]
pin-project = { version = "1.1", default-features = false }
rand = { version = "0.8", default-features = false, features = ["small_rng", "std", "std_rng"] }
convert_case = { version = "0.6", default-features = false }
uuid = { workspace = true }                                           # 实际使用 workspace 继承
http = { version = "1.0", default-features = false }
bytes = { version = "1.10", default-features = false }
once_cell = { version = "1.20", default-features = false }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros"] }
```

> **workspace 依赖说明**: 标注 `workspace = true` 的依赖由工作区根 `Cargo.toml` 统一管理版本。AI 编码时如需添加新依赖，优先使用 workspace 继承；如 workspace 中无此依赖，则使用显式版本号并注明 crates.io 链接。

**禁止引入的依赖**（铁律 #4 合规）:

| 禁止依赖 | 原因 | 替代方案 |
|----------|------|---------|
| `jsonwebtoken` | 不支持 `sign_type: "SIGN"` 自定义 Header | `hmac` + `sha2` + `base64` 组合 |
| `cookie`（独立 crate） | `reqwest::cookie::Jar` 已满足需求 | `reqwest::cookie::Jar` |
| 任何非 crates.io 依赖 | 铁律 #2 | — |
| 任何非 MIT/Apache-2.0 依赖 | 铁律 #3 | — |

### 26.3 实现约束（AI 编码必须遵循）

#### 26.3.1 RwLock 选型规则

| 场景 | 使用 | 原因 |
|------|------|------|
| 同步上下文（不跨 await） | `parking_lot::RwLock` | 性能更优，无 poison |
| 异步上下文（跨 await 持有） | `tokio::sync::RwLock` | Send-safe，不会阻塞 tokio 运行时 |

**具体映射**:
- `EnvCredentialProvider.cache` → `parking_lot::RwLock` ✓
- `ConfigCredentialProvider.keys` → `parking_lot::RwLock` ✓
- `OAuthCredentialProvider.tokens` → `parking_lot::RwLock` ✓
- `PortalAuthCredentialProvider.token` → `tokio::sync::RwLock` ✓（刷新跨 await）
- `PortalAuthProvider.cached_token` → `parking_lot::RwLock` ✓
- `RateLimiter.info` → `parking_lot::RwLock` ✓

#### 26.3.2 Serde 注解规则

| 场景 | 注解 | 示例 |
|------|------|------|
| 可选字段（序列化时省略 None） | `#[serde(default)]` | `ProviderConfig.extra_headers` |
| 非序列化字段 | `#[serde(skip)]` | `LanguageModelRequest.timeout` |
| 枚举判别 | `#[serde(tag = "type")]` | `ContentBlock`（内部表示） |
| **禁止** `#[serde(untagged)]` 用于枚举 | 会导致序列化歧义 | `ProviderKind` 使用手动 impl |
| `MessageContent` 例外 | `#[serde(untagged)]` | 仅此一处允许，因为 `String` vs `Vec<ContentBlock>` 无歧义 |

#### 26.3.3 `#[allow(dead_code)]` 规则

- 仅用于 `SseParser.provider` 和 `SseParser.model` 等预留字段
- 必须添加注释说明预留原因
- 其他 `dead_code` 警告必须通过实际使用解决

#### 26.3.4 `base64` features 约束

```toml
base64 = { version = "0.22", features = ["std"] }  # 正确
# base64 = { version = "0.22", features = ["alloc"] }  # 错误！DESIGN.md §10.2 明确要求 std
```

#### 26.3.5 `normalize_object_schema` 和 `sanitize_tool_message_pairing`

这两个函数位于 `src/provider/openai_compatible.rs`，是 `OpenAiCompatibleModel::build_request_body()` 的内部工具：

- `normalize_object_schema`: 修复 `type: "object"` 缺少 `properties` 字段的 JSON Schema
- `sanitize_tool_message_pairing`: 确保每个 `tool_calls` 消息后都有对应的 `tool` 角色结果消息

**AI 编码时**: 新增 Provider 如果复用 `OpenAiCompatibleModel`，这两个函数自动生效，无需重复实现。

#### 26.3.6 SSE 映射器 `reasoning_content` 优先级

`src/stream/openai_mapper.rs` 中，`reasoning_content` 必须在 `content` 之前检查：

```rust
// 正确顺序（ADR-008）
if let Some(reasoning) = delta.get("reasoning_content") { ... }
else if let Some(content) = delta.get("content") { ... }

// 错误顺序（会导致思维链内容丢失）
// if let Some(content) = delta.get("content") { ... }
// else if let Some(reasoning) = delta.get("reasoning_content") { ... }
```

### 26.4 新增模块注册清单

实现新模块时，必须在以下位置同步注册：

1. **`src/lib.rs`**: 添加 `pub mod module_name;` 和 `pub use module_name::{...};`
2. **`Cargo.toml`**: 如需新依赖，必须验证 crates.io 存在性 + License
3. **`src/provider/mod.rs`**: 如为 Provider 模块，添加 `pub mod provider_name;`
4. **本文件 §26.1**: 更新源文件映射表

### 26.5 实现优先级与测试规范

#### 26.5.1 实现优先级 — 全部已完成 ✅

| 优先级 | 模块 | 文件 | 状态 |
|--------|------|------|------|
| P0 | `LanguageModelRequest` Builder | `src/provider/types.rs` | ✅ 已实现 |
| P0 | `LlmError::Cancelled` | `src/error.rs` | ✅ 已实现 |
| P0 | `StopReason` 方法 | `src/provider/types.rs` | ✅ 已实现 |
| P0 | `TokenUsage` 方法 | `src/provider/types.rs` | ✅ 已实现 |
| P0 | `StreamEvent` 方法 | `src/stream/mod.rs` | ✅ 已实现 |
| P0 | `ToolDefinition/ToolCall/ToolResult` 方法 | `src/tool/mod.rs` | ✅ 已实现 |
| P1 | `ContentBlock` + `MessageContent` | `src/provider/content_block.rs` | ✅ 已实现 |
| P1 | `CompletionResponse` | `src/response.rs` | ✅ 已实现 |
| P1 | `dyn LanguageModel` 扩展 | `src/provider/ext.rs` | ✅ 已实现 |
| P1 | `ModelRegistry` 扩展 + `ModelRole` | `src/registry/mod.rs` | ✅ 已实现 |
| P1 | `ProviderConfig` 方法 | `src/provider/types.rs` | ✅ 已实现 |
| P1 | 标识符 `as_str()` | `src/provider/types.rs` | ✅ 已实现 |
| P1 | `PortalAuthToken::is_expired/remaining_secs` | `src/credential/portal_auth.rs` | ✅ 已实现 |
| P2 | `RequestMetadata` | `src/provider/types.rs` | ✅ 已实现 |
| P2 | `LanguageModelRequest` 新字段 | `src/provider/types.rs` | ✅ 已实现 |
| P2 | `CancellationToken` | `src/cancel.rs` | ✅ 已实现 |
| P2 | `Middleware` + `MiddlewarePipeline` + 内置中间件 | `src/middleware.rs` | ✅ 已实现 |
| P2 | `MetricsCollector` + `NoopMetricsCollector` + `LogLevel` | `src/observability.rs` | ✅ 已实现 |

#### 26.5.2 测试规范

**测试位置**:
- 单元测试：各源文件底部 `#[cfg(test)] mod tests { ... }` 块
- 集成测试：`tests/` 目录下独立文件（如 `tests/test_providers.rs`）

**测试命名规范**:
```
test_{模块}_{场景}_{预期结果}
```
示例：`test_alias_provider_fallback_alias_env`, `test_kimi_alias_priority`

**环境变量测试规范**:
- 使用 `std::env::set_var()` 设置测试环境变量
- 测试结束前必须调用 `std::env::remove_var()` 清理
- 环境变量名必须包含 `std::process::id()` 避免并行测试冲突

**当前测试数量**: 266 tests (255 unit + 11 integration)

#### 26.5.3 `cargo` 验证命令

实现后必须运行以下命令确保零回归：

```bash
cargo test -p ullm          # 全部测试通过
cargo clippy -p ullm -- -W clippy::all  # 零警告
```
