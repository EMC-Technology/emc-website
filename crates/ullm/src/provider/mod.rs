/// 消息内容块定义
pub mod content_block;
/// 供应商扩展 trait
pub mod ext;
/// 供应商公共类型定义
pub mod types;

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::LlmError;

pub use types::*;

/// Anthropic Claude 供应商实现
pub mod anthropic;
/// `CodeGeeX` 供应商实现
pub mod codegeex;
/// `DeepSeek` 供应商实现
pub mod deepseek;
/// 供应商工厂函数
pub mod factory;
/// Google Gemini 供应商实现
pub mod gemini;
/// 智谱 GLM 供应商实现
pub mod glm;
/// Groq 供应商实现
pub mod groq;
/// Kimi（月之暗面）供应商实现
pub mod kimi;
/// Mistral 供应商实现
pub mod mistral;
/// Ollama 本地模型供应商实现
pub mod ollama;
/// `OpenAI` 供应商实现
pub mod openai;
/// `OpenAI` 兼容协议通用实现
pub mod openai_compatible;
/// `OpenRouter` 供应商实现
pub mod openrouter;
/// 通义千问（Qwen）供应商实现
pub mod qwen;
/// Web Scraper 供应商实现
pub mod web_scraper;
/// xAI 供应商实现
pub mod xai;

/// 语言模型抽象，定义单个 LLM 模型的能力与交互接口。
#[async_trait]
pub trait LanguageModel: Send + Sync + 'static {
    /// 返回模型唯一标识。
    fn id(&self) -> &types::ModelId;
    /// 返回模型显示名称。
    fn name(&self) -> &types::ModelName;
    /// 返回所属供应商标识。
    fn provider_id(&self) -> &types::ProviderId;
    /// 返回所属供应商显示名称。
    fn provider_name(&self) -> &types::ProviderName;
    /// 是否支持工具调用（Function Calling）。
    fn supports_tools(&self) -> bool;
    /// 是否支持流式工具调用。
    fn supports_streaming_tools(&self) -> bool;
    /// 是否支持图片输入。
    fn supports_images(&self) -> bool;
    /// 是否支持思维链（Thinking）输出。
    fn supports_thinking(&self) -> bool;
    /// 最大上下文 Token 数。
    fn max_token_count(&self) -> u64;
    /// 最大输出 Token 数。
    fn max_output_tokens(&self) -> Option<u64>;
    /// 估算请求的 Token 数量。
    fn count_tokens(
        &self,
        request: &types::LanguageModelRequest,
    ) -> crate::token_count::TokenCountFuture<'_>;
    /// 发起流式补全请求。
    fn stream_completion(
        &self,
        request: types::LanguageModelRequest,
    ) -> crate::stream::StreamFuture<'_>;
}

/// 语言模型供应商抽象，管理一组模型及其认证状态。
#[async_trait]
pub trait LanguageModelProvider: Send + Sync + 'static {
    /// 返回供应商标识。
    fn id(&self) -> &types::ProviderId;
    /// 返回供应商显示名称。
    fn name(&self) -> &types::ProviderName;
    /// 返回该供应商提供的所有模型。
    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>>;
    /// 是否已完成认证。
    fn is_authenticated(&self) -> bool;
    /// 执行认证流程。
    async fn authenticate(&self) -> Result<(), LlmError>;
    /// 重置凭证（使缓存失效）。
    async fn reset_credentials(&self) -> Result<(), LlmError>;
    /// 返回供应商配置。
    fn configuration(&self) -> &types::ProviderConfig;
}

/// 供应商类型枚举，标识所有支持的 LLM 供应商。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    /// `OpenAI`
    OpenAi,
    /// Anthropic
    Anthropic,
    /// Ollama
    Ollama,
    /// xAI
    XAI,
    /// `CodeGeeX`
    CodeGeeX,
    /// Web Scraper
    WebScraper,
    /// 智谱 GLM
    Glm,
    /// `DeepSeek`
    DeepSeek,
    /// Kimi（月之暗面）
    Kimi,
    /// 通义千问
    Qwen,
    /// `OpenRouter`
    OpenRouter,
    /// Google Gemini
    Gemini,
    /// Mistral
    Mistral,
    /// Groq
    Groq,
    /// `OpenAI` 兼容协议的自定义供应商
    OpenAiCompatible(String),
}

