//! 语义知识图谱数据模型
//!
//! 详见文档: SDD-001 §2 | 用例: UC-001, UC-002

use serde::{Deserialize, Serialize};

use crate::model::RecordIdType;

/// 语义实体类型
///
/// 详见文档: §2.1 | 用例: UC-001
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    /// 人物（真实或虚构）
    Person,
    /// 组织（公司、机构、团体等）
    Organization,
    /// 概念（抽象思想、理论等）
    Concept,
    /// 技术（技术、工具、框架等）
    Technology,
    /// 地点（地理位置）
    Location,
    /// 事件（历史事件、活动等）
    Event,
    /// 文档（书籍、文章、规范等）
    Document,
    /// 其他未分类类型
    Other,
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityType::Person => write!(f, "person"),
            EntityType::Organization => write!(f, "organization"),
            EntityType::Concept => write!(f, "concept"),
            EntityType::Technology => write!(f, "technology"),
            EntityType::Location => write!(f, "location"),
            EntityType::Event => write!(f, "event"),
            EntityType::Document => write!(f, "document"),
            EntityType::Other => write!(f, "other"),
        }
    }
}

/// 语义实体（独立于结构化引用图的语义知识图谱节点）
///
/// 详见文档: §2.1 | 用例: UC-001
///
/// # 设计说明
/// - `id`：SurrealDB 记录 ID，插入前为 None
/// - `source_tokens`：桥接字段，记录该实体来自哪些 Token
/// - `embedding`：实体名称的向量表示，用于实体消歧
/// - `confidence`：抽取置信度，0.0-1.0
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticEntity {
    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "crate::model::optional_record_id_serde::serialize",
            deserialize_with = "crate::model::optional_record_id_serde::deserialize",
            default
        )
    )]
    #[cfg_attr(not(feature = "db"), serde(default))]
    /// 实体唯一标识符（数据库 RecordId，新建时为 None）
    pub id: Option<RecordIdType>,

    /// 实体名称（主名称）
    pub name: String,

    /// 实体类型
    pub entity_type: EntityType,

    /// 实体描述（可选）
    pub description: Option<String>,

    /// 别名列表
    #[serde(default)]
    pub aliases: Vec<String>,

    /// 实体名称的向量表示，用于实体消歧
    pub embedding: Option<Vec<f32>>,

    /// 来源 Token ID 列表（桥接字段）
    #[serde(default)]
    pub source_tokens: Vec<RecordIdType>,

    /// 来源文档 ID（可选）
    pub source_document: Option<RecordIdType>,

    /// 来源 Block ID（可选）
    pub source_block: Option<RecordIdType>,

    /// 抽取置信度（0.0-1.0）
    #[serde(default = "default_confidence")]
    pub confidence: f64,

    /// 所属社区 ID（可选）
    pub community_id: Option<RecordIdType>,

    /// 创建时间
    #[serde(default = "chrono::Utc::now")]
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// 更新时间
    #[serde(default = "chrono::Utc::now")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

fn default_confidence() -> f64 {
    1.0
}

impl SemanticEntity {
    /// 创建新的语义实体
    ///
    /// 详见文档: §2.1 | 用例: UC-001 | 方法: M-001
    pub fn new(name: String, entity_type: EntityType) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: None,
            name,
            entity_type,
            description: None,
            aliases: Vec::new(),
            embedding: None,
            source_tokens: Vec::new(),
            source_document: None,
            source_block: None,
            confidence: 1.0,
            community_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// 获取用于消歧的主键字符串（名称 + 类型）
    ///
    /// 详见文档: §2.1 | 用例: UC-001 | 方法: M-002
    pub fn disambiguation_key(&self) -> String {
        format!("{}:{}", self.name, self.entity_type)
    }

    /// 添加别名
    ///
    /// 详见文档: §2.1 | 用例: UC-001 | 方法: M-003
    ///
    /// # Errors
    /// 当别名为空字符串时返回 `Err`
    pub fn add_alias(&mut self, alias: String) -> Result<(), error_core::ErrorObject> {
        if alias.is_empty() {
            return Err(crate::error::helpers::validation_error("别名不能为空", "add_alias"));
        }
        if !self.aliases.contains(&alias) {
            self.aliases.push(alias);
        }
        Ok(())
    }

    /// 返回所有可能的名称匹配项（主名称 + 别名）
    ///
    /// 详见文档: §2.1 | 用例: UC-001 | 方法: M-004
    pub fn all_names(&self) -> Vec<&str> {
        let mut names = vec![self.name.as_str()];
        names.extend(self.aliases.iter().map(String::as_str));
        names
    }
}

/// 语义关系类型
///
/// 详见文档: §2.2 | 用例: UC-002
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    /// 属于关系（A is a B）
    IsA,
    /// 部分关系（A part of B）
    PartOf,
    /// 位于关系（A located in B）
    LocatedIn,
    /// 使用关系（A uses B）
    Uses,
    /// 相关关系（A related to B）
    RelatedTo,
    /// 创建关系（A created by B）
    CreatedBy,
    /// 实现关系（A implements B）
    Implements,
    /// 依赖关系（A depends on B）
    DependsOn,
    /// 冲突关系（A conflicts with B）
    ConflictsWith,
    /// 相似关系（A similar to B）
    SimilarTo,
}

