use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use crate::Result;

/// 统一 LLM Provider trait — 整合 4 个碎片化 LLM 抽象
///
/// # 碎片化现状
///
/// 项目中存在 4 个功能高度重叠的 LLM trait：
///
/// | Trait | Crate | 方法 | dyn-safe |
/// |-------|-------|------|----------|
/// | `LLMBackend` | knowledge-api | `complete` + `complete_stream` | 否（RPITIT） |
/// | `LLMClient` | knowledge-api | `generate` + `generate_stream` | 是 |
/// | `LlmLanguageModel` | knowledge-core | `generate(&str)` | 是 |
/// | `LanguageModel` | knowledge-parser/extractor | `generate(&str)` | 是 |
///
/// # 统一方案
///
/// `LLMProvider` 作为 `AgentBackend` 下的 LLM 子 trait，
/// 提供统一的 LLM 调用接口，同时保持 dyn-safe。
///
/// 现有 trait 通过 blanket implementation 或适配器自动获得 `LLMProvider` 实现：
/// - [`LLMClientAdapter`]：将 `LLMClient` 适配为 `LLMProvider`
/// - [`LlmLanguageModelAdapter`]：将 `LlmLanguageModel` 适配为 `LLMProvider`
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
    ///
    /// 接受提示词字符串，返回生成的文本。
    /// 这是最通用的 LLM 调用方式，所有 LLM 后端都应支持。
    async fn generate(&self, prompt: &str) -> Result<String>;

    /// 带消息列表的生成
    ///
    /// 接受结构化的消息列表和生成选项，
    /// 适用于需要多轮对话或精细控制的场景。
    async fn generate_with_messages(
        &self,
        messages: &[ProviderMessage],
        options: &ProviderOptions,
    ) -> Result<ProviderResponse>;

    /// 流式生成
    ///
    /// 返回逐 token 产出的异步流，
    /// 适用于需要实时展示生成过程的场景。
    async fn generate_stream(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<String>> + Send>>>;

    /// 健康检查
    async fn health_check(&self) -> Result<bool>;
}

/// 统一消息格式
///
/// 跨越 `LLMBackend::LLMMessage` 和 `LLMClient::LLMMessage` 的统一消息类型。
/// 两个现有 `LLMMessage` 类型字段高度重叠但定义不同，
/// `ProviderMessage` 作为插件层的统一格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMessage {
    /// 消息角色
    pub role: ProviderRole,
    /// 消息内容
    pub content: String,
}

/// 统一消息角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderRole {
    /// 系统指令
    System,
    /// 用户输入
    User,
    /// 模型回复
    Assistant,
}

/// 统一生成选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderOptions {
    /// 温度参数（0.0 - 2.0）
    pub temperature: f64,
    /// 最大生成 token 数
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

/// 统一生成响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    /// 生成的文本内容
    pub content: String,
    /// 使用的 token 数
    pub tokens_used: Option<u32>,
}

/// `LLMClient` → `LLMProvider` 的适配器
///
/// 将 `knowledge_api::rag::engine::LLMClient` 适配为 `LLMProvider`。
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::llm_provider::LLMClientAdapter;
///
/// let client = MyLLMClient::new();
/// let provider = LLMClientAdapter::new(client);
/// let text = provider.generate("你好").await?;
/// ```
pub struct LLMClientAdapter<T> {
    inner: T,
}

impl<T> LLMClientAdapter<T> {
    /// 创建适配器
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<T: crate::rag::engine::LLMClient + 'static> LLMProvider for LLMClientAdapter<T> {
    async fn generate(&self, prompt: &str) -> Result<String> {
        let messages = vec![crate::rag::engine::LLMMessage {
            role: crate::rag::engine::MessageRole::User,
            content: prompt.to_string(),
        }];
        let options = crate::rag::engine::GenerateOptions::default();
        let response = self.inner.generate(&messages, &options).await?;
        Ok(response.content)
    }

    async fn generate_with_messages(
        &self,
        messages: &[ProviderMessage],
        options: &ProviderOptions,
    ) -> Result<ProviderResponse> {
        let llm_messages: Vec<crate::rag::engine::LLMMessage> = messages
            .iter()
            .map(|m| crate::rag::engine::LLMMessage {
                role: match m.role {
                    ProviderRole::System => crate::rag::engine::MessageRole::System,
                    ProviderRole::User => crate::rag::engine::MessageRole::User,
                    ProviderRole::Assistant => crate::rag::engine::MessageRole::Assistant,
                },
                content: m.content.clone(),
            })
            .collect();
        let gen_options = crate::rag::engine::GenerateOptions {
            temperature: options.temperature,
            max_tokens: options.max_tokens as usize,
            ..Default::default()
        };
        let response = self.inner.generate(&llm_messages, &gen_options).await?;
        Ok(ProviderResponse {
            content: response.content,
            tokens_used: None,
        })
    }

    async fn generate_stream(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<String>> + Send>>> {
        let messages = vec![crate::rag::engine::LLMMessage {
            role: crate::rag::engine::MessageRole::User,
            content: prompt.to_string(),
        }];
        let options = crate::rag::engine::GenerateOptions::default();
        let stream = self.inner.generate_stream(&messages, &options).await?;
        let mapped = stream.map(|chunk| match chunk {
            Ok(c) => Ok(c.content),
            Err(e) => Err(e),
        });
        Ok(Box::pin(mapped))
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}

/// `LlmLanguageModel` → `LLMProvider` 的适配器
///
/// 将 `knowledge_core::reranker::llm_judger::LlmLanguageModel` 适配为 `LLMProvider`。
/// 此适配器仅支持 `generate` 方法（最简接口），
/// `generate_with_messages` 和 `generate_stream` 将返回不支持错误。
pub struct LlmLanguageModelAdapter<T> {
    inner: T,
}

impl<T> LlmLanguageModelAdapter<T> {
    /// 创建适配器
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<T: knowledge_core::reranker::llm_judger::LlmLanguageModel + 'static> LLMProvider
    for LlmLanguageModelAdapter<T>
{
    async fn generate(&self, prompt: &str) -> Result<String> {
        self.inner.generate(prompt).await.map_err(|e| {
            error_core::helpers::llm_api_error(
                &format!("LLM 生成失败: {e}"),
                "LlmLanguageModelAdapter::generate",
            )
        })
    }

    async fn generate_with_messages(
        &self,
        messages: &[ProviderMessage],
        _options: &ProviderOptions,
    ) -> Result<ProviderResponse> {
        let prompt = messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let content = self.generate(&prompt).await?;
        Ok(ProviderResponse {
            content,
            tokens_used: None,
        })
    }

    async fn generate_stream(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<String>> + Send>>> {
        let content = self.generate(prompt).await?;
        Ok(Box::pin(futures::stream::once(async move { Ok(content) })))
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
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
