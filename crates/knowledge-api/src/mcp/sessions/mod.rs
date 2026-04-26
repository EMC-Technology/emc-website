//! 会话管理增强模块
//!
//! 提供企业级的会话管理能力，支持滑动窗口、上下文压缩和自动清理。

pub mod session_manager;

pub use session_manager::{
    MCPSession,
    SessionContextWindow,
    Message,
    MessageRole,
    ToolCallRecord,
    SessionState,
    SessionMetadata,
    SessionConfig,
    CompressionStrategy,
    SessionManager,
    SessionStore,
    InMemorySessionStore,
};
