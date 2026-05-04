//! 前端 API 客户端与状态管理
//!
//! 提供 `RESTful` API 访问与 WebSocket 实时通信功能。
//! 使用 `gloo-net` 实现 HTTP 请求，
//! 使用 `web_sys::WebSocket` 实现实时通信。

#![allow(clippy::future_not_send)]

use error_core::helpers;
use serde_json::Value;
use wasm_bindgen::prelude::*;

use knowledge_core::model::{Block, Document, Token};

type Result<T> = error_core::Result<T>;

const API_BASE: &str = "/api/v1";

#[allow(clippy::needless_pass_by_value)]
fn extract_typed_data<T: serde::de::DeserializeOwned>(resp: Value) -> Result<T> {
    if resp
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let data = resp
            .get("data")
            .ok_or_else(|| helpers::api_deserialize_error("响应缺少 data 字段"))?;
        serde_json::from_value(data.clone())
            .map_err(|e| helpers::api_deserialize_error(&format!("数据反序列化失败: {e}")))
    } else {
        let msg = resp
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        let code = resp.get("code").and_then(Value::as_str).unwrap_or("E9999");
        Err(helpers::net_api_error(&format!("[{code}] {msg}")))
    }
}

#[allow(clippy::needless_pass_by_value)]
fn extract_typed_data_vec<T: serde::de::DeserializeOwned>(resp: Value) -> Result<Vec<T>> {
    if resp
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let data = resp
            .get("data")
            .ok_or_else(|| helpers::api_deserialize_error("响应缺少 data 字段"))?;
        let arr = data
            .as_array()
            .ok_or_else(|| helpers::api_deserialize_error("数据不是数组"))?;
        arr.iter()
            .map(|item| {
                serde_json::from_value(item.clone())
                    .map_err(|e| helpers::api_deserialize_error(&format!("数据反序列化失败: {e}")))
            })
            .collect()
    } else {
        let msg = resp
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        let code = resp.get("code").and_then(Value::as_str).unwrap_or("E9999");
        Err(helpers::net_api_error(&format!("[{code}] {msg}")))
    }
}

/// 创建 API 客户端实例
#[must_use]
pub fn create_api_client() -> ApiClient {
    ApiClient::new()
}

/// 创建实时同步服务
#[must_use]
pub fn create_realtime_sync() -> RealtimeSync {
    RealtimeSync::new()
}

/// 创建知识图谱状态管理器
#[must_use]
pub const fn create_knowledge_graph() -> KnowledgeGraphState {
    KnowledgeGraphState::new()
}

/// `RESTful` API 客户端
///
/// 封装所有 API 的 HTTP 通信，支持自定义基础 URL。
/// 通过 Dioxus Context 注入确保在单页应用的地址变更后仍然有效。
#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    auth_token: Option<String>,
}

impl ApiClient {
    /// 创建 API 客户端实例，使用默认基础路径
    #[must_use]
    pub fn new() -> Self {
        Self {
            base_url: API_BASE.to_string(),
            auth_token: None,
        }
    }

