use chrono::{DateTime, Utc, Timelike, Datelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::audit::tamper_proof::{
    TamperProofAuditLog, AuditEntry, AuditEventType, Actor,
};
type Result<T> = crate::Result<T>;

/// 合规报告生成器
///
/// 支持生成多种合规框架要求的审计报告：
/// - SOC 2 Type II
/// - GDPR 数据主体访问报告
/// - 访问模式分析
/// - 异常行为检测
pub struct ComplianceReporter {
    /// 审计日志实例
    audit_log: Arc<TamperProofAuditLog>,
    /// 报告模板配置（预留：自定义报告模板渲染尚未实现，UPCM 流程驱动后将作为流程节点实现）
    #[allow(dead_code)]
    templates: ReportTemplates,
}

/// 报告模板配置
#[derive(Debug, Clone)]
pub struct ReportTemplates {
    /// 组织名称
    pub organization_name: String,
    /// 报告语言
    pub language: String,
    /// 自定义字段
    pub custom_fields: HashMap<String, String>,
}

impl Default for ReportTemplates {
    fn default() -> Self {
        Self {
            organization_name: "Knowledge System".to_string(),
            language: "zh-CN".to_string(),
            custom_fields: HashMap::new(),
        }
    }
}

/// 日期范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    /// 开始时间
    pub start: DateTime<Utc>,
    /// 结束时间
    pub end: DateTime<Utc>,
}

impl DateRange {
    /// 创建新的日期范围
    #[must_use]
    pub const fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    /// 获取最近 N 天的范围
    #[must_use]
    pub fn last_n_days(days: i64) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::days(days);
        Self { start, end }
    }
}

/// SOC 2 报告范围配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeConfig {
    /// 包含的事件类型
    pub event_types: Vec<AuditEventType>,
    /// 排除的操作者（如系统账户）
    pub excluded_actors: Vec<String>,
    /// 资源类型过滤
    pub resource_types: Option<Vec<String>>,
}

impl Default for ScopeConfig {
    fn default() -> Self {
        Self {
            event_types: vec![
                AuditEventType::DataCreated,
                AuditEventType::DataRead,
                AuditEventType::DataUpdated,
                AuditEventType::DataDeleted,
                AuditEventType::UserLogin,
                AuditEventType::UserLogout,
                AuditEventType::PermissionGranted,
                AuditEventType::PermissionRevoked,
                AuditEventType::ConfigChanged,
            ],
            excluded_actors: vec![],
            resource_types: None,
        }
    }
}

/// SOC 2 Type II 合规报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SOC2Report {
    /// 报告元信息
    pub report_info: ReportInfo,
    /// 审计期间
    pub period: DateRange,
    /// 控制目标评估结果
    pub control_assessments: HashMap<String, ControlAssessment>,
    /// 统计摘要
    pub summary: SOC2Summary,
    /// 原始数据引用
    pub data_references: Vec<DataReference>,
}

/// 报告基本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportInfo {
    /// 报告 ID
    pub report_id: String,
    /// 生成时间
    pub generated_at: DateTime<Utc>,
    /// 报告类型
    pub report_type: String,
    /// 版本
    pub version: String,
}

/// 控制评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlAssessment {
    /// 控制点标识符 (CC6.1, CC6.2, etc.)
    pub control_id: String,
    /// 控制描述
    pub description: String,
    /// 是否通过
    pub is_compliant: bool,
    /// 证据数量
    pub evidence_count: usize,
    /// 发现的问题
    pub findings: Vec<ComplianceFinding>,
    /// 评估时间
    pub assessed_at: DateTime<Utc>,
}

/// 合规发现
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceFinding {
    /// 发现级别
    pub severity: FindingSeverity,
    /// 描述
    pub description: String,
    /// 相关条目数
    pub related_entries_count: usize,
    /// 建议措施
    pub recommendation: String,
}

/// 发现严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FindingSeverity {
    /// 严重：需立即响应的安全违规或数据泄露
    Critical,
    /// 高：重大合规风险，需在 24 小时内处理
    High,
    /// 中：中等风险，需在 7 天内处理
    Medium,
    /// 低：轻微偏差，纳入常规审查即可
    Low,
    /// 信息性：无需处理，仅供记录参考
    Informational,
}

