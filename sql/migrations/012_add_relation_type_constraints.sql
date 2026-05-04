-- ========================================
-- 迁移 012：关系类型组合约束
-- 确保 relation_type 与 source/target 的 entity_type 组合语义合法
-- 详见文档: §2.2 | Schema-Constrained 关系抽取
-- ========================================

-- 添加源实体类型和目标实体类型字段（冗余存储，用于 ASSERT 约束）
-- 这些字段在插入时由应用层填充，与 source_entity/target_entity 的 entity_type 保持一致
DEFINE FIELD source_entity_type ON extracted_relation TYPE string
    ASSERT $value IN ["person", "organization", "concept", "technology", "location", "event", "document", "other"];
DEFINE FIELD target_entity_type ON extracted_relation TYPE string
    ASSERT $value IN ["person", "organization", "concept", "technology", "location", "event", "document", "other"];

-- 关系类型组合约束
-- related_to 和 similar_to 对所有类型组合开放（通用关系）
-- is_a 对所有类型组合开放（继承关系）
-- 其他关系类型有特定的类型组合约束
DEFINE FIELD relation_type ON extracted_relation TYPE string
    ASSERT $value IN ["is_a", "part_of", "located_in", "uses", "related_to", "created_by", "implements", "depends_on", "conflicts_with", "similar_to"]
    AND (
        $value IN ["is_a", "related_to", "similar_to"]
        OR ($value = "part_of" AND NOT ($source_entity_type = "person" AND $target_entity_type = "technology") AND NOT ($source_entity_type = "technology" AND $target_entity_type = "person"))
        OR ($value = "located_in" AND ($target_entity_type = "location" OR $source_entity_type IN ["location", "organization", "event"]))
        OR ($value = "uses" AND $source_entity_type IN ["person", "organization", "technology"] AND $target_entity_type != "location")
        OR ($value = "created_by" AND $target_entity_type IN ["person", "organization"])
        OR ($value = "implements" AND $source_entity_type = "technology")
        OR ($value = "depends_on" AND $source_entity_type IN ["technology", "document"] AND $target_entity_type IN ["technology", "document"])
        OR ($value = "conflicts_with" AND $source_entity_type IN ["technology", "concept"] AND $target_entity_type IN ["technology", "concept"])
    );

-- 同样为 semantic_relation 表添加约束
DEFINE FIELD source_entity_type ON semantic_relation TYPE string
    ASSERT $value IN ["person", "organization", "concept", "technology", "location", "event", "document", "other"];
DEFINE FIELD target_entity_type ON semantic_relation TYPE string
    ASSERT $value IN ["person", "organization", "concept", "technology", "location", "event", "document", "other"];

DEFINE FIELD relation_type ON semantic_relation TYPE string
    ASSERT $value IN ["is_a", "part_of", "located_in", "uses", "related_to", "created_by", "implements", "depends_on", "conflicts_with", "similar_to"]
    AND (
        $value IN ["is_a", "related_to", "similar_to"]
        OR ($value = "part_of" AND NOT ($source_entity_type = "person" AND $target_entity_type = "technology") AND NOT ($source_entity_type = "technology" AND $target_entity_type = "person"))
        OR ($value = "located_in" AND ($target_entity_type = "location" OR $source_entity_type IN ["location", "organization", "event"]))
        OR ($value = "uses" AND $source_entity_type IN ["person", "organization", "technology"] AND $target_entity_type != "location")
        OR ($value = "created_by" AND $target_entity_type IN ["person", "organization"])
        OR ($value = "implements" AND $source_entity_type = "technology")
        OR ($value = "depends_on" AND $source_entity_type IN ["technology", "document"] AND $target_entity_type IN ["technology", "document"])
        OR ($value = "conflicts_with" AND $source_entity_type IN ["technology", "concept"] AND $target_entity_type IN ["technology", "concept"])
    );
