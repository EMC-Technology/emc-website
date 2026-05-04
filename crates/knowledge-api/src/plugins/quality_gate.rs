use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::Result;

/// 质量门禁插件 — 闭源 knowledge-infra 的"插座"
///
/// 开源默认实现：[`AlwaysPassGate`](crate::plugins::defaults::AlwaysPassGate)（全部通过）
/// 闭源增强实现：InfraQualityGate（三段式门禁 + 形式化验证）
///
/// # 架构角色
///
/// 在 L4 质量保障闭环中，`QualityGatePlugin` 负责评估代码变更的质量，
/// 决定是否通过、需要人工审查或自动回退。
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::QualityGatePlugin;
/// use std::sync::Arc;
///
/// let gate: Arc<dyn QualityGatePlugin> = Arc::new(InfraQualityGate::new(engine, scanner));
/// registry.register_gate(gate);
/// ```
#[async_trait]
pub trait QualityGatePlugin: Send + Sync {
    /// 门禁唯一标识
    fn gate_id(&self) -> &str;

    /// 评估代码变更的质量
    async fn evaluate(&self, change: &CodeChange) -> Result<QualityVerdict>;

    /// 列出所有可用的检查规则
    async fn list_rules(&self) -> Result<Vec<RuleInfo>>;

    /// 健康检查
    async fn health_check(&self) -> Result<bool>;
}

/// 代码变更描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChange {
    /// 仓库名称
    pub repository: String,
    /// 分支名称
    pub branch: String,
    /// 提交 SHA
    pub commit_sha: String,
    /// 变更文件列表
    pub changed_files: Vec<String>,
    /// Diff 内容
    pub diff: Option<String>,
    /// 提交作者
    pub author: Option<String>,
    /// 提交消息
    pub message: Option<String>,
}

/// 质量评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityVerdict {
    /// 是否通过
    pub passed: bool,
    /// 质量评分（0.0 - 1.0）
    pub score: f64,
    /// 违规项列表
    pub violations: Vec<Violation>,
    /// 是否有自动修复可用
    pub auto_fix_available: bool,
    /// 详细信息
    pub details: serde_json::Value,
}

/// 违规项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    /// 规则 ID
    pub rule_id: String,
    /// 严重程度
    pub severity: Severity,
    /// 违规描述
    pub message: String,
    /// 涉及文件
    pub file: String,
    /// 涉及行号
    pub line: Option<u32>,
}

/// 严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    /// 信息
    Info,
    /// 警告
    Warning,
    /// 错误
    Error,
    /// 严重
    Critical,
}

/// 规则信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleInfo {
    /// 规则 ID
    pub rule_id: String,
    /// 规则名称
    pub name: String,
    /// 规则描述
    pub description: String,
    /// 严重程度
    pub severity: Severity,
    /// 是否启用
    pub enabled: bool,
}
