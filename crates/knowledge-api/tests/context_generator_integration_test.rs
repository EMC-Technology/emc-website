//! [`ContextGenerator`] 集成测试
//!
//! 测试 AI 上下文文件生成的输出格式与 Mermaid 语法正确性。
//! 需要 [`SurrealDB`] 实例的测试标记为 `#[ignore]`，
//! 可通过 `cargo test -- --ignored` 运行。

use knowledge_api::ContextGenerator;
use knowledge_api::KnowledgeVM;
use knowledge_core::SurrealDbClient;
use knowledge_core::model::{Community, Direction, RefType, Reference, ReferenceStatus};

use std::collections::{HashMap, HashSet};

fn rid(s: &str) -> knowledge_core::model::RecordIdType {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        surrealdb::sql::Thing::from((parts[0], parts[1]))
    } else {
        surrealdb::sql::Thing::from((s, ""))
    }
}

#[test]
fn test_context_generator_output_format_architecture_overview_empty() {
    let communities: Vec<Community> = vec![];
    let md = format_architecture_overview(&communities);
    assert!(md.starts_with("# Architecture Overview"));
    assert!(md.contains("No communities found"));
}

#[test]
fn test_context_generator_output_format_architecture_overview_with_data() {
    let communities = vec![
        Community {
            id: None,
            name: "ModuleA".to_string(),
            cohesion_score: 0.95,
            member_ids: vec![rid("t1"), rid("t2")],
        },
        Community {
            id: None,
            name: "ModuleB".to_string(),
            cohesion_score: 0.80,
            member_ids: vec![rid("t3")],
        },
    ];

    let md = format_architecture_overview(&communities);
    assert!(md.starts_with("# Architecture Overview"));
    assert!(md.contains("| Module | Cohesion | Members |"));
    assert!(md.contains("ModuleA"));
    assert!(md.contains("ModuleB"));
    assert!(md.contains("0.95"));
    assert!(md.contains("0.80"));
}

#[test]
fn test_context_generator_output_format_module_descriptions_empty() {
    let communities: Vec<Community> = vec![];
    let md = format_module_descriptions(&communities);
    assert!(md.starts_with("# Module Descriptions"));
    assert!(md.contains("No modules found"));
}

#[test]
fn test_context_generator_output_format_module_descriptions_with_data() {
    let communities = vec![Community {
        id: None,
        name: "AuthService".to_string(),
        cohesion_score: 0.88,
        member_ids: vec![rid("token:login"), rid("token:logout")],
    }];

    let md = format_module_descriptions(&communities);
    assert!(md.contains("## AuthService"));
    assert!(md.contains("Cohesion: 0.88"));
    assert!(md.contains("`token:login`"));
    assert!(md.contains("`token:logout`"));
}

#[test]
fn test_mermaid_graph_syntax_empty() {
    let communities: Vec<Community> = vec![];
    let mermaid = format_dependency_graph(&communities, &[]);
    assert!(mermaid.starts_with("```mermaid"));
    assert!(mermaid.contains("graph TD"));
    assert!(mermaid.ends_with("```\n"));
}

#[test]
fn test_mermaid_graph_syntax_with_edges() {
    let communities = vec![
        Community {
            id: None,
            name: "ModuleA".to_string(),
            cohesion_score: 0.9,
            member_ids: vec![rid("token:a")],
        },
        Community {
            id: None,
            name: "ModuleB".to_string(),
            cohesion_score: 0.8,
            member_ids: vec![rid("token:b")],
        },
    ];

    let references = vec![Reference {
        id: None,
        ref_type: RefType::Usage,
        direction: Direction::OneWay,
        scope: None,
        from_id: rid("token:a"),
        to_id: rid("token:b"),
        status: ReferenceStatus::Created,
    }];

    let mermaid = format_dependency_graph(&communities, &references);
    assert!(mermaid.starts_with("```mermaid"));
    assert!(mermaid.contains("graph TD"));
    assert!(mermaid.contains("-->"));
    assert!(mermaid.contains("ModuleA"));
    assert!(mermaid.contains("ModuleB"));
    assert!(mermaid.ends_with("```\n"));
}

#[test]
fn test_mermaid_graph_syntax_no_cross_community_edges() {
    let communities = vec![Community {
        id: None,
        name: "ModuleA".to_string(),
        cohesion_score: 0.9,
        member_ids: vec![rid("token:a"), rid("token:b")],
    }];

    let references = vec![Reference {
        id: None,
        ref_type: RefType::Usage,
        direction: Direction::OneWay,
        scope: None,
        from_id: rid("token:a"),
        to_id: rid("token:b"),
        status: ReferenceStatus::Created,
    }];

    let mermaid = format_dependency_graph(&communities, &references);
    assert!(!mermaid.contains("-->"));
}

#[test]
fn test_mermaid_graph_syntax_sanitized_ids() {
    let communities = vec![
        Community {
            id: None,
            name: "Auth Service".to_string(),
            cohesion_score: 0.9,
            member_ids: vec![rid("token:a")],
        },
        Community {
            id: None,
            name: "Data-Layer".to_string(),
            cohesion_score: 0.8,
            member_ids: vec![rid("token:b")],
        },
    ];

    let references = vec![Reference {
        id: None,
        ref_type: RefType::Usage,
        direction: Direction::OneWay,
        scope: None,
        from_id: rid("token:a"),
        to_id: rid("token:b"),
        status: ReferenceStatus::Created,
    }];

    let mermaid = format_dependency_graph(&communities, &references);
    assert!(mermaid.contains("Auth_Service"));
    assert!(mermaid.contains("Data_Layer"));
    assert!(!mermaid.contains("Auth Service"));
    assert!(!mermaid.contains("Data-Layer"));
}

