use async_trait::async_trait;
use std::pin::Pin;
use tokio_stream::Stream;

use super::agent_backend::{
    AgentBackend, AgentCapabilities, AgentChunk, AgentChunkType, AgentResult, AgentTask,
};
use super::knowledge_source::{KnowledgeSource, SourceChange, SourceDocument};
use super::quality_gate::{CodeChange, QualityGatePlugin, QualityVerdict, RuleInfo};
use crate::Result;

/// 本地文件知识源（开源默认实现）
///
/// 遍历本地目录中的文件，将其作为知识源文档提供。
/// 适用于单机部署和开发测试场景。
pub struct LocalFileSource {
    base_path: String,
}

impl LocalFileSource {
    /// 创建默认的本地文件知识源（当前目录）
    #[must_use]
    pub fn new() -> Self {
        Self {
            base_path: ".".to_string(),
        }
    }

    /// 创建指定根路径的本地文件知识源
    #[must_use]
    pub fn with_path(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
        }
    }
}

impl Default for LocalFileSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KnowledgeSource for LocalFileSource {
    fn source_id(&self) -> &'static str {
        "local-file"
    }

    async fn list_documents(&self) -> Result<Vec<SourceDocument>> {
        let mut entries = tokio::fs::read_dir(&self.base_path)
            .await
            .map_err(|e| error_core::helpers::io_error(&format!("读取目录失败: {e}")))?;

        let mut documents = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| error_core::helpers::io_error(&format!("遍历目录失败: {e}")))?
        {
            let path = entry.path();
            if path.is_file() {
                let metadata = entry.metadata().await.map_err(|e| {
                    error_core::helpers::io_error(&format!("读取文件元数据失败: {e}"))
                })?;

                let id = path.to_string_lossy().to_string();
                let title = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .and_then(|d| {
                        #[allow(clippy::cast_possible_wrap)]
                        let secs = d.as_secs() as i64;
                        chrono::DateTime::from_timestamp(secs, 0)
                    })
                    .unwrap_or_default();

                documents.push(SourceDocument {
                    id,
                    title,
                    source_type: "local-file".to_string(),
                    content: Vec::new(),
                    metadata: serde_json::json!({
                        "size": metadata.len(),
                        "is_dir": metadata.is_dir(),
                    }),
                    hash: String::new(),
                    updated_at: modified,
                });
            }
        }

        Ok(documents)
    }

    async fn fetch_document(&self, doc_id: &str) -> Result<SourceDocument> {
        let content = tokio::fs::read(doc_id)
            .await
            .map_err(|e| error_core::helpers::io_error(&format!("读取文件失败: {e}")))?;

        let hash = blake3::hash(&content).to_hex().to_string();
        let path = std::path::Path::new(doc_id);
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let metadata = tokio::fs::metadata(doc_id)
            .await
            .map_err(|e| error_core::helpers::io_error(&format!("读取文件元数据失败: {e}")))?;

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| {
                #[allow(clippy::cast_possible_wrap)]
                let secs = d.as_secs() as i64;
                chrono::DateTime::from_timestamp(secs, 0)
            })
            .unwrap_or_default();

        Ok(SourceDocument {
            id: doc_id.to_string(),
            title,
            source_type: "local-file".to_string(),
            content,
            metadata: serde_json::json!({
                "size": metadata.len(),
            }),
            hash,
            updated_at: modified,
        })
    }

    async fn watch_changes(&self) -> Result<Pin<Box<dyn Stream<Item = SourceChange> + Send>>> {
        todo!("实现文件系统监听（需引入 notify crate）")
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(tokio::fs::metadata(&self.base_path).await.is_ok())
    }
}

/// 全通过质量门禁（开源默认实现）
///
/// 所有代码变更均通过评估，不执行任何检查。
/// 适用于开源社区用户和开发测试场景。
pub struct AlwaysPassGate;