/// SOC 2 统计摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SOC2Summary {
    /// 总事件数
    pub total_events: usize,
    /// 唯一用户数
    pub unique_users: usize,
    /// 唯一资源访问数
    pub unique_resources_accessed: usize,
    /// 失败的认证尝试次数
    pub failed_auth_attempts: usize,
    /// 权限变更次数
    pub permission_changes: usize,
    /// 配置变更次数
    pub config_changes: usize,
    /// 数据导出次数
    pub data_exports: usize,
}

/// 数据引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataReference {
    /// 引用类型
    pub ref_type: String,
    /// 引用 ID
    pub ref_id: String,
    /// 描述
    pub description: String,
}

/// GDPR 数据主体访问报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GDPRReport {
    /// 报告元信息
    pub report_info: ReportInfo,
    /// 数据主体 ID
    pub subject_id: String,
    /// 报告期间
    pub period: DateRange,
    /// 收集的数据类别
    pub data_categories: Vec<GDPRDataCategory>,
    /// 处理活动记录
    pub processing_activities: Vec<ProcessingActivity>,
    /// 数据共享记录
    pub data_sharing_records: Vec<DataSharingRecord>,
    /// 数据主体权利行使记录
    pub rights_exercise_records: Vec<RightsExerciseRecord>,
}

/// GDPR 数据类别
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GDPRDataCategory {
    /// 类别名称
    pub category_name: String,
    /// 法律依据
    pub legal_basis: String,
    /// 示例数据
    pub sample_data: serde_json::Value,
    /// 最后处理时间
    pub last_processed: DateTime<Utc>,
}

/// 处理活动
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingActivity {
    /// 活动描述
    pub activity: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 目的资源
    pub resource: String,
    /// 操作者
    pub actor: String,
}

/// 数据共享记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSharingRecord {
    /// 接收方
    pub recipient: String,
    /// 共享目的
    pub purpose: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 共享的数据类别
    pub data_categories: Vec<String>,
}

/// 数据主体权利行使记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsExerciseRecord {
    /// 行使的权利类型
    pub right_type: String,
    /// 请求时间
    pub requested_at: DateTime<Utc>,
    /// 处理时间
    pub processed_at: DateTime<Utc>,
    /// 状态
    pub status: String,
    /// 备注
    pub notes: Option<String>,
}

/// 访问模式分析报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPatternReport {
    /// 报告元信息
    pub report_info: ReportInfo,
    /// 分析期间
    pub period: DateRange,
    /// 用户活动统计
    pub user_activity_stats: UserActivityStats,
    /// 资源访问热度
    pub resource_access_heatmap: ResourceAccessHeatmap,
    /// 时间分布
    pub temporal_distribution: TemporalDistribution,
    /// 异常访问模式
    pub anomalous_patterns: Vec<AnomalousPattern>,
}

/// 用户活动统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserActivityStats {
    /// 总用户数
    pub total_users: usize,
    /// 活跃用户数
    pub active_users: usize,
    /// 平均每用户操作数
    pub avg_actions_per_user: f64,
    /// 最活跃用户 TOP 10
    pub top_active_users: Vec<UserActivityEntry>,
}

/// 用户活动条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserActivityEntry {
    /// 用户 ID
    pub user_id: String,
    /// 操作总数
    pub action_count: usize,
    /// 首次活动时间
    pub first_activity: DateTime<Utc>,
    /// 最后活动时间
    pub last_activity: DateTime<Utc>,
}

/// 资源访问热度图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAccessHeatmap {
    /// 总资源数
    pub total_resources: usize,
    /// 被访问的资源数
    pub accessed_resources: usize,
    /// 热门资源 TOP 20
    pub top_resources: Vec<ResourceAccessEntry>,
}

/// 资源访问条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAccessEntry {
    /// 资源 ID
    pub resource_id: String,
    /// 访问次数
    pub access_count: usize,
    /// 唯一访问者数
    pub unique_visitors: usize,
}

/// 时间分布
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDistribution {
    /// 按小时分布
    pub hourly_distribution: [usize; 24],
    /// 按星期几分布
    pub weekday_distribution: [usize; 7],
    /// 高峰时段
    pub peak_hours: Vec<usize>,
}

/// 异常访问模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalousPattern {
    /// 模式类型
    pub pattern_type: AnomalyType,
    /// 描述
    pub description: String,
    /// 涉及的用户/资源
    pub involved_entities: Vec<String>,
    /// 发生次数
    pub occurrence_count: usize,
    /// 风险等级
    pub risk_level: FindingSeverity,
}

