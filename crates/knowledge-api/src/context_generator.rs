//! AI 上下文文件生成器
//!
//! 从知识图谱中提取架构概览、模块描述与依赖关系，
//! 生成 CLAUDE.md / AGENTS.md 等 AI 上下文文件，
//! 为 AI 编程助手提供项目结构的结构化认知。

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::Path;

use error_core::helpers;
use knowledge_core::model::{Community, Reference};

use crate::KnowledgeVM;
use crate::Result;

/// AI 上下文文件生成器
///
/// 从知识图谱中提取社区（`Community`）→ 模块映射、模块间依赖关系，
/// 生成 Markdown 格式的架构概览与 Mermaid 格式的依赖图。
///
/// # 设计说明
///
/// - 所有数据库查询通过 [`KnowledgeVM`] 完成，本结构体不直接访问数据库
/// - 当 community / process 表尚未填充时，返回优雅的空默认值
/// - 生成的 Markdown 遵循 AI 上下文文件的最佳实践结构
///
/// # Example
///
/// ```ignore
/// use knowledge_api::{KnowledgeVM, ContextGenerator};
///
/// let vm = KnowledgeVM::with_client(db);
/// let generator = ContextGenerator::new(vm);
/// generator.generate_claude_md(Path::new("CLAUDE.md")).await?;
/// ```
pub struct ContextGenerator {
    vm: KnowledgeVM,
}

impl ContextGenerator {
    /// 使用已有的 `KnowledgeVM` 构造 `ContextGenerator` 实例
    #[must_use]
    pub const fn new(vm: KnowledgeVM) -> Self {
        Self { vm }
    }

    /// 从知识图谱生成架构概览
    ///
    /// 查询所有社区（Community），按内聚度降序排列，
    /// 生成包含社区名称与成员数量的 Markdown 概览表格。
    ///
    /// # Errors
    ///
    /// 数据库查询失败时返回 `DbQuery`。
    pub fn generate_architecture_overview(&self) -> Result<String> {
        let communities = self.query_communities();
        if communities.is_empty() {
            return Ok(
                "# Architecture Overview\n\nNo communities found in the knowledge graph.\n"
                    .to_string(),
            );
        }

        let mut sorted = communities;
        sorted.sort_by(|a, b| {
            b.cohesion_score
                .partial_cmp(&a.cohesion_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut md = String::from("# Architecture Overview\n\n");
        md.push_str("| Module | Cohesion | Members |\n");
        md.push_str("|--------|----------|--------|\n");

        for community in &sorted {
            let _ = writeln!(
                md,
                "| {} | {:.2} | {} |",
                community.name,
                community.cohesion_score,
                community.member_ids.len()
            );
        }

        md.push('\n');
        Ok(md)
    }

    /// 生成核心模块描述
    ///
    /// 为每个社区（Community）生成独立的 Markdown 章节，
    /// 列出社区名称作为标题，成员符号 ID 作为列表项。
    ///
    /// # Errors
    ///
    /// 数据库查询失败时返回 `DbQuery`。
    pub fn generate_module_descriptions(&self) -> Result<String> {
        let communities = self.query_communities();

        if communities.is_empty() {
            return Ok("# Module Descriptions\n\nNo modules found.\n".to_string());
        }

        let mut md = String::from("# Module Descriptions\n\n");

        for community in &communities {
            let _ = write!(md, "## {}\n\n", community.name);
            let _ = write!(md, "Cohesion: {:.2}\n\n", community.cohesion_score);

            if community.member_ids.is_empty() {
                md.push_str("No members.\n\n");
            } else {
                md.push_str("Members:\n");
                for member_id in &community.member_ids {
                    let _ = writeln!(md, "- `{member_id}`");
                }
                md.push('\n');
            }
        }

        Ok(md)
    }

    /// 生成 Mermaid 格式的依赖图
    ///
    /// 查询社区间的引用关系，生成 Mermaid `graph TD` 语法。
    /// 每条边表示源社区中至少有一个成员引用了目标社区的成员。
    ///
    /// # Mermaid 语法
    ///
    /// ```mermaid
    /// graph TD
    ///     CommunityA --> CommunityB
    ///     CommunityA --> CommunityC
    /// ```
    ///
    /// # Errors
    ///
    /// 数据库查询失败时返回 `DbQuery`。
    pub fn generate_dependency_graph(&self) -> Result<String> {
        let communities = self.query_communities();

        if communities.is_empty() {
            return Ok("```mermaid\ngraph TD\n```\n".to_string());
        }

        let member_to_community = Self::build_member_community_map(&communities);
        let references = self.query_all_references();

        let edges = Self::compute_community_edges(&member_to_community, &references);

        let mut mermaid = String::from("```mermaid\ngraph TD\n");

        for (from, to_set) in &edges {
            for to in to_set {
                let _ = writeln!(
                    mermaid,
                    "    {} --> {}",
                    Self::sanitize_mermaid_id(from),
                    Self::sanitize_mermaid_id(to)
                );
            }
        }

        mermaid.push_str("```\n");
        Ok(mermaid)
    }

    /// 生成 CLAUDE.md 文件
    ///
    /// 组合架构概览、模块描述与依赖图，写入指定路径。
    ///
    /// # Errors
    ///
    /// - 数据库查询失败时返回 `DbQuery`
    /// - 文件写入失败时返回 `Internal`
    pub async fn generate_claude_md(&self, output_path: &Path) -> Result<()> {
        let content = self.build_full_context()?;
        self.write_file(output_path, &content).await
    }

    /// 生成 AGENTS.md 文件
    ///
    /// 组合架构概览、模块描述与依赖图，写入指定路径。
    /// 内容与 CLAUDE.md 相同，但文件名不同，用于不同的 AI 工具链。
    ///
    /// # Errors
    ///
    /// - 数据库查询失败时返回 `DbQuery`
    /// - 文件写入失败时返回 `Internal`
    pub async fn generate_agents_md(&self, output_path: &Path) -> Result<()> {
        let content = self.build_full_context()?;
        self.write_file(output_path, &content).await
    }

    fn build_full_context(&self) -> Result<String> {
        let overview = self.generate_architecture_overview()?;
        let modules = self.generate_module_descriptions()?;
        let deps = self.generate_dependency_graph()?;

        let mut content = String::new();
        content.push_str(&overview);
        content.push('\n');
        content.push_str(&modules);
        content.push('\n');
        content.push_str("## Dependency Graph\n\n");
        content.push_str(&deps);

        Ok(content)
    }

    async fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| helpers::internal_error(&format!("创建目录失败: {e}")))?;
        }

        tokio::fs::write(path, content)
            .await
            .map_err(|e| helpers::internal_error(&format!("写入文件失败: {e}")))?;

        Ok(())
    }

