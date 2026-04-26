use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Command trait - 表示写操作的意图
///
/// 所有命令（Command）必须实现此 trait，用于 CQRS 模式中的写操作。
/// 命令是意图性的操作请求，会触发聚合根产生领域事件（Event）。
///
/// # 类型参数
///
/// - `Result`: Command 执行成功后的返回值类型
///
/// # 示例
///
/// ```rust
/// use knowledge_core::cqrs::Command;
/// use uuid::Uuid;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Debug, Serialize, Deserialize)]
/// struct MyCommand {
///     command_id: Uuid,
///     aggregate_id: String,
/// }
///
/// impl Command for MyCommand {
///     type Result = String;
///
///     fn command_id(&self) -> Uuid { self.command_id }
///     fn aggregate_id(&self) -> &str { &self.aggregate_id }
///     fn expected_version(&self) -> Option<u64> { None }
/// }
/// ```
pub trait Command: Send + Sync + Serialize + Deserialize<'static> + std::fmt::Debug {
    /// Command 执行成功后的返回值类型
    type Result: Send + Sync;

    /// 返回命令的唯一标识符
    ///
    /// 用于追踪和审计，每个命令必须有唯一 ID
    fn command_id(&self) -> Uuid;

    /// 返回目标聚合根 ID
    ///
    /// 标识此命令作用于哪个聚合根实体
    fn aggregate_id(&self) -> &str;

    /// 返回命令发生的预期版本（用于乐观锁并发控制）
    ///
    /// - `Some(version)`: 期望聚合根当前版本为此值，否则拒绝执行
    /// - `None`: 不检查版本（适用于新创建的聚合根）
    fn expected_version(&self) -> Option<u64>;
}

/// 文档创建命令的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCreatedResult {
    /// 新创建的文档 ID
    pub document_id: String,
    /// 创建后的聚合根版本号
    pub version: u64,
    /// 命令执行时间戳
    pub timestamp: DateTime<Utc>,
}

/// 文档更新命令的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentUpdatedResult {
    /// 被更新的文档 ID
    pub document_id: String,
    /// 更新后的聚合根版本号
    pub version: u64,
    /// 更新前的聚合根版本号
    pub previous_version: u64,
    /// 命令执行时间戳
    pub timestamp: DateTime<Utc>,
}

/// 文档删除命令的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDeletedResult {
    /// 被删除的文档 ID
    pub document_id: String,
    /// 删除时的最终版本号
    pub final_version: u64,
    /// 命令执行时间戳
    pub timestamp: DateTime<Utc>,
}

/// 节点创建命令的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCreatedResult {
    /// 新创建的节点 ID
    pub node_id: String,
    /// 所属文档 ID
    pub document_id: String,
    /// 创建后的聚合根版本号
    pub version: u64,
    /// 命令执行时间戳
    pub timestamp: DateTime<Utc>,
}

/// 节点关联命令的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodesLinkedResult {
    /// 源节点 ID
    pub source_node_id: String,
    /// 目标节点 ID
    pub target_node_id: String,
    /// 关联类型（如 Usage、Definition）
    pub relation_type: String,
    /// 关联后的聚合根版本号
    pub version: u64,
    /// 命令执行时间戳
    pub timestamp: DateTime<Utc>,
}

/// 文件导入命令的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIngestedResult {
    /// 导入后创建的文档 ID
    pub document_id: String,
    /// 原始文件路径
    pub file_path: String,
    /// 解析产生的块数量
    pub blocks_count: usize,
    /// 解析产生的词元数量
    pub tokens_count: usize,
    /// 命令执行时间戳
    pub timestamp: DateTime<Utc>,
}

/// 重建索引命令的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReindexResult {
    /// 已处理的文档数量
    pub documents_processed: usize,
    /// 重建耗时（毫秒）
    pub duration_ms: u64,
    /// 命令执行时间戳
    pub timestamp: DateTime<Utc>,
}