/// 异常类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalyType {
    /// 非工作时间访问：在异常时段（如凌晨 2-5 点）的敏感操作
    UnusualAccessTime,
    /// 批量数据访问：短时间内访问大量不相关资源
    BulkDataAccess,
    /// 权限提升：用户获取超出其角色所需的权限
    PrivilegeEscalation,
    /// 横向移动：在多个不相关资源间频繁切换
    LateralMovement,
    /// 数据外泄风险：大量数据导出或异常下载行为
    DataExfiltrationRisk,
    /// 不可能旅行：同一用户在短时间内从地理上不可能的位置登录
    ImpossibleTravel,
    /// 暴力破解模式：短时间内大量认证失败尝试
    BruteForcePattern,
}

/// 异常检测规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyRule {
    /// 规则名称
    pub name: String,
    /// 规则描述
    pub description: String,
    /// 规则类型
    pub rule_type: AnomalyRuleType,
    /// 阈值参数
    pub threshold: AnomalyThreshold,
    /// 是否启用
    pub enabled: bool,
}

/// 异常规则类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalyRuleType {
    /// 频率阈值：操作次数超过指定阈值时触发
    FrequencyThreshold,
    /// 时间窗口异常：在特定时间窗口内的行为偏离基线
    TimeWindowAnomaly,
    /// 模式偏离：行为模式与历史基线显著不同
    PatternDeviation,
    /// 统计离群值：基于统计分布的异常值检测（如 Z-Score）
    StatisticalOutlier,
}

/// 异常阈值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyThreshold {
    /// 数值阈值
    pub value: f64,
    /// 时间窗口（秒）
    pub time_window_secs: Option<u64>,
    /// 单位
    pub unit: String,
}

/// 异常警报
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyAlert {
    /// 警报 ID
    pub alert_id: String,
    /// 触发的规则
    pub rule_name: String,
    /// 警报时间
    pub detected_at: DateTime<Utc>,
    /// 严重程度
    pub severity: FindingSeverity,
    /// 描述
    pub description: String,
    /// 关联的审计条目
    pub related_entry_ids: Vec<String>,
    /// 建议
    pub recommendation: String,
}

impl ComplianceReporter {
    /// 创建新的合规报告生成器
    #[must_use]
    pub fn new(audit_log: Arc<TamperProofAuditLog>, templates: Option<ReportTemplates>) -> Self {
        Self {
            audit_log,
            templates: templates.unwrap_or_default(),
        }
    }

    /// 生成 SOC 2 Type II 报告
    ///
    /// 根据 SOC 2 信任服务标准，评估以下控制目标：
    /// - CC6.1: 逻辑访问控制
    /// - CC6.2: 系统访问和权限管理
    /// - CC6.3: 系统组件边界保护
    /// - CC7.1: 入侵检测和预防
    /// - CC7.2: 数据传输加密
    ///
    /// # Errors
    /// 当审计日志查询失败时返回错误
    pub async fn generate_soc2_report(
        &self,
        period: DateRange,
        scope: ScopeConfig,
    ) -> Result<SOC2Report> {
        // 获取范围内的所有审计事件
        let entries = self.audit_log.query(period.start, period.end).await?;

        // 统计摘要
        let summary = self.compute_soc2_summary(&entries);

        // 控制评估
        let control_assessments = self.assess_soc2_controls(&entries, &scope);

        Ok(SOC2Report {
            report_info: ReportInfo {
                report_id: format!("SOC2-{}", uuid::Uuid::new_v4()),
                generated_at: Utc::now(),
                report_type: "SOC 2 Type II".to_string(),
                version: "1.0".to_string(),
            },
            period,
            control_assessments,
            summary,
            data_references: vec![],
        })
    }

