//! 社区摘要生成核心引擎

use std::sync::Arc;

use knowledge_core::model::{CommunitySummary, RecordIdType, SemanticEntity, SemanticRelation};
use tracing::{debug, info, warn};

use error_core::helpers;

use super::config::SummarizerConfig;
use super::prompt::{SummarizationContext, SummarizationPromptTemplate};

/// 语言模型抽象（本地 trait，替代不存在的 ullm crate）
///
/// 后续集成 ullm 时替换为 `ullm::core::LanguageModel`
#[async_trait::async_trait]
pub trait LanguageModel: Send + Sync {
    /// 生成文本
    async fn generate(&self, prompt: &str) -> Result<String, String>;
}

/// 社区输入
///
/// 详见文档: §3.1 | 用例: UC-047
#[derive(Debug, Clone)]
pub struct CommunityInput {
    pub id: RecordIdType,
    pub entities: Vec<SemanticEntity>,
    pub relations: Vec<SemanticRelation>,
    pub cohesion_score: f64,
}

/// 社区摘要生成引擎
///
/// 详见文档: §3.1 | 用例: UC-047
pub struct CommunitySummarizer {
    llm: Arc<dyn LanguageModel>,
    config: SummarizerConfig,
}

impl CommunitySummarizer {
    /// 创建社区摘要生成器
    ///
    /// 详见文档: §3.1 | 用例: UC-047 | 方法: M-068
    pub fn new(llm: Arc<dyn LanguageModel>, config: SummarizerConfig) -> Self {
        Self { llm, config }
    }

    /// 生成单个社区的摘要
    ///
    /// 详见文档: §3.1 | 用例: UC-047 | 方法: M-069
    ///
    /// # Errors
    ///
    /// 当 LLM 调用失败或返回空摘要时返回错误
    pub async fn summarize_community(
        &self,
        community_id: &RecordIdType,
        entities: &[SemanticEntity],
        relations: &[SemanticRelation],
        cohesion_score: f64,
    ) -> crate::Result<CommunitySummary> {
        debug!(
            community_id = %community_id,
            entity_count = entities.len(),
            relation_count = relations.len(),
            "开始生成社区摘要"
        );

        let context = self.build_context(entities, relations);
        let system_prompt = SummarizationPromptTemplate::system_prompt();
        let user_prompt = SummarizationPromptTemplate::community_summary(&context);
        let summary_text = self.call_llm(&system_prompt, &user_prompt).await?;
        let key_concepts = Self::extract_key_concepts(entities, relations);

        let summary = CommunitySummary::new(
            community_id.clone(),
            summary_text,
            key_concepts,
            entities.len(),
            relations.len(),
            cohesion_score,
            Self::llm_model_name(),
        );

        info!(
            community_id = %community_id,
            summary_len = summary.summary_text.len(),
            "社区摘要生成完成"
        );
        Ok(summary)
    }

    fn build_context(
        &self,
        entities: &[SemanticEntity],
        relations: &[SemanticRelation],
    ) -> SummarizationContext {
        SummarizationContext {
            entities: entities
                .iter()
                .take(self.config.max_entities_per_summary)
                .cloned()
                .collect(),
            relations: relations
                .iter()
                .take(self.config.max_relations_per_summary)
                .cloned()
                .collect(),
            total_entity_count: entities.len(),
            total_relation_count: relations.len(),
        }
    }

    fn extract_key_concepts(
        entities: &[SemanticEntity],
        relations: &[SemanticRelation],
    ) -> Vec<String> {
        let mut concept_scores: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        for entity in entities {
            *concept_scores.entry(entity.name.clone()).or_insert(0) += 1;
        }
        for relation in relations {
            *concept_scores
                .entry(relation.relation_type.to_string())
                .or_insert(0) += 1;
        }
        let mut concepts: Vec<_> = concept_scores.into_iter().collect();
        concepts.sort_by_key(|b| std::cmp::Reverse(b.1));
        concepts
            .into_iter()
            .take(10)
            .map(|(name, _)| name)
            .collect()
    }

    /// 调用LLM生成摘要（带超时和重试）
    ///
    /// 详见文档: §3.1 | 用例: UC-047 | 方法: M-070
    async fn call_llm(&self, system_prompt: &str, user_prompt: &str) -> crate::Result<String> {
        let combined_prompt = format!("{system_prompt}\n\n{user_prompt}");

        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let backoff = 1000u64 * 2u64.pow(attempt - 1);
                tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                debug!(attempt, "重试 LLM 摘要调用");
            }

