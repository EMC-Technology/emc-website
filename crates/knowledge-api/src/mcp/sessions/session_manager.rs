//! MCP 会话管理器
//!
//! 提供企业级的会话管理能力，支持：

#![allow(clippy::significant_drop_tightening)]
//! - 滑动窗口上下文管理
//! - 多种上下文压缩策略
//! - 自动过期清理
//! - 会话持久化

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use error_core::{ErrorObject, helpers};

use crate::mcp::registry::ToolCallResult;

/// MCP 会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPSession {
    /// 唯一会话 ID
    pub id: Uuid,
    /// 关联的用户 ID（可选）
    pub user_id: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
    /// 过期时间
    pub expires_at: DateTime<Utc>,
    /// 上下文窗口
    pub context_window: SessionContextWindow,
    /// 当前状态
    pub state: SessionState,
    /// 元数据
    pub metadata: SessionMetadata,
}

/// 上下文窗口（滑动窗口实现）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContextWindow {
    /// 最大消息数（默认: 100）
    pub max_messages: usize,
    /// 最大 token 数（默认: 128K）
    pub max_tokens: usize,
    /// 当前已使用 token 数
    pub current_tokens: usize,
    /// 消息列表
    pub messages: Vec<Message>,
}

impl Default for SessionContextWindow {
    fn default() -> Self {
        Self {
            max_messages: 100,
            max_tokens: 128_000,
            current_tokens: 0,
            messages: Vec::new(),
        }
    }
}

/// 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 角色
    pub role: MessageRole,
    /// 内容
    pub content: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 工具调用记录
    pub tool_calls: Vec<ToolCallRecord>,
    /// token 计数（估算值）
    pub token_count: usize,
}

/// 会话消息角色类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    /// 用户发送的消息
    User,
    /// Agent 生成的回复
    Assistant,
    /// 系统指令或上下文注入
    System,
    /// 工具调用结果消息
    Tool,
}

/// 工具调用记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// 工具名称
    pub tool_name: String,
    /// 调用参数
    pub arguments: serde_json::Value,
    /// 调用结果
    pub result: ToolCallResult,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
}

/// 会话状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SessionState {
    /// 活跃状态 —— 会话正在进行中，有活跃的交互
    #[default]
    Active,
    /// 空闲状态 —— 会话存在但暂时无活动（可被回收）
    Idle,
    /// 已过期 —— 超过最大存活时间，等待清理
    Expired,
    /// 已关闭 —— 用户或系统主动终止
    Closed,
}

impl SessionState {
    /// 验证状态转换是否合法
    ///
    /// 合法转换路径：
    /// - `Active` → `Idle` | `Closed`
    /// - `Idle` → `Active` | `Expired` | `Closed`
    /// - `Expired` → `Closed`
    ///
    /// `Closed` 为终态，无出边。
    #[must_use]
    pub fn can_transition_to(&self, target: &Self) -> bool {
        matches!(
            (self, target),
            (Self::Active, Self::Idle | Self::Closed)
                | (Self::Idle, Self::Active | Self::Expired | Self::Closed)
                | (Self::Expired, Self::Closed)
        )
    }

    /// 是否为终态
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Closed)
    }
}

/// 会话元数据
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionMetadata {
    /// 来源客户端标识
    pub client_id: Option<String>,
    /// IP 地址
    pub ip_address: Option<String>,
    /// 用户代理
    pub user_agent: Option<String>,
    /// 自定义标签
    pub tags: Vec<String>,
}

/// 上下文压缩策略
#[derive(Debug, Clone, Copy)]
pub enum CompressionStrategy {
    /// 截断最旧的消息
    TruncateOldest,
    /// 使用 LLM 总结（需要外部服务）
    Summarize,
    /// 保留最近 N 条消息
    SlidingWindow(usize),
    /// 重要性采样（基于启发式规则）
    ImportanceSampling,
}

/// 会话存储接口
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// 保存会话
    async fn save(&self, session: &MCPSession) -> Result<(), ErrorObject>;

    /// 获取会话
    async fn get(&self, session_id: Uuid) -> Result<Option<MCPSession>, ErrorObject>;

    /// 删除会话
    async fn delete(&self, session_id: Uuid) -> Result<(), ErrorObject>;

    /// 列出用户的所有会话
    async fn list_by_user(&self, user_id: &str) -> Result<Vec<MCPSession>, ErrorObject>;
}

