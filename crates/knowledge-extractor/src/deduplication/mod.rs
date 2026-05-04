//! 实体去重模块
//!
//! 详见文档: §4 | 用例: UC-015~UC-017

use knowledge_core::model::semantic::SemanticEntity;

use crate::config::ExtractorConfig;

/// 去重引擎
///
/// 详见文档: §4 | 用例: UC-015~UC-017
pub struct Deduplicator {
    config: ExtractorConfig,
}

impl Deduplicator {
    /// 创建去重引擎
    ///
    /// 详见文档: §4.1 | 用例: UC-015 | 方法: M-024
    #[must_use]
    pub fn new(config: ExtractorConfig) -> Self {
        Self { config }
    }

    /// 模糊去重
    ///
    /// 使用 Damerau-Levenshtein 归一化距离计算名称相似度，
    /// 超过阈值的实体被视为重复，仅保留首次出现的实例。
    ///
    /// 详见文档: §4.2 | 用例: UC-016 | 方法: M-025
    #[must_use]
    pub fn deduplicate(&self, entities: &[SemanticEntity]) -> Vec<SemanticEntity> {
        let mut unique = Vec::with_capacity(entities.len());

        for entity in entities {
            let is_duplicate = unique.iter().any(|existing: &SemanticEntity| {
                if entity.entity_type != existing.entity_type {
                    return false;
                }
                let similarity = strsim::normalized_damerau_levenshtein(&entity.name, &existing.name);
                similarity >= self.config.dedup_threshold
            });

            if !is_duplicate {
                unique.push(entity.clone());
            }
        }

        unique
    }

    /// 带统计的去重
    ///
    /// 返回去重后的实体列表和被移除的重复数量。
    ///
    /// 详见文档: §4.3 | 用例: UC-017 | 方法: M-026
    #[must_use]
    pub fn deduplicate_with_stats(&self, entities: &[SemanticEntity]) -> (Vec<SemanticEntity>, usize) {
        let original_count = entities.len();
        let unique = self.deduplicate(entities);
        let removed_count = original_count - unique.len();
        (unique, removed_count)
    }

    /// 批量去重
    ///
    /// 将多个批次的实体合并后进行全局去重。
    ///
    /// 详见文档: §4.3 | 用例: UC-017 | 方法: M-027
    #[must_use]
    pub fn deduplicate_batch(&self, batches: &[Vec<SemanticEntity>]) -> Vec<SemanticEntity> {
        let mut all_entities = Vec::new();
        for batch in batches {
            all_entities.extend(batch.iter().cloned());
        }
        self.deduplicate(&all_entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knowledge_core::model::semantic::EntityType;

    fn make_entity(name: &str) -> SemanticEntity {
        SemanticEntity::new(name.to_string(), EntityType::Technology)
    }

    fn test_config() -> ExtractorConfig {
        ExtractorConfig::default()
    }

    #[test]
    fn test_deduplicate_removes_exact_duplicates() {
        let deduplicator = Deduplicator::new(test_config());
        let entities = vec![make_entity("Rust"), make_entity("Rust")];

        let result = deduplicator.deduplicate(&entities);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Rust");
    }

    #[test]
    fn test_deduplicate_keeps_different_entities() {
        let deduplicator = Deduplicator::new(test_config());
        let entities = vec![make_entity("Rust"), make_entity("Python")];

        let result = deduplicator.deduplicate(&entities);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_deduplicate_removes_fuzzy_duplicates() {
        let mut config = test_config();
        config.dedup_threshold = 0.8;
        let deduplicator = Deduplicator::new(config);

        let entities = vec![make_entity("Rust Programming"), make_entity("Rust Programing")];

        let result = deduplicator.deduplicate(&entities);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_deduplicate_preserves_order() {
        let deduplicator = Deduplicator::new(test_config());
        let entities = vec![make_entity("A"), make_entity("B"), make_entity("A"), make_entity("C")];

        let result = deduplicator.deduplicate(&entities);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].name, "A");
        assert_eq!(result[1].name, "B");
        assert_eq!(result[2].name, "C");
    }

    #[test]
    fn test_deduplicate_with_stats_counts_removals() {
        let deduplicator = Deduplicator::new(test_config());
        let entities = vec![make_entity("Rust"), make_entity("Rust"), make_entity("Python")];

        let (unique, removed) = deduplicator.deduplicate_with_stats(&entities);
        assert_eq!(unique.len(), 2);
        assert_eq!(removed, 1);
    }

    #[test]
    fn test_deduplicate_with_stats_no_removals() {
        let deduplicator = Deduplicator::new(test_config());
        let entities = vec![make_entity("Rust"), make_entity("Python")];

        let (unique, removed) = deduplicator.deduplicate_with_stats(&entities);
        assert_eq!(unique.len(), 2);
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_deduplicate_batch_merges_and_deduplicates() {
        let deduplicator = Deduplicator::new(test_config());
        let batch1 = vec![make_entity("Rust"), make_entity("Python")];
        let batch2 = vec![make_entity("Rust"), make_entity("Tokio")];

        let result = deduplicator.deduplicate_batch(&[batch1, batch2]);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_deduplicate_batch_empty() {
        let deduplicator = Deduplicator::new(test_config());
        let result = deduplicator.deduplicate_batch(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_deduplicate_empty_input() {
        let deduplicator = Deduplicator::new(test_config());
        let result = deduplicator.deduplicate(&[]);
        assert!(result.is_empty());
    }
}
