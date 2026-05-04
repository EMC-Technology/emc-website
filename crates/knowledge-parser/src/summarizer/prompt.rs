//! 摘要提示词模板构建

use knowledge_core::model::{SemanticEntity, SemanticRelation};

/// 摘要上下文
pub(crate) struct SummarizationContext {
    pub entities: Vec<SemanticEntity>,
    pub relations: Vec<SemanticRelation>,
    pub total_entity_count: usize,
    pub total_relation_count: usize,
}

/// 摘要提示词模板
///
/// 详见文档: §4.1 | 用例: UC-050
pub struct SummarizationPromptTemplate;

impl SummarizationPromptTemplate {
    /// 生成系统提示词
    ///
    /// 详见文档: §4.1 | 用例: UC-050 | 方法: M-073
    #[must_use]
    pub fn system_prompt() -> String {
        "You are an expert knowledge graph summarizer. \
         Your task is to write a concise, informative summary of a topic community \
         detected in a knowledge graph. \
         The summary should capture the core theme and key relationships within the community. \
         Write in the same language as the entity names and descriptions. \
         Keep the summary to 3-5 sentences. \
         Do not include bullet points or numbered lists. \
         Write in flowing prose."
            .to_string()
    }

    /// 构建社区摘要用户提示词
    ///
    /// 详见文档: §4.2 | 用例: UC-051 | 方法: M-074
    pub(crate) fn community_summary(context: &SummarizationContext) -> String {
        let entity_list: String = context
            .entities
            .iter()
            .map(|e| {
                let desc = e.description.as_deref().unwrap_or("无描述");
                format!("- {} ({}): {}", e.name, e.entity_type, desc)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let relation_list: String = context
            .relations
            .iter()
            .take(30)
            .map(|r| {
                let end = crate::floor_char_boundary(&r.evidence, 80);
                let evidence_preview = &r.evidence[..end];
                format!(
                    "- {} --[{}]→ {} (evidence: {})",
                    r.source_entity, r.relation_type, r.target_entity, evidence_preview
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let truncation_note = if context.entities.len() < context.total_entity_count {
            format!(
                "\n（注：社区共 {} 个实体，以下仅列出 {} 个；共 {} 个关系，以下仅列出 {} 个）",
                context.total_entity_count,
                context.entities.len(),
                context.total_relation_count,
                context.relations.len()
            )
        } else {
            String::new()
        };

        format!(
            r"请为以下知识图谱社区生成摘要：

## 社区实体 ({} 个)
{}

## 社区关系 ({} 个)
{}
{}

请用 3-5 句话概括这个社区的核心主题和关键关联。",
            context.entities.len(),
            entity_list,
            context.relations.len(),
            relation_list,
            truncation_note,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knowledge_core::model::{EntityType, RelationType};

    fn make_rid(s: &str) -> knowledge_core::model::RecordIdType {
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

    #[test]
    fn test_system_prompt() {
        let prompt = SummarizationPromptTemplate::system_prompt();
        assert!(prompt.contains("knowledge graph summarizer"));
        assert!(prompt.contains("3-5 sentences"));
    }

    #[test]
    fn test_community_summary_basic() {
        let entity = SemanticEntity::new("Rust".to_string(), EntityType::Technology);
        let context = SummarizationContext {
            entities: vec![entity],
            relations: vec![],
            total_entity_count: 1,
            total_relation_count: 0,
        };
        let prompt = SummarizationPromptTemplate::community_summary(&context);
        assert!(prompt.contains("Rust"));
        assert!(prompt.contains("1 个"));
    }

    #[test]
    fn test_community_summary_with_relations() {
        let entity1 = SemanticEntity::new("Tokio".to_string(), EntityType::Technology);
        let entity2 = SemanticEntity::new("Rust".to_string(), EntityType::Technology);
        let relation = SemanticRelation {
            id: None,
            source_entity: make_rid("semantic_entity:tokio"),
            target_entity: make_rid("semantic_entity:rust"),
            relation_type: RelationType::DependsOn,
            evidence: "Tokio is built on top of Rust's async runtime".to_string(),
            confidence: 0.95,
            source_document: None,
            created_at: chrono::Utc::now(),
        };
        let context = SummarizationContext {
            entities: vec![entity1, entity2],
            relations: vec![relation],
            total_entity_count: 2,
            total_relation_count: 1,
        };
        let prompt = SummarizationPromptTemplate::community_summary(&context);
        assert!(prompt.contains("Tokio"));
        assert!(prompt.contains("depends_on"));
    }

    #[test]
    fn test_community_summary_truncation_note() {
        let entities: Vec<SemanticEntity> = (0..5)
            .map(|i| SemanticEntity::new(format!("Entity{i}"), EntityType::Concept))
            .collect();
        let context = SummarizationContext {
            entities,
            relations: vec![],
            total_entity_count: 100,
            total_relation_count: 200,
        };
        let prompt = SummarizationPromptTemplate::community_summary(&context);
        assert!(prompt.contains("注"));
        assert!(prompt.contains("100 个实体"));
    }
}