/// 内存存储实现（用于测试和开发环境）
pub struct InMemorySessionStore {
    store: Arc<tokio::sync::RwLock<DashMap<Uuid, MCPSession>>>,
}

impl InMemorySessionStore {
    /// 创建新的内存会话存储
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Arc::new(tokio::sync::RwLock::new(DashMap::new())),
        }
    }
}

impl Default for InMemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn save(&self, session: &MCPSession) -> Result<(), ErrorObject> {
        let store = self.store.read().await;
        store.insert(session.id, session.clone());
        Ok(())
    }

    async fn get(&self, session_id: Uuid) -> Result<Option<MCPSession>, ErrorObject> {
        let store = self.store.read().await;
        Ok(store.get(&session_id).map(|v| v.value().clone()))
    }

    async fn delete(&self, session_id: Uuid) -> Result<(), ErrorObject> {
        let store = self.store.write().await;
        if store.remove(&session_id).is_some() {
            Ok(())
        } else {
            Err(helpers::not_found("会话", &session_id.to_string()))
        }
    }

    async fn list_by_user(&self, user_id: &str) -> Result<Vec<MCPSession>, ErrorObject> {
        let store = self.store.read().await;
        let sessions: Vec<MCPSession> = store
            .iter()
            .filter(|s| s.user_id.as_deref().is_some_and(|u| u == user_id))
            .map(|v| v.value().clone())
            .collect();
        Ok(sessions)
    }
}

/// 会话配置
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// 默认会话超时时间（默认: 30 分钟）
    pub default_ttl: Duration,
    /// 最大消息数
    pub max_messages: usize,
    /// 最大 token 数
    pub max_tokens: usize,
    /// 清理间隔（默认: 5 分钟）
    pub cleanup_interval: Duration,
    /// 空闲超时时间（默认: 10 分钟）
    pub idle_timeout: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(1800),
            max_messages: 100,
            max_tokens: 128_000,
            cleanup_interval: Duration::from_secs(300),
            idle_timeout: Duration::from_secs(600),
        }
    }
}

/// 会话管理器
///
/// 提供完整的会话生命周期管理，包括创建、消息管理、
/// 上下文压缩和自动清理。
///
/// # Examples
///
/// ```ignore
/// let config = SessionConfig::default();
/// let store = Box::new(InMemorySessionStore::new());
/// let manager = SessionManager::with_config(config, store);
///
/// let session = manager.create_session(Some("user-001")).await?;
/// let message = Message { ... };
/// manager.add_message(session.id, message).await?;
/// ```
pub struct SessionManager {
    store: Box<dyn SessionStore>,
    active_sessions: DashMap<Uuid, MCPSession>,
    config: SessionConfig,
}

impl SessionManager {
    /// 使用自定义配置和存储创建管理器
    #[must_use]
    pub fn with_config(config: SessionConfig, store: Box<dyn SessionStore>) -> Self {
        Self {
            store,
            active_sessions: DashMap::new(),
            config,
        }
    }

    /// 使用默认配置创建管理器
    #[must_use]
    pub fn new(store: Box<dyn SessionStore>) -> Self {
        Self::with_config(SessionConfig::default(), store)
    }

    /// 创建新会话
    ///
    /// # Errors
    ///
    /// - 当存储操作失败时返回错误
    ///
    /// # Panics
    ///
    /// 当默认 TTL 无法转换为 `chrono::Duration` 时 panic（配置错误）。
    pub async fn create_session(&self, user_id: Option<String>) -> Result<MCPSession, ErrorObject> {
        let now = Utc::now();
        let session_id = Uuid::new_v4();

        let session = MCPSession {
            id: session_id,
            user_id,
            created_at: now,
            updated_at: now,
            expires_at: now
                + chrono::Duration::from_std(self.config.default_ttl)
                    .map_err(|e| helpers::config_error(&format!("default_ttl 配置值无效: {e}")))?,
            context_window: SessionContextWindow {
                max_messages: self.config.max_messages,
                max_tokens: self.config.max_tokens,
                ..Default::default()
            },
            state: SessionState::Active,
            metadata: SessionMetadata::default(),
        };

        self.store.save(&session).await?;
        self.active_sessions.insert(session_id, session.clone());

        tracing::info!("会话已创建: {} (user={:?})", session_id, session.user_id);

        Ok(session)
    }

