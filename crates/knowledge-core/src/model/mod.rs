//! 核心领域实体定义（三层节点 + 有向边）
//!
//! 本模块定义知识图谱系统的四层核心数据结构：
//! - **L1 Document**：文档根节点，表示原始文件
//! - **L2 Block**：容器节点，表示文档的逻辑分块（段落、代码段等）
//! - **L3 Token**：叶子节点，表示最小语义单元（词、标点、符号）
//! - **LR Reference**：有向边，表示实体间的引用关系
//! - **语义知识图谱扩展**：SemanticEntity, SemanticRelation, CommunitySummary
//!
//! 所有实体均支持 serde 序列化/反序列化。
//! 当启用 `db` feature 时，ID 字段使用 SurrealDB 的 `RecordId` 类型；
//! 否则使用字符串表示。

pub mod community_summary;
pub mod ids;
pub mod semantic;

pub use community_summary::CommunitySummary;
pub use ids::{
    AggregateId, BlockId, CausationId, CommunityId, CorrelationId, DocumentId, EdgeId, EventId,
    NodeId, PrincipalId, ProcessId, ProcessStepId, ReferenceId, ResourceId, ScopeId, TokenId,
    TraceId, UserId,
};
pub use semantic::{EntityType, RelationType, SemanticEntity, SemanticRelation};

use serde::{Deserialize, Serialize};

#[cfg(feature = "db")]
use surrealdb::opt::RecordId;

/// SurrealDB RecordId 类型别名（启用 db feature 时为原生 RecordId，否则为 String）
#[cfg(feature = "db")]
pub type RecordIdType = RecordId;
/// SurrealDB RecordId 类型别名（未启用 db feature 时回退为 String）
#[cfg(not(feature = "db"))]
pub type RecordIdType = String;

#[cfg(feature = "db")]
/// SurrealDB RecordId 序列化/反序列化辅助模块
///
/// 将 `RecordId` 与 JSON 字符串之间进行双向转换，
/// 确保 SurrealDB 原生类型可跨 serde 边界传输。
pub mod record_id_serde {
    use serde::Serializer;
    use serde::de::{self, Deserializer};
    use surrealdb::opt::RecordId;

    /// 将 `RecordId` 序列化为字符串
    pub fn serialize<S>(value: &RecordId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    /// 从字符串反序列化为 `RecordId`
    ///
    /// # Errors
    ///
    /// 当字符串无法解析为有效的 `RecordId` 时返回反序列化错误
    pub fn deserialize<'de, D>(deserializer: D) -> Result<RecordId, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::Deserialize as _;
        let s = String::deserialize(deserializer)?;
        s.parse::<RecordId>()
            .map_err(|e| de::Error::custom(format!("RecordId 解析失败: {e:?}")))
    }
}

#[cfg(feature = "db")]
/// SurrealDB Option<RecordId> 序列化/反序列化辅助模块
///
/// 将 `Option<RecordId>` 与 JSON 字符串/null 之间进行双向转换，
/// 处理新建实体 ID 为 None 的场景。
pub mod optional_record_id_serde {
    use serde::Serializer;
    use serde::de::{self, Deserializer};
    use surrealdb::opt::RecordId;

    #[allow(clippy::ref_option)]
    /// 将 `Option<RecordId>` 序列化为字符串或 null
    pub fn serialize<S>(value: &Option<RecordId>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(rid) => serializer.serialize_str(&rid.to_string()),
            None => serializer.serialize_none(),
        }
    }

    /// 从字符串或 null 反序列化为 `Option<RecordId>`
    ///
    /// # Errors
    ///
    /// 当字符串无法解析为有效的 `RecordId` 时返回反序列化错误
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<RecordId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::Deserialize as _;
        let opt = Option::<String>::deserialize(deserializer)?;
        opt.map_or_else(
            || Ok(None),
            |s| {
                s.parse::<RecordId>()
                    .map(Some)
                    .map_err(|e| de::Error::custom(format!("RecordId 解析失败: {e:?}")))
            },
        )
    }
}

// ============================================================================
// 枚举类型定义
// ============================================================================

/// 文档来源类型分类
///
/// 用于标识文档的原始格式，影响后续的解析策略选择。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceType {
    /// Markdown 格式文档（使用 comrak 解析）
    Markdown,
    /// 源代码文件（使用 tree-sitter 解析）
    Code,
    /// 纯文本文件（使用 icu_segmenter 分词）
    Plain,
}

/// 文档内容类型枚举
///
/// 用于标识文档内容的 MIME 类型，替代原先的 `String` 类型。
/// 在 CQRS 命令、事件和聚合根中统一使用此枚举，
/// 确保编译期类型安全，杜绝无效内容类型值。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentType {
    /// Markdown 格式（`text/markdown`）
    Markdown,
    /// 纯文本格式（`text/plain`）
    Plain,
    /// 源代码格式（`text/code`）
    Code,
    /// HTML 格式（`text/html`）
    Html,
    /// JSON 格式（`application/json`）
    Json,
}

