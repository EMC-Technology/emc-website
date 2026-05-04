//! LLM 驱动的实体/关系抽取器
//!
//! 详见文档: §2 | 用例: UC-007, UC-008, UC-009

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use knowledge_core::model::semantic::{
    EntityType, RelationType, SemanticEntity,
};

use crate::config::ExtractorConfig;
use crate::error::{self, Result};

/// 语言模型抽象接口
///
/// 封装 LLM 文本生成能力，支持依赖注入和测试替身。
/// 未来集成 `ullm` crate 时，可实现此 trait 适配 14+ 提供商。
#[async_trait]
pub trait LanguageModel: Send + Sync {
    /// 根据提示词生成文本
    ///
    /// # Errors
    ///
    /// 当 LLM 调用失败时返回错误消息
    async fn generate(&self, prompt: &str) -> std::result::Result<String, String>;
}

/// LLM 响应中的原始实体数据
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntityRaw {
    name: String,
    #[serde(rename = "type")]
    entity_type: String,
    #[serde(default)]
    description: Option<String>,
}

/// LLM 响应中的原始关系数据
///
/// 使用实体名称而非 ID，因为抽取时尚未持久化实体。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelationRaw {
    source: String,
    target: String,
    #[serde(rename = "type")]
    relation_type: String,
}

/// 抽取的关系中间表示
///
/// 新抽取的关系尚无实体 ID，使用名称引用源和目标实体。
/// 在管道的持久化阶段，名称将被解析为数据库 `RecordId`。
#[derive(Debug, Clone)]
pub struct ExtractedRelation {
    /// 源实体名称
    pub source_name: String,
    /// 目标实体名称
    pub target_name: String,
    /// 关系类型
    pub relation_type: RelationType,
    /// 证据文本
    pub evidence: String,
    /// 置信度
    pub confidence: f64,
}

/// LLM 驱动的实体抽取器
///
/// 详见文档: §2.1 | 用例: UC-007
pub struct LlmExtractor {
    llm: Arc<dyn LanguageModel>,
    pub(crate) config: ExtractorConfig,
}