    /// 获取会话
    ///
    /// 优先从内存缓存读取，未命中时查询存储。
    ///
    /// # Errors
    ///
    /// 当存储查询失败时返回错误。
    pub async fn get_session(&self, session_id: Uuid) -> Result<Option<MCPSession>, ErrorObject> {
        if let Some(session) = self.active_sessions.get(&session_id) {
            let session = session.value().clone();
            if session.state == SessionState::Expired || session.expires_at < Utc::now() {
                return Ok(None);
            }
            return Ok(Some(session));
        }

        if let Some(session) = self.store.get(session_id).await? {
            if session.state == SessionState::Closed
                || session.state == SessionState::Expired
                || session.expires_at < Utc::now()
            {
                return Ok(None);
            }
            return Ok(Some(session));
        }

        Ok(None)
    }

    /// 添加消息到上下文窗口
    ///
    /// 自动检查容量限制，必要时触发压缩。
    ///
    /// # Errors
    ///
    /// 当会话不存在或已过期时返回错误
    /// - 当消息添加后超出限制且压缩失败时返回错误
    ///
    /// # Panics
    ///
    /// 当 `default_ttl` 或 `idle_timeout` 无效（为 0 秒）时 panic。
    pub async fn add_message(
        &self,
        session_id: Uuid,
        mut message: Message,
    ) -> Result<(), ErrorObject> {
        let estimated_tokens = estimate_token_count(&message.content);
        message.token_count = estimated_tokens;

        {
            let mut session = self
                .active_sessions
                .get_mut(&session_id)
                .ok_or_else(|| helpers::not_found("会话", &session_id.to_string()))?;

            if session.state == SessionState::Closed || session.state == SessionState::Expired {
                return Err(helpers::auth_error(
                    &format!("会话 '{session_id}' 已关闭或过期"),
                    "add_message",
                ));
            }

            session.context_window.messages.push(message);
            session.context_window.current_tokens += estimated_tokens;
            session.updated_at = Utc::now();

            if Self::should_compress(&session) {
                drop(session);
                self.compress_context(session_id, CompressionStrategy::TruncateOldest)?;
            }
        }

        self.persist_session(session_id).await
    }

    /// 获取上下文窗口内容
    /// # Errors
    ///
    /// - 当会话不存在或已过期时返回错误
    #[must_use = "会话上下文查询结果必须被使用"]
    pub async fn get_context(&self, session_id: Uuid) -> Result<SessionContextWindow, ErrorObject> {
        let session = self
            .get_session(session_id)
            .await?
            .ok_or_else(|| helpers::not_found("会话", &session_id.to_string()))?;

        Ok(session.context_window)
    }

