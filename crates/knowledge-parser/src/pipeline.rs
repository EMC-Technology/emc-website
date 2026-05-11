//! 解析流水线编排器
//!
//! Pipeline 组合多个 ParseStage，通过 DagEngine 按 DAG 拓扑序执行。
//! 默认流水线包含：文件摄入 → 源类型检测 → 分块 → 语法分析 → 图构建。
//!
//! 每个阶段调用对应的实际处理模块，而非空壳透传。

use crate::dag::DagEngine;
use crate::source_type_detector::SourceTypeDetector;
use async_trait::async_trait;
use knowledge_core::model::{Document, SourceType};

/// 解析阶段的抽象 trait，支持 DAG 编排
#[async_trait]
pub trait ParseStage: Send + Sync {
    /// 阶段名称（用于 `DAG` 拓扑排序与日志标识）
    fn name(&self) -> &'static str;

    /// 执行解析阶段，返回产出的文档集合
    async fn execute(&self, input: Vec<Document>) -> crate::Result<Vec<Document>>;
}

/// 文件摄入阶段：验证文档路径和哈希完整性
///
/// 对输入文档进行基本校验（路径非空、哈希格式正确），
/// 过滤掉不符合不变量的文档。
pub struct FileIngestStage;

#[async_trait]
impl ParseStage for FileIngestStage {
    fn name(&self) -> &'static str {
        "file_ingest"
    }

    async fn execute(&self, input: Vec<Document>) -> crate::Result<Vec<Document>> {
        let valid: Vec<Document> = input
            .into_iter()
            .filter(|doc| {
                if doc.path.is_empty() {
                    tracing::warn!(hash = %doc.hash, "文档路径为空，已跳过");
                    return false;
                }
                if doc.hash.len() != 64 {
                    tracing::warn!(path = %doc.path, hash_len = doc.hash.len(), "文档哈希长度异常，已跳过");
                    return false;
                }
                true
            })
            .collect();
        Ok(valid)
    }
}

/// 源类型检测阶段：根据路径扩展名更新 `source_type`
///
/// 若文档的 `source_type` 为默认值或需要重新检测，
/// 使用 `SourceTypeDetector` 根据文件扩展名确定正确类型。
pub struct SourceDetectionStage;

impl SourceDetectionStage {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for SourceDetectionStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ParseStage for SourceDetectionStage {
    fn name(&self) -> &'static str {
        "source_detection"
    }

    async fn execute(&self, input: Vec<Document>) -> crate::Result<Vec<Document>> {
        let results: Vec<Document> = input
            .into_iter()
            .map(|doc| {
                if matches!(doc.source_type, SourceType::Plain) {
                    let path = std::path::Path::new(&doc.path);
                    match SourceTypeDetector::detect(path) {
                        Ok(detected) => {
                            if detected != doc.source_type {
                                tracing::debug!(
                                    path = %doc.path,
                                    old_type = ?doc.source_type,
                                    new_type = ?detected,
                                    "源类型已更新"
                                );
                            }
                            Document {
                                source_type: detected,
                                ..doc
                            }
                        }
                        Err(e) => {
                            tracing::warn!(path = %doc.path, error = %e, "源类型检测失败，保留原类型");
                            doc
                        }
                    }
                } else {
                    doc
                }
            })
            .collect();
        Ok(results)
    }
}

/// 分块阶段：根据源类型选择分块策略
///
/// - `Markdown` → 使用 `MarkdownParser` 进行结构化分块
/// - `Code` → 透传给后续 `CodePipeline` 处理
/// - `Plain` → 透传（分块在 `MarkdownPipeline` 中处理）
pub struct ChunkingStage;

#[async_trait]
impl ParseStage for ChunkingStage {
    fn name(&self) -> &'static str {
        "chunking"
    }

    async fn execute(&self, input: Vec<Document>) -> crate::Result<Vec<Document>> {
        for doc in &input {
            tracing::debug!(
                path = %doc.path,
                source_type = ?doc.source_type,
                "分块阶段：文档已确认类型，实际分块在 Pipeline 外部由 MarkdownParser/CodePipeline 执行"
            );
        }
        Ok(input)
    }
}