impl ContentType {
    /// 从 MIME 类型字符串解析为 `ContentType`
    ///
    /// # Errors
    /// 当传入无法识别的 MIME 类型时返回错误消息
    pub fn from_mime(mime: &str) -> std::result::Result<Self, String> {
        match mime.to_lowercase().as_str() {
            "text/markdown" | "markdown" | "md" => Ok(Self::Markdown),
            "text/plain" | "plain" | "txt" => Ok(Self::Plain),
            "text/code" | "code" => Ok(Self::Code),
            "text/html" | "html" => Ok(Self::Html),
            "application/json" | "json" => Ok(Self::Json),
            other => Err(format!("无法识别的内容类型: {other}")),
        }
    }

    /// 转换为标准 MIME 类型字符串
    #[must_use]
    pub fn to_mime(&self) -> &'static str {
        match self {
            Self::Markdown => "text/markdown",
            Self::Plain => "text/plain",
            Self::Code => "text/code",
            Self::Html => "text/html",
            Self::Json => "application/json",
        }
    }
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_mime())
    }
}

/// 知识节点类型枚举
///
/// 用于标识知识图谱中节点的语义类型。
/// 在 CQRS 命令、事件和查询视图中统一使用此枚举，
/// 确保编译期类型安全。
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    Document,
    Block,
    Token,
    SemanticEntity,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Document => write!(f, "document"),
            Self::Block => write!(f, "block"),
            Self::Token => write!(f, "token"),
            Self::SemanticEntity => write!(f, "semantic_entity"),
        }
    }
}

/// 块类型分类
///
/// 表示文档逻辑分块的语义类型，决定如何提取和存储内容。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlockType {
    /// 标题块（Markdown # 或代码函数声明）
    Heading,
    /// 普通文本段落
    Paragraph,
    /// 代码块（`\`\`\`` 包裹或缩进代码）
    Code,
    /// 列表项（有序或无序）
    List,
    /// 表格（Markdown 表格语法）
    Table,
}

/// 词元（Token）类型分类
///
/// 标识词元的语言学或编程语言属性，用于精确检索和关系抽取。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TokenType {
    /// 自然语言词汇（中文词、英文单词）
    Word,
    /// 标点符号（句号、逗号、括号等）
    Punct,
    /// 特殊符号（@、#、$ 等）
    Symbol,
    /// 编程语言关键字（fn、let、if 等）
    Keyword,
    /// 标识符（变量名、函数名、类名）
    Identifier,
}

/// 引用关系类型
///
/// 定义知识图谱中边的语义类别，用于构建实体间的有向图。
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RefType {
    Definition,
    Usage,
    Link,
    Inherit,
    Implement,
    Constrain,
}

impl std::fmt::Display for RefType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Definition => write!(f, "definition"),
            Self::Usage => write!(f, "usage"),
            Self::Link => write!(f, "link"),
            Self::Inherit => write!(f, "inherit"),
            Self::Implement => write!(f, "implement"),
            Self::Constrain => write!(f, "constrain"),
        }
    }
}

/// 引用方向性
///
/// 控制边的遍历方向，优化查询性能。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Direction {
    /// 单向边（从 from → to）
    OneWay,
    /// 双向显式边（同时存在 to → from 的反向记录）
    TwoWay,
    /// 双向隐式边（单向存储，查询时自动反向匹配）
    ImplicitTwoWay,
}

/// 块生命周期状态（Axiom-7: 统一生命周期）
///
/// 描述 Block 从创建到归档的完整 FSM 生命周期。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockStatus {
    /// 已创建，尚未索引
    Created,
    /// 已索引，嵌入向量已生成
    Indexed,
    /// 已归档，不再参与检索
    Archived,
}

impl BlockStatus {
    /// 默认状态：Created
    #[must_use]
    pub const fn default_status() -> Self {
        Self::Created
    }

    /// 检查是否可以转换到目标状态
    #[must_use]
    pub fn can_transition_to(&self, target: &Self) -> bool {
        matches!(
            (self, target),
            (Self::Created, Self::Indexed | Self::Archived) | (Self::Indexed, Self::Archived)
        )
    }
}

/// 词元生命周期状态（Axiom-7: 统一生命周期）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenStatus {
    /// 已创建
    Created,
    /// 已索引
    Indexed,
    /// 已归档
    Archived,
}

impl TokenStatus {
    /// 默认状态：Created
    #[must_use]
    pub const fn default_status() -> Self {
        Self::Created
    }
}

/// 引用生命周期状态（Axiom-7: 统一生命周期）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceStatus {
    /// 已创建，尚未验证
    Created,
    /// 已验证，链接有效
    Verified,
    /// 链接失效
    Broken,
    /// 已归档
    Archived,
}

impl ReferenceStatus {
    /// 默认状态：Created
    #[must_use]
    pub const fn default_status() -> Self {
        Self::Created
    }

    /// 检查是否可以转换到目标状态
    #[must_use]
    pub fn can_transition_to(&self, target: &Self) -> bool {
        matches!(
            (self, target),
            (
                Self::Created,
                Self::Verified | Self::Broken | Self::Archived
            ) | (Self::Verified, Self::Broken | Self::Archived)
                | (Self::Broken, Self::Archived)
        )
    }
}

