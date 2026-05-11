use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::cancel::CancellationToken;

pub use crate::provider::content_block::{ContentBlock, ImageDetail, ImageSource, MessageContent};

/// 模型标识（Arc<str> 包装，零拷贝克隆）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId(Arc<str>);

impl ModelId {
    /// 创建模型标识
    pub fn new(id: impl AsRef<str>) -> Self {
        Self(Arc::from(id.as_ref()))
    }

    /// 获取字符串引用
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 是否为空
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for ModelId {
    fn from(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl From<&str> for ModelId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

impl AsRef<str> for ModelId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 模型显示名称（Arc<str> 包装）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelName(Arc<str>);

impl ModelName {
    /// 创建模型显示名称
    pub fn new(name: impl AsRef<str>) -> Self {
        Self(Arc::from(name.as_ref()))
    }

    /// 获取字符串引用
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ModelName {
    fn from(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl From<&str> for ModelName {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

impl AsRef<str> for ModelName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 供应商标识（Arc<str> 包装）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderId(Arc<str>);

impl ProviderId {
    /// 创建供应商标识
    pub fn new(id: impl AsRef<str>) -> Self {
        Self(Arc::from(id.as_ref()))
    }

    /// 获取字符串引用
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProviderId {
    fn from(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl From<&str> for ProviderId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

impl AsRef<str> for ProviderId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 供应商显示名称（Arc<str> 包装）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderName(Arc<str>);

impl ProviderName {
    /// 创建供应商显示名称
    pub fn new(name: impl AsRef<str>) -> Self {
        Self(Arc::from(name.as_ref()))
    }

    /// 获取字符串引用
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProviderName {
    fn from(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl From<&str> for ProviderName {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

impl AsRef<str> for ProviderName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 消息角色
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// 系统指令
    System,
    /// 用户消息
    User,
    /// 助手回复
    Assistant,
    /// 工具结果
    Tool,
}

impl Role {
    /// 获取角色的字符串表示
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 对话消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 消息角色
    pub role: Role,
    /// 消息内容
    pub content: MessageContent,
    /// 发送者名称
    pub name: Option<String>,
    /// 工具调用标识（仅 Tool 角色使用）
    pub tool_call_id: Option<String>,
}

impl Message {
    /// 创建系统消息
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: MessageContent::text(content),
            name: None,
            tool_call_id: None,
        }
    }

    /// 创建用户文本消息
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::text(content),
            name: None,
            tool_call_id: None,
        }
    }

    /// 创建用户多模态消息
    #[must_use]
    pub fn user_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::blocks(blocks),
            name: None,
            tool_call_id: None,
        }
    }

    /// 创建助手文本消息
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::text(content),
            name: None,
            tool_call_id: None,
        }
    }

    /// 创建助手多模态消息
    #[must_use]
    pub fn assistant_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::blocks(blocks),
            name: None,
            tool_call_id: None,
        }
    }

    /// 创建工具结果消息
    #[must_use]
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: MessageContent::text(content),
            name: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// 创建工具错误结果消息
    #[must_use]
    pub fn tool_result_error(tool_call_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: MessageContent::text(error),
            name: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// 设置发送者名称
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 设置工具调用标识
    #[must_use]
    pub fn with_tool_call_id(mut self, id: impl Into<String>) -> Self {
        self.tool_call_id = Some(id.into());
        self
    }
}

/// 停止原因
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StopReason {
    /// 模型正常结束
    EndTurn,
    /// 模型发起工具调用
    ToolCall,
    /// 达到最大 Token 数
    MaxTokens,
    /// 命中停止序列
    StopSequence,
    /// 请求被取消
    Cancelled,
}

impl StopReason {
    /// 获取停止原因的字符串表示
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            StopReason::EndTurn => "end_turn",
            StopReason::ToolCall => "tool_call",
            StopReason::MaxTokens => "max_tokens",
            StopReason::StopSequence => "stop_sequence",
            StopReason::Cancelled => "cancelled",
        }
    }

    /// 是否为工具调用停止
    #[must_use]
    pub fn is_tool_call(&self) -> bool {
        matches!(self, StopReason::ToolCall)
    }
}

impl fmt::Display for StopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 请求元数据
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestMetadata {
    /// 请求唯一标识
    pub request_id: Option<String>,
    /// 用户标识
    pub user_id: Option<String>,
    /// 会话标识
    pub session_id: Option<String>,
    /// 自定义标签
    pub tags: HashMap<String, String>,
}

impl RequestMetadata {
    /// 创建空元数据
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置请求标识
    #[must_use]
    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    /// 设置用户标识
    #[must_use]
    pub fn with_user_id(mut self, id: impl Into<String>) -> Self {
        self.user_id = Some(id.into());
        self
    }

    /// 设置会话标识
    #[must_use]
    pub fn with_session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    /// 添加自定义标签
    #[must_use]
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }
}

/// LLM 请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageModelRequest {
    /// 目标模型标识
    pub model: ModelId,
    /// 对话消息列表
    pub messages: Vec<Message>,
    /// 采样温度
    pub temperature: Option<f32>,
    /// 最大输出 Token 数
    pub max_tokens: Option<u32>,
    /// Top-P 采样参数
    pub top_p: Option<f32>,
    /// 频率惩罚
    pub frequency_penalty: Option<f32>,
    /// 存在惩罚
    pub presence_penalty: Option<f32>,
    /// 停止序列
    pub stop: Option<Vec<String>>,
    /// 可用工具定义
    pub tools: Option<Vec<crate::tool::ToolDefinition>>,
    /// 思维链配置
    pub thinking: Option<crate::thinking::ThinkingConfig>,
    /// 是否启用流式响应
    pub stream: bool,
    /// 请求元数据
    pub metadata: Option<RequestMetadata>,
    /// 请求超时
    #[serde(skip)]
    pub timeout: Option<Duration>,
    /// 取消令牌
    #[serde(skip)]
    pub cancel: Option<CancellationToken>,
}

