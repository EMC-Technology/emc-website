//! 评估报告模块
//!
//! 详见文档: §6 | 用例: UC-044~UC-045

pub mod aggregator;
pub mod exporter;

pub use aggregator::MetricAggregator;
pub use exporter::MarkdownExporter;
