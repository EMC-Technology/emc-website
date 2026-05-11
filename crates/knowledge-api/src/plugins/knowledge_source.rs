use async_trait::async_trait;
use chrono::{DateTime, Utc};
use knowledge_core::model::SourceType;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio_stream::Stream;

use crate::Result;

/// 知识源插件 — 闭源模块的"插座"
///
/// 开源默认实现：[`LocalFileSource`](crate::plugins::defaults::LocalFileSource)（读取本地文件）
/// 闭源增强实现：`GitKnowledgeSource`（Git 仓库增量索引）
///
/// # 架构角色
///
/// 在插件化架构中，`KnowledgeSource` 是"插座"（trait 定义），
/// 闭源模块制造"插头"（trait 实现），运行时由 [`PluginRegistry`](crate::plugins::registry::PluginRegistry) 组装。
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::KnowledgeSource;
/// use std::sync::Arc;
///
/// let source: Arc<dyn KnowledgeSource> = Arc::new(MySource::new());
/// registry.register_source(source);
/// ```
#[async_trait]
pub trait KnowledgeSource: Send + Sync {
    /// 知识源唯一标识
    fn source_id(&self) -> &str;

    /// 获取文档列表
    async fn list_documents(&self) -> Result<Vec<SourceDocument>>;

    /// 获取单个文档内容
    async fn fetch_document(&self, doc_id: &str) -> Result<SourceDocument>;

    /// 监听变更流（增量索引的核心）
    ///
    /// 返回一个异步流，持续产出知识源的变更事件。
    /// 闭源 `GitKnowledgeSource` 通过 Git Hook 实现此方法，
    /// 构成 L4 质量保障闭环的起点。
    async fn watch_changes(&self) -> Result<Pin<Box<dyn Stream<Item = SourceChange> + Send>>>;

    /// 健康检查
    async fn health_check(&self) -> Result<bool>;
}

/// 知识源文档
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDocument {
    /// 文档唯一标识
    pub id: String,
    /// 文档标题
    pub title: String,
    /// 知识源类型标识
    pub source_type: SourceType,
    /// 文档原始内容
    pub content: Vec<u8>,
    /// 附加元数据
    pub metadata: serde_json::Value,
    /// 内容哈希（用于增量索引判断）
    pub hash: String,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
}

/// 知识源变更事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceChange {
    /// 变更类型
    pub change_type: ChangeType,
    /// 受影响的文档 ID
    pub document_id: String,
    /// 变更时间戳
    pub timestamp: DateTime<Utc>,
    /// 变更附加元数据
    pub metadata: serde_json::Value,
}

/// 变更类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// 新建文档
    Created,
    /// 修改文档
    Modified,
    /// 删除文档
    Deleted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_document_serialization() {
        let doc = SourceDocument {
            id: "doc1".to_string(),
            title: "Test Doc".to_string(),
            source_type: SourceType::Markdown,
            content: b"hello".to_vec(),
            metadata: serde_json::json!({"key": "value"}),
            hash: "abc123".to_string(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&doc).unwrap();
        let de: SourceDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(de.id, "doc1");
        assert_eq!(de.content, b"hello".to_vec());
    }

    #[test]
    fn test_source_change_serialization() {
        let change = SourceChange {
            change_type: ChangeType::Modified,
            document_id: "doc1".to_string(),
            timestamp: Utc::now(),
            metadata: serde_json::json!({}),
        };
        let json = serde_json::to_string(&change).unwrap();
        let de: SourceChange = serde_json::from_str(&json).unwrap();
        assert_eq!(de.change_type, ChangeType::Modified);
        assert_eq!(de.document_id, "doc1");
    }

    #[test]
    fn test_change_type_serialization_roundtrip() {
        let types = [
            ChangeType::Created,
            ChangeType::Modified,
            ChangeType::Deleted,
        ];
        for t in &types {
            let json = serde_json::to_string(t).unwrap();
            let de: ChangeType = serde_json::from_str(&json).unwrap();
            assert_eq!(*t, de);
        }
    }
}