// ============================================================================
// L1: Document 结构体
// ============================================================================

/// 文档根节点（Document, L1）
///
/// 表示系统中的原始文件实体，是所有 Block 的父容器。
/// 每个文档通过 blake3 哈希实现幂等性去重。
///
/// # 示例
///
/// ```rust
/// use knowledge_core::model::{Document, SourceType};
///
/// let doc = Document {
///     id: None,
///     path: "/docs/api.md".to_string(),
///     title: "API 文档".to_string(),
///     source_type: SourceType::Markdown,
///     hash: "a1b2c3d4...".to_string(), // 64字符 hex
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    #[cfg(feature = "db")]
    #[serde(
        serialize_with = "optional_record_id_serde::serialize",
        deserialize_with = "optional_record_id_serde::deserialize",
        default
    )]
    /// 文档唯一标识符（数据库 RecordId，新建时为 None）
    pub id: Option<RecordIdType>,

    #[cfg(not(feature = "db"))]
    #[serde(default)]
    /// 文档唯一标识符（字符串形式，新建时为 None）
    pub id: Option<String>,

    /// 文件路径（如 `/docs/api.md`）
    pub path: String,

    /// 文档标题
    pub title: String,

    /// 来源类型（Markdown / Code / Plain）
    pub source_type: SourceType,

    /// blake3 哈希值（64 字符 hex），用于幂等性去重
    pub hash: String,
}

impl Document {
    /// 创建新的 Document 实例
    ///
    /// # 参数
    ///
    /// * `path` - 文件路径
    /// * `title` - 文档标题
    /// * `source_type` - 来源类型
    /// * `hash` - blake3 哈希值（必须为 64 字符 hex）
    ///
    /// # 错误
    ///
    /// 当哈希长度不为 64 时返回错误
    pub fn new(
        path: impl Into<String>,
        title: impl Into<String>,
        source_type: SourceType,
        hash: impl Into<String>,
    ) -> crate::Result<Self> {
        let hash = hash.into();
        if hash.len() != 64 {
            return Err(crate::error::helpers::parse_error(&format!(
                "哈希长度无效: 期望 64, 实际 {}",
                hash.len()
            )));
        }

        Ok(Self {
            #[cfg(feature = "db")]
            id: None,
            #[cfg(not(feature = "db"))]
            id: None,
            path: path.into(),
            title: title.into(),
            source_type,
            hash,
        })
    }

    /// 验证哈希格式是否合法
    pub fn is_valid_hash(&self) -> bool {
        self.hash.len() == 64 && self.hash.chars().all(|c| c.is_ascii_hexdigit())
    }
}

// ============================================================================
// L2: Block 结构体
// ============================================================================

/// 块容器节点（Block, L2）
///
/// 表示文档的逻辑分块单元，是 Document 的子节点、Token 的父容器。
/// 每个块包含可选的向量嵌入（embedding），用于语义相似度检索。
///
/// # 设计说明
///
/// - `start_line` / `end_line`：基于行的位置信息，用于源码定位
/// - `embedding`：2560 维浮点向量（GEMMA4-E4B 模型输出维度）
/// - `idempotency_key`：幂等键，防止重复插入相同块
///
/// # 示例
///
/// ```rust,ignore
/// use knowledge_core::model::{Block, BlockType};
/// use surrealdb::opt::RecordId;
///
/// let block = Block {
///     id: None,
///     doc_id: "document:abc123".parse::<RecordId>().unwrap(),
///     block_type: BlockType::Paragraph,
///     start_line: 10,
///     end_line: 25,
///     embedding: Some(vec![0.1; 2560]),
///     idempotency_key: Some("para_10_25".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    #[cfg(feature = "db")]
    #[serde(
        serialize_with = "optional_record_id_serde::serialize",
        deserialize_with = "optional_record_id_serde::deserialize",
        default
    )]
    /// 块唯一标识符（数据库 RecordId，新建时为 None）
    pub id: Option<RecordIdType>,

    #[cfg(not(feature = "db"))]
    #[serde(default)]
    /// 块唯一标识符（字符串形式，新建时为 None）
    pub id: Option<String>,

    #[cfg(feature = "db")]
    #[serde(
        rename = "document_id",
        serialize_with = "record_id_serde::serialize",
        deserialize_with = "record_id_serde::deserialize"
    )]
    /// 父文档 ID
    pub doc_id: RecordIdType,

    #[cfg(not(feature = "db"))]
    #[serde(rename = "document_id")]
    /// 父文档 ID
    pub doc_id: String,

    /// 块类型（Heading / Paragraph / Code / List / Table）
    pub block_type: BlockType,

    /// 起始行号（含）
    pub start_line: u32,

    /// 结束行号（含）
    pub end_line: u32,

    /// 幂等键，防止重复插入相同块
    pub idempotency_key: Option<String>,

    /// 块生命周期状态（Axiom-7: 统一生命周期）
    #[serde(default = "BlockStatus::default_status")]
    pub status: BlockStatus,
}