            let result = tokio::time::timeout(
                std::time::Duration::from_secs(self.config.llm_timeout_secs),
                self.llm.generate(&combined_prompt),
            )
            .await;

            match result {
                Ok(Ok(response)) => {
                    if response.trim().is_empty() {
                        last_error = Some(helpers::net_api_error("LLM 返回空摘要"));
                        continue;
                    }
                    return Ok(response.trim().to_string());
                }
                Ok(Err(e)) => {
                    last_error = Some(helpers::net_api_error(&format!("LLM 调用失败: {e}")));
                }
                Err(_) => {
                    last_error = Some(helpers::net_api_error("LLM 调用超时"));
                }
            }
        }
        Err(last_error.unwrap_or_else(|| helpers::net_api_error("LLM 摘要调用失败且重试耗尽")))
    }

    /// 批量生成社区摘要
    ///
    /// 详见文档: §3.2 | 用例: UC-048 | 方法: M-071
    ///
    /// # Errors
    ///
    /// 当内部 LLM 调用失败时，该社区摘要被跳过，不传播错误
    #[allow(clippy::missing_panics_doc)]
    pub async fn summarize_all(
        &self,
        communities: &[CommunityInput],
    ) -> crate::Result<Vec<CommunitySummary>> {
        info!(community_count = communities.len(), "开始批量生成社区摘要");

        let semaphore = Arc::new(tokio::sync::Semaphore::new(
            self.config.max_concurrent_summaries,
        ));
        let mut futures = Vec::new();

        for community in communities {
            let sem = semaphore.clone();
            let id = community.id.clone();
            let entities = community.entities.clone();
            let relations = community.relations.clone();
            let score = community.cohesion_score;
            let summarizer = Arc::new(self);

            let fut = async move {
                let _permit = sem
                    .acquire()
                    .await
                    .expect("信号量获取失败: Semaphore 不应被 close"); // SAFETY: 本方法内部创建的 Semaphore 不会被 close
                match summarizer
                    .summarize_community(&id, &entities, &relations, score)
                    .await
                {
                    Ok(summary) => Some(summary),
                    Err(e) => {
                        warn!(community_id = %id, error = ?e, "社区摘要生成失败，跳过");
                        None
                    }
                }
            };
            futures.push(fut);
        }

        let results = futures::future::join_all(futures).await;
        let summaries: Vec<CommunitySummary> = results.into_iter().flatten().collect();
        info!(
            successful = summaries.len(),
            total = communities.len(),
            "批量社区摘要生成完成"
        );
        Ok(summaries)
    }

    /// 仅更新变更超阈值的社区摘要
    ///
    /// 详见文档: §3.3 | 用例: UC-049 | 方法: M-072
    ///
    /// # Errors
    ///
    /// 当 LLM 调用失败时返回错误
    pub async fn update_changed(
        &self,
        changed_communities: &[CommunityInput],
        existing_summaries: &[CommunitySummary],
    ) -> crate::Result<Vec<CommunitySummary>> {
        let mut updated = Vec::new();

        for community in changed_communities {
            let existing = existing_summaries
                .iter()
                .find(|s| s.community_id == community.id);

            if let Some(existing_summary) = existing {
                let entity_change = Self::extract_entity_change(
                    community.entities.len(),
                    existing_summary.entity_count,
                );

                if entity_change > self.config.entity_change_threshold {
                    let new_summary = self
                        .summarize_community(
                            &community.id,
                            &community.entities,
                            &community.relations,
                            community.cohesion_score,
                        )
                        .await?;
                    updated.push(new_summary);
                }
            } else {
                let summary = self
                    .summarize_community(
                        &community.id,
                        &community.entities,
                        &community.relations,
                        community.cohesion_score,
                    )
                    .await?;
                updated.push(summary);
            }
        }
        Ok(updated)
    }

    fn extract_entity_change(current_count: usize, previous_count: usize) -> f64 {
        if previous_count == 0 {
            1.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            let current = current_count as f64;
            #[allow(clippy::cast_precision_loss)]
            let previous = previous_count as f64;
            (current - previous).abs() / previous
        }
    }

    fn llm_model_name() -> String {
        "ullm-llm".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knowledge_core::model::EntityType;

    fn make_rid(s: &str) -> RecordIdType {
        #[cfg(feature = "db")]
        {
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() == 2 {
                surrealdb::sql::Thing::from((parts[0], parts[1]))
            } else {
                surrealdb::sql::Thing::from((s, ""))
            }
        }
        #[cfg(not(feature = "db"))]
        {
            s.to_string()
        }
    }

    struct MockLlm {
        response: String,
    }

    #[async_trait::async_trait]
    impl LanguageModel for MockLlm {
        async fn generate(&self, _prompt: &str) -> Result<String, String> {
            Ok(self.response.clone())
        }
    }

    fn make_summarizer(response: &str) -> CommunitySummarizer {
        let llm = Arc::new(MockLlm {
            response: response.to_string(),
        });
        CommunitySummarizer::new(llm, SummarizerConfig::default())
    }

    #[tokio::test]
    async fn test_summarize_community() {
        let summarizer = make_summarizer("Rust 异步编程生态包含 Tokio、async-trait 等核心组件。");
        let community_id = make_rid("community:rust_async");
        let entities = vec![
            SemanticEntity::new("Tokio".to_string(), EntityType::Technology),
            SemanticEntity::new("async-trait".to_string(), EntityType::Technology),
        ];

        let result = summarizer
            .summarize_community(&community_id, &entities, &[], 0.85)
            .await;
        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(summary.summary_text.contains("Rust"));
        assert_eq!(summary.entity_count, 2);
        assert!((summary.cohesion_score - 0.85).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_summarize_community_empty_response() {
        let summarizer = make_summarizer("   ");
        let community_id = make_rid("community:empty");
        let entities = vec![SemanticEntity::new("Test".to_string(), EntityType::Concept)];

        let result = summarizer
            .summarize_community(&community_id, &entities, &[], 0.5)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_summarize_all() {
        let summarizer = make_summarizer("社区摘要内容");
        let communities = vec![
            CommunityInput {
                id: make_rid("community:c1"),
                entities: vec![SemanticEntity::new("A".to_string(), EntityType::Concept)],
                relations: vec![],
                cohesion_score: 0.8,
            },
            CommunityInput {
                id: make_rid("community:c2"),
                entities: vec![SemanticEntity::new("B".to_string(), EntityType::Concept)],
                relations: vec![],
                cohesion_score: 0.7,
            },
        ];

        let result = summarizer.summarize_all(&communities).await;
        assert!(result.is_ok());
        let summaries = result.unwrap();
        assert_eq!(summaries.len(), 2);
    }

    #[tokio::test]
    async fn test_update_changed_new_community() {
        let summarizer = make_summarizer("新社区摘要");
        let communities = vec![CommunityInput {
            id: make_rid("community:new"),
            entities: vec![SemanticEntity::new("X".to_string(), EntityType::Concept)],
            relations: vec![],
            cohesion_score: 0.9,
        }];

        let result = summarizer.update_changed(&communities, &[]).await;
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.len(), 1);
    }

    #[tokio::test]
    async fn test_update_changed_below_threshold() {
        let summarizer = make_summarizer("更新摘要");
        let community_id = make_rid("community:stable");
        let communities = vec![CommunityInput {
            id: community_id.clone(),
            entities: vec![SemanticEntity::new("A".to_string(), EntityType::Concept)],
            relations: vec![],
            cohesion_score: 0.8,
        }];
        let existing = vec![CommunitySummary::new(
            community_id,
            "旧摘要".to_string(),
            vec![],
            1,
            0,
            0.8,
            "test".to_string(),
        )];

        let result = summarizer.update_changed(&communities, &existing).await;
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.len(), 0);
    }

    #[tokio::test]
    async fn test_update_changed_above_threshold() {
        let summarizer = make_summarizer("更新摘要");
        let community_id = make_rid("community:changed");
        let communities = vec![CommunityInput {
            id: community_id.clone(),
            entities: (0..10)
                .map(|i| SemanticEntity::new(format!("E{i}"), EntityType::Concept))
                .collect(),
            relations: vec![],
            cohesion_score: 0.8,
        }];
        let existing = vec![CommunitySummary::new(
            community_id,
            "旧摘要".to_string(),
            vec![],
            2,
            0,
            0.8,
            "test".to_string(),
        )];

        let result = summarizer.update_changed(&communities, &existing).await;
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.len(), 1);
    }

    #[test]
    fn test_extract_key_concepts() {
        let entities = vec![
            SemanticEntity::new("Rust".to_string(), EntityType::Technology),
            SemanticEntity::new("Tokio".to_string(), EntityType::Technology),
            SemanticEntity::new("Rust".to_string(), EntityType::Technology),
        ];
        let concepts = CommunitySummarizer::extract_key_concepts(&entities, &[]);
        assert_eq!(concepts.len(), 2);
        assert_eq!(concepts[0], "Rust");
    }

    struct FailingLlm;

    #[async_trait::async_trait]
    impl LanguageModel for FailingLlm {
        async fn generate(&self, _prompt: &str) -> Result<String, String> {
            Err("LLM 调用失败".to_string())
        }
    }

    fn make_failing_summarizer() -> CommunitySummarizer {
        let llm = Arc::new(FailingLlm);
        let config = SummarizerConfig {
            max_retries: 0,
            ..SummarizerConfig::default()
        };
        CommunitySummarizer::new(llm, config)
    }

    #[tokio::test]
    async fn test_call_llm_failure_returns_error() {
        let summarizer = make_failing_summarizer();
        let community_id = make_rid("community:fail");
        let entities = vec![SemanticEntity::new("Test".to_string(), EntityType::Concept)];

        let result = summarizer
            .summarize_community(&community_id, &entities, &[], 0.5)
            .await;
        assert!(result.is_err(), "LLM 调用失败应返回错误");
    }

    #[tokio::test]
    async fn test_summarize_all_skips_failed_communities() {
        let summarizer = make_failing_summarizer();
        let communities = vec![CommunityInput {
            id: make_rid("community:skip"),
            entities: vec![SemanticEntity::new("X".to_string(), EntityType::Concept)],
            relations: vec![],
            cohesion_score: 0.5,
        }];

        let result = summarizer.summarize_all(&communities).await;
        assert!(result.is_ok());
        let summaries = result.unwrap();
        assert_eq!(summaries.len(), 0, "失败的社区应被跳过");
    }

    #[test]
    fn test_extract_entity_change_zero_previous() {
        let change = CommunitySummarizer::extract_entity_change(5, 0);
        assert!(
            (change - 1.0).abs() < f64::EPSILON,
            "previous_count 为 0 时变化率应为 1.0"
        );
    }

    #[test]
    fn test_extract_entity_change_nonzero_previous() {
        let change = CommunitySummarizer::extract_entity_change(15, 10);
        let expected = (15.0_f64 - 10.0_f64).abs() / 10.0_f64;
        assert!((change - expected).abs() < f64::EPSILON, "变化率计算应正确");
    }

    #[test]
    fn test_extract_key_concepts_with_relations() {
        let entities = vec![SemanticEntity::new(
            "Rust".to_string(),
            EntityType::Technology,
        )];
        let relations = vec![SemanticRelation::new(
            make_rid("semantic_entity:a"),
            make_rid("semantic_entity:b"),
            knowledge_core::model::RelationType::DependsOn,
            "evidence".to_string(),
        )];
        let concepts = CommunitySummarizer::extract_key_concepts(&entities, &relations);
        assert!(concepts.contains(&"Rust".to_string()), "应包含实体名");
        assert!(
            concepts.contains(&"depends_on".to_string()),
            "应包含关系类型"
        );
    }

    #[test]
    fn test_extract_key_concepts_deduplication_and_sort() {
        let entities: Vec<SemanticEntity> = (0..5)
            .map(|_| SemanticEntity::new("Repeated".to_string(), EntityType::Concept))
            .chain((0..3).map(|i| SemanticEntity::new(format!("Unique{i}"), EntityType::Concept)))
            .collect();
        let concepts = CommunitySummarizer::extract_key_concepts(&entities, &[]);
        assert_eq!(concepts[0], "Repeated", "出现最多的概念应排在首位");
        assert_eq!(concepts.len(), 4, "应去重后保留 4 个概念");
    }

    #[tokio::test]
    async fn test_summarize_community_with_relations() {
        let summarizer = make_summarizer("社区包含多个实体和关系。");
        let community_id = make_rid("community:with_relations");
        let entities = vec![
            SemanticEntity::new("A".to_string(), EntityType::Concept),
            SemanticEntity::new("B".to_string(), EntityType::Concept),
        ];
        let relations = vec![SemanticRelation::new(
            make_rid("semantic_entity:a"),
            make_rid("semantic_entity:b"),
            knowledge_core::model::RelationType::RelatedTo,
            "A and B are related".to_string(),
        )];

        let result = summarizer
            .summarize_community(&community_id, &entities, &relations, 0.9)
            .await;
        assert!(result.is_ok());
        let summary = result.unwrap();
        assert_eq!(summary.relation_count, 1);
    }

    #[test]
    fn test_llm_model_name() {
        let name = CommunitySummarizer::llm_model_name();
        assert_eq!(name, "ullm-llm", "模型名称应为 ullm-llm");
    }
}