/// 创建文档命令
///
/// 请求创建一个新的文档聚合根，包含标题、内容和元数据。
#[derive(Serialize, Deserialize)]
pub struct CreateDocumentCommand {
    /// 命令唯一标识符
    #[serde(rename = "commandId")]
    pub command_id: Uuid,
    /// 目标聚合根 ID
    #[serde(rename = "aggregateId")]
    pub aggregate_id: String,
    /// 文档标题
    pub title: String,
    /// 文档内容（Markdown 或纯文本）
    pub content: String,
    /// 内容 MIME 类型（如 `text/markdown`、`text/plain`）
    pub content_type: String,
    /// 附加元数据（键值对）
    pub metadata: serde_json::Value,
    /// 预期版本号（乐观锁）
    #[serde(rename = "expectedVersion")]
    pub expected_version: Option<u64>,
}

impl Command for CreateDocumentCommand {
    type Result = DocumentCreatedResult;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }

    fn expected_version(&self) -> Option<u64> {
        self.expected_version
    }
}

impl std::fmt::Debug for CreateDocumentCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateDocumentCommand")
            .field("command_id", &self.command_id)
            .field("aggregate_id", &self.aggregate_id)
            .field("title", &self.title)
            .field("content", &format!("{} chars", self.content.len()))
            .field("content_type", &self.content_type)
            .field("metadata", &self.metadata)
            .field("expected_version", &self.expected_version)
            .finish()
    }
}

/// 更新文档命令
///
/// 对已有文档进行部分更新，所有字段均为可选（补丁语义）。
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateDocumentCommand {
    /// 命令唯一标识符
    #[serde(rename = "commandId")]
    pub command_id: Uuid,
    /// 目标聚合根 ID
    #[serde(rename = "aggregateId")]
    pub aggregate_id: String,
    /// 新标题（None 表示不修改）
    pub title: Option<String>,
    /// 新内容（None 表示不修改）
    pub content: Option<String>,
    /// 新内容类型（None 表示不修改）
    pub content_type: Option<String>,
    /// 新元数据（None 表示不修改）
    pub metadata: Option<serde_json::Value>,
    /// 预期版本号（乐观锁）
    #[serde(rename = "expectedVersion")]
    pub expected_version: Option<u64>,
}

impl Command for UpdateDocumentCommand {
    type Result = DocumentUpdatedResult;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }

    fn expected_version(&self) -> Option<u64> {
        self.expected_version
    }
}

/// 删除文档命令
///
/// 请求归档（软删除）指定的文档聚合根。
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteDocumentCommand {
    /// 命令唯一标识符
    #[serde(rename = "commandId")]
    pub command_id: Uuid,
    /// 目标聚合根 ID
    #[serde(rename = "aggregateId")]
    pub aggregate_id: String,
    /// 预期版本号（乐观锁）
    #[serde(rename = "expectedVersion")]
    pub expected_version: Option<u64>,
}

impl Command for DeleteDocumentCommand {
    type Result = DocumentDeletedResult;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }

    fn expected_version(&self) -> Option<u64> {
        self.expected_version
    }
}

/// 创建节点命令
///
/// 在指定文档下创建一个新的知识节点（Block 或 Token）。
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateNodeCommand {
    /// 命令唯一标识符
    #[serde(rename = "commandId")]
    pub command_id: Uuid,
    /// 目标聚合根 ID
    #[serde(rename = "aggregateId")]
    pub aggregate_id: String,
    /// 所属文档 ID
    pub document_id: String,
    /// 节点类型（如 `Heading`、`Paragraph`、`Code`）
    pub node_type: String,
    /// 节点文本内容
    pub content: String,
    /// 节点在源文件中的位置信息
    pub position: Option<NodePosition>,
    /// 附加元数据
    pub metadata: serde_json::Value,
    /// 预期版本号（乐观锁）
    #[serde(rename = "expectedVersion")]
    pub expected_version: Option<u64>,
}

/// 节点在源文件中的行列位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePosition {
    /// 起始行号（0-based）
    pub start_line: u32,
    /// 结束行号（含）
    pub end_line: u32,
    /// 起始字符偏移（行内）
    pub start_char: u32,
    /// 结束字符偏移（行内，不含）
    pub end_char: u32,
}