impl Block {
    /// 创建新的 Block 实例
    ///
    /// # 参数
    ///
    /// * `doc_id` - 父文档 ID
    /// * `block_type` - 块类型
    /// * `start_line` - 起始行号
    /// * `end_line` - 结束行号
    pub const fn new(
        #[cfg(feature = "db")] doc_id: RecordIdType,
        #[cfg(not(feature = "db"))] doc_id: String,
        block_type: BlockType,
        start_line: u32,
        end_line: u32,
    ) -> Self {
        Self {
            #[cfg(feature = "db")]
            id: None,
            #[cfg(not(feature = "db"))]
            id: None,
            doc_id,
            block_type,
            start_line,
            end_line,
            idempotency_key: None,
            status: BlockStatus::Created,
        }
    }

    /// 设置幂等性键
    #[must_use]
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// 获取块的行范围
    pub const fn line_range(&self) -> std::ops::RangeInclusive<u32> {
        self.start_line..=self.end_line
    }

    /// 验证行号合法性
    pub const fn is_valid_position(&self) -> bool {
        self.start_line <= self.end_line
    }
}

// ============================================================================
// BlockEmbedding: Block 的语义向量表示（概念-载体解耦）
// ============================================================================

/// 块语义向量嵌入（与 Block 解耦的独立实体）
///
/// 遵循关注点分离原则：Block 承载结构化文本信息（行号、类型、状态），
/// BlockEmbedding 承载语义向量信息（嵌入维度、向量数据）。
/// 两者通过 `block_id` 关联，生命周期独立管理。
///
/// # 设计动机
///
/// - Block 的结构信息在解析时确定后几乎不变
/// - Embedding 可能因模型升级、内容修改、缓存失效而频繁重算
/// - 序列化/反序列化 Block 时无需携带 ~6KB 的向量数据
/// - 维度不再硬编码，支持不同嵌入模型
///
/// # Example
///
/// ```
/// use knowledge_core::model::BlockEmbedding;
///
/// let embedding = BlockEmbedding::new("block_123".to_string(), vec![0.1; 768], 768)
///     .expect("维度匹配");
/// assert_eq!(embedding.dimension(), 768);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEmbedding {
    /// 关联的 Block ID
    pub block_id: String,
    /// 嵌入向量
    pub vector: Vec<f32>,
    /// 声明的向量维度
    pub declared_dimension: usize,
}

impl BlockEmbedding {
    /// 创建新的 BlockEmbedding
    ///
    /// # Errors
    ///
    /// 当向量实际长度与声明的维度不一致时返回验证错误。
    pub fn new(
        block_id: String,
        vector: Vec<f32>,
        declared_dimension: usize,
    ) -> crate::Result<Self> {
        if vector.len() != declared_dimension {
            return Err(crate::error::helpers::validation_error(
                &format!(
                    "嵌入向量维度不匹配：声明 {} 维，实际 {} 维",
                    declared_dimension,
                    vector.len()
                ),
                "BlockEmbedding::new",
            ));
        }
        Ok(Self {
            block_id,
            vector,
            declared_dimension,
        })
    }

    /// 获取向量维度
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.declared_dimension
    }

    /// 获取向量引用
    #[must_use]
    pub fn vector(&self) -> &[f32] {
        &self.vector
    }
}

// ============================================================================
// L3: Token 结构体
// ============================================================================

/// 词元叶子节点（Token, L3）
///
/// 表示最小的不可分割语义单元，是 Block 的子节点。
/// Token 保留精确的位置信息，支持源码级精确定位。
///
/// # 位置系统
///
/// - `start_char`：块内相对偏移（用于块内定位）
/// - `global_offset`：全局绝对偏移（用于跨块定位，使用 u64 防止大文件溢出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    #[cfg(feature = "db")]
    #[serde(
        serialize_with = "optional_record_id_serde::serialize",
        deserialize_with = "optional_record_id_serde::deserialize",
        default
    )]
    /// 词元唯一标识符（数据库 RecordId，新建时为 None）
    pub id: Option<RecordIdType>,

    #[cfg(not(feature = "db"))]
    #[serde(default)]
    /// 词元唯一标识符（字符串形式，新建时为 None）
    pub id: Option<String>,

    #[cfg(feature = "db")]
    #[serde(
        serialize_with = "record_id_serde::serialize",
        deserialize_with = "record_id_serde::deserialize"
    )]
    /// 父块 ID
    pub block_id: RecordIdType,

    #[cfg(not(feature = "db"))]
    /// 父块 ID
    pub block_id: String,

    /// 词元文本内容
    pub content: String,

    /// 词元类型（Word / Punct / Symbol / Keyword / Identifier）
    pub token_type: TokenType,

    /// 块内相对字符偏移
    pub start_char: u32,

    /// 全局绝对字节偏移（使用 u64 防止大文件溢出）
    pub global_offset: u64,

    /// 词元生命周期状态（Axiom-7: 统一生命周期）
    #[serde(default = "TokenStatus::default_status")]
    pub status: TokenStatus,
}

