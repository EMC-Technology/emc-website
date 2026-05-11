#[cfg(feature = "ullm-adapter")]
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing::warn;

use super::{JudgeResult, LLMJudge};
use crate::error::{self, Result};

#[async_trait]
trait LlmGenerate: Send + Sync {
    async fn generate(&self, prompt: &str) -> std::result::Result<String, String>;
}

#[cfg(feature = "ullm-adapter")]
struct UllmGenerate {
    api: Arc<ullm::LlmApi>,
    model_id: ullm::ModelId,
    timeout: Duration,
}

#[cfg(feature = "ullm-adapter")]
impl UllmGenerate {
    fn new(api: Arc<ullm::LlmApi>, model_id: impl Into<ullm::ModelId>, timeout: Duration) -> Self {
        Self {
            api,
            model_id: model_id.into(),
            timeout,
        }
    }
}

#[cfg(feature = "ullm-adapter")]
#[async_trait]
impl LlmGenerate for UllmGenerate {
    async fn generate(&self, prompt: &str) -> std::result::Result<String, String> {
        let request = ullm::LanguageModelRequest::new(
            self.model_id.as_str(),
            vec![ullm::Message::user(prompt)],
        )
        .with_timeout(self.timeout);

        self.api
            .complete(self.model_id.as_str(), request)
            .await
            .map(|r| r.text().unwrap_or_default().to_string())
            .map_err(|e| e.to_string())
    }
}

/// 基于 ULLM 的 LLM 评判器，使用大语言模型对评估样本进行自动评分。
pub struct UllmJudge {
    llm: Box<dyn LlmGenerate>,
    timeout_secs: u64,
    max_retries: u32,
    #[cfg(feature = "ullm-adapter")]
    retry_policy: Option<ullm::retry::RetryPolicy>,
    #[cfg(feature = "ullm-adapter")]
    cancellation_token: Option<ullm::cancel::CancellationToken>,
}

#[cfg(feature = "ullm-adapter")]
impl UllmJudge {
    /// 创建新的 ULLM 评判器实例。
    pub fn new(api: Arc<ullm::LlmApi>, model_id: impl Into<ullm::ModelId>) -> Self {
        Self {
            llm: Box::new(UllmGenerate::new(api, model_id, Duration::from_secs(30))),
            timeout_secs: 30,
            max_retries: 2,
            retry_policy: None,
            cancellation_token: None,
        }
    }

    /// 设置请求超时时间（秒）。
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// 设置最大重试次数。
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// 设置重试策略，同时自动调整最大重试次数。
    pub fn with_retry_policy(mut self, retry_policy: ullm::retry::RetryPolicy) -> Self {
        self.max_retries = retry_policy.max_attempts.saturating_sub(1) as u32;
        self.retry_policy = Some(retry_policy);
        self
    }

    /// 设置取消令牌，用于中断长时间运行的评判请求。
    pub fn with_cancellation_token(self, token: ullm::cancel::CancellationToken) -> Self {
        Self {
            cancellation_token: Some(token),
            ..self
        }
    }
}

impl UllmJudge {
    fn effective_max_attempts(&self) -> u32 {
        #[cfg(feature = "ullm-adapter")]
        {
            self.retry_policy
                .as_ref()
                .map(|p| p.max_attempts)
                .unwrap_or(self.max_retries + 1)
        }
        #[cfg(not(feature = "ullm-adapter"))]
        {
            self.max_retries + 1
        }
    }

    #[cfg_attr(not(feature = "ullm-adapter"), allow(clippy::unused_self))]
    fn compute_delay(&self, attempt: u32) -> Duration {
        #[cfg(feature = "ullm-adapter")]
        if let Some(ref policy) = self.retry_policy {
            return ullm::retry::backoff(policy.base_delay, attempt).min(policy.max_delay);
        }
        Duration::from_millis(500 * 2u64.pow(attempt))
    }

    #[cfg(feature = "ullm-adapter")]
    fn is_cancelled(&self) -> bool {
        self.cancellation_token
            .as_ref()
            .is_some_and(|t| t.is_cancelled())
    }
}