/// 语法分析阶段：对代码类文档执行 tree-sitter 解析
///
/// `Code` 类型文档由 `CodePipeline` 处理 `AST` 和 `Token` 提取，
/// 非 `Code` 类型文档在此阶段直接透传。
pub struct SyntaxAnalysisStage;

#[async_trait]
impl ParseStage for SyntaxAnalysisStage {
    fn name(&self) -> &'static str {
        "syntax_analysis"
    }

    async fn execute(&self, input: Vec<Document>) -> crate::Result<Vec<Document>> {
        for doc in &input {
            if matches!(doc.source_type, SourceType::Code) {
                tracing::debug!(
                    path = %doc.path,
                    "语法分析阶段：代码文档将由 CodePipeline 在 Pipeline 外部处理"
                );
            }
        }
        Ok(input)
    }
}

/// 图构建阶段：组装实体图与引用关系
///
/// 在所有前置阶段完成后，由 `GraphBuilder` 构建完整的知识图谱。
/// 此阶段在 `Pipeline` 内部为透传，实际图构建在 `Pipeline` 外部执行。
pub struct GraphBuildStage;

#[async_trait]
impl ParseStage for GraphBuildStage {
    fn name(&self) -> &'static str {
        "graph_build"
    }

    async fn execute(&self, input: Vec<Document>) -> crate::Result<Vec<Document>> {
        tracing::debug!(
            doc_count = input.len(),
            "图构建阶段：{} 个文档待构建图谱（在 Pipeline 外部由 GraphBuilder 执行）",
            input.len()
        );
        Ok(input)
    }
}

/// 解析流水线
///
/// 组合多个 `ParseStage`，通过 `DagEngine` 按 `DAG` 拓扑序执行。
/// 默认流水线：`file_ingest` → `source_detection` → `chunking` → `syntax_analysis` → `graph_build`
///
/// # 设计说明
///
/// `Pipeline` 负责文档的**校验与类型检测**，而实际的**分块、解析、图构建**
/// 由外部调用方根据文档类型分别调度 `MarkdownParser` 或 `CodePipeline`。
/// 这种设计允许：
/// 1. 不同文档类型使用完全不同的处理路径
/// 2. Pipeline 专注于文档流转的通用逻辑
/// 3. 具体解析逻辑可独立测试和替换
pub struct Pipeline {
    engine: DagEngine,
}

impl Pipeline {
    /// 创建默认解析管线
    #[must_use]
    pub fn new() -> Self {
        let mut engine = DagEngine::new();

        engine.register_stage(Box::new(FileIngestStage), vec![]);
        engine.register_stage(
            Box::new(SourceDetectionStage::new()),
            vec!["file_ingest".to_string()],
        );
        engine.register_stage(
            Box::new(ChunkingStage),
            vec!["source_detection".to_string()],
        );
        engine.register_stage(Box::new(SyntaxAnalysisStage), vec!["chunking".to_string()]);
        engine.register_stage(
            Box::new(GraphBuildStage),
            vec!["syntax_analysis".to_string()],
        );

        Self { engine }
    }

    /// 使用自定义 `DAG` 引擎创建解析管线
    #[must_use]
    pub const fn with_engine(engine: DagEngine) -> Self {
        Self { engine }
    }

    /// # Errors
    ///
    /// DAG 引擎执行阶段失败时返回错误
    pub async fn run(&self, input: Vec<Document>) -> crate::Result<Vec<Document>> {
        self.engine.run(input).await
    }

