//! MCP (Model Context Protocol) Hub - 企业级服务端模块
//!
//! 提供符合 MCP 规范的企业级平台能力，支持：
//! - **动态工具注册**：运行时工具发现、注册与管理
//! - **Prompt 版本管理**：版本控制、渲染、A/B 测试
//! - **会话管理**：滑动窗口上下文、自动压缩、过期清理
//! - **速率限制**：令牌桶算法、配额追踪、多维度限流
//!
//! ## 模块架构
//!
//! | 模块 | 功能 |
//! | :- | :- |
//! | `registry` | 动态 Tool Registry 与自动发现 |
//! | `prompts` | Prompt 版本管理与 A/B 测试 |
//! | `sessions` | 会话管理与上下文压缩 |
//! | `rate_limit` | 速率限制与配额追踪 |
//! | `server` | MCP Server 核心实现 |
//! | `tools` | 内置工具定义 |
//! | `resources` | 资源端点 |

pub mod server;
pub mod tools;
pub mod resources;

// 企业级模块
pub mod registry;
pub mod prompts;
pub mod sessions;
pub mod rate_limit;

// 重新导出旧版 prompts（保持向后兼容）
#[allow(clippy::module_inception)]
mod prompts_legacy;

pub use server::McpServer;
pub use server::run_mcp_server;

// 企业级 API 导出
pub use registry::{ToolRegistry, ToolDefinition, ToolHandler, CallContext, ToolCallResult};
pub use prompts::PromptManager;
pub use sessions::SessionManager;
pub use rate_limit::{TokenBucketRateLimiter, TokenUsageTracker};
