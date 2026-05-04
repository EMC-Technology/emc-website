-- ========================================
-- 迁移 010：语义知识图谱表结构
-- 详见文档: SDD-001 §5 | 用例: UC-004补充
-- ========================================

-- 语义实体表
DEFINE TABLE semantic_entity SCHEMAFULL;

DEFINE FIELD name ON semantic_entity TYPE string
    ASSERT $value != '';
DEFINE FIELD entity_type ON semantic_entity TYPE string
    ASSERT $value IN ["person", "organization", "concept", "technology", "location", "event", "document", "other"];
DEFINE FIELD description ON semantic_entity TYPE option<string>;
DEFINE FIELD aliases ON semantic_entity TYPE array<string> DEFAULT [];
DEFINE FIELD embedding ON semantic_entity TYPE option<array<float>>;
DEFINE FIELD source_tokens ON semantic_entity TYPE array<record<token>> DEFAULT [];
DEFINE FIELD source_document ON semantic_entity TYPE option<record<document>>;
DEFINE FIELD source_block ON semantic_entity TYPE option<record<block>>;
DEFINE FIELD confidence ON semantic_entity TYPE float DEFAULT 1.0f
    ASSERT $value >= 0.0f AND $value <= 1.0f;
DEFINE FIELD community_id ON semantic_entity TYPE option<record<community>>;
DEFINE FIELD created_at ON semantic_entity TYPE datetime VALUE $before OR time::now();
DEFINE FIELD updated_at ON semantic_entity TYPE datetime VALUE time::now();

DEFINE INDEX semantic_entity_name_type_idx ON semantic_entity FIELDS name, entity_type UNIQUE;
DEFINE INDEX semantic_entity_name_search_idx ON semantic_entity FIELDS name SEARCH ANALYZER simple BM25;
DEFINE INDEX semantic_entity_community_idx ON semantic_entity FIELDS community_id;
DEFINE INDEX semantic_entity_source_doc_idx ON semantic_entity FIELDS source_document;
DEFINE INDEX semantic_entity_vec_idx ON semantic_entity FIELDS embedding MTREE DIMENSION 2560;

-- 语义关系表
DEFINE TABLE semantic_relation SCHEMAFULL TYPE RELATION IN semantic_entity OUT semantic_entity;

DEFINE FIELD relation_type ON semantic_relation TYPE string
    ASSERT $value IN ["is_a", "part_of", "located_in", "uses", "related_to", "created_by", "implements", "depends_on", "conflicts_with", "similar_to"];
DEFINE FIELD evidence ON semantic_relation TYPE string
    ASSERT $value != '';
DEFINE FIELD confidence ON semantic_relation TYPE float DEFAULT 1.0f
    ASSERT $value >= 0.0f AND $value <= 1.0f;
DEFINE FIELD source_document ON semantic_relation TYPE option<record<document>>;
DEFINE FIELD created_at ON semantic_relation TYPE datetime VALUE $before OR time::now();

DEFINE INDEX semantic_relation_source_idx ON semantic_relation FIELDS in;
DEFINE INDEX semantic_relation_target_idx ON semantic_relation FIELDS out;
DEFINE INDEX semantic_relation_type_idx ON semantic_relation FIELDS relation_type;
DEFINE INDEX semantic_relation_source_type_idx ON semantic_relation FIELDS in, relation_type;

-- 社区摘要表
DEFINE TABLE community_summary SCHEMAFULL;

DEFINE FIELD community_id ON community_summary TYPE record<community>;
DEFINE FIELD summary_text ON community_summary TYPE string
    ASSERT $value != '';
DEFINE FIELD key_concepts ON community_summary TYPE array<string> DEFAULT [];
DEFINE FIELD entity_count ON community_summary TYPE int DEFAULT 0;
DEFINE FIELD relation_count ON community_summary TYPE int DEFAULT 0;
DEFINE FIELD cohesion_score ON community_summary TYPE float;
DEFINE FIELD generated_at ON community_summary TYPE datetime VALUE $before OR time::now();
DEFINE FIELD llm_model ON community_summary TYPE string
    ASSERT $value != '';

DEFINE INDEX community_summary_community_idx ON community_summary FIELDS community_id UNIQUE;
DEFINE INDEX community_summary_text_idx ON community_summary FIELDS summary_text SEARCH ANALYZER simple BM25;