impl LanguageModelRequest {
    /// 创建新的 LLM 请求
    #[must_use]
    pub fn new(model: impl Into<ModelId>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            tools: None,
            thinking: None,
            stream: false,
            metadata: None,
            timeout: None,
            cancel: None,
        }
    }

    /// 设置采样温度
    #[must_use]
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// 设置最大输出 Token 数
    #[must_use]
    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// 设置 Top-P 采样参数
    #[must_use]
    pub fn with_top_p(mut self, p: f32) -> Self {
        self.top_p = Some(p);
        self
    }

    /// 设置频率惩罚
    #[must_use]
    pub fn with_frequency_penalty(mut self, penalty: f32) -> Self {
        self.frequency_penalty = Some(penalty);
        self
    }

    /// 设置存在惩罚
    #[must_use]
    pub fn with_presence_penalty(mut self, penalty: f32) -> Self {
        self.presence_penalty = Some(penalty);
        self
    }

    /// 设置停止序列
    #[must_use]
    pub fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.stop = Some(stop);
        self
    }

    /// 设置可用工具
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<crate::tool::ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// 设置思维链配置
    #[must_use]
    pub fn with_thinking(mut self, config: crate::thinking::ThinkingConfig) -> Self {
        self.thinking = Some(config);
        self
    }

    /// 设置请求超时
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// 设置取消令牌
    #[must_use]
    pub fn with_cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel = Some(token);
        self
    }

    /// 设置请求元数据
    #[must_use]
    pub fn with_metadata(mut self, metadata: RequestMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// 启用流式响应
    #[must_use]
    pub fn stream(mut self) -> Self {
        self.stream = true;
        self
    }

    /// 追加消息
    #[must_use]
    pub fn add_message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    /// 追加系统消息
    #[must_use]
    pub fn add_system(self, content: impl Into<String>) -> Self {
        self.add_message(Message::system(content))
    }

    /// 追加用户消息
    #[must_use]
    pub fn add_user(self, content: impl Into<String>) -> Self {
        self.add_message(Message::user(content))
    }

    /// 追加助手消息
    #[must_use]
    pub fn add_assistant(self, content: impl Into<String>) -> Self {
        self.add_message(Message::assistant(content))
    }

    /// 校验请求参数的合法性。
    ///
    /// # Errors
    ///
    /// 当 `model` 为空、`messages` 为空、`temperature` 超出 [0.0, 2.0] 范围、
    /// `top_p` 超出 [0.0, 1.0] 范围、或 `frequency_penalty` / `presence_penalty`
    /// 超出 [-2.0, 2.0] 范围时返回 `LlmError::InvalidRequest`。
    pub fn validate(&self) -> Result<(), crate::error::LlmError> {
        if self.model.as_ref().is_empty() {
            return Err(crate::error::LlmError::InvalidRequest {
                message: "model field must not be empty".into(),
            });
        }
        if self.messages.is_empty() {
            return Err(crate::error::LlmError::InvalidRequest {
                message: "messages field must not be empty".into(),
            });
        }
        if let Some(temp) = self.temperature
            && !(0.0..=2.0).contains(&temp)
        {
            return Err(crate::error::LlmError::InvalidRequest {
                message: format!("temperature must be between 0.0 and 2.0, got {temp}"),
            });
        }
        if let Some(top_p) = self.top_p
            && !(0.0..=1.0).contains(&top_p)
        {
            return Err(crate::error::LlmError::InvalidRequest {
                message: format!("top_p must be between 0.0 and 1.0, got {top_p}"),
            });
        }
        if let Some(freq) = self.frequency_penalty
            && !(-2.0..=2.0).contains(&freq)
        {
            return Err(crate::error::LlmError::InvalidRequest {
                message: format!("frequency_penalty must be between -2.0 and 2.0, got {freq}"),
            });
        }
        if let Some(pres) = self.presence_penalty
            && !(-2.0..=2.0).contains(&pres)
        {
            return Err(crate::error::LlmError::InvalidRequest {
                message: format!("presence_penalty must be between -2.0 and 2.0, got {pres}"),
            });
        }
        Ok(())
    }
}

