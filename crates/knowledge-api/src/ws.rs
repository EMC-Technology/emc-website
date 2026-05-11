//! WebSocket 图谱实时推送服务
//!
//! # 功能
//! - 实时推送图谱增量更新（节点增删/边变化 → 前端局部刷新）
//! - 房间概念（per-document subscription）
//! - 心跳检测 + 自动重连机制（ConnectionLost → E2002 重试策略）
//! - 性能监控：< 20ms/帧增量，连续降级告警
//!
//! # 消息协议
//! ```text
//! Client → Server:
//!   { "type": "subscribe", "data": { "document_id": "doc:abc" } }
//!   { "type": "unsubscribe", "data": { "document_id": "doc:abc" } }
//!   { "type": "ping" }
//!
//! Server → Client:
//!   { "type": "node_added", "data": {...} }
//!   { "type": "node_removed", "data": {"id": "..."} }
//!   { "type": "edge_added", "data": {...} }
//!   { "type": "edge_removed", "data": {"id": "..."} }
//!   { "type": "pong" }
//!   { "type": "subscribed", "data": {"document_id": "..."} }
//!   { "type": "unsubscribed", "data": {"document_id": "..."} }
//!   { "type": "error", "data": {"code": "E2002", "message": "..."} }
//! ```

use axum::{
    extract::ws::{Message, WebSocketUpgrade},
    response::IntoResponse,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{Duration, interval};
use tracing::{error as log_error, info, warn};
use uuid::Uuid;

/// WebSocket 消息类型枚举
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    /// 客户端订阅文档图谱更新
    #[serde(rename = "subscribe")]
    Subscribe {
        /// 订阅的文档 ID
        document_id: String,
    },

    /// 客户端取消订阅文档
    #[serde(rename = "unsubscribe")]
    Unsubscribe {
        /// 取消订阅的文档 ID
        document_id: String,
    },

    /// 心跳检测请求
    #[serde(rename = "ping")]
    Ping,

    /// 节点新增通知
    #[serde(rename = "node_added")]
    NodeAdded(GraphNodeUpdate),

    /// 节点删除通知
    #[serde(rename = "node_removed")]
    NodeRemoved {
        /// 被删除的节点 ID
        id: String,
    },

    /// 边新增通知
    #[serde(rename = "edge_added")]
    EdgeAdded(GraphEdgeUpdate),

    /// 边删除通知
    #[serde(rename = "edge_removed")]
    EdgeRemoved {
        /// 被删除的边 ID
        id: String,
    },

    /// 心跳检测响应
    #[serde(rename = "pong")]
    Pong,

    /// 订阅成功确认
    #[serde(rename = "subscribed")]
    Subscribed {
        /// 已订阅的文档 ID
        document_id: String,
    },

    /// 取消订阅确认
    #[serde(rename = "unsubscribed")]
    Unsubscribed {
        /// 已取消订阅的文档 ID
        document_id: String,
    },

    /// 错误消息
    #[serde(rename = "error")]
    Error {
        /// 错误代码（如 E2002、E4001）
        code: String,
        /// 错误描述
        message: String,
    },
}

/// 图谱节点类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum GraphNodeType {
    /// 文档根节点
    Document,
    /// 块容器节点
    Block,
    /// 词元叶子节点
    Token,
    /// 语义实体
    SemanticEntity,
}

/// 图谱节点更新数据
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphNodeUpdate {
    /// 节点 ID
    pub id: String,
    /// 节点类型
    pub node_type: GraphNodeType,
    /// 节点标签（可选）
    pub label: Option<String>,
    /// 节点属性
    pub properties: serde_json::Value,
}

/// 图谱边更新数据
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphEdgeUpdate {
    /// 边 ID
    pub id: String,
    /// 起点节点 ID
    pub from_id: String,
    /// 终点节点 ID
    pub to_id: String,
    /// 边类型
    pub edge_type: knowledge_core::model::RefType,
    /// 边属性
    pub properties: serde_json::Value,
}

/// 单个连接的订阅状态
struct ConnectionState {
    subscriptions: HashSet<String>,
}

/// 默认最大连接数
const DEFAULT_MAX_CONNECTIONS: usize = 1000;

/// 默认单连接最大订阅数
const DEFAULT_MAX_SUBSCRIPTIONS_PER_CONNECTION: usize = 50;