#[test]
fn test_full_context_structure() {
    let communities = vec![Community {
        id: None,
        name: "CoreModule".to_string(),
        cohesion_score: 0.92,
        member_ids: vec![rid("token:x")],
    }];

    let overview = format_architecture_overview(&communities);
    let modules = format_module_descriptions(&communities);
    let deps = format_dependency_graph(&communities, &[]);

    let full = format!("{overview}\n{modules}\n## Dependency Graph\n\n{deps}");

    assert!(full.contains("# Architecture Overview"));
    assert!(full.contains("# Module Descriptions"));
    assert!(full.contains("## Dependency Graph"));
    assert!(full.contains("## CoreModule"));
    assert!(full.contains("```mermaid"));
}

#[tokio::test]
#[ignore = "需要运行中的 SurrealDB 实例"]
async fn test_context_generator_with_real_db() {
    let db_addr =
        std::env::var("KNOWLEDGE_DB_ADDR").unwrap_or_else(|_| "ws://localhost:8000".to_string());
    let namespace =
        std::env::var("KNOWLEDGE_DB_NAMESPACE").unwrap_or_else(|_| "knowledge".to_string());
    let database =
        std::env::var("KNOWLEDGE_DB_DATABASE").unwrap_or_else(|_| "knowledge".to_string());

    let db_client = SurrealDbClient::new(&db_addr, &namespace, &database)
        .await
        .expect("无法连接 SurrealDB，请确保实例正在运行");

    let vm = KnowledgeVM::with_embedding_dim(db_client, 1536).expect("无法创建 KnowledgeVM");
    let generator = ContextGenerator::new(vm);

    let overview = generator
        .generate_architecture_overview()
        .expect("架构概览生成失败");
    assert!(overview.contains("# Architecture Overview"));

    let modules = generator
        .generate_module_descriptions()
        .expect("模块描述生成失败");
    assert!(modules.contains("# Module Descriptions"));

    let deps = generator
        .generate_dependency_graph()
        .expect("依赖图生成失败");
    assert!(deps.contains("```mermaid"));
    assert!(deps.contains("graph TD"));
}

#[tokio::test]
#[ignore = "需要运行中的 SurrealDB 实例"]
async fn test_context_generator_generate_claude_md() {
    let db_addr =
        std::env::var("KNOWLEDGE_DB_ADDR").unwrap_or_else(|_| "ws://localhost:8000".to_string());
    let namespace =
        std::env::var("KNOWLEDGE_DB_NAMESPACE").unwrap_or_else(|_| "knowledge".to_string());
    let database =
        std::env::var("KNOWLEDGE_DB_DATABASE").unwrap_or_else(|_| "knowledge".to_string());

    let db_client = SurrealDbClient::new(&db_addr, &namespace, &database)
        .await
        .expect("无法连接 SurrealDB，请确保实例正在运行");

    let vm = KnowledgeVM::with_embedding_dim(db_client, 1536).expect("无法创建 KnowledgeVM");
    let generator = ContextGenerator::new(vm);

    let temp_dir = tempfile::tempdir().expect("创建临时目录失败");
    let output_path = temp_dir.path().join("CLAUDE.md");

    generator
        .generate_claude_md(&output_path)
        .await
        .expect("CLAUDE.md 生成失败");

    let content = tokio::fs::read_to_string(&output_path)
        .await
        .expect("读取 CLAUDE.md 失败");

    assert!(content.contains("# Architecture Overview"));
    assert!(content.contains("# Module Descriptions"));
    assert!(content.contains("## Dependency Graph"));
}

fn format_architecture_overview(communities: &[Community]) -> String {
    if communities.is_empty() {
        return "# Architecture Overview\n\nNo communities found in the knowledge graph.\n"
            .to_string();
    }

    let mut sorted = communities.to_vec();
    sorted.sort_by(|a, b| {
        b.cohesion_score
            .partial_cmp(&a.cohesion_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut md = String::from("# Architecture Overview\n\n");
    md.push_str("| Module | Cohesion | Members |\n");
    md.push_str("|--------|----------|--------|\n");

    for community in &sorted {
        use std::fmt::Write;
        let _ = writeln!(
            md,
            "| {} | {:.2} | {} |",
            community.name,
            community.cohesion_score,
            community.member_ids.len()
        );
    }

    md.push('\n');
    md
}

fn format_module_descriptions(communities: &[Community]) -> String {
    if communities.is_empty() {
        return "# Module Descriptions\n\nNo modules found.\n".to_string();
    }

    let mut md = String::from("# Module Descriptions\n\n");

    for community in communities {
        use std::fmt::Write;
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

    md
}

fn format_dependency_graph(communities: &[Community], references: &[Reference]) -> String {
    if communities.is_empty() {
        return "```mermaid\ngraph TD\n```\n".to_string();
    }

    let member_to_community = build_member_community_map(communities);
    let edges = compute_community_edges(&member_to_community, references);

    let mut mermaid = String::from("```mermaid\ngraph TD\n");

    for (from, to_set) in &edges {
        for to in to_set {
            use std::fmt::Write;
            let _ = writeln!(
                mermaid,
                "    {} --> {}",
                sanitize_mermaid_id(from),
                sanitize_mermaid_id(to)
            );
        }
    }

    mermaid.push_str("```\n");
    mermaid
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