impl std::fmt::Display for RelationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RelationType::IsA => write!(f, "is_a"),
            RelationType::PartOf => write!(f, "part_of"),
            RelationType::LocatedIn => write!(f, "located_in"),
            RelationType::Uses => write!(f, "uses"),
            RelationType::RelatedTo => write!(f, "related_to"),
            RelationType::CreatedBy => write!(f, "created_by"),
            RelationType::Implements => write!(f, "implements"),
            RelationType::DependsOn => write!(f, "depends_on"),
            RelationType::ConflictsWith => write!(f, "conflicts_with"),
            RelationType::SimilarTo => write!(f, "similar_to"),
        }
    }
}

impl RelationType {
    /// 判断给定源实体类型和目标实体类型之间是否允许建立此关系
    ///
    /// 基于 Schema 约束矩阵，过滤语义上荒谬的关系组合。
    /// 例如 `Person located_in Technology` 会被拒绝。
    ///
    /// 约束矩阵设计原则：
    /// - `RelatedTo` 和 `SimilarTo` 对所有类型组合开放（通用关系）
    /// - `LocatedIn` 仅允许涉及 Location 的组合
    /// - `CreatedBy` 仅允许 Person/Organization 作为目标
    /// - `Implements` 仅允许 Technology 作为源
    /// - `DependsOn` 仅允许 Technology/Document 作为源和目标
    /// - `ConflictsWith` 仅允许 Technology/Concept 作为源和目标
    #[must_use]
    #[allow(clippy::match_same_arms)]
    pub fn is_valid_type_combination(&self, source: &EntityType, target: &EntityType) -> bool {
        match self {
            RelationType::RelatedTo | RelationType::SimilarTo | RelationType::IsA => true,
            RelationType::PartOf => !matches!(
                (source, target),
                (EntityType::Person, EntityType::Technology)
                    | (EntityType::Technology, EntityType::Person)
            ),
            RelationType::LocatedIn => {
                matches!(target, EntityType::Location)
                    || matches!(
                        source,
                        EntityType::Location | EntityType::Organization | EntityType::Event
                    )
            }
            RelationType::Uses => {
                matches!(
                    source,
                    EntityType::Person | EntityType::Organization | EntityType::Technology
                ) && !matches!(target, EntityType::Location)
            }
            RelationType::CreatedBy => {
                matches!(target, EntityType::Person | EntityType::Organization)
            }
            RelationType::Implements => {
                matches!(source, EntityType::Technology)
            }
            RelationType::DependsOn => {
                matches!(source, EntityType::Technology | EntityType::Document)
                    && matches!(target, EntityType::Technology | EntityType::Document)
            }
            RelationType::ConflictsWith => {
                matches!(
                    (source, target),
                    (
                        EntityType::Technology | EntityType::Concept,
                        EntityType::Technology | EntityType::Concept
                    )
                )
            }
        }
    }

    /// 生成关系类型组合约束的人类可读描述，用于 LLM Prompt 注入
    #[must_use]
    pub fn constraint_description() -> Vec<&'static str> {
        vec![
            "is_a: 任意类型组合均可",
            "part_of: 禁止 Person↔Technology 组合",
            "located_in: 目标必须是 Location，或源为 Organization/Event/Location",
            "uses: 源必须是 Person/Organization/Technology，目标不能是 Location",
            "related_to: 任意类型组合均可",
            "created_by: 目标必须是 Person 或 Organization",
            "implements: 源必须是 Technology",
            "depends_on: 源和目标都必须是 Technology 或 Document",
            "conflicts_with: 源和目标都必须是 Technology 或 Concept",
            "similar_to: 任意类型组合均可",
        ]
    }
}

/// 语义关系（语义知识图谱的边）
///
/// 详见文档: §2.2 | 用例: UC-002
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticRelation {
    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "crate::model::optional_record_id_serde::serialize",
            deserialize_with = "crate::model::optional_record_id_serde::deserialize",
            default
        )
    )]
    #[cfg_attr(not(feature = "db"), serde(default))]
    /// 关系唯一标识符（数据库 RecordId，新建时为 None）
    pub id: Option<RecordIdType>,

    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "crate::model::record_id_serde::serialize",
            deserialize_with = "crate::model::record_id_serde::deserialize"
        )
    )]
    /// 源实体 ID
    pub source_entity: RecordIdType,

    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "crate::model::record_id_serde::serialize",
            deserialize_with = "crate::model::record_id_serde::deserialize"
        )
    )]
    /// 目标实体 ID
    pub target_entity: RecordIdType,

    /// 关系类型
    pub relation_type: RelationType,

    /// 证据（支持该关系的文本或引用）
    pub evidence: String,

    /// 置信度（0.0-1.0）
    #[serde(default = "default_relation_confidence")]
    pub confidence: f64,

    /// 来源文档 ID（可选）
    pub source_document: Option<RecordIdType>,

    /// 创建时间
    #[serde(default = "chrono::Utc::now")]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

