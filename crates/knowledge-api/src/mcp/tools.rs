//! `MCP` Tool 定义
//!
//! 定义 `MCP` 协议中的 Tool 端点，将 `KnowledgeVM` 的查询能力
//! 映射为 LLM 可调用的工具函数。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::knowledge_vm::RiskLevel;

// ============================================================================
// Tool 1: query - 混合搜索
// ============================================================================

/// query 工具参数
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct QueryParams {
    /// 搜索查询文本
    pub query: String,
    /// 返回结果数量上限，默认 10
    pub limit: Option<u32>,
}

/// query 工具结果
#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    /// 匹配的 Block 列表
    pub blocks: Vec<serde_json::Value>,
    /// 结果总数
    pub total: usize,
}

// ============================================================================
// Tool 2: context - 360° 符号上下文视图
// ============================================================================

/// context 工具参数
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ContextParams {
    /// 目标符号标识（Token ID）
    pub symbol: String,
}

/// context 工具结果
#[derive(Debug, Clone, Serialize)]
pub struct ContextResult {
    /// 目标符号
    pub symbol: String,
    /// 调用方列表（谁引用了此符号）
    pub callers: Vec<serde_json::Value>,
    /// 被调用方列表（此符号引用了谁）
    pub callees: Vec<serde_json::Value>,
    /// 所有引用关系
    pub references: Vec<serde_json::Value>,
}

// ============================================================================
// Tool 3: impact - 多跳影响分析
// ============================================================================

/// impact 工具参数
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ImpactParams {
    /// 目标符号标识（Token ID）
    pub symbol: String,
    /// BFS 遍历深度，默认 3
    pub depth: Option<u32>,
}

/// impact 工具单层结果
#[derive(Debug, Clone, Serialize)]
pub struct ImpactLayer {
    /// 当前跳数（从 1 开始）
    pub hop: u32,
    /// 本层受影响的符号列表
    pub affected: Vec<serde_json::Value>,
    /// 本层受影响数量
    pub count: usize,
}

/// impact 工具结果
#[derive(Debug, Clone, Serialize)]
pub struct ImpactResult {
    /// 起始符号
    pub symbol: String,
    /// 按跳数分层的结果
    pub layers: Vec<ImpactLayer>,
    /// 总受影响数量
    pub total_affected: usize,
    /// 风险评估
    pub risk_level: RiskLevel,
}

// ============================================================================
// Tool 4: trace - 执行流追踪
// ============================================================================

/// trace 工具参数
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TraceParams {
    /// 入口点符号标识（Token ID）
    pub entry_point: String,
    /// 最大追踪深度，默认 10
    pub max_depth: Option<u32>,
}

/// trace 工具单步结果
#[derive(Debug, Clone, Serialize)]
pub struct TraceStep {
    /// 步骤序号（从 1 开始）
    pub step: u32,
    /// 符号标识
    pub symbol: String,
    /// 引用类型
    pub ref_type: knowledge_core::model::RefType,
    /// 置信度分数（0.0 ~ 1.0）
    pub confidence: f64,
}

/// trace 工具结果
#[derive(Debug, Clone, Serialize)]
pub struct TraceResult {
    /// 入口点符号
    pub entry_point: String,
    /// 有序执行步骤
    pub steps: Vec<TraceStep>,
    /// 总步骤数
    pub total_steps: usize,
}

// ============================================================================
// Tool 5: search - 受限 SurrealQL 查询（仅允许 SELECT）
// ============================================================================

/// search 工具参数
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SearchParams {
    /// `SurrealQL` 查询语句（仅允许 SELECT 查询）
    pub query: String,
    /// 可选查询参数绑定
    pub params: Option<serde_json::Value>,
}

/// search 工具结果
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    /// 查询结果行
    pub rows: Vec<serde_json::Value>,
    /// 结果行数
    pub row_count: usize,
}

