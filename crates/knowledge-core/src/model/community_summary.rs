//! 社区摘要数据模型
//!
//! 详见文档: SDD-001 §3 | 用例: UC-003

use serde::{Deserialize, Serialize};

use crate::model::RecordIdType;

/// 社区摘要（为 Leiden 检测到的社区生成的自然语言摘要）
///
/// 详见文档: §3.1 | 用例: UC-003
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunitySummary {
    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "crate::model::optional_record_id_serde::serialize",
            deserialize_with = "crate::model::optional_record_id_serde::deserialize",
            default
        )
    )]
    #[cfg_attr(not(feature = "db"), serde(default))]
    /// 摘要唯一标识符（数据库 RecordId，新建时为 None）
    pub id: Option<RecordIdType>,

    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "crate::model::record_id_serde::serialize",
            deserialize_with = "crate::model::record_id_serde::deserialize"
        )
    )]
    /// 社区 ID
    pub community_id: RecordIdType,

    /// 摘要文本（自然语言描述）
    pub summary_text: String,

    /// 关键概念列表
    #[serde(default)]
    pub key_concepts: Vec<String>,

    /// 实体数量
    #[serde(default)]
    pub entity_count: usize,

    /// 关系数量
    #[serde(default)]
    pub relation_count: usize,

    /// 内聚性分数
    pub cohesion_score: f64,

    /// 生成时间
    #[serde(default = "chrono::Utc::now")]
    pub generated_at: chrono::DateTime<chrono::Utc>,

    /// 使用的 LLM 模型名称
    pub llm_model: String,
}

impl CommunitySummary {
    /// 创建新的社区摘要
    ///
    /// 详见文档: §3.1 | 用例: UC-003 | 方法: M-006
    pub fn new(
        community_id: RecordIdType,
        summary_text: String,
        key_concepts: Vec<String>,
        entity_count: usize,
        relation_count: usize,
        cohesion_score: f64,
        llm_model: String,
    ) -> Self {
        Self {
            id: None,
            community_id,
            summary_text,
            key_concepts,
            entity_count,
            relation_count,
            cohesion_score,
            generated_at: chrono::Utc::now(),
            llm_model,
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
    fn test_community_summary_new_sets_defaults() {
        let community_id = rid("community:rust_async");

        let summary = CommunitySummary::new(
            community_id.clone(),
            "Rust 异步编程生态".to_string(),
            vec![
                "async".to_string(),
                "tokio".to_string(),
                "futures".to_string(),
            ],
            15,
            23,
            0.87,
            "gpt-4".to_string(),
        );

        assert!(summary.id.is_none());
        assert_eq!(summary.community_id, community_id);
        assert_eq!(summary.summary_text, "Rust 异步编程生态");
        assert_eq!(summary.key_concepts.len(), 3);
        assert_eq!(summary.entity_count, 15);
        assert_eq!(summary.relation_count, 23);
        assert!((summary.cohesion_score - 0.87).abs() < f64::EPSILON);
        assert_eq!(summary.llm_model, "gpt-4");
    }

    #[test]
    fn test_community_summary_serialization() {
        #[cfg(not(feature = "db"))]
        {
            let community_id = "community:rust_async".to_string();
            let summary = CommunitySummary::new(
                community_id.clone(),
                "Test summary".to_string(),
                vec!["concept1".to_string()],
                5,
                3,
                0.75,
                "test-model".to_string(),
            );

            let json = serde_json::to_string(&summary).expect("序列化失败");
            let de_summary: CommunitySummary = serde_json::from_str(&json).expect("反序列化失败");

            assert_eq!(summary.community_id, de_summary.community_id);
            assert_eq!(summary.summary_text, de_summary.summary_text);
            assert_eq!(summary.key_concepts, de_summary.key_concepts);
            assert_eq!(summary.entity_count, de_summary.entity_count);
            assert_eq!(summary.relation_count, de_summary.relation_count);
        }
    }

    #[cfg(feature = "db")]
    #[test]
    fn test_rid_helper_no_colon() {
        let thing = rid("nocolon");
        assert_eq!(thing.tb, "nocolon");
    }

    #[test]
    fn test_community_summary_cohesion_score() {
        let community_id = rid("community:test");
        let summary = CommunitySummary::new(
            community_id,
            "Test".to_string(),
            vec![],
            0,
            0,
            0.0,
            "model".to_string(),
        );
        assert!((summary.cohesion_score - 0.0).abs() < f64::EPSILON);
    }
}
