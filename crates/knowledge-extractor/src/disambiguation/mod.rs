//! 实体消歧模块
//!
//! 详见文档: §3 | 用例: UC-010~UC-014

use knowledge_core::model::semantic::SemanticEntity;

use crate::config::{DisambiguationStrategy, ExtractorConfig};
use crate::extractors::LanguageModel;

/// 消歧引擎
///
/// 详见文档: §3 | 用例: UC-010~UC-014
pub struct Disambiguator {
    config: ExtractorConfig,
    llm: Option<std::sync::Arc<dyn LanguageModel>>,
}

impl Disambiguator {
    /// 创建消歧引擎
    ///
    /// 详见文档: §3.1 | 用例: UC-010 | 方法: M-018
    #[must_use]
    pub fn new(config: ExtractorConfig) -> Self {
        Self { config, llm: None }
    }

    /// 创建带 LLM 判断能力的消歧引擎
    ///
    /// 当使用 `LlmJudged` 或 `Hybrid` 策略时，需要传入 LLM 实例
    /// 以在模糊区间内调用 LLM 做最终裁决。
    #[must_use]
    pub fn with_llm(config: ExtractorConfig, llm: std::sync::Arc<dyn LanguageModel>) -> Self {
        Self { config, llm: Some(llm) }
    }

