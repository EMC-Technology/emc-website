//! 动态工具注册中心模块
//!
//! 提供符合 MCP 规范的工具注册、发现和管理能力。

pub mod tool_registry;

#[cfg(feature = "event-driven")]
pub mod auto_discover;

pub use tool_registry::{
    ToolDefinition,
    ToolCapabilities,
    RateLimitConfig,
    AuthRequirements,
    ToolMetadata,
    ToolCategory,
    ToolRegistry,
    ToolFilter,
    ToolStats,
    ToolHandler,
    CallContext,
    ToolCallResult,
    ContentBlock,
    EmbeddedResource,
    CallMetadata,
};

#[cfg(feature = "event-driven")]
pub use auto_discover::mcp_tool;