impl AlwaysPassGate {
    /// 创建全通过质量门禁
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for AlwaysPassGate {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl QualityGatePlugin for AlwaysPassGate {
    fn gate_id(&self) -> &'static str {
        "always-pass"
    }

    async fn evaluate(&self, _change: &CodeChange) -> Result<QualityVerdict> {
        Ok(QualityVerdict {
            passed: true,
            score: 1.0,
            violations: vec![],
            auto_fix_available: false,
            details: serde_json::json!({"note": "default always-pass gate"}),
        })
    }

    async fn list_rules(&self) -> Result<Vec<RuleInfo>> {
        Ok(vec![])
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}

/// `ReAct` Agent 后端（开源默认实现）
///
/// 基于推理-行动循环的基础 Agent 实现。
/// 不支持流式输出和任务取消，适用于简单的知识问答场景。
pub struct ReActAgentBackend;

impl ReActAgentBackend {
    /// 创建 `ReAct` Agent 后端
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReActAgentBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentBackend for ReActAgentBackend {
    fn backend_id(&self) -> &'static str {
        "react"
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            supports_streaming: false,
            supports_cancellation: false,
            max_context_tokens: 4096,
            available_tools: vec!["search".to_string(), "read_file".to_string()],
            supports_multi_agent: false,
            supports_human_in_loop: false,
        }
    }

    async fn execute(&self, _task: &AgentTask) -> Result<AgentResult> {
        todo!("委托给 knowledge-api/src/agent/react_agent.rs")
    }

    async fn execute_stream(
        &self,
        task: &AgentTask,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentChunk> + Send>>> {
        let result = self.execute(task).await?;
        let chunk = AgentChunk {
            task_id: result.task_id.clone(),
            chunk_type: AgentChunkType::FinalAnswer,
            content: result.output.clone(),
        };
        Ok(Box::pin(tokio_stream::once(chunk)))
    }

    async fn cancel(&self, _task_id: &str) -> Result<()> {
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_file_source_default() {
        let source = LocalFileSource::new();
        assert_eq!(source.source_id(), "local-file");
        assert_eq!(source.base_path, ".");
    }

    #[test]
    fn test_local_file_source_with_path() {
        let source = LocalFileSource::with_path("/data/docs");
        assert_eq!(source.base_path, "/data/docs");
    }

    #[test]
    fn test_always_pass_gate_id() {
        let gate = AlwaysPassGate::new();
        assert_eq!(gate.gate_id(), "always-pass");
    }

    #[tokio::test]
    async fn test_always_pass_gate_evaluate() {
        let gate = AlwaysPassGate::new();
        let change = CodeChange {
            repository: "test".to_string(),
            branch: "main".to_string(),
            commit_sha: "abc123".to_string(),
            changed_files: vec!["src/main.rs".to_string()],
            diff: None,
            author: None,
            message: None,
        };
        let verdict = gate.evaluate(&change).await.unwrap();
        assert!(verdict.passed);
        assert!((verdict.score - 1.0).abs() < f64::EPSILON);
        assert!(verdict.violations.is_empty());
    }

    #[tokio::test]
    async fn test_always_pass_gate_list_rules() {
        let gate = AlwaysPassGate::new();
        let rules = gate.list_rules().await.unwrap();
        assert!(rules.is_empty());
    }

    #[tokio::test]
    async fn test_always_pass_gate_health_check() {
        let gate = AlwaysPassGate::new();
        assert!(gate.health_check().await.unwrap());
    }

    #[test]
    fn test_react_agent_backend_id() {
        let agent = ReActAgentBackend::new();
        assert_eq!(agent.backend_id(), "react");
    }

    #[test]
    fn test_react_agent_capabilities() {
        let agent = ReActAgentBackend::new();
        let caps = agent.capabilities();
        assert!(!caps.supports_streaming);
        assert!(!caps.supports_cancellation);
        assert_eq!(caps.max_context_tokens, 4096);
        assert!(!caps.supports_multi_agent);
        assert!(!caps.supports_human_in_loop);
    }

    #[tokio::test]
    async fn test_react_agent_health_check() {
        let agent = ReActAgentBackend::new();
        assert!(agent.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_local_file_source_health_check_nonexistent() {
        let source = LocalFileSource::with_path("/nonexistent/path/that/does/not/exist");
        let result = source.health_check().await.unwrap();
        assert!(!result);
    }
}
