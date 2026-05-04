//! Markdown 评估报告导出器
//!
//! 详见文档: §6.2 | 用例: UC-045

use std::fmt::Write;
use std::path::Path;

use crate::dataset::sample::EvaluationReport;
use crate::error::{self, Result};

/// Markdown 评估报告导出器
///
/// 详见文档: §6.2 | 用例: UC-045
pub struct MarkdownExporter;

impl MarkdownExporter {
    /// 导出为 Markdown 字符串
    ///
    /// 详见文档: §6.2 | 用例: UC-045 | 方法: M-065
    #[must_use]
    pub fn export(report: &EvaluationReport) -> String {
        let mut md = String::new();

        md.push_str("# RAG 评估报告\n\n");
        let _ = writeln!(
            md,
            "- **报告 ID**: {}\n- **数据集**: {} (v{})\n- **评估时间**: {}\
             \n- **总耗时**: {}ms\n- **样本数**: {} (成功: {}, 失败: {})",
            report.report_id,
            report.dataset_id,
            report.dataset_version,
            report.evaluated_at,
            report.total_duration_ms,
            report.total_samples,
            report.successful_samples,
            report.failed_samples,
        );
        md.push('\n');

        md.push_str("## 聚合评分\n\n");
        md.push_str("| 指标 | 均值 | 中位数 | 标准差 | 最小 | 最大 | 样本数 |\n");
        md.push_str("|------|------|--------|--------|------|------|--------|\n");
        for metric in &report.aggregated_scores {
            let _ = writeln!(
                md,
                "| {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {} |",
                metric.metric_name,
                metric.mean,
                metric.median,
                metric.std_dev,
                metric.min,
                metric.max,
                metric.sample_count,
            );
        }

        if !report.scores_by_difficulty.is_empty() {
            md.push_str("\n## 按难度分组\n\n");
            for (difficulty, metrics) in &report.scores_by_difficulty {
                let _ = writeln!(md, "### {difficulty}");
                md.push('\n');
                for m in metrics {
                    let _ = writeln!(
                        md,
                        "- **{}**: 均值={:.4}, 中位数={:.4}",
                        m.metric_name, m.mean, m.median,
                    );
                }
            }
        }

        if !report.scores_by_category.is_empty() {
            md.push_str("\n## 按类别分组\n\n");
            for (category, metrics) in &report.scores_by_category {
                let _ = writeln!(md, "### {category}");
                md.push('\n');
                for m in metrics {
                    let _ = writeln!(
                        md,
                        "- **{}**: 均值={:.4}, 中位数={:.4}",
                        m.metric_name, m.mean, m.median,
                    );
                }
            }
        }

        md
    }

    /// 导出到文件
    ///
    /// 详见文档: §6.2 | 用例: UC-045 | 方法: M-066
    ///
    /// # Errors
    ///
    /// 当文件写入失败时返回错误
    pub fn export_to_file(report: &EvaluationReport, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, Self::export(report))
    }

    /// 导出为 JSON 文件
    ///
    /// # Errors
    ///
    /// 当序列化或文件写入失败时返回错误
    pub fn to_json(report: &EvaluationReport, path: &Path) -> Result<()> {
        let json = serde_json::to_vec_pretty(report).map_err(|e| error::serialization_error(&e))?;
        std::fs::write(path, json).map_err(|e| error_core::helpers::io_error(&format!("报告写入失败: {e}")))
    }

    /// 导出为 JSON 字符串
    ///
    /// # Errors
    ///
    /// 当序列化失败时返回错误
    pub fn to_json_string(report: &EvaluationReport) -> Result<String> {
        serde_json::to_string_pretty(report).map_err(|e| error::serialization_error(&e))
    }

    /// 导出为 CSV
    #[must_use]
    pub fn to_csv(report: &EvaluationReport) -> String {
        let mut csv = String::from("sample_id,metric_name,score,explanation\n");
        for result in &report.sample_results {
            for score in &result.scores {
                let explanation = score.explanation.replace('"', "\"\"").replace('\n', " ").replace('\r', "");
                let _ = writeln!(
                    csv,
                    "{},{},{:.6},\"{}\"",
                    result.sample_id, score.metric_name, score.score, explanation,
                );
            }
        }
        csv
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::sample::{AggregatedMetric, MetricScore, SampleEvalResult};
    use std::collections::HashMap;

    fn make_report() -> EvaluationReport {
        EvaluationReport {
            report_id: "r1".to_string(),
            dataset_id: "ds1".to_string(),
            dataset_version: "1.0".to_string(),
            evaluated_at: chrono::Utc::now(),
            total_duration_ms: 200,
            total_samples: 1,
            successful_samples: 1,
            failed_samples: 0,
            aggregated_scores: vec![AggregatedMetric {
                metric_name: "faithfulness".to_string(),
                mean: 0.85,
                median: 0.85,
                std_dev: 0.0,
                min: 0.85,
                max: 0.85,
                sample_count: 1,
            }],
            sample_results: vec![SampleEvalResult {
                sample_id: "s1".to_string(),
                scores: vec![MetricScore {
                    metric_name: "faithfulness".to_string(),
                    score: 0.85,
                    explanation: "test".to_string(),
                    llm_judgment: None,
                }],
                duration_ms: 200,
            }],
            scores_by_difficulty: HashMap::new(),
            scores_by_category: HashMap::new(),
        }
    }

    #[test]
    fn test_export() {
        let report = make_report();
        let md = MarkdownExporter::export(&report);
        assert!(md.contains("# RAG 评估报告"));
        assert!(md.contains("faithfulness"));
    }

    #[test]
    fn test_to_json_string() {
        let report = make_report();
        let json = MarkdownExporter::to_json_string(&report).unwrap();
        assert!(json.contains("faithfulness"));
    }

    #[test]
    fn test_to_csv() {
        let report = make_report();
        let csv = MarkdownExporter::to_csv(&report);
        assert!(csv.contains("sample_id,metric_name,score,explanation"));
        assert!(csv.contains("faithfulness"));
    }
}