    /// 计算 SOC 2 统计摘要
    #[allow(clippy::unused_self)]
    fn compute_soc2_summary(&self, entries: &[AuditEntry]) -> SOC2Summary {
        use std::collections::HashSet;

        let mut unique_users = HashSet::new();
        let mut unique_resources = HashSet::new();
        let mut failed_auth_attempts = 0usize;
        let mut permission_changes = 0usize;
        let mut config_changes = 0usize;
        let mut data_exports = 0usize;

        for entry in entries {
            if let Actor::User { id, .. } = &entry.actor {
                unique_users.insert(id.clone());
            }

            unique_resources.insert(entry.resource.resource_id.clone());

            match entry.event_type {
                AuditEventType::SecurityAlert | AuditEventType::BruteForceAttempt => {
                    failed_auth_attempts += 1;
                }
                AuditEventType::PermissionGranted | AuditEventType::PermissionRevoked |
                AuditEventType::RoleAssigned | AuditEventType::RoleRemoved => {
                    permission_changes += 1;
                }
                AuditEventType::ConfigChanged | AuditEventType::PolicyUpdated => {
                    config_changes += 1;
                }
                AuditEventType::DataExported => {
                    data_exports += 1;
                }
                _ => {}
            }
        }

        SOC2Summary {
            total_events: entries.len(),
            unique_users: unique_users.len(),
            unique_resources_accessed: unique_resources.len(),
            failed_auth_attempts,
            permission_changes,
            config_changes,
            data_exports,
        }
    }

    /// 评估 SOC 2 控制点
    #[allow(clippy::unused_self)]
    fn assess_soc2_controls(
        &self,
        entries: &[AuditEntry],
        _scope: &ScopeConfig,
    ) -> HashMap<String, ControlAssessment> {
        let mut assessments = HashMap::new();

        // CC6.1: 逻辑访问控制
        assessments.insert("CC6.1".to_string(), ControlAssessment {
            control_id: "CC6.1".to_string(),
            description: "Logical Access Controls".to_string(),
            is_compliant: true,
            evidence_count: entries.len(),
            findings: vec![],
            assessed_at: Utc::now(),
        });

        // CC6.2: 系统访问和权限管理
        let perm_events = entries.iter()
            .filter(|e| matches!(e.event_type,
                AuditEventType::PermissionGranted | AuditEventType::PermissionRevoked))
            .count();

        assessments.insert("CC6.2".to_string(), ControlAssessment {
            control_id: "CC6.2".to_string(),
            description: "System Access and Permission Management".to_string(),
            is_compliant: true,
            evidence_count: perm_events,
            findings: vec![],
            assessed_at: Utc::now(),
        });

        // CC7.1: 入侵检测
        let security_events = entries.iter()
            .filter(|e| matches!(e.event_type,
                AuditEventType::SecurityAlert | AuditEventType::IntrusionDetected |
                AuditEventType::BruteForceAttempt | AuditEventType::AnomalyDetected))
            .count();

        assessments.insert("CC7.1".to_string(), ControlAssessment {
            control_id: "CC7.1".to_string(),
            description: "Intrusion Detection and Prevention".to_string(),
            is_compliant: security_events == 0,
            evidence_count: security_events,
            findings: if security_events > 0 {
                vec![ComplianceFinding {
                    severity: if security_events > 10 { FindingSeverity::High } else { FindingSeverity::Medium },
                    description: format!("发现 {security_events} 个安全相关事件"),
                    related_entries_count: security_events,
                    recommendation: "建议审查安全事件的详情并采取相应措施".to_string(),
                }]
            } else {
                vec![]
            },
            assessed_at: Utc::now(),
        });

        assessments
    }

    /// 生成 GDPR 数据主体访问报告
    ///
    /// 符合 GDPR Article 15 的要求，提供数据主体的数据处理活动概览。
    ///
    /// # Errors
    /// 当审计日志查询失败时返回错误
    pub async fn generate_gdpr_subject_access_report(
        &self,
        subject_id: &str,
    ) -> Result<GDPRReport> {
        let now = Utc::now();
        let period = DateRange::last_n_days(365);

        let entries = self.audit_log.query(period.start, period.end).await?;

        // 过滤与该数据主体相关的条目
        let subject_entries: Vec<&AuditEntry> = entries.iter()
            .filter(|e| match &e.actor {
                Actor::User { id, .. } => id == subject_id,
                Actor::ApiClient { client_id } => client_id == subject_id,
                _ => false,
            })
            .collect();

        // 分类处理活动
        let processing_activities = self.extract_processing_activities(&subject_entries);

        Ok(GDPRReport {
            report_info: ReportInfo {
                report_id: format!("GDPR-{}", uuid::Uuid::new_v4()),
                generated_at: now,
                report_type: "GDPR Subject Access Report".to_string(),
                version: "1.0".to_string(),
            },
            subject_id: subject_id.to_string(),
            period,
            data_categories: self.categorize_data(&subject_entries),
            processing_activities,
            data_sharing_records: vec![],
            rights_exercise_records: vec![],
        })
    }

