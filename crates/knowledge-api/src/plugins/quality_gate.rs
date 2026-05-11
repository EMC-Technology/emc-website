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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_change_serialization() {
        let change = CodeChange {
            repository: "my-repo".to_string(),
            branch: "main".to_string(),
            commit_sha: "abc123".to_string(),
            changed_files: vec!["src/main.rs".to_string()],
            diff: Some("@@ -1,3 +1,3 @@".to_string()),
            author: Some("dev".to_string()),
            message: Some("fix bug".to_string()),
        };
        let json = serde_json::to_string(&change).unwrap();
        let de: CodeChange = serde_json::from_str(&json).unwrap();
        assert_eq!(de.repository, "my-repo");
        assert_eq!(de.commit_sha, "abc123");
        assert!(de.diff.is_some());
    }

    #[test]
    fn test_quality_verdict_serialization() {
        let verdict = QualityVerdict {
            passed: true,
            score: 0.95,
            violations: vec![],
            auto_fix_available: false,
            details: serde_json::json!({"checks": 10}),
        };
        let json = serde_json::to_string(&verdict).unwrap();
        let de: QualityVerdict = serde_json::from_str(&json).unwrap();
        assert!(de.passed);
        assert!((de.score - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_violation_serialization() {
        let violation = Violation {
            rule_id: "R001".to_string(),
            severity: Severity::Warning,
            message: "Unused variable".to_string(),
            file: "src/main.rs".to_string(),
            line: Some(42),
        };
        let json = serde_json::to_string(&violation).unwrap();
        let de: Violation = serde_json::from_str(&json).unwrap();
        assert_eq!(de.rule_id, "R001");
        assert_eq!(de.severity, Severity::Warning);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
        assert!(Severity::Error < Severity::Critical);
    }

    #[test]
    fn test_severity_serialization_roundtrip() {
        let severities = [
            Severity::Info,
            Severity::Warning,
            Severity::Error,
            Severity::Critical,
        ];
        for s in &severities {
            let json = serde_json::to_string(s).unwrap();
            let de: Severity = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, de);
        }
    }

    #[test]
    fn test_rule_info_serialization() {
        let rule = RuleInfo {
            rule_id: "R001".to_string(),
            name: "No Unused".to_string(),
            description: "Checks for unused variables".to_string(),
            severity: Severity::Warning,
            enabled: true,
        };
        let json = serde_json::to_string(&rule).unwrap();
        let de: RuleInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(de.rule_id, "R001");
        assert!(de.enabled);
    }
}