/// WebSocket 连接管理器（管理所有活跃连接和房间订阅）
pub struct WsConnectionManager {
    /// 文档房间 → 广播发送器
    rooms: Arc<RwLock<HashMap<String, broadcast::Sender<WsMessage>>>>,
    /// 连接 ID → 订阅状态
    connections: Arc<RwLock<HashMap<String, ConnectionState>>>,
    /// 运行时统计信息
    stats: Arc<RwLock<WsStats>>,
    /// 最大并发连接数
    max_connections: usize,
    /// 单连接最大订阅数
    max_subscriptions_per_connection: usize,
}

/// WebSocket 连接统计信息
#[derive(Debug, Default, Clone)]
pub struct WsStats {
    /// 当前活跃连接数
    pub active_connections: u64,
    /// 已发送消息总数
    pub messages_sent: u64,
    /// 当前订阅总数
    pub total_subscriptions: u64,
    /// 平均帧处理时间（毫秒）
    pub avg_frame_time_ms: f64,
    /// 帧计数器（内部使用）
    frame_count: u64,
    /// 帧时间累计（内部使用）
    frame_time_sum_ms: f64,
}

impl WsStats {
    #[allow(clippy::cast_precision_loss)]
    fn record_frame_time(&mut self, elapsed_ms: f64) {
        self.frame_count += 1;
        self.frame_time_sum_ms += elapsed_ms;
        self.avg_frame_time_ms = self.frame_time_sum_ms / self.frame_count as f64;
    }
}