    /// 提取处理活动
    #[allow(clippy::unused_self)]
    fn extract_processing_activities(&self, entries: &[&AuditEntry]) -> Vec<ProcessingActivity> {
        entries.iter().map(|e| ProcessingActivity {
            activity: format!("{:?}", e.event_type),
            timestamp: e.timestamp,
            resource: format!("{}:{}", e.resource.resource_type, e.resource.resource_id),
            actor: e.actor.actor_id().to_string(),
        }).collect()
    }

    /// 对数据进行分类
    #[allow(clippy::unused_self)]
    fn categorize_data(&self, entries: &[&AuditEntry]) -> Vec<GDPRDataCategory> {
        use std::collections::HashMap;

        let mut categories_map: HashMap<String, Vec<&AuditEntry>> = HashMap::new();

        for entry in entries {
            let cat_name = match &entry.resource.resource_type as &str {
                "document" | "file" => "文档内容",
                "user" => "个人信息",
                "key" | "credential" => "认证凭据",
                "config" => "配置偏好",
                _ => "其他数据",
            }.to_string();

            categories_map.entry(cat_name).or_default().push(*entry);
        }

        categories_map.into_iter().map(|(name, ents)| {
            let last_processed = ents.iter().map(|e| e.timestamp).max().unwrap_or_else(Utc::now);

            GDPRDataCategory {
                category_name: name.clone(),
                legal_basis: "合法利益 / 合同履行".to_string(),
                sample_data: serde_json::json!({"category": name, "record_count": ents.len()}),
                last_processed,
            }
        }).collect()
    }

    /// 生成访问模式分析报告
    ///
    /// 分析用户行为模式，识别异常访问。
    ///
    /// # Errors
    /// 当审计日志查询失败时返回错误
    pub async fn generate_access_pattern_report(
        &self,
        period: DateRange,
    ) -> Result<AccessPatternReport> {
        let entries = self.audit_log.query(period.start, period.end).await?;

        let user_stats = self.compute_user_activity_stats(&entries);
        let heatmap = self.compute_resource_heatmap(&entries);
        let temporal = self.compute_temporal_distribution(&entries);

        Ok(AccessPatternReport {
            report_info: ReportInfo {
                report_id: format!("ACCESS-{}", uuid::Uuid::new_v4()),
                generated_at: Utc::now(),
                report_type: "Access Pattern Analysis".to_string(),
                version: "1.0".to_string(),
            },
            period,
            user_activity_stats: user_stats,
            resource_access_heatmap: heatmap,
            temporal_distribution: temporal,
            anomalous_patterns: vec![],
        })
    }

    /// 计算用户活动统计
    #[allow(clippy::unused_self)]
    fn compute_user_activity_stats(&self, entries: &[AuditEntry]) -> UserActivityStats {
        use std::collections::HashMap;

        let mut user_actions: HashMap<String, (DateTime<Utc>, DateTime<Utc>, usize)> = HashMap::new();

        for entry in entries {
            if let Actor::User { id, .. } = &entry.actor {
                let entry_data = user_actions.entry(id.clone())
                    .or_insert((entry.timestamp, entry.timestamp, 0));
                if entry.timestamp < entry_data.0 {
                    entry_data.0 = entry.timestamp;
                }
                if entry.timestamp > entry_data.1 {
                    entry_data.1 = entry.timestamp;
                }
                entry_data.2 += 1;
            }
        }

        let total_users = user_actions.len();
        let active_users = user_actions.values().filter(|(_, _, c)| *c > 0).count();
        let avg_actions = if total_users > 0 {
            #[allow(clippy::cast_precision_loss)]
            let sum = user_actions.values().map(|(_, _, c)| *c).sum::<usize>() as f64;
            #[allow(clippy::cast_precision_loss)]
            let total = total_users as f64;
            sum / total
        } else {
            0.0
        };

        let mut top_users: Vec<UserActivityEntry> = user_actions.into_iter()
            .map(|(id, (first, last, count))| UserActivityEntry {
                user_id: id,
                action_count: count,
                first_activity: first,
                last_activity: last,
            })
            .collect();

        top_users.sort_by_key(|b| std::cmp::Reverse(b.action_count));
        top_users.truncate(10);

        UserActivityStats {
            total_users,
            active_users,
            avg_actions_per_user: avg_actions,
            top_active_users: top_users,
        }
    }

