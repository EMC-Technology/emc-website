//! Ollama LLM 客户端适配器
//!
//! 通过 Ollama REST API 与本地大语言模型通信，
//! 支持 `generate` 和 `chat` 两种模式，以及流式输出。
//!
//! 详见文档: §4 | ADR-005

use futures::Stream;
use serde::{Deserialize, Serialize};

use super::engine::{
    GenerateOptions, LLMClient, LLMMessage, LLMResponse, MessageRole, StreamChunk, TokenUsage,
};

/// Ollama 客户端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Ollama 服务地址（默认: `http://localhost:11434`）
    pub base_url: String,
    /// 模型名称（默认: `qwen2.5:7b`）
    pub model: String,
    /// 请求超时秒数（默认: 120）
    pub timeout_secs: u64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            model: "qwen2.5:7b".to_string(),
            timeout_secs: 120,
        }
    }
}

/// Ollama `chat` 请求体
#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

/// Chat 角色类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    /// 系统指令
    System,
    /// 用户消息
    User,
    /// 助手回复
    Assistant,
    /// 工具调用结果
    Tool,
}

impl std::fmt::Display for ChatRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::Tool => write!(f, "tool"),
        }
    }
}

/// Ollama 消息格式
#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: ChatRole,
    content: String,
}

/// Ollama 生成选项
#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f64,
    num_predict: usize,
    top_p: f64,
    frequency_penalty: f64,
    presence_penalty: f64,
}

/// Ollama `chat` 非流式响应
#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
    model: String,
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<usize>,
    #[serde(default)]
    eval_count: Option<usize>,
}

/// Ollama 流式响应块
#[derive(Debug, Deserialize)]
struct OllamaStreamChunk {
    message: OllamaMessage,
    model: String,
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<usize>,
    #[serde(default)]
    eval_count: Option<usize>,
}

/// Ollama LLM 客户端
///
/// 通过 `Ollama REST API` 与本地部署的大语言模型通信。
/// 支持同步和流式两种生成模式。
pub struct OllamaLLMClient {
    config: OllamaConfig,
    client: reqwest::Client,
}

impl OllamaLLMClient {
    /// 创建新的 Ollama 客户端
    #[must_use]
    pub fn new(config: OllamaConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();
        Self { config, client }
    }

    fn convert_messages(messages: &[LLMMessage]) -> Vec<OllamaMessage> {
        messages
            .iter()
            .map(|m| OllamaMessage {
                role: match m.role {
                    MessageRole::System => ChatRole::System,
                    MessageRole::User => ChatRole::User,
                    MessageRole::Assistant => ChatRole::Assistant,
                },
                content: m.content.clone(),
            })
            .collect()
    }

    fn convert_options(options: &GenerateOptions) -> OllamaOptions {
        OllamaOptions {
            temperature: options.temperature,
            num_predict: options.max_tokens,
            top_p: options.top_p,
            frequency_penalty: options.frequency_penalty,
            presence_penalty: options.presence_penalty,
        }
    }
}

