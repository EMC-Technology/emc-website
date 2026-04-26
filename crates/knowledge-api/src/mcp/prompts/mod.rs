//! Prompt 版本管理模块
//!
//! 提供企业级的 Prompt 模板管理能力，支持版本控制、渲染和 A/B 测试。
//! 同时包含 MCP 协议 Prompt 端点的参数定义。

pub mod prompt_manager;

pub use prompt_manager::{
    PromptTemplate,
    PromptVariable,
    VariableType,
    PromptMetadata,
    PromptManager,
    PromptStore,
    InMemoryPromptStore,
    ABTestConfig,
    ABTestMetrics,
    ABTestStatus,
    Variables,
};

use schemars::JsonSchema;
use serde::Deserialize;

/// `detect_impact` prompt 参数
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DetectImpactParams {
    /// 待分析的符号列表，逗号分隔
    pub symbols: String,
}

/// `generate_map` prompt 参数（无参数）
/// 使用空结构体以保持与 rmcp Parameters 模式的一致性
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GenerateMapParams {}
