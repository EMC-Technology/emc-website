-- ========================================
-- 迁移 011：抽取结果表结构
-- 详见文档: §8 | 用例: UC-022
-- ========================================

-- 实体抽取结果表
DEFINE TABLE extracted_entity SCHEMAFULL;

DEFINE FIELD name ON extracted_entity TYPE string
    ASSERT $value != '';
DEFINE FIELD entity_type ON extracted_entity TYPE string
    ASSERT $value IN ["person", "organization", "concept", "technology", "location", "event", "document", "other"];
DEFINE FIELD description ON extracted_entity TYPE option<string>;
DEFINE FIELD source_block ON extracted_entity TYPE record<block>;
DEFINE FIELD confidence ON extracted_entity TYPE float DEFAULT 1.0f
    ASSERT $value >= 0.0f AND $value <= 1.0f;
DEFINE FIELD created_at ON extracted_entity TYPE datetime VALUE $before OR time::now();

DEFINE INDEX extracted_entity_name_idx ON extracted_entity FIELDS name;
DEFINE INDEX extracted_entity_source_block_idx ON extracted_entity FIELDS source_block;

-- 关系抽取结果表
DEFINE TABLE extracted_relation SCHEMAFULL;

DEFINE FIELD source_entity ON extracted_relation TYPE record<extracted_entity>;
DEFINE FIELD target_entity ON extracted_relation TYPE record<extracted_entity>;
DEFINE FIELD relation_type ON extracted_relation TYPE string
    ASSERT $value IN ["is_a", "part_of", "located_in", "uses", "related_to", "created_by", "implements", "depends_on", "conflicts_with", "similar_to"];
DEFINE FIELD source_block ON extracted_relation TYPE record<block>;
DEFINE FIELD confidence ON extracted_relation TYPE float DEFAULT 1.0f
    ASSERT $value >= 0.0f AND $value <= 1.0f;
DEFINE FIELD created_at ON extracted_relation TYPE datetime VALUE $before OR time::now();

DEFINE INDEX extracted_relation_source_idx ON extracted_relation FIELDS source_entity;
DEFINE INDEX extracted_relation_target_idx ON extracted_relation FIELDS target_entity;