impl Serialize for ProviderKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ProviderKind::OpenAi => serializer.serialize_str("openai"),
            ProviderKind::Anthropic => serializer.serialize_str("anthropic"),
            ProviderKind::Ollama => serializer.serialize_str("ollama"),
            ProviderKind::XAI => serializer.serialize_str("xai"),
            ProviderKind::CodeGeeX => serializer.serialize_str("codegeex"),
            ProviderKind::WebScraper => serializer.serialize_str("web-scraper"),
            ProviderKind::Glm => serializer.serialize_str("glm"),
            ProviderKind::DeepSeek => serializer.serialize_str("deepseek"),
            ProviderKind::Kimi => serializer.serialize_str("kimi"),
            ProviderKind::Qwen => serializer.serialize_str("qwen"),
            ProviderKind::OpenRouter => serializer.serialize_str("openrouter"),
            ProviderKind::Gemini => serializer.serialize_str("gemini"),
            ProviderKind::Mistral => serializer.serialize_str("mistral"),
            ProviderKind::Groq => serializer.serialize_str("groq"),
            ProviderKind::OpenAiCompatible(name) => serializer.serialize_str(name),
        }
    }
}

impl<'de> Deserialize<'de> for ProviderKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "openai" => Ok(ProviderKind::OpenAi),
            "anthropic" => Ok(ProviderKind::Anthropic),
            "ollama" => Ok(ProviderKind::Ollama),
            "xai" => Ok(ProviderKind::XAI),
            "codegeex" => Ok(ProviderKind::CodeGeeX),
            "web-scraper" => Ok(ProviderKind::WebScraper),
            "glm" => Ok(ProviderKind::Glm),
            "deepseek" => Ok(ProviderKind::DeepSeek),
            "kimi" => Ok(ProviderKind::Kimi),
            "qwen" => Ok(ProviderKind::Qwen),
            "openrouter" => Ok(ProviderKind::OpenRouter),
            "gemini" => Ok(ProviderKind::Gemini),
            "mistral" => Ok(ProviderKind::Mistral),
            "groq" => Ok(ProviderKind::Groq),
            other => Ok(ProviderKind::OpenAiCompatible(other.to_string())),
        }
    }
}

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
            ProviderKind::OpenAiCompatible(name) => write!(f, "openai-compatible:{name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_kind_serde_roundtrip_openai() {
        let kind = ProviderKind::OpenAi;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"openai\"");
        let deserialized: ProviderKind = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, kind);
    }

    #[test]
    fn test_provider_kind_serde_roundtrip_glm() {
        let kind = ProviderKind::Glm;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"glm\"");
        let deserialized: ProviderKind = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, kind);
    }

    #[test]
    fn test_provider_kind_serde_roundtrip_deepseek() {
        let kind = ProviderKind::DeepSeek;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"deepseek\"");
        let deserialized: ProviderKind = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, kind);
    }

    #[test]
    fn test_provider_kind_serde_roundtrip_openai_compatible() {
        let kind = ProviderKind::OpenAiCompatible("custom-provider".to_string());
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"custom-provider\"");
        let deserialized: ProviderKind = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, kind);
    }

    #[test]
    fn test_provider_kind_display_consistent_with_serde() {
        let kinds = vec![
            ProviderKind::OpenAi,
            ProviderKind::Anthropic,
            ProviderKind::Ollama,
            ProviderKind::XAI,
            ProviderKind::CodeGeeX,
            ProviderKind::WebScraper,
            ProviderKind::Glm,
            ProviderKind::DeepSeek,
            ProviderKind::Kimi,
            ProviderKind::Qwen,
            ProviderKind::OpenRouter,
            ProviderKind::Gemini,
            ProviderKind::Mistral,
            ProviderKind::Groq,
        ];
        for kind in kinds {
            let display = format!("{kind}");
            let json = serde_json::to_string(&kind).unwrap();
            let expected_json = format!("\"{display}\"");
            assert_eq!(
                json, expected_json,
                "Display and serde mismatch for {kind:?}"
            );
        }
    }

    #[test]
    fn test_provider_kind_deserialize_from_string() {
        let kind: ProviderKind = serde_json::from_str("\"qwen\"").unwrap();
        assert_eq!(kind, ProviderKind::Qwen);

        let kind: ProviderKind = serde_json::from_str("\"kimi\"").unwrap();
        assert_eq!(kind, ProviderKind::Kimi);

        let kind: ProviderKind = serde_json::from_str("\"openrouter\"").unwrap();
        assert_eq!(kind, ProviderKind::OpenRouter);
    }

    #[test]
    fn test_provider_kind_unknown_falls_to_openai_compatible() {
        let kind: ProviderKind = serde_json::from_str("\"some-unknown-provider\"").unwrap();
        assert_eq!(
            kind,
            ProviderKind::OpenAiCompatible("some-unknown-provider".to_string())
        );
    }
}
