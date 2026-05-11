//! 动态工具注册中心模块
//!
//! 提供符合 MCP 规范的工具注册、发现和管理能力。

pub mod tool_registry;

#[cfg(feature = "event-driven")]
pub mod auto_discover;

pub use tool_registry::{
    AuthRequirements, CallContext, CallMetadata, ContentBlock, EmbeddedResource, RateLimitConfig,
    ToolCallResult, ToolCapabilities, ToolCategory, ToolDefinition, ToolFilter, ToolHandler,
    ToolMetadata, ToolRegistry, ToolStats,
};

#[cfg(feature = "event-driven")]
pub use auto_discover::mcp_tool;