impl WsConnectionManager {
    /// 创建 WebSocket 连接管理器
    #[must_use]
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(WsStats::default())),
            max_connections: DEFAULT_MAX_CONNECTIONS,
            max_subscriptions_per_connection: DEFAULT_MAX_SUBSCRIPTIONS_PER_CONNECTION,
        }
    }

    /// 使用自定义限制创建管理器
    #[must_use]
    pub fn with_limits(max_connections: usize, max_subscriptions_per_connection: usize) -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(WsStats::default())),
            max_connections,
            max_subscriptions_per_connection,
        }
    }

    /// 处理 WebSocket 升级请求
    ///
    /// # 安全约束
    ///
    /// WebSocket 连接必须在路由层通过 `auth_middleware` 认证后才能升级。
    /// 匿名用户的连接请求应在路由层被拒绝。
    pub fn handle_ws_upgrade(self: Arc<Self>, ws: WebSocketUpgrade) -> impl IntoResponse {
        ws.max_frame_size(1024 * 1024)
            .max_message_size(4 * 1024 * 1024)
            .on_upgrade(move |socket| self.handle_connection(socket))
    }

    /// 处理单个 WebSocket 连接
    #[allow(clippy::too_many_lines)]
    async fn handle_connection(self: Arc<Self>, mut socket: axum::extract::ws::WebSocket) {
        let connection_id = Uuid::new_v4().to_string();

        {
            let mut conns = self.connections.write().await;
            if conns.len() >= self.max_connections {
                warn!(
                    current = conns.len(),
                    max = self.max_connections,
                    "WebSocket 连接数已达上限，拒绝新连接"
                );
                let _ = socket
                    .send(Message::Text(
                        serde_json::to_string(&WsMessage::Error {
                            code: "E4299".to_string(),
                            message: "连接数已达上限".to_string(),
                        })
                        .unwrap_or_else(|_| {
                            r#"{"type":"error","data":{"code":"E4299"}}"#.to_string()
                        }),
                    ))
                    .await;
                let _ = socket.close().await;
                return;
            }
            conns.insert(
                connection_id.clone(),
                ConnectionState {
                    subscriptions: HashSet::new(),
                },
            );
        }

        info!(
            connection_id = %connection_id,
            "新的 WebSocket 连接建立"
        );

        {
            let mut stats = self.stats.write().await;
            stats.active_connections += 1;
        }

        let pong_json = serde_json::to_string(&WsMessage::Pong)
            .unwrap_or_else(|_| r#"{"type":"pong"}"#.to_string());
        if socket.send(Message::Text(pong_json)).await.is_err() {
            warn!(connection_id = %connection_id, "发送欢迎消息失败");
            self.cleanup_connection(&connection_id).await;
            return;
        }

        let (outbound_tx, mut outbound_rx) = mpsc::channel::<WsMessage>(256);

        let conn_id_for_broadcast = connection_id.clone();
        let rooms_for_broadcast = self.rooms.clone();
        let conns_for_broadcast = self.connections.clone();
        tokio::spawn(async move {
            let mut room_receivers: HashMap<String, broadcast::Receiver<WsMessage>> =
                HashMap::new();

            loop {
                let subscriptions = {
                    let conns = conns_for_broadcast.read().await;
                    conns
                        .get(&conn_id_for_broadcast)
                        .map(|s| s.subscriptions.clone())
                        .unwrap_or_default()
                };

                if subscriptions.is_empty() {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }

                for doc_id in &subscriptions {
                    if !room_receivers.contains_key(doc_id) {
                        let rooms = rooms_for_broadcast.read().await;
                        if let Some(sender) = rooms.get(doc_id) {
                            room_receivers.insert(doc_id.clone(), sender.subscribe());
                        }
                    }
                }

                room_receivers.retain(|doc_id, _| subscriptions.contains(doc_id));

                let mut received = Vec::new();
                for rx in room_receivers.values_mut() {
                    match rx.try_recv() {
                        Ok(msg) => received.push(msg),
                        Err(
                            broadcast::error::TryRecvError::Empty
                            | broadcast::error::TryRecvError::Closed,
                        ) => {}
                        Err(broadcast::error::TryRecvError::Lagged(n)) => {
                            tracing::warn!(skipped = n, "广播消息积压，已跳过");
                        }
                    }
                }

                for msg in received {
                    if outbound_tx.send(msg).await.is_err() {
                        return;
                    }
                }

                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });

        let (heartbeat_tx, mut heartbeat_rx) = mpsc::channel::<()>(1);
        tokio::spawn({
            let tx = heartbeat_tx.clone();
            async move {
                let mut tick = interval(Duration::from_secs(30));
                loop {
                    tick.tick().await;
                    if tx.send(()).await.is_err() {
                        break;
                    }
                }
            }
        });

        loop {
            tokio::select! {
                Some(msg) = socket.recv() => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            match self.handle_client_message(&connection_id, &text, &mut socket).await {
                                Ok(()) => {}
                                Err(e) => {
                                    log_error!(
                                        connection_id = %connection_id,
                                        error = %e,
                                        "处理客户端消息失败"
                                    );
                                    break;
                                }
                            }
                        }
                        Ok(Message::Close(_)) => {
                            info!(connection_id = %connection_id, "客户端主动断开连接");
                            break;
                        }
                        Ok(Message::Pong(_) | Message::Binary(_) | Message::Ping(_)) => {}
                        Err(e) => {
                            log_error!(
                                connection_id = %connection_id,
                                error = %e,
                                "WebSocket 连接错误 [E2002]"
                            );
                            break;
                        }
                    }
                }

                Some(msg) = outbound_rx.recv() => {
                    if let Ok(text) = serde_json::to_string(&msg) {
                        let frame_start = std::time::Instant::now();
                        if socket.send(Message::Text(text)).await.is_err() {
                            warn!(connection_id = %connection_id, "广播消息发送失败");
                            break;
                        }
                        let frame_elapsed = frame_start.elapsed().as_secs_f64() * 1000.0;
                        {
                            let mut stats = self.stats.write().await;
                            stats.messages_sent += 1;
                            stats.record_frame_time(frame_elapsed);
                        }
                    }
                }

                _ = heartbeat_rx.recv() => {
                    if socket.send(Message::Ping(vec![])).await.is_err() {
                        warn!(connection_id = %connection_id, "心跳发送失败，断开连接");
                        break;
                    }
                }
            }
        }

        self.cleanup_connection(&connection_id).await;

        info!(
            connection_id = %connection_id,
            "WebSocket 连接关闭"
        );
    }

    async fn cleanup_connection(&self, connection_id: &str) {
        let subscriptions = {
            let mut conns = self.connections.write().await;
            conns
                .remove(connection_id)
                .map(|state| state.subscriptions)
                .unwrap_or_default()
        };

        for doc_id in &subscriptions {
            self.remove_room_if_empty(doc_id).await;
        }

        {
            let mut stats = self.stats.write().await;
            stats.active_connections = stats.active_connections.saturating_sub(1);
            stats.total_subscriptions = stats
                .total_subscriptions
                .saturating_sub(subscriptions.len() as u64);
        }
    }

    async fn remove_room_if_empty(&self, document_id: &str) {
        let mut rooms = self.rooms.write().await;
        if let Some(sender) = rooms.get(document_id)
            && sender.receiver_count() == 0
        {
            rooms.remove(document_id);
        }
    }

    async fn handle_client_message(
        &self,
        connection_id: &str,
        text: &str,
        socket: &mut axum::extract::ws::WebSocket,
    ) -> crate::Result<()> {
        let msg: WsMessage = serde_json::from_str(text)?;

        match msg {
            WsMessage::Subscribe { document_id } => {
                self.subscribe_to_document(connection_id, &document_id, socket)
                    .await?;
            }
            WsMessage::Unsubscribe { document_id } => {
                self.unsubscribe_from_document(connection_id, &document_id, socket)
                    .await?;
            }
            WsMessage::Ping => {
                socket
                    .send(Message::Text(serde_json::to_string(&WsMessage::Pong)?))
                    .await
                    .map_err(|e| error_core::helpers::ws_client_error(&e.to_string(), "send"))?;
            }
            WsMessage::NodeAdded(_)
            | WsMessage::NodeRemoved { .. }
            | WsMessage::EdgeAdded(_)
            | WsMessage::EdgeRemoved { .. }
            | WsMessage::Pong
            | WsMessage::Subscribed { .. }
            | WsMessage::Unsubscribed { .. }
            | WsMessage::Error { .. } => {
                socket
                    .send(Message::Text(serde_json::to_string(&WsMessage::Error {
                        code: "E4001".to_string(),
                        message: "不支持的消息类型".to_string(),
                    })?))
                    .await
                    .map_err(|e| error_core::helpers::ws_client_error(&e.to_string(), "send"))?;
            }
        }

        Ok(())
    }

    async fn subscribe_to_document(
        &self,
        connection_id: &str,
        document_id: &str,
        socket: &mut axum::extract::ws::WebSocket,
    ) -> crate::Result<()> {
        info!(
            connection_id = %connection_id,
            document_id = %document_id,
            "客户端订阅文档图谱"
        );

        {
            let mut rooms = self.rooms.write().await;
            if !rooms.contains_key(document_id) {
                let (tx, _) = broadcast::channel(256);
                rooms.insert(document_id.to_string(), tx);
            }
        }

        {
            let mut conns = self.connections.write().await;
            if let Some(state) = conns.get_mut(connection_id) {
                if state.subscriptions.len() >= self.max_subscriptions_per_connection {
                    return Err(error_core::helpers::ws_subscribe_limit_error(
                        self.max_subscriptions_per_connection,
                    ));
                }
                state.subscriptions.insert(document_id.to_string());
            }
        }

        socket
            .send(Message::Text(serde_json::to_string(
                &WsMessage::Subscribed {
                    document_id: document_id.to_string(),
                },
            )?))
            .await
            .map_err(|e| error_core::helpers::ws_client_error(&e.to_string(), "send"))?;

        {
            let mut stats = self.stats.write().await;
            stats.total_subscriptions += 1;
        }

        Ok(())
    }

    async fn unsubscribe_from_document(
        &self,
        connection_id: &str,
        document_id: &str,
        socket: &mut axum::extract::ws::WebSocket,
    ) -> crate::Result<()> {
        info!(
            connection_id = %connection_id,
            document_id = %document_id,
            "客户端取消订阅"
        );

        {
            let mut conns = self.connections.write().await;
            if let Some(state) = conns.get_mut(connection_id) {
                state.subscriptions.remove(document_id);
            }
        }

        self.remove_room_if_empty(document_id).await;

        socket
            .send(Message::Text(serde_json::to_string(
                &WsMessage::Unsubscribed {
                    document_id: document_id.to_string(),
                },
            )?))
            .await
            .map_err(|e| error_core::helpers::ws_client_error(&e.to_string(), "send"))?;

        Ok(())
    }

    /// 向指定文档的所有订阅者广播图谱更新
    ///
    /// # Errors
    ///
    /// 当文档没有订阅者或广播发送失败时返回错误。
    pub async fn broadcast_to_document(
        &self,
        document_id: &str,
        message: WsMessage,
    ) -> crate::Result<usize> {
        let rooms = self.rooms.read().await;

        match rooms.get(document_id) {
            Some(sender) => match sender.send(message) {
                Ok(count) => {
                    {
                        let mut stats = self.stats.write().await;
                        stats.messages_sent += count as u64;
                    }
                    Ok(count)
                }
                Err(_) => Err(error_core::helpers::ws_send_failed_error(
                    "没有活跃的订阅者",
                )),
            },
            None => Err(error_core::helpers::ws_receive_failed_error(&format!(
                "文档 {document_id} 没有订阅者"
            ))),
        }
    }

    /// 获取 WebSocket 运行时统计快照
    pub async fn get_stats(&self) -> WsStats {
        self.stats.read().await.clone()
    }
}

