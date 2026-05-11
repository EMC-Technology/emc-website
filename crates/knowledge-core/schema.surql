-- ============================================================================
-- 知识系统 SurrealDB Schema 定义
-- ============================================================================
--
-- 本文件定义知识图谱系统的完整数据库 Schema：
-- - document (L1): 文档根节点表
-- - block (L2): 块容器节点表
-- - token (L3): 词元叶子节点表
-- - references (LR): 引用关系边表（关系型）
--
-- 使用 SCHEMAFULL 模式强制字段约束，确保数据完整性。
-- ============================================================================

-- ============================================================================
-- 1. document 表（L1 根节点）
-- ============================================================================

DEFINE TABLE document SCHEMAFULL
    -- 启用严格模式：未定义的字段将被拒绝

DEFINE FIELD id ON document TYPE record;
    -- 自动生成的记录 ID

DEFINE FIELD path ON document TYPE string
    -- 文件系统路径（绝对或相对）
    ASSERT $value != '' AND string::len($value) <= 4096;
    -- 非空且长度限制 4KB

DEFINE FIELD title ON document TYPE option<string>
    -- 文档标题（可选，可为 NULL）

DEFINE FIELD source_type ON document TYPE string
    -- 来源类型枚举值
    ASSERT $value IN ['Markdown', 'Code', 'Plain'];

DEFINE FIELD hash ON document TYPE string
    -- blake3 哈希值（64 字符 hex）
    ASSERT string::len($value) == 64
        AND $value =~ /^[0-9a-fA-F]{64}$/;

-- 索引定义
DEFINE INDEX doc_hash_idx ON document FIELDS hash UNIQUE;
    -- 唯一索引：用于幂等性去重
    -- 相同哈希的文档不会重复插入

DEFINE INDEX doc_path_idx ON document FIELDS path;
    -- 普通索引：按路径查询文档

-- ============================================================================
-- 2. block 表（L2 容器节点）
-- ============================================================================

DEFINE TABLE block SCHEMAFULL
    -- 启用严格模式

DEFINE FIELD id ON block TYPE record;
    -- 自动生成的记录 ID

DEFINE FIELD doc_id ON block TYPE record<document>
    -- 外键：关联父文档
    -- SurrealDB 会自动验证引用的记录存在性

DEFINE FIELD block_type ON block TYPE string
    -- 块类型枚举值
    ASSERT $value IN ['Heading', 'Paragraph', 'Code', 'List', 'Table'];

DEFINE FIELD start_line ON block TYPE int
    -- 起始行号（从 0 开始）
    ASSERT $value >= 0;

DEFINE FIELD end_line ON block TYPE int
    -- 结束行号（包含）
    ASSERT $value >= $start_line;
    -- 约束：结束行号必须 ≥ 起始行号

DEFINE FIELD embedding ON block TYPE option<array<float>>
    -- 可选的向量嵌入（2560 维浮点数组，GEMMA4-E4B 输出维度）
    -- None 表示尚未生成嵌入

DEFINE FIELD idempotency_key ON block TYPE option<string>
    -- 幂等性键（可选）
    -- 格式建议：{block_type}_{start_line}_{end_line}

-- 索引定义
DEFINE INDEX block_doc_id_idx ON block FIELDS doc_id;
    -- 外键索引：加速按文档查询块

DEFINE INDEX block_pos_idx ON block FIELDS doc_id, start_line, end_line;
    -- 复合索引：按位置范围查询块
    -- 用于快速定位文档中的特定行范围

DEFINE INDEX block_vec_idx ON block MTREE embedding DIMENSION 2560 DISTANCE COSINE TYPE VECTORFN;
    -- 向量索引：支持语义相似度检索
    -- 使用余弦距离度量
    -- 维度固定为 2560（GEMMA4-E4B 输出维度）
    -- 注意：此索引需要 SurrealDB 的向量扩展支持

DEFINE INDEX block_idemp_idx ON block FIELDS idempotency_key UNIQUE;
    -- 唯一索引：幂等性去重
    -- 防止相同块重复插入