impl Command for CreateNodeCommand {
    type Result = NodeCreatedResult;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }

    fn expected_version(&self) -> Option<u64> {
        self.expected_version
    }
}

/// 关联节点命令
///
/// 在两个节点之间建立有向引用关系（边）。
#[derive(Debug, Serialize, Deserialize)]
pub struct LinkNodesCommand {
    /// 命令唯一标识符
    #[serde(rename = "commandId")]
    pub command_id: Uuid,
    /// 目标聚合根 ID
    #[serde(rename = "aggregateId")]
    pub aggregate_id: String,
    /// 源节点 ID（边的起点）
    pub source_node_id: String,
    /// 目标节点 ID（边的终点）
    pub target_node_id: String,
    /// 关联类型（如 `Usage`、`Definition`、`Link`）
    pub relation_type: String,
    /// 方向性（`OneWay`、`TwoWay`、`ImplicitTwoWay`）
    pub direction: String,
    /// 附加元数据
    pub metadata: serde_json::Value,
    /// 预期版本号（乐观锁）
    #[serde(rename = "expectedVersion")]
    pub expected_version: Option<u64>,
}

impl Command for LinkNodesCommand {
    type Result = NodesLinkedResult;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }

    fn expected_version(&self) -> Option<u64> {
        self.expected_version
    }
}

/// 文件导入命令
///
/// 将外部文件解析为知识图谱结构（Document → Block → Token → Reference）。
#[derive(Debug, Serialize, Deserialize)]
pub struct IngestFileCommand {
    /// 命令唯一标识符
    #[serde(rename = "commandId")]
    pub command_id: Uuid,
    /// 目标聚合根 ID
    #[serde(rename = "aggregateId")]
    pub aggregate_id: String,
    /// 待导入文件的路径
    pub file_path: String,
    /// 文件来源类型（`markdown`、`code`、`plain`）
    pub source_type: String,
    /// 导入选项（分块大小、是否提取引用等）
    pub options: IngestOptions,
    /// 预期版本号（乐观锁）
    #[serde(rename = "expectedVersion")]
    pub expected_version: Option<u64>,
}

/// 文件导入选项
///
/// 控制解析器的行为，包括分块策略和功能开关。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestOptions {
    /// 分块大小（字符数），None 使用解析器默认值
    pub chunk_size: Option<usize>,
    /// 分块重叠大小（字符数），None 使用解析器默认值
    pub overlap: Option<usize>,
    /// 是否提取节点间的引用关系
    pub extract_references: bool,
    /// 是否生成向量嵌入（需要嵌入模型）
    pub generate_embeddings: bool,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            chunk_size: Some(1024),
            overlap: Some(128),
            extract_references: true,
            generate_embeddings: false,
        }
    }
}

impl Command for IngestFileCommand {
    type Result = FileIngestedResult;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }

    fn expected_version(&self) -> Option<u64> {
        self.expected_version
    }
}

/// 重建索引命令
///
/// 对指定范围内的文档重新执行索引构建流程。
#[derive(Debug, Serialize, Deserialize)]
pub struct ReindexCommand {
    /// 命令唯一标识符
    #[serde(rename = "commandId")]
    pub command_id: Uuid,
    /// 重建索引的范围
    pub scope: ReindexScope,
    /// 是否强制全量重建（忽略增量检测）
    pub force_full: bool,
}

/// 重建索引的范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReindexScope {
    /// 全量重建所有文档
    All,
    /// 仅重建指定 ID 列表中的文档
    Documents(Vec<String>),
    /// 重建指定时间之后变更的文档
    Since(DateTime<Utc>),
}

impl Command for ReindexCommand {
    type Result = ReindexResult;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    #[allow(clippy::unnecessary_literal_bound)]
    fn aggregate_id(&self) -> &str {
        "system"
    }