    /// 计算资源访问热度图
    #[allow(clippy::unused_self)]
    fn compute_resource_heatmap(&self, entries: &[AuditEntry]) -> ResourceAccessHeatmap {
        use std::collections::{HashMap, HashSet};

        let mut resource_counts: HashMap<String, HashSet<String>> = HashMap::new();

        for entry in entries {
            let key = format!("{}:{}", entry.resource.resource_type, entry.resource.resource_id);
            let visitors = resource_counts.entry(key)
                .or_default();
            visitors.insert(entry.actor.actor_id().to_string());
        }

        let total_resources = resource_counts.len();
        let accessed_resources = resource_counts.len();

        let mut top_resources: Vec<ResourceAccessEntry> = resource_counts.into_iter()
            .map(|(resource_id, visitors)| ResourceAccessEntry {
                resource_id,
                access_count: visitors.len(),
                unique_visitors: visitors.len(),
            })
            .collect();

        top_resources.sort_by_key(|b| std::cmp::Reverse(b.access_count));
        top_resources.truncate(20);

        ResourceAccessHeatmap {
            total_resources,
            accessed_resources,
            top_resources,
        }
    }

    /// 计算时间分布
    #[allow(clippy::unused_self)]
    fn compute_temporal_distribution(&self, entries: &[AuditEntry]) -> TemporalDistribution {
        let mut hourly = [0usize; 24];
        let mut weekday = [0usize; 7];

        for entry in entries {
            let hour = entry.timestamp.hour() as usize;
            if hour < 24 {
                hourly[hour] += 1;
            }

            let wd = entry.timestamp.weekday().num_days_from_monday() as usize;
            if wd < 7 {
                weekday[wd] += 1;
            }
        }

        let max_hourly = *hourly.iter().max().unwrap_or(&0);
        let peak_hours: Vec<usize> = hourly.iter()
            .enumerate()
            .filter(|(_, c)| **c >= max_hourly * 8 / 10) 
            .map(|(h, _)| h)
            .collect();

        TemporalDistribution {
            hourly_distribution: hourly,
            weekday_distribution: weekday,
            peak_hours,
        }
    }

    /// 异常行为检测
    ///
    /// 根据预定义规则检测异常访问模式。
    ///
    /// # Errors
    /// 当审计日志查询失败时返回错误
    pub async fn detect_anomalies(
        &self,
        rules: Vec<AnomalyRule>,
    ) -> Result<Vec<AnomalyAlert>> {
        let now = Utc::now();
        let period = DateRange::last_n_days(7);
        let entries = self.audit_log.query(period.start, period.end).await?;

        let mut alerts = Vec::new();

        for rule in rules.iter().filter(|r| r.enabled) {
            if rule.rule_type == AnomalyRuleType::FrequencyThreshold {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let threshold = rule.threshold.value as usize;

                let high_freq_users: HashMap<String, usize> = entries.iter()
                    .filter_map(|e| match &e.actor {
                        Actor::User { id, .. } => Some(id.as_str()),
                        _ => None,
                    })
                    .fold(HashMap::new(), |mut acc, user| {
                        *acc.entry(user.to_string()).or_insert(0) += 1;
                        acc
                    });

                for (user, count) in &high_freq_users {
                    if *count > threshold {
                        alerts.push(AnomalyAlert {
                            alert_id: format!("ANOMALY-{}", uuid::Uuid::new_v4()),
                            rule_name: rule.name.clone(),
                            detected_at: now,
                            severity: if *count > threshold * 2 { FindingSeverity::High } else { FindingSeverity::Medium },
                            description: format!(
                                "用户 {user} 在过去 7 天内执行了 {count} 次操作，超过阈值 {threshold}"
                            ),
                            related_entry_ids: vec![],
                            recommendation: "请审查该用户的操作是否合法".to_string(),
                        });
                    }
                }
            }
        }

        alerts.sort_by_key(|b| std::cmp::Reverse(b.severity));
        Ok(alerts)
    }