impl Token {
    /// 创建新的 Token 实例
    ///
    /// # 参数
    ///
    /// * `block_id` - 父块 ID
    /// * `content` - 文本内容
    /// * `token_type` - Token 类型
    /// * `start_char` - 块内偏移
    /// * `global_offset` - 全局偏移
    pub fn new(
        #[cfg(feature = "db")] block_id: RecordIdType,
        #[cfg(not(feature = "db"))] block_id: String,
        content: impl Into<String>,
        token_type: TokenType,
        start_char: u32,
        global_offset: u64,
    ) -> Self {
        Self {
            #[cfg(feature = "db")]
            id: None,
            #[cfg(not(feature = "db"))]
            id: None,
            block_id,
            content: content.into(),
            token_type,
            start_char,
            global_offset,
            status: TokenStatus::Created,
        }
    }

    /// 获取内容的字节长度
    pub fn byte_len(&self) -> usize {
        self.content.len()
    }

    /// 获取内容的字符长度
    pub fn char_len(&self) -> usize {
        self.content.chars().count()
    }
}

// ============================================================================
// LR: Reference 结构体
// ============================================================================

/// 引用关系边（Reference, LR）
///
/// 表示知识图谱中有向图的边，连接任意两个实体（Document/Block/Token）。
/// 使用 SurrealDB 的关系表（RELATION）特性实现图查询。
///
/// # 边的方向性
///
/// - `OneWay`：单向边，仅支持 from → to 查询
/// - `TwoWay`：双向显式边，需要插入两条记录
/// - `ImplicitTwoWay`：双向隐式边，单条记录但查询时反向匹配
///
/// # 示例
///
/// ```rust,ignore
/// use knowledge_core::model::{Reference, RefType, Direction};
/// use surrealdb::opt::RecordId;
///
/// let ref_edge = Reference {
///     id: None,
///     ref_type: RefType::Usage,
///     direction: Direction::OneWay,
///     scope: Some("function_scope".to_string()),
///     from_id: "token:abc".parse::<RecordId>().unwrap(),
///     to_id: "token:def".parse::<RecordId>().unwrap(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    #[cfg(feature = "db")]
    #[serde(
        serialize_with = "optional_record_id_serde::serialize",
        deserialize_with = "optional_record_id_serde::deserialize",
        default
    )]
    /// 引用唯一标识符（数据库 RecordId，新建时为 None）
    pub id: Option<RecordIdType>,

    #[cfg(not(feature = "db"))]
    #[serde(default)]
    /// 引用唯一标识符（字符串形式，新建时为 None）
    pub id: Option<String>,

    /// 引用关系类型
    pub ref_type: RefType,

    /// 边的方向性
    pub direction: Direction,

    /// 作用域（可选，用于限定引用的上下文）
    pub scope: Option<String>,

    #[cfg(feature = "db")]
    #[serde(
        rename = "source_id",
        serialize_with = "record_id_serde::serialize",
        deserialize_with = "record_id_serde::deserialize"
    )]
    /// 起点实体 ID
    pub from_id: RecordIdType,

    #[cfg(not(feature = "db"))]
    #[serde(rename = "source_id")]
    /// 起点实体 ID
    pub from_id: String,

    #[cfg(feature = "db")]
    #[serde(
        rename = "target_id",
        serialize_with = "record_id_serde::serialize",
        deserialize_with = "record_id_serde::deserialize"
    )]
    /// 终点实体 ID
    pub to_id: RecordIdType,

    #[cfg(not(feature = "db"))]
    #[serde(rename = "target_id")]
    /// 终点实体 ID
    pub to_id: String,

    /// 引用生命周期状态（Axiom-7: 统一生命周期）
    #[serde(default = "ReferenceStatus::default_status")]
    pub status: ReferenceStatus,
}

impl Reference {
    /// 创建新的 Reference 实例
    ///
    /// # 参数
    ///
    /// * `ref_type` - 引用类型
    /// * `direction` - 方向性
    /// * `from_id` - 起点 ID
    /// * `to_id` - 终点 ID
    pub const fn new(
        ref_type: RefType,
        direction: Direction,
        #[cfg(feature = "db")] from_id: RecordIdType,
        #[cfg(not(feature = "db"))] from_id: String,
        #[cfg(feature = "db")] to_id: RecordIdType,
        #[cfg(not(feature = "db"))] to_id: String,
    ) -> Self {
        Self {
            #[cfg(feature = "db")]
            id: None,
            #[cfg(not(feature = "db"))]
            id: None,
            ref_type,
            direction,
            scope: None,
            from_id,
            to_id,
            status: ReferenceStatus::Created,
        }
    }

    /// 设置作用域
    #[must_use]
    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// 判断是否为双向边
    pub const fn is_bidirectional(&self) -> bool {
        matches!(
            self.direction,
            Direction::TwoWay | Direction::ImplicitTwoWay
        )
    }
}

// ============================================================================
// L4: Community 结构体
// ============================================================================