/// Token 用量统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// 输入 Token 数
    pub prompt_tokens: u32,
    /// 输出 Token 数
    pub completion_tokens: u32,
    /// 缓存读取 Token 数
    pub cache_read_tokens: u32,
    /// 缓存写入 Token 数
    pub cache_write_tokens: u32,
}

impl TokenUsage {
    /// 总 Token 数（输入 + 输出）
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        u64::from(self.prompt_tokens) + u64::from(self.completion_tokens)
    }

    /// 是否所有计数均为零
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.prompt_tokens == 0
            && self.completion_tokens == 0
            && self.cache_read_tokens == 0
            && self.cache_write_tokens == 0
    }

    /// 合并另一组 Token 用量（饱和加法）
    pub fn merge(&mut self, other: &TokenUsage) {
        self.prompt_tokens = self.prompt_tokens.saturating_add(other.prompt_tokens);
        self.completion_tokens = self
            .completion_tokens
            .saturating_add(other.completion_tokens);
        self.cache_read_tokens = self
            .cache_read_tokens
            .saturating_add(other.cache_read_tokens);
        self.cache_write_tokens = self
            .cache_write_tokens
            .saturating_add(other.cache_write_tokens);
    }
}

/// 模型能力描述
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCapabilities {
    /// 是否支持工具调用
    pub tools: bool,
    /// 是否支持图片输入
    pub images: bool,
    /// 是否支持流式工具调用
    pub streaming_tools: bool,
    /// 是否支持并行工具调用
    pub parallel_tool_calls: bool,
    /// 是否支持思维链
    pub thinking: bool,
    /// 最大上下文 Token 数
    pub max_tokens: u64,
}

/// 供应商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// 供应商标识
    pub id: ProviderId,
    /// 供应商显示名称
    pub name: ProviderName,
    /// API 基地址
    pub api_url: String,
    /// API Key 环境变量名
    pub api_key_env: Option<String>,
    /// 模型配置列表
    pub models: Vec<ModelConfig>,
    /// 速率限制配置
    pub rate_limit: Option<RateLimitConfig>,
    /// 重试配置
    pub retry: Option<RetryConfig>,
    /// 额外请求头
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,
    /// 最大请求体字节数
    #[serde(default)]
    pub max_request_body_bytes: Option<usize>,
}

impl ProviderConfig {
    /// 从 JSON 字符串解析 Provider 配置。
    ///
    /// # Errors
    ///
    /// 当 JSON 格式无效或配置校验失败时返回 `LlmError::Config`。
    pub fn from_json(json: &str) -> Result<Self, crate::error::LlmError> {
        let config: Self = serde_json::from_str(json).map_err(|e| {
            crate::error::LlmError::Config(format!("invalid provider config JSON: {e}"))
        })?;
        config.validate()?;
        Ok(config)
    }