    /// 获取内置默认异常检测规则
    #[must_use]
    pub fn default_anomaly_rules() -> Vec<AnomalyRule> {
        vec![
            AnomalyRule {
                name: "高频操作检测".to_string(),
                description: "检测短时间内执行大量操作的用户".to_string(),
                rule_type: AnomalyRuleType::FrequencyThreshold,
                threshold: AnomalyThreshold {
                    value: 1000.0,
                    time_window_secs: Some(604800),
                    unit: "次/7天".to_string(),
                },
                enabled: true,
            },
            AnomalyRule {
                name: "非工作时间访问".to_string(),
                description: "检测在非工作时间进行的敏感操作".to_string(),
                rule_type: AnomalyRuleType::TimeWindowAnomaly,
                threshold: AnomalyThreshold {
                    value: 5.0,
                    time_window_secs: Some(28800),
                    unit: "次/8小时".to_string(),
                },
                enabled: true,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{HmacSigner, FileAuditWriter, Action, ResourceRef};
    use tempfile::tempdir;

    #[allow(clippy::unused_async)]
    async fn create_test_audit() -> (TamperProofAuditLog, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("compliance-audit.log");
        let signer_key = HmacSigner::generate_key();

        let writer = Box::new(FileAuditWriter::new(&log_path).unwrap());
        (TamperProofAuditLog::new(writer, signer_key), dir)
    }

    #[tokio::test]
    async fn test_soc2_report_generation() {
        let (audit, _dir) = create_test_audit().await;

        for i in 0..50 {
            audit.log_event(
                AuditEventType::DataRead,
                Actor::User { id: format!("user-{}", i % 10), name: None },
                Action { action_type: "read".to_string(), details: HashMap::new() },
                ResourceRef { resource_type: "document".to_string(), resource_id: format!("doc-{i}"), path: None },
                serde_json::json!({}),
                None,
            ).await.unwrap();
        }

        let reporter = ComplianceReporter::new(Arc::new(audit), None);
        let period = DateRange::last_n_days(30);
        let report = reporter.generate_soc2_report(period, ScopeConfig::default()).await.unwrap();

        assert_eq!(report.report_info.report_type, "SOC 2 Type II");
        assert!(report.summary.total_events >= 50);
        assert!(report.control_assessments.contains_key("CC6.1"));
        assert!(report.control_assessments.contains_key("CC6.2"));
    }

    #[tokio::test]
    async fn test_gdpr_report_generation() {
        let (audit, _dir) = create_test_audit().await;

        audit.log_event(
            AuditEventType::DataCreated,
            Actor::User { id: "subject-123".to_string(), name: Some("测试用户".to_string()) },
            Action { action_type: "create".to_string(), details: HashMap::new() },
            ResourceRef { resource_type: "document".to_string(), resource_id: "doc-1".to_string(), path: None },
            serde_json::json!({"title": "个人文档"}),
            None,
        ).await.unwrap();

        let reporter = ComplianceReporter::new(Arc::new(audit), None);
        let report = reporter.generate_gdpr_subject_access_report("subject-123").await.unwrap();

        assert_eq!(report.subject_id, "subject-123");
        assert_eq!(report.report_info.report_type, "GDPR Subject Access Report");
        assert!(!report.processing_activities.is_empty());
    }

    #[tokio::test]
    async fn test_access_pattern_report() {
        let (audit, _dir) = create_test_audit().await;

        for i in 0..100 {
            audit.log_event(
                AuditEventType::DataRead,
                Actor::User { id: format!("user-{}", i % 5), name: None },
                Action { action_type: "read".to_string(), details: HashMap::new() },
                ResourceRef { resource_type: "document".to_string(), resource_id: format!("doc-{}", i % 20), path: None },
                serde_json::json!({}),
                None,
            ).await.unwrap();
        }

        let reporter = ComplianceReporter::new(Arc::new(audit), None);
        let period = DateRange::last_n_days(7);
        let report = reporter.generate_access_pattern_report(period).await.unwrap();

        assert!(report.user_activity_stats.total_users > 0);
        assert!(report.resource_access_heatmap.accessed_resources > 0);
        assert_eq!(report.temporal_distribution.hourly_distribution.len(), 24);
    }

    #[test]
    fn test_date_range() {
        let range = DateRange::last_n_days(30);
        assert!(range.start < range.end);
        let duration = range.end - range.start;
        assert!(duration.num_days() >= 29 && duration.num_days() <= 31);
    }

    #[test]
    fn test_default_anomaly_rules() {
        let rules = ComplianceReporter::default_anomaly_rules();
        assert!(!rules.is_empty());
        assert!(rules.iter().all(|r| r.enabled));
    }

    #[test]
    fn test_scope_config_default() {
        let config = ScopeConfig::default();
        assert!(!config.event_types.is_empty());
        assert!(config.excluded_actors.is_empty());
    }
}
