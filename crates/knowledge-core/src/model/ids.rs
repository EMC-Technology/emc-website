//! 领域实体类型化 ID 定义
//!
//! 遵循 Axiom-3（ID 语义明确性），为每个领域实体定义独立的 ID 类型，
//! 在编译期杜绝不同实体 ID 的误用。
//!
//! # 设计原则
//!
//! - 所有 ID 均为 newtype 包装，实现 `From<String>`/`From<&str>`/`AsRef<str>`/`Display`
//! - ID 内部使用 `Arc<str>` 存储以避免克隆开销
//! - 启用 `serde` feature 时支持序列化/反序列化

use std::fmt;
use std::sync::Arc;

macro_rules! define_typed_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        pub struct $name(Arc<str>);

        impl $name {
            /// 从字符串创建 ID
            #[must_use]
            pub fn new(id: impl AsRef<str>) -> Self {
                Self(Arc::from(id.as_ref()))
            }

            /// 获取 ID 的字符串引用
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(Arc::from(s.as_str()))
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(Arc::from(s))
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.0.as_ref().cmp(other.0.as_ref())
            }
        }
    };
}

define_typed_id!(DocumentId, "文档唯一标识符");
define_typed_id!(BlockId, "文档分块唯一标识符");
define_typed_id!(NodeId, "知识节点唯一标识符");
define_typed_id!(EdgeId, "知识边唯一标识符");
define_typed_id!(TokenId, "词法 Token 唯一标识符");
define_typed_id!(ReferenceId, "引用关系唯一标识符");
define_typed_id!(UserId, "用户唯一标识符");
define_typed_id!(PrincipalId, "授权主体唯一标识符");
define_typed_id!(ResourceId, "授权资源唯一标识符");
define_typed_id!(ScopeId, "授权范围唯一标识符");
define_typed_id!(AggregateId, "CQRS 聚合根唯一标识符");
define_typed_id!(EventId, "领域事件唯一标识符");
define_typed_id!(
    CorrelationId,
    "事件关联 ID，用于追踪同一业务流程中的多个事件"
);
define_typed_id!(CausationId, "事件因果 ID，标识触发当前事件的父事件");
define_typed_id!(TraceId, "分布式追踪 ID");
define_typed_id!(ProcessId, "代码分析进程唯一标识符");
define_typed_id!(ProcessStepId, "代码分析步骤唯一标识符");
define_typed_id!(CommunityId, "社区发现结果唯一标识符");
define_typed_id!(SessionId, "MCP 会话唯一标识符");
define_typed_id!(WorkflowId, "Agent 工作流唯一标识符");
define_typed_id!(ActivityId, "ULLM 活动唯一标识符");
define_typed_id!(ProviderDefId, "LLM 提供商定义唯一标识符");
define_typed_id!(ProviderInstId, "LLM 提供商实例唯一标识符");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_id_from_string() {
        let id: DocumentId = "doc:123".to_string().into();
        assert_eq!(id.as_str(), "doc:123");
    }

    #[test]
    fn test_document_id_from_str() {
        let id: DocumentId = "doc:456".into();
        assert_eq!(id.as_str(), "doc:456");
    }

    #[test]
    fn test_document_id_as_ref_str() {
        let id = DocumentId::new("doc:789");
        let ref_str: &str = id.as_ref();
        assert_eq!(ref_str, "doc:789");
    }

    #[test]
    fn test_document_id_borrow_str() {
        let id = DocumentId::new("doc:abc");
        let borrowed: &str = std::borrow::Borrow::borrow(&id);
        assert_eq!(borrowed, "doc:abc");
    }

    #[test]
    fn test_document_id_display() {
        let id = DocumentId::new("doc:xyz");
        assert_eq!(format!("{id}"), "doc:xyz");
    }

    #[test]
    fn test_typed_id_ordering() {
        let a = DocumentId::new("aaa");
        let b = DocumentId::new("bbb");
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_typed_id_partial_ord() {
        let a = BlockId::new("block:1");
        let b = BlockId::new("block:2");
        assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Less));
    }

    #[test]
    fn test_typed_id_equality() {
        let id1 = NodeId::new("node:1");
        let id2 = NodeId::new("node:1");
        let id3 = NodeId::new("node:2");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_typed_id_hash_in_hashmap() {
        use std::collections::HashMap;
        let mut map: HashMap<DocumentId, String> = HashMap::new();
        let id = DocumentId::new("doc:1");
        map.insert(id.clone(), "test".to_string());
        assert_eq!(map.get(&id), Some(&"test".to_string()));
    }

    #[test]
    fn test_typed_id_serde_roundtrip() {
        let id = EdgeId::new("edge:abc");
        let json = serde_json::to_string(&id).unwrap();
        let de: EdgeId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, de);
    }

    #[test]
    fn test_multiple_id_types_are_distinct() {
        let doc_id = DocumentId::new("same_value");
        let block_id = BlockId::new("same_value");
        assert_eq!(doc_id.as_str(), block_id.as_str());
    }
}