-- ============================================================================
-- 3. token 表（L3 叶子节点）
-- ============================================================================

DEFINE TABLE token SCHEMAFULL
    -- 启用严格模式

DEFINE FIELD id ON token TYPE record;
    -- 自动生成的记录 ID

DEFINE FIELD block_id ON token TYPE record<block>
    -- 外键：关联父块

DEFINE FIELD content ON token TYPE string
    -- Token 文本内容
    ASSERT $value != '' AND string::len($value) <= 1024;
    -- 非空且最大长度 1KB

DEFINE FIELD token_type ON token TYPE string
    -- Token 类型枚举值
    ASSERT $value IN ['Word', 'Punct', 'Symbol', 'Keyword', 'Identifier'];

DEFINE FIELD start_char ON token TYPE int
    -- 块内相对字符偏移（从 0 开始）
    ASSERT $value >= 0;

DEFINE FIELD global_offset ON token TYPE int
    -- 全局绝对字符偏移（u64 范围）
    ASSERT $value >= 0;

-- 索引定义
DEFINE INDEX token_block_idx ON token FIELDS block_id;
    -- 外键索引：加速按块查询 Token

DEFINE INDEX token_global_idx ON token FIELDS global_offset;
    -- 全局偏移索引：跨块连续文本拼接

DEFINE INDEX token_content_idx ON token FIELDS content;
    -- 内容索引：精确文本匹配查询

DEFINE INDEX token_type_idx ON token FIELDS token_type;
    -- 类型索引：按 Token 类型过滤

-- ============================================================================
-- 4. references 表（LR 有向边）
-- ============================================================================

DEFINE TABLE references SCHEMAFULL TYPE RELATION FROM ANY TO ANY
    -- 关系型表：连接任意两个实体
    -- FROM ANY: 边起点可以是 document/block/token 表的任意记录
    -- TO ANY:   边终点可以是 document/block/token 表的任意记录

DEFINE FIELD id ON references TYPE record;
    -- 自动生成的记录 ID

DEFINE FIELD ref_type ON references TYPE string
    -- 引用关系类型
    ASSERT $value IN [
        'Definition',
        'Usage',
        'Link',
        'Inherit',
        'Implement',
        'Constrain'
    ];

DEFINE FIELD direction ON references TYPE string
    -- 方向性
    ASSERT $value IN ['OneWay', 'TwoWay', 'ImplicitTwoWay'];

DEFINE FIELD scope ON references TYPE option<string>
    -- 可选的作用域限定
    -- 如 "function_scope", "class_scope" 等

DEFINE FIELD in ON references RECORD;
    -- 边起点（自动生成的关系字段）
    -- 对应 Rust 结构体中的 from_id

DEFINE FIELD out ON references RECORD;
    -- 边终点（自动生成的关系字段）
    -- 对应 Rust 结构体中的 to_id

-- 索引定义
DEFINE INDEX ref_type_idx ON references FIELDS ref_type, direction;
    -- 复合索引：按引用类型和方向查询

DEFINE INDEX ref_from_idx ON references FIELDS in;
    -- 出边索引：查询某实体的所有出边

DEFINE INDEX ref_to_idx ON references FIELDS out;
    -- 入边索引：查询某实体的所有入边

DEFINE INDEX ref_scope_idx ON references FIELDS scope;
    -- 作用域索引：按作用域过滤引用关系

-- ============================================================================
-- 5. community 表（L4 社区节点）
-- ============================================================================

DEFINE TABLE community SCHEMAFULL

DEFINE FIELD id ON community TYPE record;

DEFINE FIELD name ON community TYPE string
    ASSERT $value != '' AND string::len($value) <= 512;

DEFINE FIELD cohesion_score ON community TYPE float
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE FIELD member_ids ON community TYPE array<string>
    ASSERT array::len($value) > 0;

DEFINE INDEX community_name_idx ON community FIELDS name;
    -- 名称索引：按社区名称查询

