//! MCP Server 集成测试
//!
//! 测试 [`McpServer`] 的构造与工具注册。
//! 需要运行中 [`SurrealDB`] 实例的测试标记为 `#[ignore]`，
//! 可通过 `cargo test -- --ignored` 运行。

use knowledge_api::KnowledgeVM;
use knowledge_api::mcp::McpServer;
use knowledge_core::SurrealDbClient;

const EXPECTED_TOOL_NAMES: &[&str] = &[
    "query",
    "context",
    "impact",
    "trace",
    "search",
    "graph",
    "detect_changes",
];

#[test]
fn test_mcp_server_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<McpServer>();
}

#[test]
fn test_expected_tool_count_is_seven() {
    assert_eq!(EXPECTED_TOOL_NAMES.len(), 7);
}

#[test]
fn test_expected_tool_names_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for name in EXPECTED_TOOL_NAMES {
        assert!(seen.insert(*name), "重复的工具名称: {name}");
    }
}

#[tokio::test]
#[ignore = "需要运行中的 SurrealDB 实例"]
async fn test_mcp_server_construct_with_knowledge_vm() {
    let db_addr =
        std::env::var("KNOWLEDGE_DB_ADDR").unwrap_or_else(|_| "ws://localhost:8000".to_string());
    let namespace =
        std::env::var("KNOWLEDGE_DB_NAMESPACE").unwrap_or_else(|_| "knowledge".to_string());
    let database =
        std::env::var("KNOWLEDGE_DB_DATABASE").unwrap_or_else(|_| "knowledge".to_string());

    let db_client = SurrealDbClient::new(&db_addr, &namespace, &database)
        .await
        .expect("无法连接 SurrealDB，请确保实例正在运行");

    let vm = KnowledgeVM::with_embedding_dim(db_client, 1536)
        .expect("无法创建 KnowledgeVM");
    let _server = McpServer::new(vm);
}

#[tokio::test]
#[ignore = "需要运行中的 SurrealDB 实例"]
async fn test_mcp_server_tool_list_includes_all_tools() {
    let db_addr =
        std::env::var("KNOWLEDGE_DB_ADDR").unwrap_or_else(|_| "ws://localhost:8000".to_string());
    let namespace =
        std::env::var("KNOWLEDGE_DB_NAMESPACE").unwrap_or_else(|_| "knowledge".to_string());
    let database =
        std::env::var("KNOWLEDGE_DB_DATABASE").unwrap_or_else(|_| "knowledge".to_string());

    let db_client = SurrealDbClient::new(&db_addr, &namespace, &database)
        .await
        .expect("无法连接 SurrealDB，请确保实例正在运行");

    let vm = KnowledgeVM::with_embedding_dim(db_client, 1536)
        .expect("无法创建 KnowledgeVM");
    let server = McpServer::new(vm);

    let tool_names = server.registered_tool_names();
    let tool_name_set: std::collections::HashSet<&str> =
        tool_names.iter().map(|s: &String| s.as_str()).collect();

    for expected in EXPECTED_TOOL_NAMES {
        assert!(
            tool_name_set.contains(expected),
            "缺少工具: {expected}，已注册工具: {tool_names:?}"
        );
    }

    assert_eq!(
        tool_names.len(),
        EXPECTED_TOOL_NAMES.len(),
        "工具数量不匹配，期望 {} 个，实际 {} 个: {:?}",
        EXPECTED_TOOL_NAMES.len(),
        tool_names.len(),
        tool_names
    );
}

#[tokio::test]
#[ignore = "需要运行中的 SurrealDB 实例"]
async fn test_mcp_server_get_info_returns_valid_server_info() {
    use rmcp::ServerHandler;

    let db_addr =
        std::env::var("KNOWLEDGE_DB_ADDR").unwrap_or_else(|_| "ws://localhost:8000".to_string());
    let namespace =
        std::env::var("KNOWLEDGE_DB_NAMESPACE").unwrap_or_else(|_| "knowledge".to_string());
    let database =
        std::env::var("KNOWLEDGE_DB_DATABASE").unwrap_or_else(|_| "knowledge".to_string());

    let db_client = SurrealDbClient::new(&db_addr, &namespace, &database)
        .await
        .expect("无法连接 SurrealDB，请确保实例正在运行");

    let vm = KnowledgeVM::with_embedding_dim(db_client, 1536)
        .expect("无法创建 KnowledgeVM");
    let server = McpServer::new(vm);

    let info = server.get_info();

    assert!(!info.server_info.name.is_empty(), "服务名称不应为空");
    assert!(!info.server_info.version.is_empty(), "版本号不应为空");
    assert!(info.capabilities.tools.is_some(), "应启用 tools 能力");
    assert!(info.capabilities.prompts.is_some(), "应启用 prompts 能力");
    assert!(
        info.capabilities.resources.is_some(),
        "应启用 resources 能力"
    );
}
