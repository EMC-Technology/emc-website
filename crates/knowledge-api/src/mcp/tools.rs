//! `MCP` Tool 定义
//!
//! 定义 `MCP` 协议中的 Tool 端点，将 `KnowledgeVM` 的查询能力
//! 映射为 LLM 可调用的工具函数。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    /// 风险评估（low / medium / high）
    pub risk_level: String,
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
    pub ref_type: String,
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

        let first_word = normalized.split_whitespace().next().unwrap_or("");
        if first_word != "SELECT" {
            return Err(format!(
                "仅允许 SELECT 查询，当前查询以 '{first_word}' 开头"
            ));
        }

        let dangerous_keywords = [
            "CREATE", "UPDATE", "DELETE", "INSERT", "RELATE",
            "DEFINE", "REMOVE", "REBUILD", "BEGIN", "COMMIT",
            "CANCEL", "LET ", "RETURN ",
        ];

        let after_select = &normalized[6..];
        for keyword in &dangerous_keywords {
            if after_select.contains(keyword) {
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

/// `detect_changes` 工具结果
#[derive(Debug, Clone, Serialize)]
pub struct DetectChangesResult {
    /// 文档 ID
    pub doc_id: String,
    /// 变更状态（unchanged / modified / deleted）
    pub status: String,
    /// 当前哈希值
    pub current_hash: Option<String>,
    /// 受影响的引用数量
    pub affected_references: usize,
    /// 受影响的引用列表
    pub affected_refs_detail: Vec<serde_json::Value>,
}