    /// 使用自定义基础 URL 创建客户端
    #[must_use]
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            auth_token: None,
        }
    }

    /// 设置认证令牌
    #[must_use]
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// 尝试获取已存储的认证令牌
    ///
    /// 注意：出于安全考虑（防 XSS 攻击），不再从 localStorage 读取令牌。
    /// 令牌应通过 `with_auth_token()` 方法显式设置。
    #[must_use]
    pub const fn with_stored_auth(self) -> Self {
        self
    }

    /// 更新基础 URL（用于运行时动态修改 API 地址）
    pub fn set_base_url(&mut self, base_url: impl Into<String>) {
        self.base_url = base_url.into();
    }

    /// 获取当前基础 URL
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 获取文档列表
    ///
    /// # Errors
    ///
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn get_documents(&self) -> Result<Vec<Value>> {
        let url = format!("{}/documents", self.base_url);
        self.get_json(&url).await.and_then(Self::extract_data_array)
    }

    /// 获取单个文档详情
    ///
    /// # Errors
    ///
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn get_document(&self, id: &str) -> Result<Value> {
        let encoded_id = js_sys::encode_uri_component(id);
        let url = format!("{}/documents/{}", self.base_url, encoded_id);
        self.get_json(&url).await.and_then(Self::extract_data)
    }

    /// 上传文件到服务端
    ///
    /// # Errors
    ///
    /// 路径或内容为空时返回错误
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn upload_file(
        &self,
        path: &str,
        title: Option<&str>,
        content: &str,
    ) -> Result<Value> {
        if path.trim().is_empty() {
            return Err(helpers::validation_error("文件路径不能为空", "upload"));
        }
        if content.trim().is_empty() {
            return Err(helpers::validation_error("文件内容不能为空", "upload"));
        }
        let url = format!("{}/documents", self.base_url);
        let body = serde_json::json!({
            "path": path,
            "title": title,
            "content": content,
        });
        self.post_json(&url, &body)
            .await
            .and_then(Self::extract_data)
    }

    /// 删除文档
    ///
    /// # Errors
    ///
    /// 网络请求失败时返回错误
    pub async fn delete_document(&self, id: &str) -> Result<()> {
        let encoded_id = js_sys::encode_uri_component(id);
        let url = format!("{}/documents/{}", self.base_url, encoded_id);
        self.delete(&url).await
    }

    /// 获取文档的文本块列表
    ///
    /// # Errors
    ///
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn get_document_blocks(&self, doc_id: &str) -> Result<Vec<Value>> {
        let encoded_id = js_sys::encode_uri_component(doc_id);
        let url = format!("{}/documents/{}/blocks", self.base_url, encoded_id);
        self.get_json(&url).await.and_then(Self::extract_data_array)
    }

    /// 获取单个文本块详情
    ///
    /// # Errors
    ///
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn get_block(&self, id: &str) -> Result<Value> {
        let encoded_id = js_sys::encode_uri_component(id);
        let url = format!("{}/blocks/{}", self.base_url, encoded_id);
        self.get_json(&url).await.and_then(Self::extract_data)
    }

    /// 获取文本块的词法单元列表
    ///
    /// # Errors
    ///
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn get_block_tokens(&self, block_id: &str) -> Result<Vec<Value>> {
        let encoded_id = js_sys::encode_uri_component(block_id);
        let url = format!("{}/blocks/{}/tokens", self.base_url, encoded_id);
        self.get_json(&url).await.and_then(Self::extract_data_array)
    }

    /// 获取单个词法单元详情
    ///
    /// # Errors
    ///
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn get_token(&self, id: &str) -> Result<Value> {
        let encoded_id = js_sys::encode_uri_component(id);
        let url = format!("{}/tokens/{}", self.base_url, encoded_id);
        self.get_json(&url).await.and_then(Self::extract_data)
    }

    /// 追加查询词的引用关系
    ///
    /// # Errors
    ///
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn trace_references(&self, token_id: &str, ref_type: Option<&str>) -> Result<Value> {
        let encoded_id = js_sys::encode_uri_component(token_id);
        let url = ref_type.map_or_else(
            || format!("{}/tokens/{}/references", self.base_url, encoded_id),
            |rt| {
                let encoded_rt = js_sys::encode_uri_component(rt);
                format!(
                    "{}/tokens/{}/references?ref_type={}",
                    self.base_url, encoded_id, encoded_rt
                )
            },
        );
        self.get_json(&url).await.and_then(Self::extract_data)
    }

    /// 执行全文搜索
    ///
    /// # Errors
    ///
    /// 搜索关键词为空时返回错误
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn full_text_search(&self, query: &str, limit: Option<u32>) -> Result<Vec<Value>> {
        if query.trim().is_empty() {
            return Err(helpers::validation_error("搜索关键词不能为空", "search"));
        }
        let encoded_query = js_sys::encode_uri_component(query);
        let url = limit.map_or_else(
            || format!("{}/search/fulltext?q={}", self.base_url, encoded_query),
            |l| {
                format!(
                    "{}/search/fulltext?q={}&limit={}",
                    self.base_url, encoded_query, l
                )
            },
        );
        self.get_json(&url).await.and_then(Self::extract_data_array)
    }

    /// 健康检查
    ///
    /// # Errors
    ///
    /// 网络请求失败时返回错误
    pub async fn health_check(&self) -> Result<Value> {
        let url = format!("{}/healthz", self.base_url);
        self.get_json(&url).await
    }

    /// 获取文档列表（类型化）
    ///
    /// # Errors
    ///
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn get_documents_typed(&self) -> Result<Vec<Document>> {
        let url = format!("{}/documents", self.base_url);
        self.get_json(&url).await.and_then(extract_typed_data_vec)
    }

    /// 获取单个文档详情（类型化）
    ///
    /// # Errors
    ///
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn get_document_typed(&self, id: &str) -> Result<Document> {
        let encoded_id = js_sys::encode_uri_component(id);
        let url = format!("{}/documents/{}", self.base_url, encoded_id);
        self.get_json(&url).await.and_then(extract_typed_data)
    }

    /// 获取文档的文本块列表（类型化）
    ///
    /// # Errors
    ///
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn get_document_blocks_typed(&self, doc_id: &str) -> Result<Vec<Block>> {
        let encoded_id = js_sys::encode_uri_component(doc_id);
        let url = format!("{}/documents/{}/blocks", self.base_url, encoded_id);
        self.get_json(&url).await.and_then(extract_typed_data_vec)
    }

    /// 获取文本块的词法单元列表（类型化）
    ///
    /// # Errors
    ///
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn get_block_tokens_typed(&self, block_id: &str) -> Result<Vec<Token>> {
        let encoded_id = js_sys::encode_uri_component(block_id);
        let url = format!("{}/blocks/{}/tokens", self.base_url, encoded_id);
        self.get_json(&url).await.and_then(extract_typed_data_vec)
    }

    /// 执行全文搜索（类型化）
    ///
    /// # Errors
    ///
    /// 搜索关键词为空时返回错误
    /// 网络请求失败或响应解析失败时返回错误
    pub async fn full_text_search_typed(
        &self,
        query: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Block>> {
        if query.trim().is_empty() {
            return Err(helpers::validation_error("搜索关键词不能为空", "search"));
        }
        let encoded_query = js_sys::encode_uri_component(query);
        let url = limit.map_or_else(
            || format!("{}/search/fulltext?q={}", self.base_url, encoded_query),
            |l| {
                format!(
                    "{}/search/fulltext?q={}&limit={}",
                    self.base_url, encoded_query, l
                )
            },
        );
        self.get_json(&url).await.and_then(extract_typed_data_vec)
    }

    #[allow(clippy::needless_pass_by_value)]
    fn extract_data(resp: Value) -> Result<Value> {
        if resp
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            Ok(resp.get("data").cloned().unwrap_or(Value::Null))
        } else {
            let msg = resp
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("未知错误");
            let code = resp.get("code").and_then(Value::as_str).unwrap_or("E9999");
            Err(helpers::net_api_error(&format!("[{code}] {msg}")))
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn extract_data_array(resp: Value) -> Result<Vec<Value>> {
        let data = Self::extract_data(resp)?;
        match data {
            Value::Array(arr) => Ok(arr),
            other => Err(helpers::api_deserialize_error(&format!(
                "数据类型错误，期望数组，实际得到: {other}"
            ))),
        }
    }

    async fn get_json(&self, url: &str) -> Result<Value> {
        fetch_json("GET", url, None, self.auth_token.as_deref()).await
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value> {
        fetch_json("POST", url, Some(body), self.auth_token.as_deref()).await
    }

    async fn delete(&self, url: &str) -> Result<()> {
        fetch_json("DELETE", url, None, self.auth_token.as_deref()).await?;
        Ok(())
    }
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}

async fn fetch_json(
    method: &str,
    url: &str,
    body: Option<&Value>,
    auth_token: Option<&str>,
) -> Result<Value> {
    use gloo_net::http::Request;

    let builder = match method {
        "POST" => Request::post(url),
        "DELETE" => Request::delete(url),
        _ => Request::get(url),
    };

    let builder = if let Some(token) = auth_token {
        builder.header("Authorization", &format!("Bearer {token}"))
    } else {
        builder
    };

    let request = if let Some(b) = body {
        let json_str = serde_json::to_string(b).map_err(|e| {
            helpers::api_request_error(&format!("序列化请求体失败: {e}"), "serialize")
        })?;
        builder
            .header("Content-Type", "application/json")
            .body(json_str)
            .map_err(|e| helpers::api_request_error(&format!("设置请求体失败: {e}"), "set_body"))?
    } else {
        builder
            .header("Content-Type", "application/json")
            .body("")
            .map_err(|e| helpers::api_request_error(&format!("设置请求体失败: {e}"), "set_body"))?
    };

    let response = request
        .send()
        .await
        .map_err(|e| helpers::net_api_error(&format!("请求失败: {e}")))?;

    let status = response.status();
    if status >= 400 {
        return Err(helpers::net_api_error(&format!("HTTP 错误: {status}")));
    }

    let text = response
        .text()
        .await
        .map_err(|e| helpers::net_api_error(&format!("读取响应失败: {e}")))?;

    if text.is_empty() {
        return Ok(Value::Null);
    }

    serde_json::from_str(&text)
        .map_err(|e| helpers::api_deserialize_error(&format!("JSON 解析失败: {e}")))
}

/// WebSocket 实时同步服务
///
/// 封装服务端的 WebSocket 连接（待实现）。
/// 自动根据页面协议选择 ws:// 或 wss://。
pub struct RealtimeSync {
    url: String,
}

impl RealtimeSync {
    /// 创建实时同步服务实例
    #[must_use]
    pub fn new() -> Self {
        let protocol = web_sys::window()
            .and_then(|w| w.location().protocol().ok())
            .map_or("ws", |p| if p == "https:" { "wss" } else { "ws" });
        Self {
            url: format!("{protocol}://{}/api/v1/ws", window_host()),
        }
    }

    /// 建立 WebSocket 连接
    #[must_use]
    pub fn connect(&self) -> Option<RealtimeConnection> {
        let ws = web_sys::WebSocket::new(&self.url).ok()?;
        let connection = RealtimeConnection {
            ws: Some(ws.clone()),
            message_handler: None,
            error_handler: None,
            close_handler: None,
        };

        // 设置消息监听器
        let connection_ptr = std::rc::Rc::new(std::cell::RefCell::new(connection));
        let connection_ptr_clone = connection_ptr.clone();

        let on_message = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
            if let Some(text) = event.data().as_string() {
                if let Ok(json) = serde_json::from_str(&text) {
                    let connection = connection_ptr_clone.borrow();
                    if let Some(handler) = &connection.message_handler {
                        handler(json);
                    }
                }
            }
        }) as Box<dyn FnMut(_)>);

        // 设置错误监听器
        let connection_ptr_clone2 = connection_ptr.clone();
        let on_error = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            let connection = connection_ptr_clone2.borrow();
            if let Some(handler) = &connection.error_handler {
                handler("WebSocket 错误".to_string());
            }
        }) as Box<dyn FnMut(_)>);

        // 设置关闭监听器
        let connection_ptr_clone3 = connection_ptr.clone();
        let on_close = Closure::wrap(Box::new(move |_event: web_sys::CloseEvent| {
            let connection = connection_ptr_clone3.borrow();
            if let Some(handler) = &connection.close_handler {
                handler();
            }
        }) as Box<dyn FnMut(_)>);

        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        on_message.forget();
        on_error.forget();
        on_close.forget();

        // 创建一个新的连接结构体返回给调用者
        let conn_ref = connection_ptr.borrow();
        let ws_clone = conn_ref.ws.clone();
        drop(conn_ref);
        Some(RealtimeConnection {
            ws: ws_clone,
            message_handler: None,
            error_handler: None,
            close_handler: None,
        })
    }
}