    /// 精确匹配消歧
    ///
    /// 当候选实体与已有实体的名称和类型完全一致时返回匹配项。
    ///
    /// 详见文档: §3.2 | 用例: UC-011 | 方法: M-019
    #[must_use]
    pub fn exact_match<'a>(
        &self,
        candidate: &SemanticEntity,
        existing: &'a [SemanticEntity],
    ) -> Option<&'a SemanticEntity> {
        existing
            .iter()
            .find(|e| e.name == candidate.name && e.entity_type == candidate.entity_type)
    }

    /// 编辑距离消歧
    ///
    /// 使用 Levenshtein 距离计算名称相似度，
    /// 返回超过阈值且相似度最高的匹配项。
    ///
    /// 详见文档: §3.3 | 用例: UC-012 | 方法: M-020
    #[must_use]
    pub fn edit_distance_match<'a>(
        &self,
        candidate: &SemanticEntity,
        existing: &'a [SemanticEntity],
    ) -> Option<(&'a SemanticEntity, f64)> {
        existing
            .iter()
            .map(|e| {
                let dist = strsim::levenshtein(&e.name, &candidate.name);
                let max_len = e.name.len().max(candidate.name.len());
                let similarity = if max_len == 0 {
                    1.0
                } else {
                    #[allow(clippy::cast_precision_loss)]
                    let dist_f64 = dist as f64;
                    #[allow(clippy::cast_precision_loss)]
                    let max_len_f64 = max_len as f64;
                    1.0 - (dist_f64 / max_len_f64)
                };
                (e, similarity)
            })
            .filter(|(_, sim)| *sim >= self.config.entity_similarity_threshold)
            .max_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.name.cmp(&b.0.name))
            })
    }

    /// 向量相似度消歧
    ///
    /// 使用余弦相似度计算实体嵌入向量的相似度，
    /// 返回超过阈值且相似度最高的匹配项。
    ///
    /// 详见文档: §3.4 | 用例: UC-013 | 方法: M-021
    #[must_use]
    pub fn embedding_similarity_match<'a>(
        &self,
        candidate: &SemanticEntity,
        existing: &'a [SemanticEntity],
    ) -> Option<(&'a SemanticEntity, f64)> {
        let candidate_emb = candidate.embedding.as_ref()?;

        existing
            .iter()
            .filter_map(|e| {
                let emb = e.embedding.as_ref()?;
                let similarity = cosine_similarity(candidate_emb, emb);
                Some((e, similarity))
            })
            .filter(|(_, sim)| *sim >= self.config.entity_similarity_threshold)
            .max_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.name.cmp(&b.0.name))
            })
    }

    /// LLM 判断消歧
    ///
    /// 当向量相似度处于模糊区间 `[lower_bound, upper_bound)` 时，
    /// 调用 LLM 判断候选实体与已有实体是否为同一实体。
    ///
    /// - 相似度 >= `upper_bound`：直接视为同一实体（无需 LLM）
    /// - 相似度 < `lower_bound`：直接视为不同实体（无需 LLM）
    /// - 相似度在 `[lower_bound, upper_bound)` 内：调用 LLM 裁决
    ///
    /// 详见文档: §3.4 | 用例: UC-013 补充
    pub async fn llm_judged_match<'a>(
        &self,
        candidate: &SemanticEntity,
        existing: &'a [SemanticEntity],
    ) -> Option<&'a SemanticEntity> {
        let candidate_emb = candidate.embedding.as_ref()?;
        let llm = self.llm.as_ref()?;

        let lower = self.config.disambiguation_llm_lower_bound;
        let upper = self.config.disambiguation_llm_upper_bound;

        let best_match = existing
            .iter()
            .filter_map(|e| {
                let emb = e.embedding.as_ref()?;
                let similarity = cosine_similarity(candidate_emb, emb);
                Some((e, similarity))
            })
            .filter(|(_, sim)| *sim >= lower)
            .max_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.name.cmp(&b.0.name))
            })?;

        let (_, similarity) = best_match;

        if similarity >= upper {
            return Some(best_match.0);
        }

        let prompt = Self::build_disambiguation_prompt(candidate, best_match.0);
        match llm.generate(&prompt).await {
            Ok(response) => {
                let normalized = response.trim().to_lowercase();
                let trimmed = normalized.trim().to_lowercase();
            if trimmed == "same" || trimmed == "yes" || trimmed == "同一" || trimmed.starts_with("same,") || trimmed.starts_with("yes,") {
                    Some(best_match.0)
                } else {
                    None
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "LLM 消歧判断失败，视为不同实体");
                None
            }
        }
    }

    /// 构建 LLM 消歧判断提示词
    fn build_disambiguation_prompt(candidate: &SemanticEntity, existing: &SemanticEntity) -> String {
        format!(
            r#"判断以下两个实体是否为同一实体：

实体A：名称="{}", 类型={}, 描述="{}"
实体B：名称="{}", 类型={}, 描述="{}"

请仅回答 "same" 或 "different"。"#
            ,
            candidate.name,
            candidate.entity_type,
            candidate.description.as_deref().unwrap_or(""),
            existing.name,
            existing.entity_type,
            existing.description.as_deref().unwrap_or(""),
        )
    }

    /// 综合消岐决策（同步版本，不含 LLM 判断）
    ///
    /// 按策略优先级尝试匹配：
    /// - `ExactMatch`：仅精确匹配
    /// - `EditDistance`：仅编辑距离
    /// - `EmbeddingSimilarity`：仅向量相似度
    /// - `LlmJudged`：仅向量相似度（LLM 判断需使用 `disambiguate_async`）
    /// - `Hybrid`：依次尝试精确→编辑距离→向量
    ///
    /// 详见文档: §3.5 | 用例: UC-014 | 方法: M-022
    #[must_use]
    pub fn disambiguate<'a>(
        &self,
        candidate: &SemanticEntity,
        existing: &'a [SemanticEntity],
    ) -> Option<&'a SemanticEntity> {
        match self.config.disambiguation_strategy {
            DisambiguationStrategy::ExactMatch => self.exact_match(candidate, existing),
            DisambiguationStrategy::EditDistance => {
                self.edit_distance_match(candidate, existing).map(|(e, _)| e)
            }
            DisambiguationStrategy::EmbeddingSimilarity | DisambiguationStrategy::LlmJudged => {
                self.embedding_similarity_match(candidate, existing).map(|(e, _)| e)
            }
            DisambiguationStrategy::Hybrid => self
                .exact_match(candidate, existing)
                .or_else(|| self.edit_distance_match(candidate, existing).map(|(e, _)| e))
                .or_else(|| {
                    self.embedding_similarity_match(candidate, existing).map(|(e, _)| e)
                }),
        }
    }

    /// 综合消歧决策（异步版本，支持 LLM 判断）
    ///
    /// 在 `Hybrid` 策略下，当精确/编辑距离/向量均未命中时，
    /// 会调用 LLM 对模糊区间内的候选进行裁决。
    pub async fn disambiguate_async<'a>(
        &self,
        candidate: &SemanticEntity,
        existing: &'a [SemanticEntity],
    ) -> Option<&'a SemanticEntity> {
        match self.config.disambiguation_strategy {
            DisambiguationStrategy::ExactMatch => self.exact_match(candidate, existing),
            DisambiguationStrategy::EditDistance => {
                self.edit_distance_match(candidate, existing).map(|(e, _)| e)
            }
            DisambiguationStrategy::EmbeddingSimilarity => {
                self.embedding_similarity_match(candidate, existing).map(|(e, _)| e)
            }
            DisambiguationStrategy::LlmJudged => {
                self.llm_judged_match(candidate, existing).await
            }
            DisambiguationStrategy::Hybrid => {
                if let Some(e) = self.exact_match(candidate, existing) {
                    return Some(e);
                }
                if let Some((e, _)) = self.edit_distance_match(candidate, existing) {
                    return Some(e);
                }
                if let Some((e, _)) = self.embedding_similarity_match(candidate, existing) {
                    return Some(e);
                }
                self.llm_judged_match(candidate, existing).await
            }
        }
    }

    /// 批量解析实体
    ///
    /// 对每个候选实体尝试消歧，若匹配到已有实体则使用已有实体，
    /// 否则保留候选实体本身。
    ///
    /// 详见文档: §3.5 | 用例: UC-014 | 方法: M-023
    #[must_use]
    pub fn resolve_entities(
        &self,
        candidates: &[SemanticEntity],
        existing: &[SemanticEntity],
    ) -> Vec<SemanticEntity> {
        let mut resolved = Vec::with_capacity(candidates.len());

        for candidate in candidates {
            match self.disambiguate(candidate, existing) {
                Some(existing_entity) => resolved.push(existing_entity.clone()),
                None => resolved.push(candidate.clone()),
            }
        }

        resolved
    }

    /// 批量解析实体（异步版本，支持 LLM 判断）
    ///
    /// 对每个候选实体尝试消歧，若匹配到已有实体则使用已有实体，
    /// 否则保留候选实体本身。
    pub async fn resolve_entities_async(
        &self,
        candidates: &[SemanticEntity],
        existing: &[SemanticEntity],
    ) -> Vec<SemanticEntity> {
        let mut resolved = Vec::with_capacity(candidates.len());

        for candidate in candidates {
            match self.disambiguate_async(candidate, existing).await {
                Some(existing_entity) => resolved.push(existing_entity.clone()),
                None => resolved.push(candidate.clone()),
            }
        }

        resolved
    }
}