    /// 压缩上下文
    ///
    /// 根据指定策略压缩上下文窗口，释放空间。
    ///
    /// # Errors
    ///
    /// - 当会话不存在时返回错误
    ///
    /// # Panics
    ///
    /// 当 `default_ttl` 或 `idle_timeout` 无效（为 0 秒）时 panic。
    #[must_use = "上下文压缩结果必须被使用"]
    pub fn compress_context(
        &self,
        session_id: Uuid,
        strategy: CompressionStrategy,
    ) -> Result<(), ErrorObject> {
        let mut session = self
            .active_sessions
            .get_mut(&session_id)
            .ok_or_else(|| helpers::not_found("会话", &session_id.to_string()))?;

        let window = &mut session.context_window;

        match strategy {
            CompressionStrategy::TruncateOldest => {
                while (window.messages.len() > window.max_messages
                    || window.current_tokens > window.max_tokens)
                    && !window.messages.is_empty()
                {
                    let remove_idx = window
                        .messages
                        .iter()
                        .position(|m| !matches!(m.role, MessageRole::System));
                    if let Some(idx) = remove_idx {
                        let removed = window.messages.remove(idx);
                        window.current_tokens -= removed.token_count;
                    } else {
                        break;
                    }
                }
            }
            CompressionStrategy::SlidingWindow(keep_last) => {
                let to_remove = window.messages.len().saturating_sub(keep_last);
                for _ in 0..to_remove {
                    let removed = window.messages.remove(0);
                    window.current_tokens -= removed.token_count;
                }
            }
            CompressionStrategy::ImportanceSampling => {
                Self::importance_sampling(window);
            }
            CompressionStrategy::Summarize => {
                tracing::warn!("Summarize 策略需要外部 LLM 服务，回退到 TruncateOldest");
                while (window.messages.len() > window.max_messages
                    || window.current_tokens > window.max_tokens)
                    && !window.messages.is_empty()
                {
                    let removed = window.messages.remove(0);
                    window.current_tokens -= removed.token_count;
                }
            }
        }

        tracing::info!(
            "上下文已压缩: {session_id}, 剩余消息数={}, tokens={}",
            window.messages.len(),
            window.current_tokens
        );

        Ok(())
    }

    /// 关闭会话
    /// # Errors
    ///
    /// - 当会话不存在或持久化失败时返回错误
    #[must_use = "会话关闭结果必须被使用"]
    pub async fn close_session(&self, session_id: Uuid) -> Result<(), ErrorObject> {
        {
            let mut session = self
                .active_sessions
                .get_mut(&session_id)
                .ok_or_else(|| helpers::not_found("会话", &session_id.to_string()))?;

            let old_state = session.state;
            if !old_state.can_transition_to(&SessionState::Closed) {
                return Err(helpers::validation_error(
                    &format!("无法从 {old_state:?} 状态关闭会话"),
                    "close_session",
                ));
            }
            session.state = SessionState::Closed;
            session.updated_at = Utc::now();
            tracing::info!(
                session_id = %session_id,
                old_state = ?old_state,
                new_state = ?SessionState::Closed,
                triggered_by = "close_session",
                "Axiom-2: 会话状态变更已记录"
            );
        }

        self.persist_session(session_id).await?;
        self.active_sessions.remove(&session_id);

        tracing::info!("会话已关闭: {}", session_id);

        Ok(())
    }

    /// 清理过期会话
    ///
    /// 扫描所有活跃会话，移除已过期或空闲的会话。
    /// 返回清理的会话数量。
    /// 返回清理的会话数量。
    ///
    /// # Errors
    ///
    /// - 当存储删除操作失败时返回错误（仅记录警告，不中断流程）
    ///
    /// # Panics
    ///
    /// 当 `idle_timeout` 无效（为 0 秒）时 panic。
    #[must_use = "清理结果必须被使用"]
    pub async fn cleanup_expired(&self) -> Result<usize, ErrorObject> {
        let now = Utc::now();
        let idle_threshold = now
            - chrono::Duration::from_std(self.config.idle_timeout)
                .map_err(|e| helpers::config_error(&format!("idle_timeout 配置值无效: {e}")))?;

        let expired_ids: Vec<Uuid> = self
            .active_sessions
            .iter()
            .filter(|entry| {
                let session = entry.value();
                session.state == SessionState::Expired
                    || session.expires_at < now
                    || (session.state == SessionState::Idle && session.updated_at < idle_threshold)
            })
            .map(|entry| *entry.key())
            .collect();

        for id in &expired_ids {
            self.active_sessions.remove(id);
            if let Err(e) = self.store.delete(*id).await {
                tracing::warn!("清理会话失败 {}: {}", id, e);
            }
        }

        let count = expired_ids.len();
        if count > 0 {
            tracing::info!("已清理 {} 个过期/空闲会话", count);
        }

        Ok(count)
    }