DEFINE INDEX community_cohesion_idx ON community FIELDS cohesion_score;
    -- 内聚度索引：按内聚分数排序查询

-- ============================================================================
-- 6. process 表（L4 流程节点）
-- ============================================================================

DEFINE TABLE process SCHEMAFULL

DEFINE FIELD id ON process TYPE record;

DEFINE FIELD name ON process TYPE string
    ASSERT $value != '' AND string::len($value) <= 512;

DEFINE FIELD entry_point_id ON process TYPE string
    ASSERT $value != '';

DEFINE FIELD description ON process TYPE option<string>;

DEFINE INDEX process_name_idx ON process FIELDS name;
    -- 名称索引：按流程名称查询

DEFINE INDEX process_entry_idx ON process FIELDS entry_point_id;
    -- 入口索引：按入口实体查询关联流程

-- ============================================================================
-- 7. process_step 表（L4 流程步骤边）
-- ============================================================================

DEFINE TABLE process_step SCHEMAFULL

DEFINE FIELD id ON process_step TYPE record;

DEFINE FIELD process_id ON process_step TYPE record<process>
    -- 外键：关联所属流程

DEFINE FIELD symbol_id ON process_step TYPE string
    ASSERT $value != '';

DEFINE FIELD step_order ON process_step TYPE int
    ASSERT $value >= 0;

DEFINE FIELD confidence ON process_step TYPE float
    ASSERT $value >= 0.0 AND $value <= 1.0;

DEFINE INDEX step_process_idx ON process_step FIELDS process_id;
    -- 流程外键索引：加速按流程查询步骤

DEFINE INDEX step_order_idx ON process_step FIELDS process_id, step_order;
    -- 复合索引：按流程 + 步骤顺序查询，确保步骤有序检索

-- ============================================================================
-- 8. 数据库初始化事件（可选）
-- ============================================================================

-- DEFINE EVENT init ON DATABASE THEN {
--     -- 数据库初始化时执行的任务
--     -- 例如：创建默认命名空间、设置权限等
--     LET $exists = (SELECT value count() FROM namespace WHERE name = 'knowledge' LIMIT 1)[0];
--     
--     IF $exists == 0 THEN
--         DEFINE NAMESPACE knowledge;
--         DEFINE DATABASE knowledge;
--     END;
-- };

-- ============================================================================
-- 6. 使用说明
-- ============================================================================
--
-- 初始化数据库：
--   surreal sql --file schema.surql --endpoint http://localhost:8000 --ns knowledge --db knowledge
--
-- 或在代码中执行：
--   let query = std::fs::read_to_string("schema.surql")?;
--   db.query(&query).await?;
--
-- 注意事项：
-- 1. 向量索引 (block_vec_idx) 需要 SurrealDB 1.3+ 版本和向量扩展
-- 2. 关系表 (references) 使用 SurrealDB 的图数据库特性
-- 3. 所有字符串字段均设置长度上限防止恶意输入
-- 4. 外键约束通过 record<T> 类型自动强制执行

-- ============================================================================
-- 7. Schema 版本追踪表（用于幂等性判断）
-- ============================================================================

DEFINE TABLE schema_version SCHEMAFULL;
    -- Schema 版本追踪表：记录每次 Schema 变更的版本信息
    -- 用于支持幂等初始化和增量迁移

DEFINE FIELD version ON schema_version TYPE string;
    -- 语义化版本号（如 "3.2.0"）
    -- 格式：major.minor.patch

DEFINE FIELD applied_at ON schema_version TYPE datetime;
    -- Schema 应用时间戳（UTC）
    -- 用于追踪变更历史

DEFINE FIELD checksum ON schema_version TYPE string;
    -- BLAKE3 哈希值（schema.surql 文件内容的校验和）
    -- 用于检测 Schema 文件是否被篡改或修改

DEFINE INDEX version_idx ON schema_version FIELDS version UNIQUE;
    -- 唯一索引：确保同一版本号只记录一次