/// 计算余弦相似度
///
/// 两个向量的点积除以各自模长的乘积。
/// 当任一向量为零向量时返回 0.0。
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| f64::from(*x) * f64::from(*y)).sum();
    let norm_a: f64 = a.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use knowledge_core::model::semantic::EntityType;

    fn make_entity(name: &str, entity_type: EntityType) -> SemanticEntity {
        SemanticEntity::new(name.to_string(), entity_type)
    }

    fn make_entity_with_embedding(name: &str, entity_type: EntityType, emb: Vec<f32>) -> SemanticEntity {
        let mut entity = SemanticEntity::new(name.to_string(), entity_type);
        entity.embedding = Some(emb);
        entity
    }

    fn test_config() -> ExtractorConfig {
        ExtractorConfig::default()
    }

    #[test]
    fn test_exact_match_finds_identical_entity() {
        let disambiguator = Disambiguator::new(test_config());
        let candidate = make_entity("Rust", EntityType::Technology);
        let existing = vec![make_entity("Rust", EntityType::Technology)];

        let result = disambiguator.exact_match(&candidate, &existing);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "Rust");
    }

    #[test]
    fn test_exact_match_returns_none_for_different_name() {
        let disambiguator = Disambiguator::new(test_config());
        let candidate = make_entity("Rust", EntityType::Technology);
        let existing = vec![make_entity("Python", EntityType::Technology)];

        let result = disambiguator.exact_match(&candidate, &existing);
        assert!(result.is_none());
    }

    #[test]
    fn test_exact_match_returns_none_for_different_type() {
        let disambiguator = Disambiguator::new(test_config());
        let candidate = make_entity("Rust", EntityType::Technology);
        let existing = vec![make_entity("Rust", EntityType::Concept)];

        let result = disambiguator.exact_match(&candidate, &existing);
        assert!(result.is_none());
    }

    #[test]
    fn test_edit_distance_match_finds_similar() {
        let mut config = test_config();
        config.entity_similarity_threshold = 0.5;
        let disambiguator = Disambiguator::new(config);

        let candidate = make_entity("Rust", EntityType::Technology);
        let existing = vec![make_entity("Rust!", EntityType::Technology)];

        let result = disambiguator.edit_distance_match(&candidate, &existing);
        assert!(result.is_some());
        let (_, similarity) = result.unwrap();
        assert!(similarity >= 0.5);
    }

    #[test]
    fn test_edit_distance_match_returns_none_below_threshold() {
        let mut config = test_config();
        config.entity_similarity_threshold = 0.99;
        let disambiguator = Disambiguator::new(config);

        let candidate = make_entity("Rust", EntityType::Technology);
        let existing = vec![make_entity("Completely Different", EntityType::Technology)];

        let result = disambiguator.edit_distance_match(&candidate, &existing);
        assert!(result.is_none());
    }

    #[test]
    fn test_embedding_similarity_match_finds_similar() {
        let mut config = test_config();
        config.entity_similarity_threshold = 0.9;
        let disambiguator = Disambiguator::new(config);

        let candidate = make_entity_with_embedding("Rust", EntityType::Technology, vec![1.0, 0.0, 0.0]);
        let existing = vec![make_entity_with_embedding("Rust-lang", EntityType::Technology, vec![0.99, 0.1, 0.0])];

        let result = disambiguator.embedding_similarity_match(&candidate, &existing);
        assert!(result.is_some());
    }

    #[test]
    fn test_embedding_similarity_match_returns_none_no_embedding() {
        let disambiguator = Disambiguator::new(test_config());

        let candidate = make_entity("Rust", EntityType::Technology);
        let existing = vec![make_entity("Rust", EntityType::Technology)];

        let result = disambiguator.embedding_similarity_match(&candidate, &existing);
        assert!(result.is_none());
    }

    #[test]
    fn test_disambiguate_hybrid_strategy() {
        let disambiguator = Disambiguator::new(test_config());

        let candidate = make_entity("Rust", EntityType::Technology);
        let existing = vec![make_entity("Rust", EntityType::Technology)];

        let result = disambiguator.disambiguate(&candidate, &existing);
        assert!(result.is_some());
    }

    #[test]
    fn test_disambiguate_hybrid_falls_through_strategies() {
        let mut config = test_config();
        config.disambiguation_strategy = DisambiguationStrategy::Hybrid;
        config.entity_similarity_threshold = 0.5;
        let disambiguator = Disambiguator::new(config);

        let candidate = make_entity("Rust", EntityType::Technology);
        let existing = vec![make_entity("Rust!", EntityType::Technology)];

        let result = disambiguator.disambiguate(&candidate, &existing);
        assert!(result.is_some());
    }

    #[test]
    fn test_resolve_entities_reuses_existing() {
        let disambiguator = Disambiguator::new(test_config());

        let candidates = vec![make_entity("Rust", EntityType::Technology)];
        let existing = vec![make_entity("Rust", EntityType::Technology)];

        let resolved = disambiguator.resolve_entities(&candidates, &existing);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "Rust");
    }

    #[test]
    fn test_resolve_entities_keeps_new_when_no_match() {
        let disambiguator = Disambiguator::new(test_config());

        let candidates = vec![make_entity("Tokio", EntityType::Technology)];
        let existing = vec![make_entity("Rust", EntityType::Technology)];

        let resolved = disambiguator.resolve_entities(&candidates, &existing);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "Tokio");
    }

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&a, &a);
        assert!((similarity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_empty_vectors() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
    }
}