impl Default for RealtimeSync {
    fn default() -> Self {
        Self::new()
    }
}

/// WebSocket 消息处理器
pub type MessageHandler = Box<dyn Fn(Value) + 'static>;

/// WebSocket 连接实例
///
/// 封装原生 WebSocket 连接的订阅、发送和关闭操作。
pub struct RealtimeConnection {
    ws: Option<web_sys::WebSocket>,
    message_handler: Option<MessageHandler>,
    error_handler: Option<Box<dyn Fn(String) + 'static>>,
    close_handler: Option<Box<dyn Fn() + 'static>>,
}

impl RealtimeConnection {
    /// 订阅文档变更通知
    ///
    /// # Errors
    ///
    /// WebSocket 未连接或发送消息失败时返回错误
    pub fn subscribe(&self, document_id: &str) -> Result<()> {
        if let Some(ws) = &self.ws {
            let msg = serde_json::json!({
                "type": "subscribe",
                "data": { "document_id": document_id }
            });
            let text = serde_json::to_string(&msg)
                .map_err(|e| helpers::ws_serialize_error(&e.to_string()))?;
            ws.send_with_str(&text)
                .map_err(|e| helpers::ws_client_error(&format!("发送消息失败: {e:?}"), "send"))?;
            Ok(())
        } else {
            Err(helpers::ws_client_error("WebSocket 未连接", "connect"))
        }
    }