impl SearchParams {
    /// 验证查询安全性：仅允许 SELECT 语句，使用白名单模式
    ///
    /// # 安全设计
    ///
    /// 采用白名单验证策略，仅允许以 `SELECT` 开头的查询。
    /// 同时检测查询体中是否包含任何 DDL/DML 语句关键字，
    /// 防止通过子查询、注释等方式注入写操作。
    ///
    /// # 防护策略
    ///
    /// 1. **白名单首词**：查询首词必须为 `SELECT`（大小写不敏感）
    /// 2. **黑名单关键字扫描**：查询体中不得包含 DDL/DML 关键字
    /// 3. **长度限制**：查询不超过 10000 字符
    /// 4. **注释剥离**：移除 `--` 和 `/* */` 注释后再检测关键字
    /// 5. **分号防护**：禁止分号，防止多语句注入
    /// 6. **字符串字面量剥离**：移除引号内内容后再检测关键字，防止合法数据值误判
    ///
    /// # Errors
    ///
    /// 当查询不满足安全条件时返回错误描述。
    pub fn validate_query(&self) -> Result<(), String> {
        if self.query.len() > 10_000 {
            return Err("查询长度超过 10000 字符限制".to_string());
        }

        let stripped = strip_surrealql_comments(&self.query);
        let normalized = stripped.trim().to_uppercase();

        if normalized.is_empty() {
            return Err("查询不能为空".to_string());
        }

        if normalized.contains(';') {
            return Err("查询不允许包含分号（防止多语句注入）".to_string());
        }

        let first_word = normalized.split_whitespace().next().unwrap_or("");
        if first_word != "SELECT" {
            return Err(format!(
                "仅允许 SELECT 查询，当前查询以 '{first_word}' 开头"
            ));
        }

        let dangerous_keywords = [
            "CREATE", "UPDATE", "DELETE", "INSERT", "RELATE", "DEFINE", "REMOVE", "REBUILD",
            "BEGIN", "COMMIT", "CANCEL", "LET ", "RETURN ",
        ];

        let after_select = &normalized[6..];
        let code_only = strip_string_literals(after_select);
        for keyword in &dangerous_keywords {
            if code_only.contains(keyword) {
                return Err(format!("查询包含禁止的关键字: {}", keyword.trim()));
            }
        }

        Ok(())
    }
}

