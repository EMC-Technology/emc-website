use async_trait::async_trait;
use std::pin::Pin;
use tokio_stream::Stream;

use super::agent_backend::{AgentBackend, AgentCapabilities, AgentChunk, AgentResult, AgentTask};
use crate::Result;

/// 远程 Agent 后端 — 通过 gRPC 连接 UPMC 服务
///
/// 这是进程外集成模式（模式 B）的适配器。
/// 本身不含 UPMC 逻辑，仅作为 gRPC 客户端将请求转发到远程 UPMC 服务。
///
/// # 接入模式
///
/// - **模式 A（进程内）**：`UpcmAgentBackend` 直接注册，零延迟
/// - **模式 B（进程外）**：`RemoteAgentBackend` 通过 gRPC 连接 ← 本适配器
/// - **模式 C（不接入）**：默认使用 `ReActAgentBackend`
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::remote_agent::RemoteAgentBackend;
/// use std::sync::Arc;
///
/// let agent = RemoteAgentBackend::connect("http://upcm-server:50051").await?;
/// registry.register_agent(Arc::new(agent));
/// ```
pub struct RemoteAgentBackend {
    endpoint: String,
}

impl RemoteAgentBackend {
    /// 创建远程 Agent 后端
    ///
    /// # Arguments
    ///
    /// * `endpoint` - UPMC 服务的 gRPC 端点地址
    #[must_use]
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
        }
    }

    /// 连接到远程 UPMC 服务
    ///
    /// # Errors
    ///
    /// 当 gRPC 连接建立失败时返回错误。
    pub fn connect(_endpoint: &str) -> Result<Self> {
        todo!("实现 gRPC 连接（需引入 tonic 依赖）")
    }

    /// 获取远程端点地址
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

#[async_trait]
impl AgentBackend for RemoteAgentBackend {
    fn backend_id(&self) -> &'static str {
        "remote-upcm"
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            supports_streaming: true,
            supports_cancellation: true,
            max_context_tokens: 128_000,
            available_tools: vec![
                "git_ops".to_string(),
                "code_search".to_string(),
                "impact_analysis".to_string(),
                "auto_fix".to_string(),
                "quality_gate".to_string(),
                "security_scan".to_string(),
            ],
            supports_multi_agent: true,
            supports_human_in_loop: true,
        }
    }

    async fn execute(&self, _task: &AgentTask) -> Result<AgentResult> {
        todo!("实现 gRPC 调用远程 UPMC 服务")
    }

    async fn execute_stream(
        &self,
        _task: &AgentTask,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentChunk> + Send>>> {
        todo!("实现 gRPC 流式调用远程 UPMC 服务")
    }

    async fn cancel(&self, _task_id: &str) -> Result<()> {
        todo!("实现 gRPC 取消远程任务")
    }

    async fn health_check(&self) -> Result<bool> {
        todo!("实现 gRPC 健康检查")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_agent_backend_id() {
        let agent = RemoteAgentBackend::new("http://localhost:50051");
        assert_eq!(agent.backend_id(), "remote-upcm");
    }

    #[test]
    fn test_remote_agent_endpoint() {
        let agent = RemoteAgentBackend::new("http://upcm-server:50051");
        assert_eq!(agent.endpoint(), "http://upcm-server:50051");
    }

    #[test]
    fn test_remote_agent_capabilities() {
        let agent = RemoteAgentBackend::new("http://localhost:50051");
        let caps = agent.capabilities();
        assert!(caps.supports_streaming);
        assert!(caps.supports_cancellation);
        assert_eq!(caps.max_context_tokens, 128_000);
        assert!(caps.supports_multi_agent);
        assert!(caps.supports_human_in_loop);
        assert_eq!(caps.available_tools.len(), 6);
    }
}
