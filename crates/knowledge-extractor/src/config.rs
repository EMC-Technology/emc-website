//! 抽取引擎配置
//!
//! 详见文档: §6 | 用例: UC-005

use crate::error::{self, Result};

/// 消歧策略枚举
///
/// 详见文档: §6 | 用例: UC-005
#[derive(Debug, Clone, PartialEq)]
pub enum DisambiguationStrategy {
    /// 精确字符串匹配
    ExactMatch,
    /// 编辑距离模糊匹配
    EditDistance,
    /// 向量嵌入相似度匹配
    EmbeddingSimilarity,
    /// LLM 判断消歧：当向量相似度处于模糊区间时，调用 LLM 做最终裁决
    LlmJudged,
    /// 混合策略：按优先级依次尝试精确→编辑距离→向量→LLM判断
    Hybrid,
}

/// 抽取引擎配置
///
/// 详见文档: §6 | 用例: UC-005
#[derive(Debug, Clone)]
pub struct ExtractorConfig {
    /// 每个块的最大实体抽取数量
    pub max_entities_per_block: usize,
    /// 每个块的最大关系抽取数量
    pub max_relations_per_block: usize,
    /// LLM 调用超时时间（秒）
    pub llm_timeout_secs: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 实体相似度阈值（用于消歧）
    pub entity_similarity_threshold: f64,
    /// 消歧策略
    pub disambiguation_strategy: DisambiguationStrategy,
    /// 去重相似度阈值
    pub dedup_threshold: f64,
    /// LLM 消歧模糊区间下界（低于此值视为不同实体，无需 LLM 判断）
    pub disambiguation_llm_lower_bound: f64,
    /// LLM 消歧模糊区间上界（高于此值视为同一实体，无需 LLM 判断）
    pub disambiguation_llm_upper_bound: f64,
}

impl Default for ExtractorConfig {
    /// 创建默认抽取配置
    ///
    /// 详见文档: §6 | 用例: UC-005 | 方法: M-007
    fn default() -> Self {
        Self {
            max_entities_per_block: 100,
            max_relations_per_block: 200,
            llm_timeout_secs: 30,
            max_retries: 3,
            entity_similarity_threshold: 0.85,
            disambiguation_strategy: DisambiguationStrategy::Hybrid,
            dedup_threshold: 0.9,
            disambiguation_llm_lower_bound: 0.75,
            disambiguation_llm_upper_bound: 0.95,
        }
    }
}

impl ExtractorConfig {
    /// 验证配置有效性
    ///
    /// 详见文档: §6 | 用例: UC-005 | 方法: M-008
    ///
    /// # Errors
    ///
    /// 当配置参数不在有效范围内时返回配置错误
    pub fn validate(&self) -> Result<()> {
        if self.max_entities_per_block == 0 {
            return Err(error::invalid_config("max_entities_per_block must be > 0"));
        }
        if self.max_relations_per_block == 0 {
            return Err(error::invalid_config("max_relations_per_block must be > 0"));
        }
        if self.llm_timeout_secs == 0 {
            return Err(error::invalid_config("llm_timeout_secs must be > 0"));
        }
        if self.max_retries == 0 {
            return Err(error::invalid_config("max_retries must be > 0"));
        }
        if !(0.0..=1.0).contains(&self.entity_similarity_threshold) {
            return Err(error::invalid_config(
                "entity_similarity_threshold must be in [0.0, 1.0]",
            ));
        }
        if !(0.0..=1.0).contains(&self.dedup_threshold) {
            return Err(error::invalid_config(
                "dedup_threshold must be in [0.0, 1.0]",
            ));
        }
        if !(0.0..=1.0).contains(&self.disambiguation_llm_lower_bound) {
            return Err(error::invalid_config(
                "disambiguation_llm_lower_bound must be in [0.0, 1.0]",
            ));
        }
        if !(0.0..=1.0).contains(&self.disambiguation_llm_upper_bound) {
            return Err(error::invalid_config(
                "disambiguation_llm_upper_bound must be in [0.0, 1.0]",
            ));
        }
        if self.disambiguation_llm_lower_bound >= self.disambiguation_llm_upper_bound {
            return Err(error::invalid_config(
                "disambiguation_llm_lower_bound must be < disambiguation_llm_upper_bound",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = ExtractorConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_zero_max_entities() {
        let config = ExtractorConfig {
            max_entities_per_block: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_zero_max_relations() {
        let config = ExtractorConfig {
            max_relations_per_block: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_zero_timeout() {
        let config = ExtractorConfig {
            llm_timeout_secs: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_zero_retries() {
        let config = ExtractorConfig {
            max_retries: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_similarity_threshold_out_of_range() {
        let config = ExtractorConfig {
            entity_similarity_threshold: 1.5,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_dedup_threshold_out_of_range() {
        let config = ExtractorConfig {
            dedup_threshold: -0.1,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_boundary_values() {
        let config = ExtractorConfig {
            entity_similarity_threshold: 0.0,
            dedup_threshold: 1.0,
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_llm_lower_bound_out_of_range() {
        let config = ExtractorConfig {
            disambiguation_llm_lower_bound: -0.1,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_llm_upper_bound_out_of_range() {
        let config = ExtractorConfig {
            disambiguation_llm_upper_bound: 1.5,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_llm_lower_bound_ge_upper_bound() {
        let config = ExtractorConfig {
            disambiguation_llm_lower_bound: 0.95,
            disambiguation_llm_upper_bound: 0.95,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_llm_lower_bound_gt_upper_bound() {
        let config = ExtractorConfig {
            disambiguation_llm_lower_bound: 0.96,
            disambiguation_llm_upper_bound: 0.95,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
