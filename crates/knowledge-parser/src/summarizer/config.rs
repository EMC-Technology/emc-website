//! 社区摘要引擎配置

/// 社区摘要引擎配置
///
/// 详见文档: §2.1 | 用例: UC-046
#[derive(Debug, Clone)]
pub struct SummarizerConfig {
    pub max_entities_per_summary: usize,
    pub max_relations_per_summary: usize,
    pub max_concurrent_summaries: usize,
    pub llm_timeout_secs: u64,
    pub max_retries: u32,
    pub entity_change_threshold: f64,
    pub temperature: f64,
    pub max_tokens: usize,
}

impl Default for SummarizerConfig {
    /// 创建默认摘要配置
    ///
    /// 详见文档: §2.1 | 用例: UC-046 | 方法: M-067
    fn default() -> Self {
        Self {
            max_entities_per_summary: 50,
            max_relations_per_summary: 100,
            max_concurrent_summaries: 8,
            llm_timeout_secs: 60,
            max_retries: 2,
            entity_change_threshold: 0.2,
            temperature: 0.3,
            max_tokens: 1024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SummarizerConfig::default();
        assert_eq!(config.max_entities_per_summary, 50);
        assert_eq!(config.max_relations_per_summary, 100);
        assert_eq!(config.max_concurrent_summaries, 8);
        assert_eq!(config.llm_timeout_secs, 60);
        assert_eq!(config.max_retries, 2);
        assert!((config.entity_change_threshold - 0.2).abs() < f64::EPSILON);
        assert!((config.temperature - 0.3).abs() < f64::EPSILON);
        assert_eq!(config.max_tokens, 1024);
    }
}