/// 社区节点（Community, L4）
///
/// 表示知识图谱中基于语义内聚性自动发现的实体社区，
/// 是 L3 Token 层之上的高层抽象。
///
/// 社区由图聚类算法（如 Louvain）生成，`cohesion_score` 反映社区内部
/// 连接的紧密程度，`member_ids` 记录社区所包含的成员实体 ID 列表。
///
/// # 示例
///
/// ```rust,ignore
/// use knowledge_core::model::Community;
/// use surrealdb::opt::RecordId;
///
/// let community = Community {
///     id: None,
///     name: "Rust 异步运行时".to_string(),
///     cohesion_score: 0.87,
///     member_ids: vec!["token:abc".parse::<RecordId>().unwrap(), "token:def".parse::<RecordId>().unwrap()],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Community {
    #[cfg(feature = "db")]
    #[serde(
        serialize_with = "optional_record_id_serde::serialize",
        deserialize_with = "optional_record_id_serde::deserialize",
        default
    )]
    /// 社区唯一标识符（数据库 RecordId，新建时为 None）
    pub id: Option<RecordIdType>,

    #[cfg(not(feature = "db"))]
    #[serde(default)]
    /// 社区唯一标识符（字符串形式，新建时为 None）
    pub id: Option<String>,

    /// 社区名称
    pub name: String,

    /// 内聚性分数（0.0 ~ 1.0，越高表示社区内部连接越紧密）
    pub cohesion_score: f64,

    /// 社区成员实体 ID 列表
    pub member_ids: Vec<RecordIdType>,
}

// ============================================================================
// L4: Process 结构体
// ============================================================================

/// 流程节点（Process, L4）
///
/// 表示知识图谱中从入口到出口的有序执行路径，
/// 是 L3 Token 层之上的高层抽象。
///
/// `entry_point_id` 指向流程的起始实体（通常为 Token 或 Block），
/// 流程的具体步骤通过 `ProcessStep` 边记录。
///
/// # 图物理一致性
///
/// `entry_point_id` 使用 `RecordIdType` 类型，确保引用边建立在
/// 独立节点 ID 上，支持 SurrealDB 的 record 关系图遍历。
///
/// # 示例
///
/// ```rust,ignore
/// use knowledge_core::model::Process;
/// use surrealdb::opt::RecordId;
///
/// let process = Process {
///     id: None,
///     name: "HTTP 请求处理流程".to_string(),
///     entry_point_id: "token:handler".parse::<RecordId>().unwrap(),
///     description: Some("描述 HTTP 请求从接收到响应的完整处理链路".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Process {
    #[cfg(feature = "db")]
    #[serde(
        serialize_with = "optional_record_id_serde::serialize",
        deserialize_with = "optional_record_id_serde::deserialize",
        default
    )]
    /// 流程唯一标识符（数据库 RecordId，新建时为 None）
    pub id: Option<RecordIdType>,

    #[cfg(not(feature = "db"))]
    #[serde(default)]
    /// 流程唯一标识符（字符串形式，新建时为 None）
    pub id: Option<String>,

    /// 流程名称
    pub name: String,

    /// 入口实体 ID（通常指向 Token 或 Block）
    pub entry_point_id: RecordIdType,

    /// 流程描述（可选）
    pub description: Option<String>,
}

// ============================================================================
// L4: ProcessStep 边结构体
// ============================================================================

/// 流程步骤边（ProcessStep, L4）
///
/// 表示流程中某一步骤的有向边，连接 Process 与其包含的符号实体。
/// 通过 `step_order` 维护步骤的执行顺序，`confidence` 记录步骤推论的置信度。
///
/// # 设计说明
///
/// - `process_id`：关联的流程 ID（必填，使用 `record_id_serde` 序列化）
/// - `symbol_id`：步骤对应的符号实体 ID
/// - `step_order`：步骤在流程中的执行顺序（从 0 开始）
/// - `confidence`：步骤推论的置信度（0.0 ~ 1.0）
///
/// # 示例
///
/// ```rust,ignore
/// use knowledge_core::model::ProcessStep;
/// use surrealdb::opt::RecordId;
///
/// let step = ProcessStep {
///     id: None,
///     process_id: "process:req_flow".parse::<RecordId>().unwrap(),
///     symbol_id: "token:parse".parse::<RecordId>().unwrap(),
///     step_order: 1,
///     confidence: 0.95,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStep {
    #[cfg(feature = "db")]
    #[serde(
        serialize_with = "optional_record_id_serde::serialize",
        deserialize_with = "optional_record_id_serde::deserialize",
        default
    )]
    /// 步骤唯一标识符（数据库 RecordId，新建时为 None）
    pub id: Option<RecordIdType>,

    #[cfg(not(feature = "db"))]
    #[serde(default)]
    /// 步骤唯一标识符（字符串形式，新建时为 None）
    pub id: Option<String>,

    #[cfg(feature = "db")]
    #[serde(
        serialize_with = "record_id_serde::serialize",
        deserialize_with = "record_id_serde::deserialize"
    )]
    /// 关联的流程 ID
    pub process_id: RecordIdType,

    #[cfg(not(feature = "db"))]
    /// 关联的流程 ID
    pub process_id: String,

    #[cfg(feature = "db")]
    #[serde(
        serialize_with = "record_id_serde::serialize",
        deserialize_with = "record_id_serde::deserialize"
    )]
    /// 步骤对应的符号实体 ID
    pub symbol_id: RecordIdType,

    #[cfg(not(feature = "db"))]
    /// 步骤对应的符号实体 ID
    pub symbol_id: String,

    /// 步骤在流程中的执行顺序（从 0 开始）
    pub step_order: u32,

    /// 步骤推论的置信度（0.0 ~ 1.0）
    pub confidence: f64,
}