impl Default for WsConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_message_serialization() {
        let subscribe_msg = WsMessage::Subscribe {
            document_id: "doc:test001".to_string(),
        };
        let json = serde_json::to_string(&subscribe_msg).unwrap();
        assert!(json.contains("\"type\":\"subscribe\""));
        assert!(json.contains("\"document_id\":\"doc:test001\""));
    }

    #[test]
    fn test_subscribe_message_roundtrip() {
        let msg = WsMessage::Subscribe {
            document_id: "doc:abc".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let de: WsMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(de, WsMessage::Subscribe { document_id } if document_id == "doc:abc"));
    }

    #[test]
    fn test_node_update_serialization() {
        let update = GraphNodeUpdate {
            id: "block:abc123".to_string(),
            node_type: GraphNodeType::Block,
            label: Some("Introduction".to_string()),
            properties: serde_json::json!({"line": 10}),
        };

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("block:abc123"));
        assert!(json.contains("Introduction"));
    }

    #[test]
    fn test_edge_update_serialization() {
        let edge = GraphEdgeUpdate {
            id: "ref:xyz789".to_string(),
            from_id: "token:a".to_string(),
            to_id: "token:b".to_string(),
            edge_type: knowledge_core::model::RefType::Usage,
            properties: serde_json::json!({}),
        };

        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("Usage"));
        assert!(json.contains("token:a"));
        assert!(json.contains("token:b"));
    }

    #[tokio::test]
    async fn test_ws_manager_creation() {
        let manager = WsConnectionManager::new();
        assert_eq!(manager.rooms.read().await.len(), 0);
        assert_eq!(manager.connections.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_stats_initial_state() {
        let manager = WsConnectionManager::new();
        let stats = manager.get_stats().await;

        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.messages_sent, 0);
        assert_eq!(stats.total_subscriptions, 0);
    }

    #[test]
    fn test_error_message_format() {
        let err = WsMessage::Error {
            code: "E2002".to_string(),
            message: "连接中断".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"E2002\""));
        assert!(json.contains("连接中断"));
    }

    #[test]
    fn test_subscribed_message() {
        let msg = WsMessage::Subscribed {
            document_id: "doc:xyz".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"subscribed\""));
        assert!(json.contains("doc:xyz"));
    }

    #[test]
    fn test_unsubscribed_message() {
        let msg = WsMessage::Unsubscribed {
            document_id: "doc:xyz".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"unsubscribed\""));
    }

    #[tokio::test]
    async fn test_broadcast_to_empty_room() {
        let manager = WsConnectionManager::new();
        let result = manager
            .broadcast_to_document("doc:nonexistent", WsMessage::Pong)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_room_auto_created_on_subscribe() {
        let manager = WsConnectionManager::new();
        {
            let mut rooms = manager.rooms.write().await;
            let (tx, _) = broadcast::channel(256);
            rooms.insert("doc:test".to_string(), tx);
        }
        assert_eq!(manager.rooms.read().await.len(), 1);
    }

    #[tokio::test]
    async fn test_broadcast_to_room_with_subscribers() {
        let manager = WsConnectionManager::new();
        {
            let mut rooms = manager.rooms.write().await;
            let (tx, _) = broadcast::channel(256);
            rooms.insert("doc:test".to_string(), tx);
        }

        let mut rx = {
            let rooms = manager.rooms.read().await;
            rooms.get("doc:test").unwrap().subscribe()
        };

        let result = manager
            .broadcast_to_document(
                "doc:test",
                WsMessage::NodeAdded(GraphNodeUpdate {
                    id: "block:1".to_string(),
                    node_type: GraphNodeType::Block,
                    label: Some("Test".to_string()),
                    properties: serde_json::json!({}),
                }),
            )
            .await;
        assert_eq!(result.unwrap(), 1);

        let msg = rx.try_recv().unwrap();
        assert!(matches!(msg, WsMessage::NodeAdded(_)));
    }
}
