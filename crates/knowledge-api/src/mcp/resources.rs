//! MCP Resource 端点定义
//!
//! 定义 MCP 协议中的 Resource 端点，将知识图谱中的
//! 结构化数据作为可订阅的资源暴露给 LLM 客户端。
//!
//! ## 资源 URI 方案
//!
//! | URI | 类型 | 说明 |
//! | :- | :- | :- |
//! | `knowledge://repos` | 静态资源 | 列出所有已索引的仓库 |
//! | `knowledge://repo/{name}/context` | 模板资源 | 仓库上下文统计（文档数、Block 数、引用数） |
//! | `knowledge://repo/{name}/schema` | 模板资源 | SurrealDB 图谱 Schema 定义 |

use rmcp::model::{
    AnnotateAble, RawResource, RawResourceTemplate, Resource, ResourceContents, ResourceTemplate,
};

/// 已索引仓库列表的静态资源 URI
pub const URI_REPOS: &str = "knowledge://repos";

/// 仓库上下文统计的模板资源 URI
pub const URI_TEMPLATE_REPO_CONTEXT: &str = "knowledge://repo/{name}/context";

/// 仓库 Schema 定义的模板资源 URI
pub const URI_TEMPLATE_REPO_SCHEMA: &str = "knowledge://repo/{name}/schema";

/// 构建所有静态资源列表
///
/// 返回不包含动态参数的 MCP Resource 列表。
#[must_use]
pub fn build_static_resources() -> Vec<Resource> {
    vec![
        RawResource {
            uri: URI_REPOS.to_string(),
            name: "indexed-repositories".to_string(),
            title: Some("已索引仓库列表".to_string()),
            description: Some("列出所有已索引的知识图谱仓库".to_string()),
            mime_type: Some("application/json".to_string()),
            size: None,
            icons: None,
        }
        .no_annotation(),
    ]
}

/// 构建所有模板资源列表
///
/// 返回包含 URI 模板的 `MCP` `ResourceTemplate` 列表，
/// 客户端可通过模板构造具体资源 URI。
#[must_use]
pub fn build_resource_templates() -> Vec<ResourceTemplate> {
    vec![
        RawResourceTemplate {
            uri_template: URI_TEMPLATE_REPO_CONTEXT.to_string(),
            name: "repo-context".to_string(),
            title: Some("仓库上下文统计".to_string()),
            description: Some("返回指定仓库的上下文统计信息：文档数、Block 数、引用数".to_string()),
            mime_type: Some("application/json".to_string()),
        }
        .no_annotation(),
        RawResourceTemplate {
            uri_template: URI_TEMPLATE_REPO_SCHEMA.to_string(),
            name: "repo-schema".to_string(),
            title: Some("仓库图谱 Schema".to_string()),
            description: Some("返回指定仓库的 SurrealDB 图谱 Schema 定义".to_string()),
            mime_type: Some("application/json".to_string()),
        }
        .no_annotation(),
    ]
}

/// 解析资源 URI，提取仓库名称和资源类型
///
/// # 支持的 URI 格式
///
/// - `knowledge://repos` → `ResourceUri::Repos`
/// - `knowledge://repo/{name}/context` → `ResourceUri::RepoContext(name)`
/// - `knowledge://repo/{name}/schema` → `ResourceUri::RepoSchema(name)`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceUri {
    /// 已索引仓库列表
    Repos,
    /// 仓库上下文统计
    RepoContext(String),
    /// 仓库图谱 Schema
    RepoSchema(String),
}

/// 从 URI 字符串解析为 [`ResourceUri`]
///
/// # Errors
///
/// 当 URI 格式不被识别时返回错误描述字符串。
pub fn parse_resource_uri(uri: &str) -> Result<ResourceUri, String> {
    if uri == URI_REPOS {
        return Ok(ResourceUri::Repos);
    }

    let prefix_ctx = "knowledge://repo/";
    let suffix_ctx = "/context";
    let suffix_schema = "/schema";

    if let Some(rest) = uri.strip_prefix(prefix_ctx) {
        if let Some(name) = rest.strip_suffix(suffix_ctx) {
            if name.is_empty() {
                return Err("仓库名称不能为空".to_string());
            }
            return Ok(ResourceUri::RepoContext(name.to_string()));
        }
        if let Some(name) = rest.strip_suffix(suffix_schema) {
            if name.is_empty() {
                return Err("仓库名称不能为空".to_string());
            }
            return Ok(ResourceUri::RepoSchema(name.to_string()));
        }
    }

    Err(format!("无法识别的资源 URI: {uri}"))
}