    /// 从 JSON 文件解析 Provider 配置。
    ///
    /// # Errors
    ///
    /// 当文件读取失败、JSON 格式无效或配置校验失败时返回 `LlmError::Config`。
    pub fn from_json_file(path: &std::path::Path) -> Result<Self, crate::error::LlmError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            crate::error::LlmError::Config(format!(
                "failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;
        Self::from_json(&content)
    }

    /// 校验 Provider 配置的合法性。
    ///
    /// # Errors
    ///
    /// 当 `api_url` 为空、`models` 为空，或模型 ID 为空时返回 `LlmError::Config`。
    pub fn validate(&self) -> Result<(), crate::error::LlmError> {
        if self.api_url.is_empty() {
            return Err(crate::error::LlmError::Config(
                "api_url must not be empty".into(),
            ));
        }
        if self.models.is_empty() {
            return Err(crate::error::LlmError::Config(
                "models must not be empty".into(),
            ));
        }
        for model in &self.models {
            if model.id.is_empty() {
                return Err(crate::error::LlmError::Config(format!(
                    "model id must not be empty in provider {}",
                    self.id
                )));
            }
        }
        Ok(())
    }
}

/// 模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// 模型标识
    pub id: ModelId,
    /// 模型显示名称
    pub display_name: String,
    /// 最大上下文 Token 数
    pub max_tokens: u64,
    /// 最大输出 Token 数
    pub max_output_tokens: Option<u64>,
    /// 模型能力
    pub capabilities: ModelCapabilities,
}

/// 速率限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// 最大并发请求数
    pub max_concurrent_requests: usize,
    /// 每分钟请求数限制
    pub requests_per_minute: Option<u32>,
}

