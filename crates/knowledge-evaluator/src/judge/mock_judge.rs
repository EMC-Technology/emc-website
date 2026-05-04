//! Mock LLM 判决器
//!
//! 详见文档: §4.2 | 用例: UC-041

use async_trait::async_trait;

use super::{JudgeResult, LLMJudge};
use crate::error::Result;

/// Mock LLM Judge（用于测试）
///
/// 详见文档: §4.2 | 用例: UC-041
pub struct MockJudge;

impl MockJudge {
    /// 创建 Mock 判决器
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockJudge {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMJudge for MockJudge {
    /// 详见文档: §4.2 | 用例: UC-041 | 方法: M-057
    async fn judge(&self, _prompt: &str, _description: &str) -> Result<JudgeResult> {
        Ok(JudgeResult {
            content: "yes".to_string(),
            model: "mock".to_string(),
            latency_ms: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_judge_returns_yes() {
        let judge = MockJudge::new();
        let result = judge.judge("test prompt", "test").await.unwrap();
        assert_eq!(result.content, "yes");
        assert_eq!(result.model, "mock");
    }
}
