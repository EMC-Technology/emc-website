//! LLM 判决器模块
//!
//! 详见文档: §4 | 用例: UC-040~UC-041

pub mod mock_judge;
pub mod ullm_judge;

pub use mock_judge::MockJudge;
pub use ullm_judge::UllmJudge;

use crate::error::Result;

/// LLM 判决结果
#[derive(Debug, Clone)]
pub struct JudgeResult {
    /// 判决内容
    pub content: String,
    /// 使用的模型名称
    pub model: String,
    /// 延迟（毫秒）
    pub latency_ms: u128,
}

/// LLM 判决器 trait
///
/// 详见文档: §4 | 用例: UC-040
#[async_trait::async_trait]
pub trait LLMJudge: Send + Sync {
    /// 执行 LLM 判决
    ///
    /// # Errors
    ///
    /// 当 LLM 调用失败时返回错误
    async fn judge(&self, prompt: &str, description: &str) -> Result<JudgeResult>;
}