    fn query_communities(&self) -> Vec<Community> {
        let sql = "SELECT * FROM community";
        let Ok(response) = self.vm.execute_parameterized_query(sql, &serde_json::json!({})) else {
            return vec![];
        };

        response
            .into_iter()
            .filter_map(|v| serde_json::from_value::<Community>(v).ok())
            .collect()
    }

    fn query_all_references(&self) -> Vec<Reference> {
        let sql = "SELECT * FROM reference";
        let Ok(response) = self.vm.execute_parameterized_query(sql, &serde_json::json!({})) else {
            return vec![];
        };

        response
            .into_iter()
            .filter_map(|v| serde_json::from_value::<Reference>(v).ok())
            .collect()
    }

    fn build_member_community_map(communities: &[Community]) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for community in communities {
            for member_id in &community.member_ids {
                map.insert(member_id.to_string(), community.name.clone());
            }
        }
        map
    }

    fn compute_community_edges(
        member_to_community: &HashMap<String, String>,
        references: &[Reference],
    ) -> HashMap<String, HashSet<String>> {
        let mut edges: HashMap<String, HashSet<String>> = HashMap::new();

        for reference in references {
            let from_str = reference.from_id.to_string();
            let to_str = reference.to_id.to_string();

            let Some(from_community) = member_to_community.get(&from_str) else {
                continue;
            };
            let Some(to_community) = member_to_community.get(&to_str) else {
                continue;
            };

            if from_community != to_community {
                edges
                    .entry(from_community.clone())
                    .or_default()
                    .insert(to_community.clone());
            }
        }

        edges
    }

    fn sanitize_mermaid_id(name: &str) -> String {
        name.chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knowledge_core::model::{Direction, RefType};

    fn rid(s: &str) -> knowledge_core::model::RecordIdType {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            surrealdb::sql::Thing::from((parts[0], parts[1]))
        } else {
            surrealdb::sql::Thing::from((s, ""))
        }
    }

    #[test]
    fn test_context_generator_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ContextGenerator>();
    }

    #[test]
    fn test_sanitize_mermaid_id_alphanumeric() {
        assert_eq!(
            ContextGenerator::sanitize_mermaid_id("HelloWorld"),
            "HelloWorld"
        );
    }

    #[test]
    fn test_sanitize_mermaid_id_with_spaces() {
        assert_eq!(
            ContextGenerator::sanitize_mermaid_id("Rust 异步运行时"),
            "Rust_异步运行时"
        );
    }

    #[test]
    fn test_sanitize_mermaid_id_with_special_chars() {
        assert_eq!(
            ContextGenerator::sanitize_mermaid_id("Module-A/B"),
            "Module_A_B"
        );
    }

    #[test]
    fn test_sanitize_mermaid_id_empty() {
        assert_eq!(ContextGenerator::sanitize_mermaid_id(""), "");
    }

    #[test]
    fn test_build_member_community_map_basic() {
        let communities = vec![
            Community {
                id: None,
                name: "ModuleA".to_string(),
                cohesion_score: 0.9,
                member_ids: vec![rid("token:a1"), rid("token:a2")],
            },
            Community {
                id: None,
                name: "ModuleB".to_string(),
                cohesion_score: 0.8,
                member_ids: vec![rid("token:b1")],
            },
        ];

        let map = ContextGenerator::build_member_community_map(&communities);

        assert_eq!(map.get("token:a1"), Some(&"ModuleA".to_string()));
        assert_eq!(map.get("token:a2"), Some(&"ModuleA".to_string()));
        assert_eq!(map.get("token:b1"), Some(&"ModuleB".to_string()));
        assert_eq!(map.get("token:c1"), None);
    }

    #[test]
    fn test_build_member_community_map_empty() {
        let communities: Vec<Community> = vec![];
        let map = ContextGenerator::build_member_community_map(&communities);
        assert!(map.is_empty());
    }

    #[test]
    fn test_compute_community_edges_cross_community() {
        let mut member_map = HashMap::new();
        member_map.insert("token:a".to_string(), "ModuleA".to_string());
        member_map.insert("token:b".to_string(), "ModuleB".to_string());
        member_map.insert("token:c".to_string(), "ModuleA".to_string());

        let references = vec![Reference {
            id: None,
            ref_type: RefType::Usage,
            direction: Direction::OneWay,
            scope: None,
            from_id: rid("token:a"),
            to_id: rid("token:b"),
        }];

        let edges = ContextGenerator::compute_community_edges(&member_map, &references);

        assert!(edges.contains_key("ModuleA"));
        assert!(edges["ModuleA"].contains("ModuleB"));
    }

    #[test]
    fn test_compute_community_edges_same_community_ignored() {
        let mut member_map = HashMap::new();
        member_map.insert("token:a".to_string(), "ModuleA".to_string());
        member_map.insert("token:b".to_string(), "ModuleA".to_string());

        let references = vec![Reference {
            id: None,
            ref_type: RefType::Usage,
            direction: Direction::OneWay,
            scope: None,
            from_id: rid("token:a"),
            to_id: rid("token:b"),
        }];

        let edges = ContextGenerator::compute_community_edges(&member_map, &references);

        assert!(!edges.contains_key("ModuleA"));
    }

    #[test]
    fn test_compute_community_edges_unknown_member_ignored() {
        let mut member_map = HashMap::new();
        member_map.insert("token:a".to_string(), "ModuleA".to_string());

        let references = vec![Reference {
            id: None,
            ref_type: RefType::Usage,
            direction: Direction::OneWay,
            scope: None,
            from_id: rid("token:a"),
            to_id: rid("token:unknown"),
        }];

        let edges = ContextGenerator::compute_community_edges(&member_map, &references);

        assert!(!edges.contains_key("ModuleA"));
    }

    #[test]
    fn test_compute_community_edges_multiple_edges() {
        let mut member_map = HashMap::new();
        member_map.insert("token:a".to_string(), "ModuleA".to_string());
        member_map.insert("token:b".to_string(), "ModuleB".to_string());
        member_map.insert("token:c".to_string(), "ModuleC".to_string());

        let references = vec![
            Reference {
                id: None,
                ref_type: RefType::Usage,
                direction: Direction::OneWay,
                scope: None,
                from_id: rid("token:a"),
                to_id: rid("token:b"),
            },
            Reference {
                id: None,
                ref_type: RefType::Definition,
                direction: Direction::OneWay,
                scope: None,
                from_id: rid("token:a"),
                to_id: rid("token:c"),
            },
        ];

        let edges = ContextGenerator::compute_community_edges(&member_map, &references);

        assert!(edges.contains_key("ModuleA"));
        assert!(edges["ModuleA"].contains("ModuleB"));
        assert!(edges["ModuleA"].contains("ModuleC"));
        assert_eq!(edges["ModuleA"].len(), 2);
    }

    #[test]
    fn test_compute_community_edges_deduplication() {
        let mut member_map = HashMap::new();
        member_map.insert("token:a".to_string(), "ModuleA".to_string());
        member_map.insert("token:b".to_string(), "ModuleB".to_string());
        member_map.insert("token:c".to_string(), "ModuleA".to_string());

        let references = vec![
            Reference {
                id: None,
                ref_type: RefType::Usage,
                direction: Direction::OneWay,
                scope: None,
                from_id: rid("token:a"),
                to_id: rid("token:b"),
            },
            Reference {
                id: None,
                ref_type: RefType::Definition,
                direction: Direction::OneWay,
                scope: None,
                from_id: rid("token:c"),
                to_id: rid("token:b"),
            },
        ];

        let edges = ContextGenerator::compute_community_edges(&member_map, &references);

        assert!(edges.contains_key("ModuleA"));
        assert!(edges["ModuleA"].contains("ModuleB"));
        assert_eq!(edges["ModuleA"].len(), 1);
    }

    #[test]
    fn test_compute_community_edges_empty_references() {
        let member_map = HashMap::<String, String>::new();
        let references: Vec<Reference> = vec![];
        let edges = ContextGenerator::compute_community_edges(&member_map, &references);
        assert!(edges.is_empty());
    }
}