/// 重试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 基础延迟（毫秒）
    pub base_delay_ms: u64,
    /// 是否重试速率限制错误
    pub retry_on_rate_limit: bool,
    /// 是否重试服务器错误
    pub retry_on_server_error: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_id_roundtrip() {
        let id = ModelId::new("gpt-4o");
        assert_eq!(id.as_ref(), "gpt-4o");
        assert_eq!(id.as_str(), "gpt-4o");
        assert_eq!(id.to_string(), "gpt-4o");
    }

    #[test]
    fn test_model_id_from_str() {
        let id: ModelId = "claude-3".into();
        assert_eq!(id.as_ref(), "claude-3");
    }

    #[test]
    fn test_model_id_equality() {
        let a = ModelId::new("gpt-4o");
        let b = ModelId::new("gpt-4o");
        assert_eq!(a, b);
    }

    #[test]
    fn test_model_name_as_str() {
        let name = ModelName::new("GPT-4o");
        assert_eq!(name.as_str(), "GPT-4o");
    }

    #[test]
    fn test_provider_id_from_string() {
        let id = ProviderId::from(String::from("openai"));
        assert_eq!(id.as_ref(), "openai");
        assert_eq!(id.as_str(), "openai");
    }

    #[test]
    fn test_provider_name_as_str() {
        let name = ProviderName::new("OpenAI");
        assert_eq!(name.as_str(), "OpenAI");
    }

    #[test]
    fn test_message_constructors() {
        let sys = Message::system("You are helpful");
        assert_eq!(sys.role, Role::System);
        assert_eq!(sys.content.as_text(), Some("You are helpful"));
        assert!(sys.name.is_none());
        assert!(sys.tool_call_id.is_none());

        let usr = Message::user("Hello");
        assert_eq!(usr.role, Role::User);

        let ast = Message::assistant("Hi there");
        assert_eq!(ast.role, Role::Assistant);

        let tool = Message::tool_result("call_123", "result data");
        assert_eq!(tool.role, Role::Tool);
        assert_eq!(tool.tool_call_id.as_deref(), Some("call_123"));
    }

    #[test]
    fn test_message_user_blocks() {
        let msg = Message::user_blocks(vec![
            ContentBlock::text("What's in this image?"),
            ContentBlock::image_url("https://example.com/photo.jpg"),
        ]);
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.content_blocks().len(), 2);
    }

    #[test]
    fn test_message_assistant_blocks() {
        let msg = Message::assistant_blocks(vec![
            ContentBlock::text("Here is the result"),
            ContentBlock::tool_use("call_1", "get_weather", serde_json::json!({"city": "SF"})),
        ]);
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content.content_blocks().len(), 2);
    }

    #[test]
    fn test_message_with_name() {
        let msg = Message::user("Hello").with_name("Alice");
        assert_eq!(msg.name.as_deref(), Some("Alice"));
    }

    #[test]
    fn test_message_with_tool_call_id() {
        let msg = Message::assistant("result").with_tool_call_id("call_123");
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_123"));
    }

    #[test]
    fn test_message_tool_result_error() {
        let msg = Message::tool_result_error("call_123", "API error");
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_123"));
        assert_eq!(msg.content.as_text(), Some("API error"));
    }

    #[test]
    fn test_stop_reason_as_str() {
        assert_eq!(StopReason::EndTurn.as_str(), "end_turn");
        assert_eq!(StopReason::ToolCall.as_str(), "tool_call");
        assert_eq!(StopReason::MaxTokens.as_str(), "max_tokens");
        assert_eq!(StopReason::StopSequence.as_str(), "stop_sequence");
        assert_eq!(StopReason::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn test_stop_reason_is_tool_call() {
        assert!(StopReason::ToolCall.is_tool_call());
        assert!(!StopReason::EndTurn.is_tool_call());
        assert!(!StopReason::MaxTokens.is_tool_call());
    }

    #[test]
    fn test_stop_reason_display() {
        assert_eq!(StopReason::EndTurn.to_string(), "end_turn");
        assert_eq!(StopReason::ToolCall.to_string(), "tool_call");
    }

    #[test]
    fn test_language_model_request_builder() {
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_temperature(0.7)
            .with_max_tokens(1024)
            .stream();
        assert_eq!(req.model.as_ref(), "gpt-4o");
        assert_eq!(req.temperature, Some(0.7));
        assert_eq!(req.max_tokens, Some(1024));
        assert!(req.stream);
    }

    #[test]
    fn test_language_model_request_with_top_p() {
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_top_p(0.9);
        assert_eq!(req.top_p, Some(0.9));
    }

    #[test]
    fn test_language_model_request_with_stop() {
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_stop(vec!["STOP".to_string(), "END".to_string()]);
        assert_eq!(req.stop, Some(vec!["STOP".to_string(), "END".to_string()]));
    }

    #[test]
    fn test_language_model_request_with_tools() {
        let tools = vec![
            crate::tool::ToolDefinition::new(
                "test_tool",
                "A test tool",
                serde_json::json!({"type": "object"}),
            )
            .unwrap(),
        ];
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_tools(tools);
        assert!(req.tools.is_some());
        assert_eq!(req.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_language_model_request_add_message() {
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .add_system("You are helpful")
            .add_assistant("Hello!")
            .add_user("How are you?");
        assert_eq!(req.messages.len(), 4);
        assert_eq!(req.messages[0].role, Role::User);
        assert_eq!(req.messages[1].role, Role::System);
        assert_eq!(req.messages[2].role, Role::Assistant);
        assert_eq!(req.messages[3].role, Role::User);
    }

    #[test]
    fn test_language_model_request_with_frequency_penalty() {
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_frequency_penalty(0.5);
        assert_eq!(req.frequency_penalty, Some(0.5));
    }

    #[test]
    fn test_language_model_request_with_presence_penalty() {
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_presence_penalty(0.3);
        assert_eq!(req.presence_penalty, Some(0.3));
    }

    #[test]
    fn test_language_model_request_with_thinking() {
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_thinking(crate::thinking::ThinkingConfig::with_budget(1000).unwrap());
        assert!(req.thinking.is_some());
        assert!(req.thinking.as_ref().unwrap().enabled);
    }

    #[test]
    fn test_language_model_request_validate_ok() {
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]);
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_language_model_request_validate_empty_model() {
        let req = LanguageModelRequest::new("", vec![Message::user("hi")]);
        let err = req.validate().unwrap_err();
        assert!(matches!(err, crate::error::LlmError::InvalidRequest { .. }));
    }

    #[test]
    fn test_language_model_request_validate_empty_messages() {
        let req = LanguageModelRequest::new("gpt-4o", vec![]);
        let err = req.validate().unwrap_err();
        assert!(matches!(err, crate::error::LlmError::InvalidRequest { .. }));
    }

    #[test]
    fn test_language_model_request_validate_invalid_temperature() {
        let req =
            LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_temperature(3.0);
        let err = req.validate().unwrap_err();
        assert!(matches!(err, crate::error::LlmError::InvalidRequest { .. }));
    }

    #[test]
    fn test_language_model_request_validate_invalid_top_p() {
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_top_p(1.5);
        let err = req.validate().unwrap_err();
        assert!(matches!(err, crate::error::LlmError::InvalidRequest { .. }));
    }

    #[test]
    fn test_token_usage_total() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            ..Default::default()
        };
        assert_eq!(usage.total_tokens(), 150);
    }

    #[test]
    fn test_token_usage_is_empty() {
        let empty = TokenUsage::default();
        assert!(empty.is_empty());

        let non_empty = TokenUsage {
            prompt_tokens: 1,
            ..Default::default()
        };
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_token_usage_merge() {
        let mut a = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            cache_read_tokens: 10,
            cache_write_tokens: 5,
        };
        let b = TokenUsage {
            prompt_tokens: 200,
            completion_tokens: 30,
            cache_read_tokens: 20,
            cache_write_tokens: 15,
        };
        a.merge(&b);
        assert_eq!(a.prompt_tokens, 300);
        assert_eq!(a.completion_tokens, 80);
        assert_eq!(a.cache_read_tokens, 30);
        assert_eq!(a.cache_write_tokens, 20);
    }

    #[test]
    fn test_role_display() {
        assert_eq!(Role::System.to_string(), "system");
        assert_eq!(Role::User.to_string(), "user");
        assert_eq!(Role::Assistant.to_string(), "assistant");
        assert_eq!(Role::Tool.to_string(), "tool");
    }

    #[test]
    fn test_provider_config_from_json() {
        let json = r#"{
            "id": "test",
            "name": "Test Provider",
            "api_url": "https://api.test.com/v1",
            "models": [{"id": "test-model", "display_name": "Test Model", "max_tokens": 4096, "capabilities": {"tools": false, "images": false, "streaming_tools": false, "parallel_tool_calls": false, "thinking": false, "max_tokens": 4096}}]
        }"#;
        let config = ProviderConfig::from_json(json).unwrap();
        assert_eq!(config.id.as_str(), "test");
        assert_eq!(config.api_url, "https://api.test.com/v1");
    }

    #[test]
    fn test_provider_config_validate_empty_api_url() {
        let json = r#"{
            "id": "test",
            "name": "Test",
            "api_url": "",
            "models": [{"id": "m", "display_name": "M", "max_tokens": 1, "capabilities": {"tools": false, "images": false, "streaming_tools": false, "parallel_tool_calls": false, "thinking": false, "max_tokens": 1}}]
        }"#;
        let err = ProviderConfig::from_json(json).unwrap_err();
        assert!(matches!(err, crate::error::LlmError::Config(_)));
    }

    #[test]
    fn test_provider_config_validate_empty_models() {
        let json = r#"{
            "id": "test",
            "name": "Test",
            "api_url": "https://api.test.com",
            "models": []
        }"#;
        let err = ProviderConfig::from_json(json).unwrap_err();
        assert!(matches!(err, crate::error::LlmError::Config(_)));
    }

    #[test]
    fn test_request_metadata_builder() {
        let meta = RequestMetadata::new()
            .with_request_id("req-123")
            .with_user_id("user-456")
            .with_session_id("sess-789")
            .with_tag("env", "production");
        assert_eq!(meta.request_id.as_deref(), Some("req-123"));
        assert_eq!(meta.user_id.as_deref(), Some("user-456"));
        assert_eq!(meta.session_id.as_deref(), Some("sess-789"));
        assert_eq!(meta.tags.get("env"), Some(&"production".to_string()));
    }

    #[test]
    fn test_request_metadata_default() {
        let meta = RequestMetadata::default();
        assert!(meta.request_id.is_none());
        assert!(meta.tags.is_empty());
    }

    #[test]
    fn test_language_model_request_with_timeout() {
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_timeout(Duration::from_secs(30));
        assert_eq!(req.timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_language_model_request_with_cancel_token() {
        let token = CancellationToken::new();
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_cancel_token(token.clone());
        assert!(req.cancel.is_some());
        assert!(!req.cancel.as_ref().unwrap().is_cancelled());
    }

    #[test]
    fn test_language_model_request_with_metadata() {
        let meta = RequestMetadata::new()
            .with_request_id("req-1")
            .with_tag("version", "2.0");
        let req =
            LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_metadata(meta);
        assert!(req.metadata.is_some());
        assert_eq!(
            req.metadata.as_ref().unwrap().request_id.as_deref(),
            Some("req-1")
        );
    }

    #[test]
    fn test_provider_config_validate_empty_model_id() {
        let json = r#"{
            "id": "test",
            "name": "Test",
            "api_url": "https://api.test.com",
            "models": [{"id": "", "display_name": "M", "max_tokens": 1, "capabilities": {"tools": false, "images": false, "streaming_tools": false, "parallel_tool_calls": false, "thinking": false, "max_tokens": 1}}]
        }"#;
        let err = ProviderConfig::from_json(json).unwrap_err();
        assert!(matches!(err, crate::error::LlmError::Config(_)));
    }

    #[test]
    fn test_provider_config_from_json_invalid_json() {
        let result = ProviderConfig::from_json("{invalid}");
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_config_from_json_file_not_found() {
        let path = std::path::Path::new("/nonexistent/path/config.json");
        let result = ProviderConfig::from_json_file(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_model_capabilities_default() {
        let caps = ModelCapabilities::default();
        assert!(!caps.tools);
        assert!(!caps.images);
        assert!(!caps.streaming_tools);
        assert!(!caps.parallel_tool_calls);
        assert!(!caps.thinking);
        assert_eq!(caps.max_tokens, 0);
    }

    #[test]
    fn test_rate_limit_config_serialize() {
        let config = RateLimitConfig {
            max_concurrent_requests: 10,
            requests_per_minute: Some(60),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RateLimitConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.max_concurrent_requests, 10);
        assert_eq!(deserialized.requests_per_minute, Some(60));
    }

    #[test]
    fn test_retry_config_serialize() {
        let config = RetryConfig {
            max_attempts: 3,
            base_delay_ms: 1000,
            retry_on_rate_limit: true,
            retry_on_server_error: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RetryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.max_attempts, 3);
        assert_eq!(deserialized.base_delay_ms, 1000);
    }

    #[test]
    fn test_model_config_serialize() {
        let config = ModelConfig {
            id: "gpt-4o".into(),
            display_name: "GPT-4o".into(),
            max_tokens: 128_000,
            max_output_tokens: Some(16384),
            capabilities: ModelCapabilities {
                tools: true,
                images: true,
                streaming_tools: true,
                parallel_tool_calls: true,
                thinking: false,
                max_tokens: 128_000,
            },
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ModelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id.as_ref(), "gpt-4o");
        assert_eq!(deserialized.max_tokens, 128_000);
    }

    #[test]
    fn test_provider_config_with_extra_headers() {
        let json = r#"{
            "id": "test",
            "name": "Test Provider",
            "api_url": "https://api.test.com/v1",
            "models": [{"id": "test-model", "display_name": "Test Model", "max_tokens": 4096, "capabilities": {"tools": false, "images": false, "streaming_tools": false, "parallel_tool_calls": false, "thinking": false, "max_tokens": 4096}}],
            "extra_headers": {"X-Custom": "value"}
        }"#;
        let config = ProviderConfig::from_json(json).unwrap();
        assert_eq!(
            config.extra_headers.get("X-Custom"),
            Some(&"value".to_string())
        );
    }

    #[test]
    fn test_provider_config_with_rate_limit() {
        let json = r#"{
            "id": "test",
            "name": "Test Provider",
            "api_url": "https://api.test.com/v1",
            "models": [{"id": "test-model", "display_name": "Test Model", "max_tokens": 4096, "capabilities": {"tools": false, "images": false, "streaming_tools": false, "parallel_tool_calls": false, "thinking": false, "max_tokens": 4096}}],
            "rate_limit": {"max_concurrent_requests": 5, "requests_per_minute": 30}
        }"#;
        let config = ProviderConfig::from_json(json).unwrap();
        assert!(config.rate_limit.is_some());
        assert_eq!(config.rate_limit.unwrap().max_concurrent_requests, 5);
    }

    #[test]
    fn test_provider_config_with_retry() {
        let json = r#"{
            "id": "test",
            "name": "Test Provider",
            "api_url": "https://api.test.com/v1",
            "models": [{"id": "test-model", "display_name": "Test Model", "max_tokens": 4096, "capabilities": {"tools": false, "images": false, "streaming_tools": false, "parallel_tool_calls": false, "thinking": false, "max_tokens": 4096}}],
            "retry": {"max_attempts": 5, "base_delay_ms": 2000, "retry_on_rate_limit": true, "retry_on_server_error": true}
        }"#;
        let config = ProviderConfig::from_json(json).unwrap();
        assert!(config.retry.is_some());
        assert_eq!(config.retry.unwrap().max_attempts, 5);
    }

    #[test]
    fn test_provider_config_with_max_request_body_bytes() {
        let json = r#"{
            "id": "test",
            "name": "Test Provider",
            "api_url": "https://api.test.com/v1",
            "models": [{"id": "test-model", "display_name": "Test Model", "max_tokens": 4096, "capabilities": {"tools": false, "images": false, "streaming_tools": false, "parallel_tool_calls": false, "thinking": false, "max_tokens": 4096}}],
            "max_request_body_bytes": 10485760
        }"#;
        let config = ProviderConfig::from_json(json).unwrap();
        assert_eq!(config.max_request_body_bytes, Some(10_485_760));
    }

    #[test]
    fn test_language_model_request_serde_roundtrip() {
        let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_temperature(0.7)
            .with_max_tokens(1024)
            .stream();
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: LanguageModelRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model.as_ref(), "gpt-4o");
        assert_eq!(deserialized.temperature, Some(0.7));
        assert!(deserialized.stream);
    }

    #[test]
    fn test_deductive_validate_boundary_temperature() {
        let req_ok =
            LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_temperature(0.0);
        assert!(req_ok.validate().is_ok());

        let req_ok2 =
            LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_temperature(2.0);
        assert!(req_ok2.validate().is_ok());

        let req_neg =
            LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_temperature(-0.1);
        assert!(req_neg.validate().is_err());

        let req_over =
            LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_temperature(2.1);
        assert!(req_over.validate().is_err());
    }

    #[test]
    fn test_deductive_validate_boundary_top_p() {
        let req_ok = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_top_p(0.0);
        assert!(req_ok.validate().is_ok());

        let req_ok2 =
            LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_top_p(1.0);
        assert!(req_ok2.validate().is_ok());

        let req_over =
            LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]).with_top_p(1.1);
        assert!(req_over.validate().is_err());
    }

    #[test]
    fn test_deductive_validate_boundary_penalties() {
        let req_freq_ok = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_frequency_penalty(-2.0);
        assert!(req_freq_ok.validate().is_ok());

        let req_freq_ok2 = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_frequency_penalty(2.0);
        assert!(req_freq_ok2.validate().is_ok());

        let req_freq_over = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_frequency_penalty(2.1);
        assert!(req_freq_over.validate().is_err());

        let req_pres_under = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_presence_penalty(-2.1);
        assert!(req_pres_under.validate().is_err());
    }

    #[test]
    fn test_deductive_token_usage_merge_commutative() {
        let a = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            cache_read_tokens: 10,
            cache_write_tokens: 5,
        };
        let b = TokenUsage {
            prompt_tokens: 200,
            completion_tokens: 30,
            cache_read_tokens: 20,
            cache_write_tokens: 15,
        };
        let mut ab = a.clone();
        ab.merge(&b);
        let mut ba = b.clone();
        ba.merge(&a);
        assert_eq!(ab.prompt_tokens, ba.prompt_tokens);
        assert_eq!(ab.completion_tokens, ba.completion_tokens);
        assert_eq!(ab.cache_read_tokens, ba.cache_read_tokens);
        assert_eq!(ab.cache_write_tokens, ba.cache_write_tokens);
    }

    #[test]
    fn test_deductive_token_usage_merge_identity() {
        let zero = TokenUsage::default();
        let mut a = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            cache_read_tokens: 10,
            cache_write_tokens: 5,
        };
        let original = a.clone();
        a.merge(&zero);
        assert_eq!(a.prompt_tokens, original.prompt_tokens);
        assert_eq!(a.completion_tokens, original.completion_tokens);
        assert_eq!(a.cache_read_tokens, original.cache_read_tokens);
        assert_eq!(a.cache_write_tokens, original.cache_write_tokens);
    }

    #[test]
    fn test_deductive_token_usage_total_tokens_formula() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            cache_read_tokens: 999,
            cache_write_tokens: 888,
        };
        assert_eq!(
            usage.total_tokens(),
            u64::from(usage.prompt_tokens) + u64::from(usage.completion_tokens),
            "total_tokens MUST equal prompt_tokens + completion_tokens"
        );
    }

    #[test]
    fn test_deductive_model_id_clone_zero_cost() {
        let id = ModelId::new("gpt-4o");
        let cloned = id.clone();
        assert_eq!(id, cloned);
        assert_eq!(id.as_str(), cloned.as_str());
    }

    #[test]
    fn test_miri_model_id_no_ub() {
        let id = ModelId::new("");
        assert!(id.is_empty());
        let id2 = ModelId::new("a".repeat(10000));
        assert!(!id2.is_empty());
    }
}