// ============================================================================
// 单元测试
// ============================================================================

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

    // -------------------------------------------------------------------------
    // 序列化/反序列化测试
    // -------------------------------------------------------------------------

    #[test]
    fn test_document_serialization() {
        let doc = Document {
            id: None,
            path: "/test.md".to_string(),
            title: "Test".to_string(),
            source_type: SourceType::Markdown,
            hash: "a".repeat(64),
        };

        let json = serde_json::to_string(&doc).expect("序列化失败");
        let de_doc: Document = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(doc.path, de_doc.path);
        assert_eq!(doc.source_type, de_doc.source_type);
        assert_eq!(doc.hash, de_doc.hash);
    }

    #[test]
    fn test_block_serialization() {
        let block = Block::new(rid("document:test"), BlockType::Code, 0, 100);

        let json = serde_json::to_string(&block).expect("序列化失败");
        let de_block: Block = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(block.block_type, de_block.block_type);
        assert_eq!(block.start_line, de_block.start_line);
    }

    #[test]
    fn test_block_embedding_creation() {
        let embedding =
            BlockEmbedding::new("block_123".to_string(), vec![0.5; 1536], 1536).expect("维度匹配");

        assert_eq!(embedding.dimension(), 1536);
        assert_eq!(embedding.vector().len(), 1536);
        assert_eq!(embedding.block_id, "block_123");
    }

    #[test]
    fn test_block_embedding_wrong_dim() {
        let result = BlockEmbedding::new("block_123".to_string(), vec![0.5; 100], 1536);

        assert!(result.is_err());
    }

    #[test]
    fn test_token_serialization() {
        let token = Token::new(rid("block:test"), "hello", TokenType::Word, 0, 1000);

        let json = serde_json::to_string(&token).expect("序列化失败");
        let de_token: Token = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(token.content, de_token.content);
        assert_eq!(token.token_type, de_token.token_type);
        assert_eq!(token.global_offset, de_token.global_offset);
    }

    #[test]
    fn test_reference_serialization() {
        let reference = Reference::new(
            RefType::Definition,
            Direction::TwoWay,
            rid("token:a"),
            rid("token:b"),
        )
        .with_scope("global");

        let json = serde_json::to_string(&reference).expect("序列化失败");
        let de_ref: Reference = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(reference.ref_type, de_ref.ref_type);
        assert_eq!(reference.scope, de_ref.scope);
        assert!(de_ref.is_bidirectional());
    }

    // -------------------------------------------------------------------------
    // 枚举序列化测试
    // -------------------------------------------------------------------------

    #[test]
    fn test_source_type_serialization() {
        let types = vec![SourceType::Markdown, SourceType::Code, SourceType::Plain];

        for t in &types {
            let json = serde_json::to_string(t).expect("序列化失败");
            let de_t: SourceType = serde_json::from_str(&json).expect("反序列化失败");
            assert_eq!(*t, de_t);
        }
    }

    #[test]
    fn test_ref_type_equality() {
        assert_eq!(RefType::Definition, RefType::Definition);
        assert_ne!(RefType::Usage, RefType::Link);
    }

    // -------------------------------------------------------------------------
    // 业务逻辑测试
    // -------------------------------------------------------------------------

    #[test]
    fn test_document_new_valid_hash() {
        let doc = Document::new("/test.md", "Test", SourceType::Markdown, "a".repeat(64));
        assert!(doc.is_ok());
        assert!(doc.unwrap().is_valid_hash());
    }

    #[test]
    fn test_document_new_invalid_hash_length() {
        let result = Document::new("/test.md", "Test", SourceType::Markdown, "short");
        assert!(result.is_err());
    }

    #[test]
    fn test_block_line_range() {
        let block = Block::new(rid("doc:x"), BlockType::Heading, 10, 20);

        assert_eq!(block.line_range(), 10..=20);
        assert!(block.is_valid_position());
    }

    #[test]
    fn test_block_invalid_position() {
        let block = Block::new(rid("doc:x"), BlockType::Paragraph, 20, 10);

        assert!(!block.is_valid_position());
    }

    #[test]
    fn test_token_lengths() {
        let token = Token::new(rid("block:x"), "你好世界", TokenType::Word, 0, 100);

        // UTF-8 中文字符每个占 3 字节
        assert_eq!(token.byte_len(), 12);
        assert_eq!(token.char_len(), 4);
    }

    #[test]
    fn test_reference_bidirectional_check() {
        let one_way = Reference::new(RefType::Link, Direction::OneWay, rid("a"), rid("b"));
        assert!(!one_way.is_bidirectional());

        let two_way = Reference::new(RefType::Inherit, Direction::TwoWay, rid("a"), rid("b"));
        assert!(two_way.is_bidirectional());
    }

    #[test]
    fn test_content_type_from_mime() {
        assert_eq!(
            ContentType::from_mime("text/markdown").unwrap(),
            ContentType::Markdown
        );
        assert_eq!(
            ContentType::from_mime("markdown").unwrap(),
            ContentType::Markdown
        );
        assert_eq!(ContentType::from_mime("md").unwrap(), ContentType::Markdown);
        assert_eq!(
            ContentType::from_mime("text/plain").unwrap(),
            ContentType::Plain
        );
        assert_eq!(ContentType::from_mime("plain").unwrap(), ContentType::Plain);
        assert_eq!(ContentType::from_mime("txt").unwrap(), ContentType::Plain);
        assert_eq!(
            ContentType::from_mime("text/code").unwrap(),
            ContentType::Code
        );
        assert_eq!(ContentType::from_mime("code").unwrap(), ContentType::Code);
        assert_eq!(
            ContentType::from_mime("text/html").unwrap(),
            ContentType::Html
        );
        assert_eq!(ContentType::from_mime("html").unwrap(), ContentType::Html);
        assert_eq!(
            ContentType::from_mime("application/json").unwrap(),
            ContentType::Json
        );
        assert_eq!(ContentType::from_mime("json").unwrap(), ContentType::Json);
        assert!(ContentType::from_mime("unknown/type").is_err());
    }

    #[test]
    fn test_content_type_to_mime() {
        assert_eq!(ContentType::Markdown.to_mime(), "text/markdown");
        assert_eq!(ContentType::Plain.to_mime(), "text/plain");
        assert_eq!(ContentType::Code.to_mime(), "text/code");
        assert_eq!(ContentType::Html.to_mime(), "text/html");
        assert_eq!(ContentType::Json.to_mime(), "application/json");
    }

    #[test]
    fn test_content_type_display() {
        assert_eq!(format!("{}", ContentType::Markdown), "text/markdown");
        assert_eq!(format!("{}", ContentType::Json), "application/json");
    }

    #[test]
    fn test_block_status_transitions() {
        assert!(BlockStatus::Created.can_transition_to(&BlockStatus::Indexed));
        assert!(BlockStatus::Created.can_transition_to(&BlockStatus::Archived));
        assert!(BlockStatus::Indexed.can_transition_to(&BlockStatus::Archived));
        assert!(!BlockStatus::Indexed.can_transition_to(&BlockStatus::Created));
        assert!(!BlockStatus::Archived.can_transition_to(&BlockStatus::Created));
        assert!(!BlockStatus::Archived.can_transition_to(&BlockStatus::Indexed));
    }

    #[test]
    fn test_block_status_default() {
        assert_eq!(BlockStatus::default_status(), BlockStatus::Created);
    }

    #[test]
    fn test_reference_status_transitions() {
        assert!(ReferenceStatus::Created.can_transition_to(&ReferenceStatus::Verified));
        assert!(ReferenceStatus::Created.can_transition_to(&ReferenceStatus::Broken));
        assert!(ReferenceStatus::Created.can_transition_to(&ReferenceStatus::Archived));
        assert!(ReferenceStatus::Verified.can_transition_to(&ReferenceStatus::Broken));
        assert!(ReferenceStatus::Verified.can_transition_to(&ReferenceStatus::Archived));
        assert!(ReferenceStatus::Broken.can_transition_to(&ReferenceStatus::Archived));
        assert!(!ReferenceStatus::Verified.can_transition_to(&ReferenceStatus::Created));
        assert!(!ReferenceStatus::Archived.can_transition_to(&ReferenceStatus::Created));
    }

    #[test]
    fn test_reference_status_default() {
        assert_eq!(ReferenceStatus::default_status(), ReferenceStatus::Created);
    }

    #[test]
    fn test_token_status_default() {
        assert_eq!(TokenStatus::default_status(), TokenStatus::Created);
    }

    #[test]
    fn test_block_with_idempotency_key() {
        let block = Block::new(rid("doc:x"), BlockType::Code, 1, 10)
            .with_idempotency_key("code_block_1_10");
        assert_eq!(block.idempotency_key, Some("code_block_1_10".to_string()));
    }

    #[test]
    fn test_reference_with_scope() {
        let reference = Reference::new(
            RefType::Usage,
            Direction::OneWay,
            rid("token:a"),
            rid("token:b"),
        )
        .with_scope("function_scope");
        assert_eq!(reference.scope, Some("function_scope".to_string()));
    }

    #[test]
    fn test_document_is_valid_hash() {
        let doc = Document {
            id: None,
            path: "/test.md".to_string(),
            title: "Test".to_string(),
            source_type: SourceType::Markdown,
            hash: "a".repeat(64),
        };
        assert!(doc.is_valid_hash());

        let invalid_doc = Document {
            id: None,
            path: "/test.md".to_string(),
            title: "Test".to_string(),
            source_type: SourceType::Markdown,
            hash: "not-hex-chars!@#".to_string(),
        };
        assert!(!invalid_doc.is_valid_hash());
    }

    #[test]
    fn test_implicit_two_way_is_bidirectional() {
        let reference =
            Reference::new(RefType::Link, Direction::ImplicitTwoWay, rid("a"), rid("b"));
        assert!(reference.is_bidirectional());
    }
}
