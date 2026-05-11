//! DAG 编排引擎 —— 有向无环图驱动的流水线调度

use crate::pipeline::ParseStage;
use error_core::helpers;
use std::collections::HashMap;

/// DAG 编排器，管理解析阶段之间的依赖关系
pub struct DagEngine {
    stages: HashMap<String, Box<dyn ParseStage>>,
    adjacency: HashMap<String, Vec<String>>,
}

impl DagEngine {
    /// 创建空的 DAG 引擎
    #[must_use]
    pub fn new() -> Self {
        Self {
            stages: HashMap::new(),
            adjacency: HashMap::new(),
        }
    }

    /// 注册一个解析阶段及其前置依赖
    pub fn register_stage(
        &mut self,
        stage: Box<dyn ParseStage + 'static>,
        dependencies: Vec<String>,
    ) {
        let name = stage.name().to_string();
        self.adjacency.insert(name.clone(), dependencies);
        self.stages.insert(name, stage);
    }

    /// 拓扑排序后依次执行所有阶段
    ///
    /// # Errors
    ///
    /// 检测到循环依赖（DAG 无效）或阶段执行失败时返回错误
    pub async fn run(
        &self,
        initial_input: Vec<knowledge_core::model::Document>,
    ) -> crate::Result<Vec<knowledge_core::model::Document>> {
        let order = self.topological_sort()?;
        let mut data = initial_input;
        for stage_name in &order {
            if let Some(stage) = self.stages.get(stage_name) {
                data = stage.execute(data).await?;
            }
        }
        Ok(data)
    }

    /// Kahn 算法进行拓扑排序
    ///
    /// # 错误检测
    ///
    /// 区分两种不同的错误场景：
    /// 1. **缺失依赖**：某阶段声明了不存在的依赖（`result` 中包含非注册阶段名）
    /// 2. **循环依赖**：已注册阶段之间存在环形依赖（`result` 中仅包含注册阶段但数量不足）
    fn topological_sort(&self) -> crate::Result<Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for (node, deps) in &self.adjacency {
            in_degree.entry(node.clone()).or_insert(0);
            for dep in deps {
                in_degree.entry(dep.clone()).or_insert(0);
            }
            *in_degree.entry(node.clone()).or_insert(0) += deps.len();
        }

        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(k, _)| k.clone())
            .collect();
        queue.sort();

        let mut result = Vec::new();
        while let Some(node) = queue.pop() {
            result.push(node.clone());
            for (succ, deps) in &self.adjacency {
                if deps.contains(&node)
                    && let Some(deg) = in_degree.get_mut(succ)
                {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(succ.clone());
                        queue.sort();
                    }
                }
            }
        }

        if result.len() != self.stages.len() {
            let registered: std::collections::HashSet<&str> = self
                .stages
                .keys()
                .map(std::string::String::as_str)
                .collect();
            let missing_deps: Vec<String> = self
                .adjacency
                .values()
                .flatten()
                .filter(|dep| !registered.contains(dep.as_str()))
                .cloned()
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            if !missing_deps.is_empty() {
                return Err(helpers::validation_error(
                    &format!(
                        "DAG 包含缺失的依赖阶段: {}（已注册阶段: {}）",
                        missing_deps.join(", "),
                        self.stages.keys().cloned().collect::<Vec<_>>().join(", ")
                    ),
                    "dag_validate",
                ));
            }

            return Err(helpers::validation_error(
                "检测到循环依赖，DAG 无效",
                "dag_validate",
            ));
        }
        Ok(result)
    }
}

impl Default for DagEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DagEngine {
    /// 获取已注册的阶段数量
    #[must_use]
    pub fn stages_count(&self) -> usize {
        self.stages.len()
    }

    /// 获取所有已注册阶段的名称列表
    #[must_use]
    pub fn stage_names(&self) -> Vec<String> {
        self.stages.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::ParseStage;
    use async_trait::async_trait;
    use knowledge_core::model::{Document, SourceType};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static STAGE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn next_stage_id() -> usize {
        STAGE_COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    struct NoopStage {
        name: String,
    }

    impl NoopStage {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl ParseStage for NoopStage {
        fn name(&self) -> &'static str {
            Box::leak(self.name.clone().into_boxed_str())
        }

        async fn execute(&self, input: Vec<Document>) -> crate::Result<Vec<Document>> {
            Ok(input)
        }
    }

    struct AppendTagStage {
        name: String,
        tag: String,
    }

    impl AppendTagStage {
        fn new(name: &str, tag: &str) -> Self {
            Self {
                name: name.to_string(),
                tag: tag.to_string(),
            }
        }
    }

    #[async_trait]
    impl ParseStage for AppendTagStage {
        fn name(&self) -> &'static str {
            Box::leak(self.name.clone().into_boxed_str())
        }

        async fn execute(&self, mut input: Vec<Document>) -> crate::Result<Vec<Document>> {
            for doc in &mut input {
                doc.title = format!("{}+{}", doc.title, self.tag);
            }
            Ok(input)
        }
    }

    #[test]
    fn test_new_creates_engine_with_no_stages() {
        let engine = DagEngine::new();
        assert_eq!(engine.stages_count(), 0, "新引擎不应有任何阶段");
    }

    #[test]
    fn test_default_creates_engine_with_no_stages() {
        let engine = DagEngine::default();
        assert_eq!(engine.stages_count(), 0, "Default 引擎不应有任何阶段");
    }

    #[test]
    fn test_register_stage_single_stage_returns_count_one() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        engine.register_stage(Box::new(NoopStage::new(&format!("stage_{id}"))), vec![]);
        assert_eq!(engine.stages_count(), 1);
    }

