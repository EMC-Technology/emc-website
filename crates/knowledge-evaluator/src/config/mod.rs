//! 评估配置模块
//!
//! 详见文档: §2.1 | 用例: UC-034

/// 评估配置
///
/// 详见文档: §2.1 | 用例: UC-034
#[derive(Debug, Clone)]
pub struct EvalConfig {
    /// 最大并发评估数
    pub max_concurrent_evals: usize,
    /// LLM 调用超时时间（秒）
    pub llm_timeout_secs: u64,
    /// LLM 最大重试次数
    pub llm_max_retries: u32,
}

impl Default for EvalConfig {
    /// 创建默认评估配置
    ///
    /// 详见文档: §2.1 | 用例: UC-034 | 方法: M-049
    fn default() -> Self {
        Self {
            max_concurrent_evals: 4,
            llm_timeout_secs: 30,
            llm_max_retries: 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EvalConfig::default();
        assert_eq!(config.max_concurrent_evals, 4);
        assert_eq!(config.llm_timeout_secs, 30);
        assert_eq!(config.llm_max_retries, 2);
    }
}