    /// 更新会话活动状态
    /// # Errors
    ///
    /// - 当会话不存在时返回错误
    #[must_use = "会话更新结果必须被使用"]
    pub fn touch_session(&self, session_id: Uuid) -> Result<(), ErrorObject> {
        if let Some(mut session) = self.active_sessions.get_mut(&session_id) {
            let old_state = session.state;
            session.updated_at = Utc::now();
            if old_state.can_transition_to(&SessionState::Active) {
                session.state = SessionState::Active;
            }
            if old_state != session.state {
                tracing::info!(
                    session_id = %session_id,
                    old_state = ?old_state,
                    new_state = ?session.state,
                    triggered_by = "touch_session",
                    "Axiom-2: 会话状态变更已记录"
                );
            }
        }
        Ok(())
    }

    /// 获取活跃会话数量
    #[must_use]
    pub fn active_session_count(&self) -> usize {
        self.active_sessions.len()
    }

    // 内部方法

    fn should_compress(session: &MCPSession) -> bool {
        let window = &session.context_window;
        window.messages.len() >= window.max_messages || window.current_tokens >= window.max_tokens
    }

    fn importance_sampling(window: &mut SessionContextWindow) {
        let messages = &mut window.messages;

        if messages.len() <= 1 {
            return;
        }

        let mut scores: Vec<(usize, f64)> = messages
            .iter()
            .enumerate()
            .map(|(i, msg)| {
                let mut score = 0.0;

                match msg.role {
                    MessageRole::System => score += 10.0,
                    MessageRole::User => score += 5.0,
                    MessageRole::Assistant => score += 3.0,
                    MessageRole::Tool => score += 1.0,
                }

                if !msg.tool_calls.is_empty() {
                    score += 4.0;
                }

                if msg.content.len() > 200 {
                    score += 2.0;
                }

                #[allow(clippy::cast_precision_loss)]
                let age_factor = (i as f64) / (messages.len() as f64);
                score *= (1.0 - age_factor).mul_add(0.5, 0.5);

                (i, score)
            })
            .collect();

        scores.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let target_len = window.max_messages * 70 / 100;
        let to_remove = messages.len().saturating_sub(target_len);

        let mut to_remove_indices: Vec<usize> =
            scores[..to_remove].iter().map(|(idx, _)| *idx).collect();
        to_remove_indices.sort_unstable();

        let mut removed_tokens = 0;
        for idx in to_remove_indices.into_iter().rev() {
            if let Some(msg) = messages.get(idx) {
                removed_tokens += msg.token_count;
            }
            messages.remove(idx);
        }

        window.current_tokens = window.current_tokens.saturating_sub(removed_tokens);
    }

    async fn persist_session(&self, session_id: Uuid) -> Result<(), ErrorObject> {
        if let Some(session) = self.active_sessions.get(&session_id) {
            self.store.save(session.value()).await?;
        }
        Ok(())
    }
}