/// 构建仓库上下文统计的 JSON 内容
#[must_use]
pub fn build_repo_context_json(
    repo_name: &str,
    doc_count: usize,
    block_count: usize,
    ref_count: usize,
) -> String {
    serde_json::json!({
        "repo": repo_name,
        "document_count": doc_count,
        "block_count": block_count,
        "ref_count": ref_count,
    })
    .to_string()
}

/// 构建 `SurrealDB` 图谱 Schema 定义
#[must_use]
pub fn build_schema_definition() -> String {
    serde_json::json!({
        "tables": {
            "document": {
                "fields": ["id", "path", "hash", "language", "created_at", "updated_at"],
                "description": "源代码文档元数据"
            },
            "block": {
                "fields": ["id", "doc_id", "block_type", "content", "start_line", "end_line", "embedding"],
                "description": "文档中的语义块（函数、类、接口等）"
            },
            "token": {
                "fields": ["id", "block_id", "content", "token_type", "start_char", "end_char"],
                "description": "语义块中的符号 Token"
            },
            "reference": {
                "fields": ["id", "from_id", "to_id", "ref_type"],
                "description": "Token 之间的引用关系（Definition / Usage / Import）"
            }
        },
        "relationships": {
            "document → block": "一对多：一个文档包含多个 Block",
            "block → token": "一对多：一个 Block 包含多个 Token",
            "token → token (reference)": "多对多：Token 之间通过 reference 表建立引用关系"
        },
        "ref_type_enum": ["Definition", "Usage", "Import"]
    })
    .to_string()
}

/// 读取资源内容
///
/// 根据解析后的 [`ResourceUri`]，通过 `KnowledgeVM` 查询数据库并返回资源内容。
///
/// # Errors
///
/// 当 URI 格式无效或数据库查询失败时返回错误。
pub fn read_resource(
    uri: &str,
    vm: &crate::KnowledgeVM,
) -> crate::Result<Vec<ResourceContents>> {
    let parsed = parse_resource_uri(uri).map_err(|e| error_core::helpers::internal_error(&e))?;

    match parsed {
        ResourceUri::Repos => {
            Err(error_core::helpers::internal_error(
                "多仓库注册表尚未实现，当前仅支持单仓库模式",
            ))
        }
        ResourceUri::RepoContext(name) => {
            let doc_count = count_documents(vm)?;
            let block_count = count_blocks(vm)?;
            let ref_count = count_references(vm)?;
            let content = build_repo_context_json(&name, doc_count, block_count, ref_count);
            Ok(vec![ResourceContents::text(content, uri)])
        }
        ResourceUri::RepoSchema(_name) => {
            let content = build_schema_definition();
            Ok(vec![ResourceContents::text(content, uri)])
        }
    }
}

fn count_documents(vm: &crate::KnowledgeVM) -> crate::Result<usize> {
    let response = vm
        .execute_parameterized_query(
            "SELECT count() AS total FROM document GROUP ALL",
            &serde_json::json!({}),
        )
        .map_err(|e| error_core::helpers::internal_error(&e.to_string()))?;
    #[allow(clippy::cast_possible_truncation)]
    let total = response
        .into_iter()
        .next()
        .and_then(|v| v.get("total").and_then(serde_json::Value::as_u64))
        .unwrap_or(0) as usize;
    Ok(total)
}


fn count_blocks(vm: &crate::KnowledgeVM) -> crate::Result<usize> {
    let response = vm
        .execute_parameterized_query(
            "SELECT count() AS total FROM block GROUP ALL",
            &serde_json::json!({}),
        )
        .map_err(|e| error_core::helpers::internal_error(&e.to_string()))?;
    #[allow(clippy::cast_possible_truncation)]
    let total = response
        .into_iter()
        .next()
        .and_then(|v| v.get("total").and_then(serde_json::Value::as_u64))
        .unwrap_or(0) as usize;
    Ok(total)
}

fn count_references(vm: &crate::KnowledgeVM) -> crate::Result<usize> {
    let response = vm
        .execute_parameterized_query(
            "SELECT count() AS total FROM reference GROUP ALL",
            &serde_json::json!({}),
        )
        .map_err(|e| error_core::helpers::internal_error(&e.to_string()))?;
    #[allow(clippy::cast_possible_truncation)]
    let total = response
        .into_iter()
        .next()
        .and_then(|v| v.get("total").and_then(serde_json::Value::as_u64))
        .unwrap_or(0) as usize;
    Ok(total)
}