/// 剥离 `SurrealQL` 中的注释，防止通过注释绕过关键字检测
///
/// 支持 `--` 单行注释和 `/* */` 多行注释。
fn strip_surrealql_comments(query: &str) -> String {
    let mut result = String::with_capacity(query.len());
    let chars: Vec<char> = query.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '-' && chars[i + 1] == '-' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < chars.len() {
                i += 2;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// 剥离 SQL 中的字符串字面量，防止合法数据值中的关键字误判
///
/// 将单引号和双引号内的内容替换为占位符 `?`，仅保留引号结构。
/// 例如 `WHERE title = 'CREATE'` 变为 `WHERE title = ?`，
/// 避免数据值中的 `CREATE` 被黑名单错误拦截。
fn strip_string_literals(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '\'' {
            result.push('\'');
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                i += 1;
            }
            if i < chars.len() {
                result.push('?');
                result.push('\'');
                i += 1;
            }
        } else if chars[i] == '"' {
            result.push('"');
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            if i < chars.len() {
                result.push('?');
                result.push('"');
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

// ============================================================================
// Tool 6: graph - 文档图谱可视化数据
// ============================================================================

/// graph 工具参数
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GraphParams {
    /// 文档 ID
    pub doc_id: String,
}

/// graph 工具结果
#[derive(Debug, Clone, Serialize)]
pub struct GraphResult {
    /// 节点列表（Block）
    pub nodes: Vec<serde_json::Value>,
    /// 边列表（Reference）
    pub edges: Vec<serde_json::Value>,
    /// 节点数量
    pub node_count: usize,
    /// 边数量
    pub edge_count: usize,
}

// ============================================================================
// Tool 7: detect_changes - 变更影响检测
// ============================================================================

/// `detect_changes` 工具参数
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DetectChangesParams {
    /// 文档 ID
    pub doc_id: String,
}

/// 变更检测状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeStatus {
    /// 无变更
    Unchanged,
    /// 已修改
    Modified,
    /// 已删除
    Deleted,
}

/// `detect_changes` 工具结果
#[derive(Debug, Clone, Serialize)]
pub struct DetectChangesResult {
    /// 文档 ID
    pub doc_id: String,
    /// 变更状态
    pub status: ChangeStatus,
    /// 当前哈希值
    pub current_hash: Option<String>,
    /// 受影响的引用数量
    pub affected_references: usize,
    /// 受影响的引用列表
    pub affected_refs_detail: Vec<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_params_validate_valid_select() {
        let params = SearchParams {
            query: "SELECT * FROM document".to_string(),
            params: None,
        };
        assert!(params.validate_query().is_ok());
    }

    #[test]
    fn test_search_params_validate_rejects_non_select() {
        let params = SearchParams {
            query: "DELETE FROM document".to_string(),
            params: None,
        };
        assert!(params.validate_query().is_err());
    }

    #[test]
    fn test_search_params_validate_rejects_empty() {
        let params = SearchParams {
            query: "   ".to_string(),
            params: None,
        };
        let err = params.validate_query().unwrap_err();
        assert!(err.contains("空"));
    }

    #[test]
    fn test_search_params_validate_rejects_semicolon() {
        let params = SearchParams {
            query: "SELECT * FROM document; DROP TABLE document".to_string(),
            params: None,
        };
        let err = params.validate_query().unwrap_err();
        assert!(err.contains("分号"));
    }

    #[test]
    fn test_search_params_validate_rejects_too_long() {
        let params = SearchParams {
            query: "SELECT * FROM document WHERE ".to_string() + &"x".repeat(10_000),
            params: None,
        };
        let err = params.validate_query().unwrap_err();
        assert!(err.contains("10000"));
    }

    #[test]
    fn test_search_params_validate_rejects_dangerous_keywords() {
        let cases = [
            (
                "SELECT * FROM document WHERE x = 1 CREATE TABLE t",
                "CREATE",
            ),
            (
                "SELECT * FROM document WHERE x = 1 UPDATE t SET a = 1",
                "UPDATE",
            ),
            ("SELECT * FROM document WHERE x = 1 DELETE FROM t", "DELETE"),
            ("SELECT * FROM document WHERE x = 1 INSERT INTO t", "INSERT"),
            (
                "SELECT * FROM document WHERE x = 1 RELATE a->b->c",
                "RELATE",
            ),
            (
                "SELECT * FROM document WHERE x = 1 DEFINE TABLE t",
                "DEFINE",
            ),
            (
                "SELECT * FROM document WHERE x = 1 REMOVE TABLE t",
                "REMOVE",
            ),
            (
                "SELECT * FROM document WHERE x = 1 REBUILD INDEX",
                "REBUILD",
            ),
            (
                "SELECT * FROM document WHERE x = 1 BEGIN TRANSACTION",
                "BEGIN",
            ),
            ("SELECT * FROM document WHERE x = 1 COMMIT", "COMMIT"),
            (
                "SELECT * FROM document WHERE x = 1 CANCEL TRANSACTION",
                "CANCEL",
            ),
            ("SELECT * FROM document WHERE x = 1 LET $x = 1", "LET"),
            ("SELECT * FROM document WHERE x = 1 RETURN 1", "RETURN"),
        ];
        for (query, keyword) in &cases {
            let params = SearchParams {
                query: (*query).to_string(),
                params: None,
            };
            let err = params.validate_query().unwrap_err();
            assert!(
                err.contains(keyword),
                "Expected error for keyword '{keyword}' in query: {query}, got: {err}"
            );
        }
    }

    #[test]
    fn test_search_params_validate_allows_safe_string_literals() {
        let params = SearchParams {
            query: "SELECT * FROM document WHERE title = 'CREATE something'".to_string(),
            params: None,
        };
        assert!(params.validate_query().is_ok());
    }

    #[test]
    fn test_search_params_validate_case_insensitive() {
        let params = SearchParams {
            query: "select * from document".to_string(),
            params: None,
        };
        assert!(params.validate_query().is_ok());
    }

    #[test]
    fn test_strip_surrealql_comments_line() {
        let input = "SELECT * FROM doc -- this is a comment\nWHERE x = 1";
        let stripped = strip_surrealql_comments(input);
        assert!(!stripped.contains("comment"));
        assert!(stripped.contains("SELECT"));
        assert!(stripped.contains("WHERE"));
    }

    #[test]
    fn test_strip_surrealql_comments_block() {
        let input = "SELECT * FROM doc /* block comment */ WHERE x = 1";
        let stripped = strip_surrealql_comments(input);
        assert!(!stripped.contains("block comment"));
        assert!(stripped.contains("SELECT"));
        assert!(stripped.contains("WHERE"));
    }

    #[test]
    fn test_strip_string_literals_single_quotes() {
        let input = " WHERE title = 'CREATE' AND x = 1 ";
        let stripped = strip_string_literals(input);
        assert!(!stripped.contains("CREATE"));
        assert!(stripped.contains("WHERE"));
        assert!(stripped.contains('?'));
    }

    #[test]
    fn test_strip_string_literals_double_quotes() {
        let input = " WHERE title = \"DELETE\" AND x = 1 ";
        let stripped = strip_string_literals(input);
        assert!(!stripped.contains("DELETE"));
        assert!(stripped.contains("WHERE"));
    }

    #[test]
    fn test_change_status_serialization() {
        let statuses = [
            ChangeStatus::Unchanged,
            ChangeStatus::Modified,
            ChangeStatus::Deleted,
        ];
        let json = serde_json::to_string(&statuses).unwrap();
        assert!(json.contains("unchanged"));
        assert!(json.contains("modified"));
        assert!(json.contains("deleted"));
    }

    #[test]
    fn test_query_params_deserialization() {
        let json = r#"{"query": "test query", "limit": 5}"#;
        let params: QueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.query, "test query");
        assert_eq!(params.limit, Some(5));
    }

    #[test]
    fn test_impact_params_deserialization() {
        let json = r#"{"symbol": "fn_main", "depth": 5}"#;
        let params: ImpactParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.symbol, "fn_main");
        assert_eq!(params.depth, Some(5));
    }

    #[test]
    fn test_trace_params_deserialization() {
        let json = r#"{"entry_point": "main", "max_depth": 20}"#;
        let params: TraceParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.entry_point, "main");
        assert_eq!(params.max_depth, Some(20));
    }

    #[test]
    fn test_graph_params_deserialization() {
        let json = r#"{"doc_id": "doc:001"}"#;
        let params: GraphParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.doc_id, "doc:001");
    }

    #[test]
    fn test_detect_changes_params_deserialization() {
        let json = r#"{"doc_id": "doc:001"}"#;
        let params: DetectChangesParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.doc_id, "doc:001");
    }

    #[test]
    fn test_query_result_serialization() {
        let result = QueryResult {
            blocks: vec![serde_json::json!({"id": 1})],
            total: 1,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("blocks"));
        assert!(json.contains("total"));
    }

    #[test]
    fn test_impact_result_serialization() {
        let result = ImpactResult {
            symbol: "fn_main".to_string(),
            layers: vec![ImpactLayer {
                hop: 1,
                affected: vec![],
                count: 0,
            }],
            total_affected: 0,
            risk_level: RiskLevel::Low,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("fn_main"));
    }

    #[test]
    fn test_detect_changes_result_serialization() {
        let result = DetectChangesResult {
            doc_id: "doc:001".to_string(),
            status: ChangeStatus::Modified,
            current_hash: Some("abc123".to_string()),
            affected_references: 5,
            affected_refs_detail: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("modified"));
        assert!(json.contains("abc123"));
    }
}