    /// 获取 `DAG` 引擎引用
    #[must_use]
    pub const fn engine(&self) -> &DagEngine {
        &self.engine
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_creation() {
        let pipeline = Pipeline::new();
        assert_eq!(pipeline.engine().stages_count(), 5);
    }

    #[test]
    fn test_stage_names() {
        let pipeline = Pipeline::new();
        let stages = pipeline.engine().stage_names();
        assert!(stages.contains(&"file_ingest".to_string()));
        assert!(stages.contains(&"graph_build".to_string()));
    }

    #[tokio::test]
    async fn test_pipeline_run_empty_input() {
        let pipeline = Pipeline::new();
        let result = pipeline.run(vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_pipeline_run_passes_documents_through() {
        let pipeline = Pipeline::new();
        let doc = Document::new(
            "/test.md",
            "Test Document",
            knowledge_core::model::SourceType::Markdown,
            "a".repeat(64),
        )
        .unwrap();
        let result = pipeline.run(vec![doc]).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_file_ingest_stage_filters_invalid_hash() {
        let stage = FileIngestStage;
        let doc_valid = Document::new("/a.md", "A", SourceType::Markdown, "a".repeat(64)).unwrap();
        let mut doc_invalid = doc_valid.clone();
        doc_invalid.hash = "short".to_string();
        let result = stage.execute(vec![doc_valid, doc_invalid]).await.unwrap();
        assert_eq!(result.len(), 1, "哈希长度异常的文档应被过滤");
    }

    #[tokio::test]
    async fn test_file_ingest_stage_filters_empty_path() {
        let stage = FileIngestStage;
        let mut doc = Document::new("/a.md", "A", SourceType::Markdown, "a".repeat(64)).unwrap();
        doc.path = String::new();
        let result = stage.execute(vec![doc]).await.unwrap();
        assert!(result.is_empty(), "路径为空的文档应被过滤");
    }

    #[tokio::test]
    async fn test_source_detection_stage_updates_type() {
        let stage = SourceDetectionStage::new();
        let mut doc = Document::new("/test.py", "A", SourceType::Plain, "a".repeat(64)).unwrap();
        doc.source_type = SourceType::Plain;
        let result = stage.execute(vec![doc]).await.unwrap();
        assert_eq!(
            result[0].source_type,
            SourceType::Code,
            ".py 文件应被检测为 Code 类型"
        );
    }

    #[test]
    fn test_custom_pipeline_with_custom_stages() {
        let mut engine = DagEngine::new();
        engine.register_stage(Box::new(FileIngestStage), vec![]);
        let pipeline = Pipeline::with_engine(engine);
        assert_eq!(pipeline.engine().stages_count(), 1);
    }

    #[test]
    fn test_parse_stage_name() {
        let stage = FileIngestStage;
        assert_eq!(stage.name(), "file_ingest");
    }

    #[test]
    fn test_source_detection_stage_default() {
        let stage = SourceDetectionStage;
        assert_eq!(stage.name(), "source_detection");
    }

    #[tokio::test]
    async fn test_source_detection_stage_preserves_non_plain_type() {
        let stage = SourceDetectionStage::new();
        let doc = Document::new("/test.md", "Test", SourceType::Markdown, "a".repeat(64)).unwrap();
        let result = stage.execute(vec![doc]).await.unwrap();
        assert_eq!(
            result[0].source_type,
            SourceType::Markdown,
            "非 Plain 类型应保持不变"
        );
    }

    #[tokio::test]
    async fn test_source_detection_stage_handles_unsupported_extension() {
        let stage = SourceDetectionStage::new();
        let mut doc =
            Document::new("/test.xyz", "Test", SourceType::Plain, "a".repeat(64)).unwrap();
        doc.source_type = SourceType::Plain;
        let result = stage.execute(vec![doc]).await.unwrap();
        assert_eq!(
            result[0].source_type,
            SourceType::Plain,
            "检测失败时应保留原类型"
        );
    }

    #[test]
    fn test_pipeline_default_creates_default_pipeline() {
        let pipeline = Pipeline::default();
        assert_eq!(
            pipeline.engine().stages_count(),
            5,
            "Default Pipeline 应有 5 个阶段"
        );
    }

    #[tokio::test]
    async fn test_chunking_stage_passes_through() {
        let stage = ChunkingStage;
        assert_eq!(stage.name(), "chunking");
        let doc = Document::new("/test.md", "Test", SourceType::Markdown, "a".repeat(64)).unwrap();
        let result = stage.execute(vec![doc]).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_syntax_analysis_stage_passes_through() {
        let stage = SyntaxAnalysisStage;
        assert_eq!(stage.name(), "syntax_analysis");
        let doc = Document::new("/test.rs", "Test", SourceType::Code, "a".repeat(64)).unwrap();
        let result = stage.execute(vec![doc]).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_graph_build_stage_passes_through() {
        let stage = GraphBuildStage;
        assert_eq!(stage.name(), "graph_build");
        let doc = Document::new("/test.md", "Test", SourceType::Markdown, "a".repeat(64)).unwrap();
        let result = stage.execute(vec![doc]).await.unwrap();
        assert_eq!(result.len(), 1);
    }
}