/// 估算 token 数量（简单启发式：约 4 字符/token）
fn estimate_token_count(text: &str) -> usize {
    (text.len() / 4).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> SessionConfig {
        SessionConfig {
            default_ttl: Duration::from_secs(3600),
            max_messages: 10,
            max_tokens: 10_000,
            cleanup_interval: Duration::from_secs(60),
            idle_timeout: Duration::from_secs(300),
        }
    }

    fn create_test_message(role: MessageRole, content: &str) -> Message {
        Message {
            role,
            content: content.to_string(),
            timestamp: Utc::now(),
            tool_calls: vec![],
            token_count: estimate_token_count(content),
        }
    }

    #[tokio::test]
    async fn test_create_and_get_session() {
        let config = create_test_config();
        let store = Box::new(InMemorySessionStore::new());
        let manager = SessionManager::with_config(config, store);

        let session = manager
            .create_session(Some("user-001".to_string()))
            .await
            .unwrap();
        assert_eq!(session.user_id.as_deref(), Some("user-001"));
        assert_eq!(session.state, SessionState::Active);

        let retrieved = manager.get_session(session.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, session.id);
    }

    #[tokio::test]
    async fn test_add_and_get_messages() {
        let config = create_test_config();
        let store = Box::new(InMemorySessionStore::new());
        let manager = SessionManager::with_config(config, store);

        let session = manager.create_session(None).await.unwrap();

        let msg = create_test_message(MessageRole::User, "Hello!");
        manager.add_message(session.id, msg).await.unwrap();

        let context = manager.get_context(session.id).await.unwrap();
        assert_eq!(context.messages.len(), 1);
        assert_eq!(context.messages[0].content, "Hello!");
    }

    #[tokio::test]
    async fn test_auto_compression_on_limit() {
        let config = SessionConfig {
            max_messages: 5,
            ..create_test_config()
        };
        let store = Box::new(InMemorySessionStore::new());
        let manager = SessionManager::with_config(config, store);

        let session = manager.create_session(None).await.unwrap();

        for i in 0..8 {
            let msg = create_test_message(MessageRole::User, &format!("Message {i}"));
            manager.add_message(session.id, msg).await.unwrap();
        }

        let context = manager.get_context(session.id).await.unwrap();
        assert!(context.messages.len() <= 5);
    }

    #[tokio::test]
    async fn test_close_session() {
        let config = create_test_config();
        let store = Box::new(InMemorySessionStore::new());
        let manager = SessionManager::with_config(config, store);

        let session = manager.create_session(None).await.unwrap();
        manager.close_session(session.id).await.unwrap();

        assert_eq!(manager.active_session_count(), 0);

        let retrieved = manager.get_session(session.id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_cleanup_expired_sessions() {
        let config = create_test_config();
        let store = Box::new(InMemorySessionStore::new());
        let manager = SessionManager::with_config(config, store);

        manager.create_session(None).await.unwrap();
        manager.create_session(None).await.unwrap();
        manager.create_session(None).await.unwrap();

        assert_eq!(manager.active_session_count(), 3);

        let cleaned = manager.cleanup_expired().await.unwrap();
        assert_eq!(cleaned, 0);
    }

    #[tokio::test]
    async fn test_sliding_window_compression() {
        let config = SessionConfig {
            max_messages: 10,
            ..create_test_config()
        };
        let store = Box::new(InMemorySessionStore::new());
        let manager = SessionManager::with_config(config, store);

        let session = manager.create_session(None).await.unwrap();

        for i in 0..15 {
            let msg = create_test_message(MessageRole::User, &format!("Test message number {i}"));
            manager.add_message(session.id, msg).await.unwrap();
        }

        manager
            .compress_context(session.id, CompressionStrategy::SlidingWindow(5))
            .unwrap();

        let context = manager.get_context(session.id).await.unwrap();
        assert_eq!(context.messages.len(), 5);
    }

    #[tokio::test]
    async fn test_importance_sampling_preserves_important_messages() {
        let config = SessionConfig {
            max_messages: 20,
            ..create_test_config()
        };
        let store = Box::new(InMemorySessionStore::new());
        let manager = SessionManager::with_config(config, store);

        let session = manager.create_session(None).await.unwrap();

        let system_msg = create_test_message(MessageRole::System, "You are a helpful assistant.");
        manager.add_message(session.id, system_msg).await.unwrap();

        for i in 0..25 {
            let msg = create_test_message(MessageRole::User, &format!("Regular message {i}"));
            manager.add_message(session.id, msg).await.unwrap();
        }

        manager
            .compress_context(session.id, CompressionStrategy::ImportanceSampling)
            .unwrap();

        let context = manager.get_context(session.id).await.unwrap();
        assert!(context.messages.len() <= 14);

        let has_system_msg = context
            .messages
            .iter()
            .any(|m| matches!(m.role, MessageRole::System));
        assert!(has_system_msg);
    }

    #[tokio::test]
    async fn test_reject_message_to_closed_session() {
        let config = create_test_config();
        let store = Box::new(InMemorySessionStore::new());
        let manager = SessionManager::with_config(config, store);

        let session = manager.create_session(None).await.unwrap();
        manager.close_session(session.id).await.unwrap();

        let msg = create_test_message(MessageRole::User, "After close");
        let result = manager.add_message(session.id, msg).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_concurrent_session_access() {
        let config = create_test_config();
        let store = Box::new(InMemorySessionStore::new());
        let manager = Arc::new(SessionManager::with_config(config, store));

        let mut handles = vec![];
        for _ in 0..50 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move { mgr.create_session(None).await }));
        }

        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }

        assert_eq!(manager.active_session_count(), 50);
    }
}
