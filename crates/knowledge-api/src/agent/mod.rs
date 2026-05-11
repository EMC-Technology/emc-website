//! AI Agent 工作流引擎
//!
//! 基于 ReAct (Reasoning + Acting) 模式的通用 Agent 框架，
//! 支持自主推理、工具调用、记忆管理和 DAG 任务编排。
//!
//! # 模块结构
//!
//! - [`react_agent`] - ReAct Agent 核心实现
//! - [`types`] - 类型定义（任务、结果、LLM 接口等）
//! - [`tools`] - 工具调用基础设施
//! - [`memory`] - 记忆系统（短期/长期/事件）
//! - [`executor`] - DAG 任务编排器

pub mod executor;
pub mod memory;
pub mod react_agent;
pub mod tools;
pub mod types;

pub use executor::{TaskNode, TaskOrchestrator, WorkflowDefinition, WorkflowResult};
pub use memory::{EpisodicMemory, LongTermMemory, MemorySystem, WorkingMemory};
pub use react_agent::{AgentConfig, AgentState, ExecutionStatus, ReActStep, ReactAgent};
pub use tools::{AgentContext, AgentToolInvoker, RetryPolicy};
pub use types::{AgentType, *};