#[async_trait]
impl LLMJudge for UllmJudge {
    async fn judge(&self, prompt: &str, _description: &str) -> Result<JudgeResult> {
        let system_prompt =
            "You are an expert evaluator. Be precise and concise. Respond in the requested format.";
        let full_prompt = format!("{system_prompt}\n\n{prompt}");

        let start = std::time::Instant::now();
        let mut last_error = None;

        for attempt in 0..self.effective_max_attempts() {
            #[cfg(feature = "ullm-adapter")]
            if self.is_cancelled() {
                return Err(error::timeout("LLM call cancelled".to_string()));
            }

            if attempt > 0 {
                tokio::time::sleep(self.compute_delay(attempt - 1)).await;
            }

            let generate_future = self.llm.generate(&full_prompt);

            let result = {
                #[cfg(feature = "ullm-adapter")]
                if let Some(ref token) = self.cancellation_token {
                    tokio::select! {
                        r = tokio::time::timeout(
                            Duration::from_secs(self.timeout_secs),
                            generate_future,
                        ) => r,
                        _ = token.cancelled() => {
                            warn!("LLM judgment cancelled (attempt {})", attempt + 1);
                            last_error = Some(error::timeout("LLM call cancelled".to_string()));
                            continue;
                        }
                    }
                } else {
                    tokio::time::timeout(Duration::from_secs(self.timeout_secs), generate_future)
                        .await
                }

                #[cfg(not(feature = "ullm-adapter"))]
                tokio::time::timeout(Duration::from_secs(self.timeout_secs), generate_future).await
            };

            match result {
                Ok(Ok(response)) => {
                    return Ok(JudgeResult {
                        content: response,
                        model: "llm".to_string(),
                        latency_ms: start.elapsed().as_millis(),
                    });
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

        Err(last_error.unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct MockLlm {
        response: String,
        should_fail: bool,
    }

    #[async_trait]
    impl LlmGenerate for MockLlm {
        async fn generate(&self, _prompt: &str) -> std::result::Result<String, String> {
            if self.should_fail {
                Err("mock error".to_string())
            } else {
                Ok(self.response.clone())
            }
        }
    }

    struct CountingMockLlm {
        fail_until: AtomicU32,
        response: String,
    }

    #[async_trait]
    impl LlmGenerate for CountingMockLlm {
        async fn generate(&self, _prompt: &str) -> std::result::Result<String, String> {
            let count = self.fail_until.fetch_sub(1, Ordering::SeqCst);
            if count > 0 {
                Err("transient error".to_string())
            } else {
                Ok(self.response.clone())
            }
        }
    }

    fn mock_judge(response: &str, should_fail: bool) -> UllmJudge {
        UllmJudge {
            llm: Box::new(MockLlm {
                response: response.to_string(),
                should_fail,
            }),
            timeout_secs: 5,
            max_retries: 1,
            #[cfg(feature = "ullm-adapter")]
            retry_policy: None,
            #[cfg(feature = "ullm-adapter")]
            cancellation_token: None,
        }
    }

    fn counting_judge(fail_until: u32, response: &str) -> UllmJudge {
        UllmJudge {
            llm: Box::new(CountingMockLlm {
                fail_until: AtomicU32::new(fail_until),
                response: response.to_string(),
            }),
            timeout_secs: 5,
            max_retries: 3,
            #[cfg(feature = "ullm-adapter")]
            retry_policy: None,
            #[cfg(feature = "ullm-adapter")]
            cancellation_token: None,
        }
    }

    #[tokio::test]
    async fn test_ullm_judge_success() {
        let judge = mock_judge("5", false);
        let result = judge.judge("test prompt", "test").await.unwrap();
        assert_eq!(result.content, "5");
    }

    #[tokio::test]
    async fn test_ullm_judge_retry_on_failure() {
        let judge = mock_judge("", true);
        let result = judge.judge("test prompt", "test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ullm_judge_eventual_success_after_retries() {
        let judge = counting_judge(2, "42");
        let result = judge.judge("test prompt", "test").await.unwrap();
        assert_eq!(result.content, "42");
    }

    #[cfg(feature = "ullm-adapter")]
    #[tokio::test]
    async fn test_ullm_judge_with_cancellation_token_immediate_cancel() {
        let token = ullm::cancel::CancellationToken::new();
        token.cancel();
        let mut judge = mock_judge("never", false);
        judge.cancellation_token = Some(token);
        let result = judge.judge("test prompt", "test").await;
        assert!(result.is_err());
    }
}