    /// 退订文档变更通知
    ///
    /// # Errors
    ///
    /// WebSocket 未连接或发送消息失败时返回错误
    pub fn unsubscribe(&self, document_id: &str) -> Result<()> {
        if let Some(ws) = &self.ws {
            let msg = serde_json::json!({
                "type": "unsubscribe",
                "data": { "document_id": document_id }
            });
            let text = serde_json::to_string(&msg)
                .map_err(|e| helpers::ws_serialize_error(&e.to_string()))?;
            ws.send_with_str(&text)
                .map_err(|e| helpers::ws_client_error(&format!("发送消息失败: {e:?}"), "send"))?;
            Ok(())
        } else {
            Err(helpers::ws_client_error("WebSocket 未连接", "connect"))
        }
    }

    /// 设置消息处理器
    pub fn set_message_handler<F>(&mut self, handler: F)
    where
        F: Fn(Value) + 'static,
    {
        self.message_handler = Some(Box::new(handler));
    }

    /// 设置错误处理器
    pub fn set_error_handler<F>(&mut self, handler: F)
    where
        F: Fn(String) + 'static,
    {
        self.error_handler = Some(Box::new(handler));
    }

    /// 设置关闭处理器
    pub fn set_close_handler<F>(&mut self, handler: F)
    where
        F: Fn() + 'static,
    {
        self.close_handler = Some(Box::new(handler));
    }