fn default_relation_confidence() -> f64 {
    1.0
}

impl SemanticRelation {
    /// 创建新的语义关系
    ///
    /// 详见文档: §2.2 | 用例: UC-002 | 方法: M-005
    pub fn new(
        source_entity: RecordIdType,
        target_entity: RecordIdType,
        relation_type: RelationType,
        evidence: String,
    ) -> Self {
        Self {
            id: None,
            source_entity,
            target_entity,
            relation_type,
            evidence,
            confidence: 1.0,
            source_document: None,
            created_at: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试辅助：创建 RecordIdType 实例
    #[cfg(feature = "db")]
    fn rid(s: &str) -> RecordIdType {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            surrealdb::sql::Thing::from((parts[0], parts[1]))
        } else {
            surrealdb::sql::Thing::from((s, ""))
        }
    }

    #[cfg(not(feature = "db"))]
    fn rid(s: &str) -> RecordIdType {
        s.to_string()
    }

    #[test]
    fn test_semantic_entity_new_creates_entity() {
        let entity = SemanticEntity::new("Rust".to_string(), EntityType::Technology);

        assert_eq!(entity.name, "Rust");
        assert!(matches!(entity.entity_type, EntityType::Technology));
        assert!(entity.id.is_none());
        assert!(entity.description.is_none());
        assert!(entity.aliases.is_empty());
        assert!(entity.embedding.is_none());
        assert!(entity.source_tokens.is_empty());
        assert!(entity.source_document.is_none());
        assert!(entity.source_block.is_none());
        assert!((entity.confidence - 1.0).abs() < f64::EPSILON);
        assert!(entity.community_id.is_none());
        assert_eq!(entity.created_at, entity.updated_at);
    }

    #[test]
    fn test_disambiguation_key_returns_name_type() {
        let entity = SemanticEntity::new("Rust".to_string(), EntityType::Technology);
        let key = entity.disambiguation_key();

        assert_eq!(key, "Rust:technology");
    }

    #[test]
    fn test_add_alias_prevents_duplicates() {
        let mut entity = SemanticEntity::new("Rust".to_string(), EntityType::Technology);

        entity.add_alias("Rust-lang".to_string()).expect("添加别名应成功");
        assert_eq!(entity.aliases.len(), 1);
        assert!(entity.aliases.contains(&"Rust-lang".to_string()));

        entity.add_alias("Rust-lang".to_string()).expect("重复别名应成功");
        assert_eq!(entity.aliases.len(), 1);

        entity.add_alias("Rust Programming Language".to_string()).expect("添加别名应成功");
        assert_eq!(entity.aliases.len(), 2);
    }

    #[test]
    fn test_add_alias_empty_returns_error() {
        let mut entity = SemanticEntity::new("Rust".to_string(), EntityType::Technology);
        let result = entity.add_alias(String::new());
        assert!(result.is_err(), "空别名应返回 Err");
    }

    #[test]
    fn test_all_names_includes_primary_and_aliases() {
        let mut entity = SemanticEntity::new("Rust".to_string(), EntityType::Technology);
        entity.add_alias("Rust-lang".to_string()).expect("添加别名应成功");
        entity.add_alias("Rust Programming Language".to_string()).expect("添加别名应成功");

        let names = entity.all_names();
        assert_eq!(names.len(), 3);
        assert_eq!(names[0], "Rust");
        assert!(names.contains(&"Rust-lang"));
        assert!(names.contains(&"Rust Programming Language"));
    }

    #[test]
    fn test_entity_type_display_formats_correctly() {
        assert_eq!(format!("{}", EntityType::Person), "person");
        assert_eq!(format!("{}", EntityType::Organization), "organization");
        assert_eq!(format!("{}", EntityType::Concept), "concept");
        assert_eq!(format!("{}", EntityType::Technology), "technology");
        assert_eq!(format!("{}", EntityType::Location), "location");
        assert_eq!(format!("{}", EntityType::Event), "event");
        assert_eq!(format!("{}", EntityType::Document), "document");
        assert_eq!(format!("{}", EntityType::Other), "other");
    }

    #[test]
    fn test_semantic_relation_new_creates_relation() {
        let source = rid("semantic_entity:rust");
        let target = rid("semantic_entity:tokio");

        let relation = SemanticRelation::new(
            source.clone(),
            target.clone(),
            RelationType::Uses,
            "Tokio is used by Rust projects".to_string(),
        );

        assert!(relation.id.is_none());
        assert_eq!(relation.source_entity, source);
        assert_eq!(relation.target_entity, target);
        assert!(matches!(relation.relation_type, RelationType::Uses));
        assert_eq!(relation.evidence, "Tokio is used by Rust projects");
        assert!((relation.confidence - 1.0).abs() < f64::EPSILON);
        assert!(relation.source_document.is_none());
    }

    #[test]
    fn test_entity_type_serde_roundtrip() {
        let entity_type = EntityType::Technology;
        let json = serde_json::to_string(&entity_type).expect("序列化失败");
        let de: EntityType = serde_json::from_str(&json).expect("反序列化失败");
        assert_eq!(entity_type, de);
    }
}
