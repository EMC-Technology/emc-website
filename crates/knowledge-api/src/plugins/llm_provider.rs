use async_trait::async_trait;
#[cfg(feature = "ullm-adapter")]
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
#[cfg(feature = "ullm-adapter")]
use std::sync::Arc;

use crate::Result;

/// 统一 LLM Provider trait — 整合碎片化 LLM 抽象
///
/// `LLMProvider` 作为 `AgentBackend` 下的 LLM 子 trait，
/// 提供统一的 LLM 调用接口，同时保持 dyn-safe。
///
/// ullm 适配器通过 [`UllmProviderAdapter`]（feature `ullm-adapter`）提供实现。
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::llm_provider::LLMProvider;
/// use std::sync::Arc;
///
/// let provider: Arc<dyn LLMProvider> = Arc::new(MyProvider::new());
/// let response = provider.generate("你好").await?;
/// ```
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// 生成文本（最简接口）
    async fn generate(&self, prompt: &str) -> Result<String>;

    /// 带消息列表的生成
    async fn generate_with_messages(
        &self,
        messages: &[ProviderMessage],
        options: &ProviderOptions,
    ) -> Result<ProviderResponse>;

    /// 流式生成
    async fn generate_stream(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<String>> + Send>>>;

    /// 健康检查
    async fn health_check(&self) -> Result<bool>;
}

/// LLM 提供者消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMessage {
    /// 消息角色
    pub role: ProviderRole,
    /// 消息内容
    pub content: String,
}

/// LLM 提供者角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderRole {
    /// 系统指令
    System,
    /// 用户输入
    User,
    /// 助手回复
    Assistant,
}

/// LLM 提供者选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderOptions {
    /// 温度参数
    pub temperature: f64,
    /// 最大 token 数
    pub max_tokens: u32,
}

impl Default for ProviderOptions {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 2048,
        }
    }
}

/// LLM 提供者响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    /// 响应内容
    pub content: String,
    /// 使用的 token 数
    pub tokens_used: Option<u32>,
}

#[cfg(feature = "ullm-adapter")]
pub struct UllmProviderAdapter {
    api: Arc<ullm::LlmApi>,
    default_model: ullm::ModelId,
}

#[cfg(feature = "ullm-adapter")]
impl UllmProviderAdapter {
    pub fn new(api: Arc<ullm::LlmApi>, default_model: impl Into<ullm::ModelId>) -> Self {
        Self {
            api,
            default_model: default_model.into(),
        }
    }
}

#[cfg(feature = "ullm-adapter")]
#[async_trait]
impl LLMProvider for UllmProviderAdapter {
    async fn generate(&self, prompt: &str) -> Result<String> {
        let request = ullm::LanguageModelRequest::new(
            self.default_model.as_str(),
            vec![ullm::Message::user(prompt)],
        );

        let response = self
            .api
            .complete(self.default_model.as_str(), request)
            .await
            .map_err(|e| e.to_error_object())?;

        Ok(response.text().unwrap_or("").to_string())
    }

    async fn generate_with_messages(
        &self,
        messages: &[ProviderMessage],
        options: &ProviderOptions,
    ) -> Result<ProviderResponse> {
        let ullm_messages: Vec<ullm::Message> = messages
            .iter()
            .map(|m| match m.role {
                ProviderRole::System => ullm::Message::system(&m.content),
                ProviderRole::User => ullm::Message::user(&m.content),
                ProviderRole::Assistant => ullm::Message::assistant(&m.content),
            })
            .collect();

        let request = ullm::LanguageModelRequest::new(self.default_model.as_str(), ullm_messages)
            .with_temperature(options.temperature as f32)
            .with_max_tokens(options.max_tokens);

        let response = self
            .api
            .complete(self.default_model.as_str(), request)
            .await
            .map_err(|e| e.to_error_object())?;

        Ok(ProviderResponse {
            content: response.text().unwrap_or("").to_string(),
            tokens_used: Some(u32::try_from(response.usage.total_tokens()).unwrap_or(u32::MAX)),
        })
    }

    async fn generate_stream(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<String>> + Send>>> {
        let request = ullm::LanguageModelRequest::new(
            self.default_model.as_str(),
            vec![ullm::Message::user(prompt)],
        )
        .stream();

        let stream = self
            .api
            .stream_text(self.default_model.as_str(), request)
            .await
            .map_err(|e| e.to_error_object())?;

        let error_stream = stream.map(|result| result.map_err(|e| e.to_error_object().into()));

        Ok(Box::pin(error_stream))
    }

    async fn health_check(&self) -> Result<bool> {
        let errors = self.api.authenticate_all().await;
        Ok(errors.is_empty())
    }
}

#[cfg(feature = "ullm-adapter")]
pub struct UllmMetricsBridge;

#[cfg(feature = "ullm-adapter")]
impl ullm::observability::MetricsCollector for UllmMetricsBridge {
    fn record_request(&self, provider: &str, model: &str) {
        metrics::counter!("llm_requests_total", "provider" => provider.to_string(), "model" => model.to_string())
            .increment(1);
    }

    fn record_response(
        &self,
        provider: &str,
        model: &str,
        latency_ms: u64,
        _usage: &ullm::provider::types::TokenUsage,
    ) {
        metrics::counter!("llm_responses_total", "provider" => provider.to_string(), "model" => model.to_string())
            .increment(1);
        metrics::histogram!("llm_request_duration_ms", "provider" => provider.to_string(), "model" => model.to_string())
            .record(latency_ms as f64);
    }

    fn record_error(&self, provider: &str, model: &str, error_class: &str) {
        metrics::counter!("llm_errors_total", "provider" => provider.to_string(), "model" => model.to_string(), "error_class" => error_class.to_string())
            .increment(1);
    }

    fn record_stream_event(&self, provider: &str, model: &str, event_type: &str) {
        metrics::counter!("llm_stream_events_total", "provider" => provider.to_string(), "model" => model.to_string(), "event_type" => event_type.to_string())
            .increment(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_options_default() {
        let opts = ProviderOptions::default();
        assert!((opts.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(opts.max_tokens, 2048);
    }

    #[test]
    fn test_provider_message_serialization() {
        let msg = ProviderMessage {
            role: ProviderRole::User,
            content: "你好".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("User"));
        assert!(json.contains("你好"));
    }

    #[test]
    fn test_provider_role_conversion() {
        assert_eq!(ProviderRole::System as u8, 0);
        assert_eq!(ProviderRole::User as u8, 1);
        assert_eq!(ProviderRole::Assistant as u8, 2);
    }
}