impl LlmExtractor {
    /// 创建新的 LLM 抽取器
    ///
    /// 详见文档: §2.1 | 用例: UC-007 | 方法: M-010
    ///
    /// # Errors
    ///
    /// 当配置验证失败时返回配置错误
    pub fn new(llm: Arc<dyn LanguageModel>, config: ExtractorConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { llm, config })
    }

    /// 获取最大重试次数
    #[must_use]
    pub fn max_retries(&self) -> u32 {
        self.config.max_retries
    }

    /// 从文本中抽取实体
    ///
    /// 详见文档: §2.1 | 用例: UC-007 | 方法: M-011
    ///
    /// # Errors
    ///
    /// 当 LLM 调用失败或响应解析失败时返回错误
    pub async fn extract_entities(&self, text: &str) -> Result<Vec<SemanticEntity>> {
        let prompt = Self::build_entity_prompt(text);
        let response = self.call_llm(&prompt).await?;
        let entities = Self::parse_entity_response(&response)?;

        let limited: Vec<_> = entities
            .into_iter()
            .take(self.config.max_entities_per_block)
            .collect();

        Ok(limited)
    }

    /// 构建实体抽取提示词
    ///
    /// 详见文档: §2.1 | 用例: UC-007 | 方法: M-012
    fn build_entity_prompt(text: &str) -> String {
        format!(
            r#"从以下文本中抽取语义实体，输出为 JSON 数组：

文本：{text}

输出格式：[{{"name": "...", "type": "person|organization|concept|technology|location|event|document|other", "description": "..."}}]

要求：
1. 仅输出 JSON 数组，不要包含其他文本
2. type 必须是上述枚举值之一
3. description 为可选字段"#
        )
    }

    /// 从文本中抽取关系
    ///
    /// 详见文档: §2.2 | 用例: UC-008 | 方法: M-013
    ///
    /// # Errors
    ///
    /// 当 LLM 调用失败或响应解析失败时返回错误
    pub async fn extract_relations(
        &self,
        text: &str,
        entities: &[SemanticEntity],
    ) -> Result<Vec<ExtractedRelation>> {
        let prompt = Self::build_relation_prompt(text, entities);
        let response = self.call_llm(&prompt).await?;
        let relations = Self::parse_relation_response(&response, entities)?;

        let limited: Vec<_> = relations
            .into_iter()
            .take(self.config.max_relations_per_block)
            .collect();

        Ok(limited)
    }

    /// 构建关系抽取提示词
    ///
    /// 详见文档: §2.2 | 用例: UC-008 | 方法: M-014
    fn build_relation_prompt(text: &str, entities: &[SemanticEntity]) -> String {
        let entity_list: String = entities
            .iter()
            .map(|e| format!("- {} ({})", e.name, e.entity_type))
            .collect::<Vec<_>>()
            .join("\n");

        let constraints: String = RelationType::constraint_description()
            .iter()
            .map(|s| format!("  - {s}"))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"从以下文本中抽取实体间的关系：

实体列表：
{entity_list}

文本：{text}

输出格式：[{{"source": "实体名", "target": "实体名", "type": "is_a|part_of|located_in|uses|related_to|created_by|implements|depends_on|conflicts_with|similar_to", "evidence": "支撑此关系的原文片段"}}]

要求：
1. 仅输出 JSON 数组，不要包含其他文本
2. source 和 target 必须是实体列表中的名称
3. type 必须是上述枚举值之一
4. 关系类型必须符合以下实体类型组合约束：
{constraints}"#
        )
    }

    /// 调用 LLM API（带超时和重试）
    ///
    /// 详见文档: §2.3 | 用例: UC-009 | 方法: M-015
    ///
    /// # Errors
    ///
    /// 当超时或所有重试均失败时返回错误
    async fn call_llm(&self, prompt: &str) -> Result<String> {
        let timeout_duration = Duration::from_secs(self.config.llm_timeout_secs);

        let generate_result = tokio::time::timeout(timeout_duration, async {
            let mut last_err: Option<String> = None;
            for attempt in 1..=self.config.max_retries {
                match self.llm.generate(prompt).await {
                    Ok(response) => return Ok(response),
                    Err(e) if attempt < self.config.max_retries => {
                        tracing::warn!("LLM call failed (attempt {attempt}): {e}");
                        last_err = Some(e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    Err(e) => {
                        last_err = Some(e);
                        break;
                    }
                }
            }
            Err(last_err.unwrap_or_else(|| "unknown error".to_string()))
        })
        .await
        .map_err(|_| error::timeout_error())?;

        generate_result.map_err(error::llm_error)
    }

    /// 解析 LLM 实体响应
    ///
    /// 详见文档: §2.3 | 用例: UC-009 | 方法: M-016
    fn parse_entity_response(response: &str) -> Result<Vec<SemanticEntity>> {
        let json_str = extract_json_array(response);
        let raw: Vec<EntityRaw> = serde_json::from_str(json_str).map_err(|e| error::serialization_error(&e))?;

        let entities: Vec<_> = raw
            .into_iter()
            .map(|r| {
                let entity_type = parse_entity_type(&r.entity_type);
                let mut entity = SemanticEntity::new(r.name, entity_type);
                entity.description = r.description;
                entity
            })
            .collect();

        Ok(entities)
    }

    /// 解析 LLM 关系响应
    ///
    /// 详见文档: §2.3 | 用例: UC-009 | 方法: M-017
    fn parse_relation_response(
        response: &str,
        entities: &[SemanticEntity],
    ) -> Result<Vec<ExtractedRelation>> {
        let json_str = extract_json_array(response);
        let raw: Vec<RelationRaw> = serde_json::from_str(json_str).map_err(|e| error::serialization_error(&e))?;

        let entity_type_map: std::collections::HashMap<&str, &EntityType> = entities
            .iter()
            .map(|e| (e.name.as_str(), &e.entity_type))
            .collect();

        let relations: Vec<_> = raw
            .into_iter()
            .filter_map(|r| {
                let relation_type = parse_relation_type(&r.relation_type)?;

                let source_type = entity_type_map.get(r.source.as_str())?;
                let target_type = entity_type_map.get(r.target.as_str())?;

                if !relation_type.is_valid_type_combination(source_type, target_type) {
                    tracing::warn!(
                        "Skipping relation with invalid type combination: {} ({}) --{}--> {} ({})",
                        r.source, source_type, relation_type, r.target, target_type
                    );
                    return None;
                }

                Some(ExtractedRelation {
                    source_name: r.source,
                    target_name: r.target,
                    relation_type,
                    evidence: String::new(),
                    confidence: 1.0,
                })
            })
            .collect();

        Ok(relations)
    }
}

/// 从 LLM 响应中提取 JSON 数组部分
///
/// LLM 可能在 JSON 前后附加 markdown 代码块标记或其他文本，
/// 此函数提取第一个 `[` 到最后一个 `]` 之间的内容。
fn extract_json_array(response: &str) -> &str {
    let start = response.find('[').unwrap_or_else(|| {
            tracing::warn!("LLM 响应中未找到 JSON 数组起始标记 '['");
            0
        });
    let end = response.rfind(']').map_or_else(|| response.len(), |i| i + 1);
    &response[start..end]
}

/// 将字符串解析为 `EntityType`
fn parse_entity_type(s: &str) -> EntityType {
    match s {
        "person" => EntityType::Person,
        "organization" => EntityType::Organization,
        "concept" => EntityType::Concept,
        "technology" => EntityType::Technology,
        "location" => EntityType::Location,
        "event" => EntityType::Event,
        "document" => EntityType::Document,
        _ => EntityType::Other,
    }
}

/// 将字符串解析为 `RelationType`
fn parse_relation_type(s: &str) -> Option<RelationType> {
    match s {
        "is_a" => Some(RelationType::IsA),
        "part_of" => Some(RelationType::PartOf),
        "located_in" => Some(RelationType::LocatedIn),
        "uses" => Some(RelationType::Uses),
        "related_to" => Some(RelationType::RelatedTo),
        "created_by" => Some(RelationType::CreatedBy),
        "implements" => Some(RelationType::Implements),
        "depends_on" => Some(RelationType::DependsOn),
        "conflicts_with" => Some(RelationType::ConflictsWith),
        "similar_to" => Some(RelationType::SimilarTo),
        _ => None,
    }
}

/// 用于测试的 Mock `LanguageModel`
#[cfg(test)]
struct MockLanguageModel {
    response: String,
    should_fail: bool,
}

#[cfg(test)]
#[async_trait]
impl LanguageModel for MockLanguageModel {
    async fn generate(&self, _prompt: &str) -> std::result::Result<String, String> {
        if self.should_fail {
            Err("mock error".into())
        } else {
            Ok(self.response.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_llm(response: &str) -> Arc<dyn LanguageModel> {
        Arc::new(MockLanguageModel {
            response: response.to_string(),
            should_fail: false,
        })
    }

    fn mock_llm_failing() -> Arc<dyn LanguageModel> {
        Arc::new(MockLanguageModel {
            response: String::new(),
            should_fail: true,
        })
    }

    #[tokio::test]
    async fn test_extract_entities_parses_valid_response() {
        let response = r#"[{"name": "Rust", "type": "technology", "description": "A systems language"}]"#;
        let llm = mock_llm(response);
        let extractor = LlmExtractor::new(llm, ExtractorConfig::default()).unwrap();

        let entities = extractor.extract_entities("Rust is a systems language").await.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "Rust");
        assert_eq!(entities[0].entity_type, EntityType::Technology);
        assert_eq!(entities[0].description.as_deref(), Some("A systems language"));
    }

    #[tokio::test]
    async fn test_extract_entities_handles_markdown_wrapped_response() {
        let response = "```json\n[{\"name\": \"Tokio\", \"type\": \"technology\"}]\n```";
        let llm = mock_llm(response);
        let extractor = LlmExtractor::new(llm, ExtractorConfig::default()).unwrap();

        let entities = extractor.extract_entities("Tokio runtime").await.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "Tokio");
    }

    #[tokio::test]
    async fn test_extract_entities_limits_count() {
        let config = ExtractorConfig { max_entities_per_block: 1, ..Default::default() };
        let response = r#"[{"name": "A", "type": "person"}, {"name": "B", "type": "person"}]"#;
        let llm = mock_llm(response);
        let extractor = LlmExtractor::new(llm, config).unwrap();

        let entities = extractor.extract_entities("text").await.unwrap();
        assert_eq!(entities.len(), 1);
    }

    #[tokio::test]
    async fn test_extract_entities_unknown_type_becomes_other() {
        let response = r#"[{"name": "X", "type": "unknown_type"}]"#;
        let llm = mock_llm(response);
        let extractor = LlmExtractor::new(llm, ExtractorConfig::default()).unwrap();

        let entities = extractor.extract_entities("text").await.unwrap();
        assert_eq!(entities[0].entity_type, EntityType::Other);
    }

    #[tokio::test]
    async fn test_extract_relations_parses_valid_response() {
        let response = r#"[{"source": "Rust", "target": "Tokio", "type": "uses"}]"#;
        let llm = mock_llm(response);
        let extractor = LlmExtractor::new(llm, ExtractorConfig::default()).unwrap();

        let entities = vec![
            SemanticEntity::new("Rust".into(), EntityType::Technology),
            SemanticEntity::new("Tokio".into(), EntityType::Technology),
        ];
        let relations = extractor.extract_relations("text", &entities).await.unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].source_name, "Rust");
        assert_eq!(relations[0].target_name, "Tokio");
        assert_eq!(relations[0].relation_type, RelationType::Uses);
    }

    #[tokio::test]
    async fn test_extract_relations_skips_invalid_type() {
        let response = r#"[{"source": "A", "target": "B", "type": "invalid"}]"#;
        let llm = mock_llm(response);
        let extractor = LlmExtractor::new(llm, ExtractorConfig::default()).unwrap();

        let entities = vec![
            SemanticEntity::new("A".into(), EntityType::Person),
            SemanticEntity::new("B".into(), EntityType::Person),
        ];
        let relations = extractor.extract_relations("text", &entities).await.unwrap();
        assert!(relations.is_empty());
    }

    #[tokio::test]
    async fn test_extract_relations_filters_invalid_type_combination() {
        let response = r#"[{"source": "Rust", "target": "Tokio", "type": "located_in"}]"#;
        let llm = mock_llm(response);
        let extractor = LlmExtractor::new(llm, ExtractorConfig::default()).unwrap();

        let entities = vec![
            SemanticEntity::new("Rust".into(), EntityType::Technology),
            SemanticEntity::new("Tokio".into(), EntityType::Technology),
        ];
        let relations = extractor.extract_relations("text", &entities).await.unwrap();
        assert!(relations.is_empty(), "Technology located_in Technology should be filtered");
    }

    #[tokio::test]
    async fn test_call_llm_returns_error_on_failure() {
        let llm = mock_llm_failing();
        let config = ExtractorConfig { max_retries: 1, llm_timeout_secs: 5, ..Default::default() };
        let extractor = LlmExtractor::new(llm, config).unwrap();

        let result = extractor.call_llm("test").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_entity_type_all_variants() {
        assert_eq!(parse_entity_type("person"), EntityType::Person);
        assert_eq!(parse_entity_type("organization"), EntityType::Organization);
        assert_eq!(parse_entity_type("concept"), EntityType::Concept);
        assert_eq!(parse_entity_type("technology"), EntityType::Technology);
        assert_eq!(parse_entity_type("location"), EntityType::Location);
        assert_eq!(parse_entity_type("event"), EntityType::Event);
        assert_eq!(parse_entity_type("document"), EntityType::Document);
        assert_eq!(parse_entity_type("other"), EntityType::Other);
        assert_eq!(parse_entity_type("unknown"), EntityType::Other);
    }

    #[test]
    fn test_parse_relation_type_all_variants() {
        assert_eq!(parse_relation_type("is_a"), Some(RelationType::IsA));
        assert_eq!(parse_relation_type("part_of"), Some(RelationType::PartOf));
        assert_eq!(parse_relation_type("located_in"), Some(RelationType::LocatedIn));
        assert_eq!(parse_relation_type("uses"), Some(RelationType::Uses));
        assert_eq!(parse_relation_type("related_to"), Some(RelationType::RelatedTo));
        assert_eq!(parse_relation_type("created_by"), Some(RelationType::CreatedBy));
        assert_eq!(parse_relation_type("implements"), Some(RelationType::Implements));
        assert_eq!(parse_relation_type("depends_on"), Some(RelationType::DependsOn));
        assert_eq!(parse_relation_type("conflicts_with"), Some(RelationType::ConflictsWith));
        assert_eq!(parse_relation_type("similar_to"), Some(RelationType::SimilarTo));
        assert_eq!(parse_relation_type("invalid"), None);
    }

    #[test]
    fn test_extract_json_array() {
        assert_eq!(extract_json_array("[1,2,3]"), "[1,2,3]");
        assert_eq!(extract_json_array("```json\n[1,2,3]\n```"), "[1,2,3]");
        assert_eq!(extract_json_array("text [1,2] more"), "[1,2]");
    }
}