#[async_trait::async_trait]
impl LLMClient for OllamaLLMClient {
    async fn generate(
        &self,
        messages: &[LLMMessage],
        options: &GenerateOptions,
    ) -> error_core::Result<LLMResponse> {
        let request = OllamaChatRequest {
            model: self.config.model.clone(),
            messages: Self::convert_messages(messages),
            stream: false,
            options: Some(Self::convert_options(options)),
        };

        let url = format!("{}/api/chat", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| error_core::helpers::llm_api_error(&e.to_string(), "ollama_chat"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(error_core::helpers::llm_api_error(
                &format!("Ollama 返回错误状态 {status}: {body}"),
                "ollama_chat",
            ));
        }

        let chat_response: OllamaChatResponse = response
            .json()
            .await
            .map_err(|e| error_core::helpers::llm_api_error(&e.to_string(), "ollama_parse"))?;

        let prompt_tokens = chat_response.prompt_eval_count.unwrap_or(0);
        let completion_tokens = chat_response.eval_count.unwrap_or(0);

        Ok(LLMResponse {
            content: chat_response.message.content,
            usage: TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
            model: chat_response.model,
            finish_reason: if chat_response.done {
                super::engine::FinishReason::Stop
            } else {
                super::engine::FinishReason::Length
            },
        })
    }

    async fn generate_stream(
        &self,
        messages: &[LLMMessage],
        options: &GenerateOptions,
    ) -> error_core::Result<Box<dyn Stream<Item = error_core::Result<StreamChunk>> + Send + Unpin>>
    {
        let request = OllamaChatRequest {
            model: self.config.model.clone(),
            messages: Self::convert_messages(messages),
            stream: true,
            options: Some(Self::convert_options(options)),
        };

        let url = format!("{}/api/chat", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error_core::helpers::llm_api_error(&e.to_string(), "ollama_chat_stream")
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(error_core::helpers::llm_api_error(
                &format!("Ollama 返回错误状态 {status}: {body}"),
                "ollama_chat_stream",
            ));
        }

        let stream = OllamaStreamMapper::new(response);
        Ok(Box::new(stream))
    }
}

/// Ollama `NDJSON` 流式响应到 `StreamChunk` 的适配器
///
/// 通过将 `reqwest::Response` 的 chunk 读取与缓冲区处理分离，
/// 避免自引用结构的借用冲突。
struct OllamaStreamMapper {
    inner: reqwest::Response,
    buffer: String,
}

impl OllamaStreamMapper {
    fn new(response: reqwest::Response) -> Self {
        Self {
            inner: response,
            buffer: String::new(),
        }
    }
}

impl futures::Stream for OllamaStreamMapper {
    type Item = error_core::Result<StreamChunk>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if let Some(newline_pos) = this.buffer.find('\n') {
                let line: String = this.buffer[..newline_pos].to_string();
                this.buffer = this.buffer[newline_pos + 1..].to_string();

                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                match serde_json::from_str::<OllamaStreamChunk>(trimmed) {
                    Ok(chunk) => {
                        let is_final = chunk.done;
                        let (usage, model) = if is_final {
                            let prompt_tokens = chunk.prompt_eval_count.unwrap_or(0);
                            let completion_tokens = chunk.eval_count.unwrap_or(0);
                            (
                                Some(TokenUsage {
                                    prompt_tokens,
                                    completion_tokens,
                                    total_tokens: prompt_tokens + completion_tokens,
                                }),
                                Some(chunk.model),
                            )
                        } else {
                            (None, None)
                        };
                        return std::task::Poll::Ready(Some(Ok(StreamChunk {
                            content: chunk.message.content,
                            is_final,
                            usage,
                            model,
                        })));
                    }
                    Err(_) => continue,
                }
            }

            let chunk_future = this.inner.chunk();
            let mut pinned = std::pin::pin!(chunk_future);

            match pinned.as_mut().poll(cx) {
                std::task::Poll::Ready(Ok(Some(bytes))) => {
                    let text = String::from_utf8_lossy(&bytes);
                    this.buffer.push_str(&text);
                }
                std::task::Poll::Ready(Ok(None)) => {
                    return std::task::Poll::Ready(Some(Ok(StreamChunk {
                        content: String::new(),
                        is_final: true,
                        usage: None,
                        model: None,
                    })));
                }
                std::task::Poll::Ready(Err(e)) => {
                    return std::task::Poll::Ready(Some(Err(error_core::helpers::llm_api_error(
                        &e.to_string(),
                        "ollama_stream_read",
                    ))));
                }
                std::task::Poll::Pending => {
                    return std::task::Poll::Pending;
                }
            }
        }
    }
}

impl Unpin for OllamaStreamMapper {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_config_default() {
        let config = OllamaConfig::default();
        assert_eq!(config.base_url, "http://localhost:11434");
        assert_eq!(config.model, "qwen2.5:7b");
        assert_eq!(config.timeout_secs, 120);
    }

    #[test]
    fn test_ollama_message_conversion() {
        let messages = vec![LLMMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
        }];
        let ollama_msgs = OllamaLLMClient::convert_messages(&messages);
        assert_eq!(ollama_msgs[0].role, ChatRole::User);
        assert_eq!(ollama_msgs[0].content, "Hello");
    }

    #[test]
    fn test_ollama_options_conversion() {
        let opts = GenerateOptions::default();
        let ollama_opts = OllamaLLMClient::convert_options(&opts);
        assert!((ollama_opts.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(ollama_opts.num_predict, 2048);
    }

    #[test]
    fn test_ollama_chat_request_serialization() {
        let request = OllamaChatRequest {
            model: "qwen2.5:7b".to_string(),
            messages: vec![OllamaMessage {
                role: ChatRole::User,
                content: "test".to_string(),
            }],
            stream: false,
            options: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("qwen2.5:7b"));
        assert!(json.contains("\"stream\":false"));
    }
}