    fn expected_version(&self) -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_command() -> CreateDocumentCommand {
        CreateDocumentCommand {
            command_id: Uuid::new_v4(),
            aggregate_id: "doc_001".to_string(),
            title: "Test Document".to_string(),
            content: "# Hello World\n\nThis is test content.".to_string(),
            content_type: "markdown".to_string(),
            metadata: serde_json::json!({"author": "test_user"}),
            expected_version: None,
        }
    }

    #[test]
    fn test_command_trait_implementation() {
        let cmd = create_test_command();

        assert_eq!(cmd.aggregate_id(), "doc_001");
        assert!(cmd.expected_version().is_none());
        assert_ne!(cmd.command_id(), Uuid::nil());
    }

    #[test]
    fn test_create_document_command_serialization() {
        let cmd = create_test_command();
        let json = serde_json::to_string(&cmd).expect("序列化失败");
        let deserialized: CreateDocumentCommand =
            serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(deserialized.title, cmd.title);
        assert_eq!(deserialized.content_type, cmd.content_type);
        assert_eq!(deserialized.aggregate_id, cmd.aggregate_id);
    }

    #[test]
    fn test_update_document_command_with_optional_fields() {
        let cmd = UpdateDocumentCommand {
            command_id: Uuid::new_v4(),
            aggregate_id: "doc_002".to_string(),
            title: Some("Updated Title".to_string()),
            content: None,
            content_type: None,
            metadata: None,
            expected_version: Some(5),
        };

        assert_eq!(cmd.expected_version(), Some(5));
        assert!(cmd.title.is_some());
        assert!(cmd.content.is_none());
    }

    #[test]
    fn test_delete_document_command_minimal() {
        let cmd = DeleteDocumentCommand {
            command_id: Uuid::new_v4(),
            aggregate_id: "doc_003".to_string(),
            expected_version: Some(10),
        };

        assert_eq!(cmd.expected_version(), Some(10));
    }

    #[test]
    fn test_ingest_file_command_with_options() {
        let options = IngestOptions::default();
        let cmd = IngestFileCommand {
            command_id: Uuid::new_v4(),
            aggregate_id: "doc_004".to_string(),
            file_path: "/path/to/file.md".to_string(),
            source_type: "markdown".to_string(),
            options,
            expected_version: None,
        };

        assert!(!cmd.options.generate_embeddings);
        assert!(cmd.options.extract_references);
    }

    #[test]
    fn test_reindex_command_scopes() {
        let all_scope = ReindexCommand {
            command_id: Uuid::new_v4(),
            scope: ReindexScope::All,
            force_full: false,
        };
        assert_eq!(all_scope.aggregate_id(), "system");
        assert!(all_scope.expected_version().is_none());

        let docs_scope = ReindexCommand {
            command_id: Uuid::new_v4(),
            scope: ReindexScope::Documents(vec!["doc1".to_string(), "doc2".to_string()]),
            force_full: true,
        };
        match docs_scope.scope {
            ReindexScope::Documents(ids) => assert_eq!(ids.len(), 2),
            _ => panic!("Expected Documents scope"),
        }
    }

    #[test]
    fn test_link_nodes_command_directions() {
        let cmd = LinkNodesCommand {
            command_id: Uuid::new_v4(),
            aggregate_id: "graph_001".to_string(),
            source_node_id: "node_a".to_string(),
            target_node_id: "node_b".to_string(),
            relation_type: "Usage".to_string(),
            direction: "OneWay".to_string(),
            metadata: serde_json::json!({}),
            expected_version: None,
        };

        assert_eq!(cmd.relation_type, "Usage");
        assert_eq!(cmd.direction, "OneWay");
    }

    #[tokio::test]
    async fn test_command_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<CreateDocumentCommand>();
        assert_send_sync::<UpdateDocumentCommand>();
        assert_send_sync::<DeleteDocumentCommand>();
        assert_send_sync::<CreateNodeCommand>();
        assert_send_sync::<LinkNodesCommand>();
        assert_send_sync::<IngestFileCommand>();
        assert_send_sync::<ReindexCommand>();
    }
}
