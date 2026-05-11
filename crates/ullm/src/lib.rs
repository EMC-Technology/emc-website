#![warn(missing_docs)]
//! Universal LLM Access Lifecycle Management Module (ullm)
//!
//! 统一 LLM 客户端库，支持多供应商（OpenAI/Kimi/Qwen/DeepSeek/Ollama）的统一接口。
//! 提供凭证管理、流式响应、速率限制、重试策略、可观测性等全生命周期管理能力。
//!
//! ## 核心能力
//!
//! - **多供应商适配**：`OpenAI` 兼容协议统一封装
//! - **凭证管理**：环境变量 / 配置文件 / OAuth / Portal 多源凭证
//! - **流式响应**：SSE 解析 + 异步迭代器
//! - **速率限制**：令牌桶 + 滑动窗口
//! - **重试策略**：确定性指数退避（FNV-1a 抖动）
//! - **可观测性**：OpenTelemetry Tracing + Metrics
//! - **错误桥接**：LLM 错误 → error-core 四维分类

/// LLM 请求 API 入口
pub mod api;
/// 请求取消令牌
pub mod cancel;
/// 凭证管理（环境变量/配置文件/OAuth/Portal/Alias）
pub mod credential;
/// LLM 错误类型定义
pub mod error;
/// LLM 错误到 error-core 的桥接
pub mod error_bridge;
/// 请求/响应中间件管道
pub mod middleware;
/// 可观测性（日志/指标/追踪）
pub mod observability;
/// 公共重导出
pub mod prelude;
/// 跨进程桥接类型（IPC 序列化）
pub mod process_bridge;
/// LLM 供应商适配层
pub mod provider;
/// 速率限制器
pub mod rate_limit;
/// 模型注册表
pub mod registry;
/// 补全响应类型
pub mod response;
/// 重试策略与指数退避
pub mod retry;
/// 流式响应解析
pub mod stream;
/// 流式响应到文本的桥接
pub mod stream_bridge;
/// 思维链（Thinking）配置
pub mod thinking;
/// Token 计数器
pub mod token_count;
/// 工具调用（Function Calling）定义与执行
pub mod tool;

pub use cancel::CancellationToken;
pub use credential::{
    ApiKeyState, ConfigCredentialProvider, CredentialsProvider, EnvCredentialProvider,
    OAuthCredentialProvider, OAuthTokenSet,
    alias::AliasCredentialProvider,
    portal::{PortalAuthConfig, PortalAuthProvider},
    portal_auth::{
        PortalAuthCredentialProvider, PortalAuthStrategy, PortalAuthToken, PortalAuthType,
    },
};
pub use error::LlmError;
pub use middleware::{
    LoggingMiddleware, MetricsMiddleware, Middleware, MiddlewarePipeline, RetryMiddleware,
};
pub use observability::{LogLevel, MetricsCollector, NoopMetricsCollector};
pub use provider::factory::create_provider_by_kind;
pub use provider::glm::{GlmModel, GlmProvider};
pub use provider::openai_compatible::{OpenAiCompatibleModel, OpenAiCompatibleProvider};
pub use provider::{
    LanguageModel, LanguageModelProvider, ProviderKind,
    types::{
        ContentBlock, ImageDetail, ImageSource, LanguageModelRequest, Message, MessageContent,
        ModelCapabilities, ModelConfig, ModelId, ModelName, ProviderConfig, ProviderId,
        ProviderName, RateLimitConfig, RequestMetadata, RetryConfig, Role, StopReason, TokenUsage,
    },
};
pub use rate_limit::{RateLimitGuard, RateLimitInfo, RateLimiter};
pub use registry::{ModelRegistry, ModelRole};
pub use response::{CompletionResponse, ThinkingContent};
pub use retry::{RetryOn, RetryPolicy, backoff, run_with_retry};
pub use stream::{ModelStream, StreamEvent, StreamFuture};
pub use thinking::{ThinkingConfig, is_reasoning_model, strip_reasoning_params};
pub use token_count::{
    ByteEstimator, FallbackTokenCounter, TiktokenCounter, TokenCountFuture, TokenCounter,
};
pub use tool::{ToolCall, ToolCallLoop, ToolDefinition, ToolExecutor, ToolResult};

pub use api::LlmApi;

/// 构建带默认超时的 HTTP 客户端，返回 `Result` 而非 panic
///
/// 替代 `Client::builder().timeout(...).build().expect(...)` 模式，
/// 在 TLS 后端缺失或系统资源耗尽时返回错误而非 panic。
///
/// # Errors
///
/// 当 HTTP 客户端构建失败时返回 `LlmError::HttpClientInit`
pub fn build_http_client(timeout: std::time::Duration) -> Result<reqwest::Client, LlmError> {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| LlmError::HttpClientInit(e.to_string()))
}

/// 构建带默认超时和 Cookie 存储的 HTTP 客户端，返回 `Result` 而非 panic
///
/// 与 [`build_http_client`] 相同，但额外启用 `.cookie_store(true)`，
/// 适用于 Portal 认证等需要自动管理 Cookie 的场景。
///
/// # Errors
///
/// 当 HTTP 客户端构建失败时返回 `LlmError::HttpClientInit`
pub fn build_http_client_with_cookies(
    timeout: std::time::Duration,
) -> Result<reqwest::Client, LlmError> {
    reqwest::Client::builder()
        .timeout(timeout)
        .cookie_store(true)
        .build()
        .map_err(|e| LlmError::HttpClientInit(e.to_string()))
}
