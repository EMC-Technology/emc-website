//! 基于 LLM API 的判决器
//!
//! 详见文档: §4.1 | 用例: UC-040

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing::warn;

use super::{JudgeResult, LLMJudge};
use crate::error::{self, Result};

/// LLM 语言模型抽象
///
/// 封装 LLM 文本生成能力，支持依赖注入和测试替身。
/// 未来集成 `ullm` crate 时可实现此 trait 适配 14+ 提供商。
#[async_trait]
pub trait LlmLanguageModel: Send + Sync {
    /// 根据提示词生成文本
    ///
    /// # Errors
    ///
    /// 当 LLM 调用失败时返回错误消息
    async fn generate(&self, prompt: &str) -> std::result::Result<String, String>;
}

/// 基于 LLM API 的判决器
///
/// 详见文档: §4.1 | 用例: UC-040
pub struct UllmJudge {
    llm: Arc<dyn LlmLanguageModel>,
    timeout_secs: u64,
    max_retries: u32,
}

impl UllmJudge {
    /// 创建 LLM 判决器
    #[must_use]
    pub fn new(llm: Arc<dyn LlmLanguageModel>) -> Self {
        Self {
            llm,
            timeout_secs: 30,
            max_retries: 2,
        }
    }

    /// 使用自定义超时和重试配置
    #[must_use]
    pub fn with_config(mut self, timeout_secs: u64, max_retries: u32) -> Self {
        self.timeout_secs = timeout_secs;
        self.max_retries = max_retries;
        self
    }
}

#[async_trait]
impl LLMJudge for UllmJudge {
    /// 详见文档: §4.1 | 用例: UC-040 | 方法: M-056
    async fn judge(&self, prompt: &str, _description: &str) -> Result<JudgeResult> {
        let system_prompt =
            "You are an expert evaluator. Be precise and concise. Respond in the requested format.";
        let full_prompt = format!("{system_prompt}\n\n{prompt}");

        let start = std::time::Instant::now();
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
            }

            let result = tokio::time::timeout(
                Duration::from_secs(self.timeout_secs),
                self.llm.generate(&full_prompt),
            )
            .await;

            match result {
                Ok(Ok(response)) => {
                    return Ok(JudgeResult {
                        content: response,
                        model: "llm".to_string(),
                        latency_ms: start.elapsed().as_millis(),
                    })
                }
                Ok(Err(e)) => {
                    warn!("LLM judgment failed (attempt {}): {e}", attempt + 1);
                    last_error = Some(error::llm_judgment_failed(e));
                }
                Err(_) => {
                    warn!("LLM judgment timed out (attempt {})", attempt + 1);
                    last_error = Some(error::timeout(format!(
                        "LLM call timed out after {}s",
                        self.timeout_secs
                    )));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            error::llm_judgment_failed("重试耗尽")
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLlm {
        response: String,
        should_fail: bool,
    }

    #[async_trait]
    impl LlmLanguageModel for MockLlm {
        async fn generate(&self, _prompt: &str) -> std::result::Result<String, String> {
            if self.should_fail {
                Err("mock error".to_string())
            } else {
                Ok(self.response.clone())
            }
        }
    }

    #[tokio::test]
    async fn test_ullm_judge_success() {
        let llm = Arc::new(MockLlm {
            response: "5".to_string(),
            should_fail: false,
        });
        let judge = UllmJudge::new(llm);
        let result = judge.judge("test prompt", "test").await.unwrap();
        assert_eq!(result.content, "5");
    }

    #[tokio::test]
    async fn test_ullm_judge_retry_on_failure() {
        let llm = Arc::new(MockLlm {
            response: String::new(),
            should_fail: true,
        });
        let judge = UllmJudge::new(llm).with_config(5, 1);
        let result = judge.judge("test prompt", "test").await;
        assert!(result.is_err());
    }
}
