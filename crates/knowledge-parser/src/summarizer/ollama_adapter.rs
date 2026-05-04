//! Ollama 语言模型适配器
//!
//! 将 Ollama REST API 适配为 `LanguageModel` trait，
//! 用于社区摘要生成等需要 LLM 能力的场景。
//!
//! 启用方式：在 `Cargo.toml` 中添加 `knowledge-parser = { features = ["llm"] }`

use serde::{Deserialize, Serialize};

use super::engine::LanguageModel;

/// Ollama 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaLlmConfig {
    /// Ollama 服务地址
    pub base_url: String,
    /// 模型名称
    pub model: String,
    /// 请求超时秒数
    pub timeout_secs: u64,
}

impl Default for OllamaLlmConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            model: "qwen2.5:7b".to_string(),
            timeout_secs: 60,
        }
    }
}

/// Ollama `generate` 请求体
#[derive(Debug, Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
}

/// Ollama `generate` 响应体
#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    done: bool,
}

/// Ollama 语言模型适配器
///
/// 实现 `LanguageModel` trait，通过 Ollama REST API 调用本地 LLM。
pub struct OllamaLlm {
    config: OllamaLlmConfig,
    client: reqwest::Client,
}

impl OllamaLlm {
    /// 创建新的 Ollama 语言模型适配器
    #[must_use]
    pub fn new(config: OllamaLlmConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();
        Self { config, client }
    }
}

#[async_trait::async_trait]
impl LanguageModel for OllamaLlm {
    async fn generate(&self, prompt: &str) -> Result<String, String> {
        let request = OllamaGenerateRequest {
            model: self.config.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
        };

        let url = format!("{}/api/generate", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Ollama 请求失败: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Ollama 返回错误状态 {status}: {body}"));
        }

        let gen_response: OllamaGenerateResponse = response
            .json()
            .await
            .map_err(|e| format!("Ollama 响应解析失败: {e}"))?;

        Ok(gen_response.response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_llm_config_default() {
        let config = OllamaLlmConfig::default();
        assert_eq!(config.base_url, "http://localhost:11434");
        assert_eq!(config.model, "qwen2.5:7b");
    }
}