    /// 关闭 WebSocket 连接
    pub fn close(&mut self) {
        if let Some(ws) = self.ws.take() {
            let _ = ws.close();
        }
    }

    /// 发送消息
    ///
    /// # Errors
    ///
    /// WebSocket 未连接或发送消息失败时返回错误
    pub fn send(&self, message: &Value) -> Result<()> {
        if let Some(ws) = &self.ws {
            let text = serde_json::to_string(message)
                .map_err(|e| helpers::ws_serialize_error(&e.to_string()))?;
            ws.send_with_str(&text)
                .map_err(|e| helpers::ws_client_error(&format!("发送消息失败: {e:?}"), "send"))?;
            Ok(())
        } else {
            Err(helpers::ws_client_error("WebSocket 未连接", "connect"))
        }
    }
}

impl Drop for RealtimeConnection {
    fn drop(&mut self) {
        self.close();
    }
}

/// 知识图谱状态
///
/// 管理知识图谱的节点和边数据。
/// 当前仅支持节点和边的增删改查，Reference 关系获取是后续功能。
pub struct KnowledgeGraphState {
    nodes: Vec<Value>,
    edges: Vec<Value>,
}

impl KnowledgeGraphState {
    /// 创建知识图谱状态
    #[must_use]
    pub const fn new() -> Self {
        Self {
            nodes: vec![],
            edges: vec![],
        }
    }

    /// 获取图谱节点列表
    #[must_use]
    pub fn nodes(&self) -> &[Value] {
        &self.nodes
    }

    /// 获取图谱边列表
    #[must_use]
    pub fn edges(&self) -> &[Value] {
        &self.edges
    }

    /// 设置图谱节点
    pub fn set_nodes(&mut self, nodes: Vec<Value>) {
        self.nodes = nodes;
    }

    /// 设置图谱边
    pub fn set_edges(&mut self, edges: Vec<Value>) {
        self.edges = edges;
    }

    /// 从文档列表加载图谱数据
    ///
    /// 仅生成节点（文档），边需要后续调用 Reference API 获取。
    ///
    /// # Errors
    ///
    /// API 请求失败时返回错误
    pub async fn load(&mut self) -> Result<()> {
        let api = create_api_client();
        let documents = api.get_documents().await?;

        self.nodes = documents
            .iter()
            .map(|doc| {
                let id = doc.get("id").and_then(Value::as_str).unwrap_or("");
                let label = doc
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("Untitled");
                serde_json::json!({
                    "id": id,
                    "label": label,
                    "type": "Document",
                })
            })
            .collect();

        self.edges = vec![];

        Ok(())
    }

    /// 从文档 Block 和 Reference 加载图谱数据
    ///
    /// 仅生成 Block 节点，Reference 边。
    ///
    /// # Errors
    ///
    /// API 请求失败时返回错误
    pub async fn load_document_graph(&mut self, doc_id: &str) -> Result<()> {
        let api = create_api_client();
        let blocks = api.get_document_blocks(doc_id).await?;

        self.nodes = blocks
            .iter()
            .map(|block| {
                let id = block.get("id").and_then(Value::as_str).unwrap_or("");
                let start_line = block.get("start_line").and_then(Value::as_u64).unwrap_or(0);
                let block_type = block
                    .get("block_type")
                    .and_then(Value::as_str)
                    .unwrap_or("?");
                let label = format!("L{start_line}-{block_type}");
                serde_json::json!({
                    "id": id,
                    "label": label,
                    "type": "Block",
                })
            })
            .collect();

        self.edges = vec![];

        Ok(())
    }
}

impl Default for KnowledgeGraphState {
    fn default() -> Self {
        Self::new()
    }
}

fn window_host() -> String {
    web_sys::window()
        .and_then(|w| w.location().host().ok())
        .unwrap_or_else(|| {
            web_sys::console::log_1(&"无法获取 window.location.host，使用默认值".into());
            std::option_env!("KNOWLEDGE_WS_HOST").unwrap_or("127.0.0.1:3000").to_string()
        })
}