    #[test]
    fn test_register_stage_multiple_stages_returns_correct_count() {
        let mut engine = DagEngine::new();
        let id1 = next_stage_id();
        let id2 = next_stage_id();
        let id3 = next_stage_id();

        engine.register_stage(Box::new(NoopStage::new(&format!("alpha_{id1}"))), vec![]);
        engine.register_stage(
            Box::new(NoopStage::new(&format!("beta_{id2}"))),
            vec![format!("alpha_{id1}")],
        );
        engine.register_stage(
            Box::new(NoopStage::new(&format!("gamma_{id3}"))),
            vec![format!("beta_{id2}")],
        );

        assert_eq!(engine.stages_count(), 3);
    }

    #[test]
    fn test_stage_names_empty_engine_returns_empty_vec() {
        let engine = DagEngine::new();
        assert!(engine.stage_names().is_empty());
    }

    #[test]
    fn test_stage_names_returns_all_registered_names() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        let name_a = format!("stage_a_{id}");
        let name_b = format!("stage_b_{id}");

        engine.register_stage(Box::new(NoopStage::new(&name_a)), vec![]);
        engine.register_stage(Box::new(NoopStage::new(&name_b)), vec![name_a.clone()]);

        let names = engine.stage_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&name_a), "应包含阶段 {name_a}");
        assert!(names.contains(&name_b), "应包含阶段 {name_b}");
    }

    #[tokio::test]
    async fn test_run_empty_engine_empty_input_returns_empty() {
        let engine = DagEngine::new();
        let result = engine.run(vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_run_single_stage_passes_documents_through() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        engine.register_stage(Box::new(NoopStage::new(&format!("noop_{id}"))), vec![]);

        let doc = Document::new("/test.md", "Test", SourceType::Markdown, "a".repeat(64)).unwrap();
        let result = engine.run(vec![doc]).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_run_multiple_stages_executes_in_topological_order() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        let stage_a = format!("tag_a_{id}");
        let stage_b = format!("tag_b_{id}");

        engine.register_stage(Box::new(AppendTagStage::new(&stage_a, "A")), vec![]);
        engine.register_stage(
            Box::new(AppendTagStage::new(&stage_b, "B")),
            vec![stage_a.clone()],
        );

        let doc = Document::new("/test.md", "Doc", SourceType::Markdown, "a".repeat(64)).unwrap();
        let result = engine.run(vec![doc]).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Doc+A+B", "阶段应按拓扑序执行：先 A 后 B");
    }

    #[tokio::test]
    async fn test_run_detects_cyclic_dependency_returns_error() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        let name_x = format!("cycle_x_{id}");
        let name_y = format!("cycle_y_{id}");

        engine.register_stage(Box::new(NoopStage::new(&name_x)), vec![name_y.clone()]);
        engine.register_stage(Box::new(NoopStage::new(&name_y)), vec![name_x.clone()]);

        let doc = Document::new("/test.md", "Test", SourceType::Markdown, "a".repeat(64)).unwrap();
        let result = engine.run(vec![doc]).await;
        assert!(result.is_err(), "循环依赖应导致 run 返回错误");
    }

    #[test]
    fn test_topological_sort_linear_chain_succeeds() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        let s1 = format!("s1_{id}");
        let s2 = format!("s2_{id}");
        let s3 = format!("s3_{id}");

        engine.register_stage(Box::new(NoopStage::new(&s1)), vec![]);
        engine.register_stage(Box::new(NoopStage::new(&s2)), vec![s1.clone()]);
        engine.register_stage(Box::new(NoopStage::new(&s3)), vec![s2.clone()]);

        let order = engine.topological_sort().unwrap();
        assert_eq!(order.len(), 3, "线性链应有 3 个节点");
        let pos1 = order.iter().position(|n| n == &s1).unwrap();
        let pos2 = order.iter().position(|n| n == &s2).unwrap();
        let pos3 = order.iter().position(|n| n == &s3).unwrap();
        assert!(pos1 < pos2, "s1 应在 s2 之前");
        assert!(pos2 < pos3, "s2 应在 s3 之前");
    }

    #[test]
    fn test_topological_sort_diamond_dependency_succeeds() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        let root = format!("root_{id}");
        let left = format!("left_{id}");
        let right = format!("right_{id}");
        let leaf = format!("leaf_{id}");

        engine.register_stage(Box::new(NoopStage::new(&root)), vec![]);
        engine.register_stage(Box::new(NoopStage::new(&left)), vec![root.clone()]);
        engine.register_stage(Box::new(NoopStage::new(&right)), vec![root.clone()]);
        engine.register_stage(
            Box::new(NoopStage::new(&leaf)),
            vec![left.clone(), right.clone()],
        );

        let order = engine.topological_sort().unwrap();
        assert_eq!(order.len(), 4);

        let pos_root = order.iter().position(|n| n == &root).unwrap();
        let pos_left = order.iter().position(|n| n == &left).unwrap();
        let pos_right = order.iter().position(|n| n == &right).unwrap();
        let pos_leaf = order.iter().position(|n| n == &leaf).unwrap();

        assert!(pos_root < pos_left, "root 应在 left 之前");
        assert!(pos_root < pos_right, "root 应在 right 之前");
        assert!(pos_left < pos_leaf, "left 应在 leaf 之前");
        assert!(pos_right < pos_leaf, "right 应在 leaf 之前");
    }

    #[test]
    fn test_topological_sort_cycle_returns_error() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        let name_a = format!("cycle_a_{id}");
        let name_b = format!("cycle_b_{id}");

        engine.register_stage(Box::new(NoopStage::new(&name_a)), vec![name_b.clone()]);
        engine.register_stage(Box::new(NoopStage::new(&name_b)), vec![name_a]);

        let result = engine.topological_sort();
        assert!(result.is_err(), "循环依赖应导致拓扑排序失败");
    }

    #[tokio::test]
    async fn test_run_independent_stages_both_execute() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        let name_a = format!("indep_a_{id}");
        let name_b = format!("indep_b_{id}");

        engine.register_stage(Box::new(AppendTagStage::new(&name_a, "X")), vec![]);
        engine.register_stage(Box::new(AppendTagStage::new(&name_b, "Y")), vec![]);

        let doc = Document::new("/test.md", "Base", SourceType::Markdown, "a".repeat(64)).unwrap();
        let result = engine.run(vec![doc]).await.unwrap();

        assert_eq!(result.len(), 1);
        let title = &result[0].title;
        assert!(
            title.contains('X') && title.contains('Y'),
            "两个独立阶段都应执行，实际 title: {title}"
        );
    }

    #[tokio::test]
    async fn test_run_preserves_document_count_through_stages() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        engine.register_stage(Box::new(NoopStage::new(&format!("pass_{id}"))), vec![]);

        let docs: Vec<Document> = (0..5)
            .map(|i| {
                Document::new(
                    format!("/doc{i}.md"),
                    format!("Doc {i}"),
                    SourceType::Markdown,
                    "a".repeat(64),
                )
                .unwrap()
            })
            .collect();

        let result = engine.run(docs).await.unwrap();
        assert_eq!(result.len(), 5, "文档数量应保持不变");
    }

    #[test]
    fn test_topological_sort_missing_dependency_returns_error() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        let name = format!("orphan_{id}");

        engine.register_stage(
            Box::new(NoopStage::new(&name)),
            vec![format!("nonexistent_stage_{id}")],
        );

        let result = engine.topological_sort();
        assert!(result.is_err(), "缺失依赖应导致拓扑排序失败");
        let err = result.unwrap_err();
        assert!(
            err.message().contains("缺失的依赖阶段"),
            "错误消息应包含'缺失的依赖阶段': {}",
            err.message()
        );
    }

    #[tokio::test]
    async fn test_run_missing_dependency_returns_error() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        let name = format!("depends_on_missing_{id}");

        engine.register_stage(
            Box::new(NoopStage::new(&name)),
            vec![format!("missing_stage_{id}")],
        );

        let doc = Document::new("/test.md", "Test", SourceType::Markdown, "a".repeat(64)).unwrap();
        let result = engine.run(vec![doc]).await;
        assert!(result.is_err(), "缺失依赖应导致 run 返回错误");
    }

    #[test]
    fn test_topological_sort_single_stage_no_deps() {
        let mut engine = DagEngine::new();
        let id = next_stage_id();
        let name = format!("solo_{id}");
        engine.register_stage(Box::new(NoopStage::new(&name)), vec![]);

        let order = engine.topological_sort().unwrap();
        assert_eq!(order.len(), 1);
        assert_eq!(order[0], name);
    }
}
