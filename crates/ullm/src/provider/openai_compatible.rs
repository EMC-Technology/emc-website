use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;

use crate::credential::{CredentialsProvider, EnvCredentialProvider};
use crate::error::LlmError;
use crate::provider::types::{
    LanguageModelRequest, ModelCapabilities, ModelConfig, ModelId, ModelName, ProviderConfig,
    ProviderId, ProviderName,
};
use crate::provider::{LanguageModel, LanguageModelProvider};
use crate::rate_limit::RateLimiter;
use crate::stream::openai_mapper::OpenAiSseMapper;
use crate::stream::sse::SseParser;
use crate::stream::{ModelStream, StreamEvent, StreamFuture};
use crate::token_count::{ByteEstimator, TokenCounter};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// 递归规范化 JSON Schema：为缺少 `properties` 的 `object` 类型节点
/// 插入空 `properties` 对象，满足 `OpenAI` API 的严格校验要求。
pub fn normalize_object_schema(schema: &mut serde_json::Value) {
    if let Some(obj) = schema.as_object_mut() {
        if obj.get("type").and_then(|t| t.as_str()) == Some("object")
            && !obj.contains_key("properties")
        {
            obj.insert(
                "properties".to_string(),
                serde_json::Value::Object(serde_json::Map::default()),
            );
        }
        for (_, v) in obj.iter_mut() {
            normalize_object_schema(v);
        }
    } else if let Some(arr) = schema.as_array_mut() {
        for v in arr.iter_mut() {
            normalize_object_schema(v);
        }
    }
}

/// 修正工具调用消息配对：当 `tool_calls` 消息后缺少足够的 `tool` 结果消息时，
/// 自动补充占位结果，避免 API 拒绝请求。
pub fn sanitize_tool_message_pairing(messages: &mut Vec<serde_json::Value>) {
    let mut i = 0;
    while i < messages.len() {
        if let Some(msg) = messages.get(i)
            && msg.get("tool_calls").is_some()
        {
            let tool_call_count = msg["tool_calls"].as_array().map_or(0, std::vec::Vec::len);
            let mut j = i + 1;
            let mut found_results = 0;
            while j < messages.len() {
                if messages[j].get("role").and_then(|r| r.as_str()) == Some("tool") {
                    found_results += 1;
                    j += 1;
                } else {
                    break;
                }
            }
            if found_results < tool_call_count {
                let missing = tool_call_count - found_results;
                for k in 0..missing {
                    let fake_result = serde_json::json!({
                        "role": "tool",
                        "content": "Tool result unavailable",
                        "tool_call_id": format!("missing_{}", k),
                    });
                    messages.insert(i + 1 + found_results + k, fake_result);
                }
            }
        }
        i += 1;
    }
}

/// `OpenAI` 兼容协议模型实例，封装通用的聊天补全请求逻辑。
#[derive(Clone)]
pub struct OpenAiCompatibleModel {
    id: ModelId,
    name: ModelName,
    provider_id: ProviderId,
    provider_name: ProviderName,
    #[allow(dead_code)]
    model_id_str: String,
    max_tokens: u64,
    max_output: Option<u64>,
    capabilities: ModelCapabilities,
    client: Client,
    api_url: String,
    credentials: Arc<dyn CredentialsProvider>,
    provider_id_str: String,
    rate_limiter: Arc<RateLimiter>,
    extra_headers: HashMap<String, String>,
    max_request_body_bytes: Option<usize>,
}

impl OpenAiCompatibleModel {
    /// 创建新的 `OpenAI` 兼容模型实例。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model_id: &str,
        display_name: &str,
        provider_id: ProviderId,
        provider_name: ProviderName,
        max_tokens: u64,
        max_output: Option<u64>,
        capabilities: ModelCapabilities,
        client: Client,
        api_url: String,
        credentials: Arc<dyn CredentialsProvider>,
        provider_id_str: String,
        rate_limiter: Arc<RateLimiter>,
        extra_headers: HashMap<String, String>,
        max_request_body_bytes: Option<usize>,
    ) -> Self {
        Self {
            id: ModelId::new(model_id),
            name: ModelName::new(display_name),
            provider_id,
            provider_name,
            model_id_str: model_id.to_string(),
            max_tokens,
            max_output,
            capabilities,
            client,
            api_url,
            credentials,
            provider_id_str,
            rate_limiter,
            extra_headers,
            max_request_body_bytes,
        }
    }

    /// 返回凭证提供者的引用。
    #[must_use]
    pub fn credentials(&self) -> &Arc<dyn CredentialsProvider> {
        &self.credentials
    }

    /// 返回供应商标识字符串。
    #[must_use]
    pub fn provider_id_str(&self) -> &str {
        &self.provider_id_str
    }

    fn build_request_body(request: &LanguageModelRequest) -> serde_json::Value {
        let mut messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|m| {
                let mut obj = serde_json::json!({
                    "role": m.role.as_str(),
                    "content": m.content,
                });
                if let Some(name) = &m.name {
                    obj["name"] = serde_json::Value::String(name.clone());
                }
                if let Some(tool_call_id) = &m.tool_call_id {
                    obj["tool_call_id"] = serde_json::Value::String(tool_call_id.clone());
                }
                obj
            })
            .collect();

        sanitize_tool_message_pairing(&mut messages);

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "stream": true,
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(stop) = &request.stop {
            body["stop"] = serde_json::json!(stop);
        }

        if let Some(tools) = &request.tools {
            let tool_defs: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    let mut params = t.input_schema.clone();
                    normalize_object_schema(&mut params);
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": params,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tool_defs);
        }

        body
    }

    fn stream_completion_inner<'a>(
        &'a self,
        request: LanguageModelRequest,
        bearer_token: &'a str,
    ) -> StreamFuture<'a> {
        Box::pin(async move {
            let _permit = self.rate_limiter.acquire_owned().await?;
            let body = Self::build_request_body(&request);

            let body_bytes = serde_json::to_vec(&body)
                .map_err(|e| LlmError::Other(format!("Serialize request body failed: {e}")))?;
            if let Some(max_bytes) = self.max_request_body_bytes
                && body_bytes.len() > max_bytes
            {
                return Err(LlmError::RequestBodySizeExceeded {
                    estimated_bytes: body_bytes.len(),
                    max_bytes,
                });
            }

            let url = format!("{}/chat/completions", self.api_url);
            let mut request_builder = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {bearer_token}"))
                .header("Content-Type", "application/json");
            for (key, value) in &self.extra_headers {
                request_builder = request_builder.header(key.as_str(), value.as_str());
            }
            let response = request_builder
                .body(body_bytes)
                .send()
                .await
                .map_err(LlmError::from)?;

            let status = response.status();
            if !status.is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("<failed to read error body: {e}>"));
                return Err(LlmError::from_http_status(status, &body_text));
            }

            let byte_stream = response.bytes_stream();
            let mapper = OpenAiSseMapper;
            let parser = SseParser::new();

            let stream: ModelStream = Box::pin(
                byte_stream
                    .scan(parser, move |parser, chunk_result| {
                        let events = match chunk_result {
                            Ok(bytes) => parser.push(&bytes, &mapper),
                            Err(e) => Err(LlmError::StreamError(e.to_string())),
                        };
                        let events =
                            events.unwrap_or_else(|e| vec![StreamEvent::Error(e.to_string())]);
                        std::future::ready(Some(events))
                    })
                    .flat_map(|events| futures_util::stream::iter(events.into_iter().map(Ok))),
            );

            Ok(stream)
        })
    }

    /// 使用指定的 Bearer Token 发起流式补全请求。
    #[must_use]
    pub fn stream_completion_with_token<'a>(
        &'a self,
        request: LanguageModelRequest,
        bearer_token: &'a str,
    ) -> StreamFuture<'a> {
        self.stream_completion_inner(request, bearer_token)
    }
}

#[async_trait]
impl LanguageModel for OpenAiCompatibleModel {
    fn id(&self) -> &ModelId {
        &self.id
    }
    fn name(&self) -> &ModelName {
        &self.name
    }
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
    fn provider_name(&self) -> &ProviderName {
        &self.provider_name
    }
    fn supports_tools(&self) -> bool {
        self.capabilities.tools
    }
    fn supports_streaming_tools(&self) -> bool {
        self.capabilities.streaming_tools
    }
    fn supports_images(&self) -> bool {
        self.capabilities.images
    }
    fn supports_thinking(&self) -> bool {
        self.capabilities.thinking
    }
    fn max_token_count(&self) -> u64 {
        self.max_tokens
    }
    fn max_output_tokens(&self) -> Option<u64> {
        self.max_output
    }

    fn count_tokens(
        &self,
        request: &LanguageModelRequest,
    ) -> crate::token_count::TokenCountFuture<'_> {
        let estimator = ByteEstimator;
        let estimate = estimator.count_tokens(
            &request
                .messages
                .iter()
                .map(|m| m.content.to_text_lossy())
                .collect::<Vec<_>>()
                .join(" "),
            request.model.as_ref(),
        );
        Box::pin(async move { Ok(estimate) })
    }

    fn stream_completion(&self, request: LanguageModelRequest) -> StreamFuture<'_> {
        let credentials = self.credentials.clone();
        let provider_id = self.provider_id_str.clone();
        Box::pin(async move {
            let api_key = credentials.get_api_key(&provider_id).await?;
            self.stream_completion_inner(request, &api_key).await
        })
    }
}

/// `OpenAI` 兼容协议供应商，管理模型列表与认证。
pub struct OpenAiCompatibleProvider {
    client: Client,
    credentials: Arc<dyn CredentialsProvider>,
    rate_limiter: Arc<RateLimiter>,
    config: ProviderConfig,
    model_specs: Vec<(String, String, u64, Option<u64>, ModelCapabilities)>,
}

impl OpenAiCompatibleProvider {
    /// 创建新的 `OpenAI` 兼容 Provider 实例。
    ///
    /// # Panics
    ///
    /// 当 HTTP 客户端构建失败时 panic。
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        api_url: impl Into<String>,
        api_key_env: Option<String>,
        models: Vec<(String, String, u64, Option<u64>, ModelCapabilities)>,
    ) -> Self {
        let id_str = id.into();
        let name_str = name.into();
        let api_url_str = api_url.into();

        let client =
            crate::build_http_client(DEFAULT_TIMEOUT).expect("HTTP client init should not fail");

        let env_key = api_key_env
            .unwrap_or_else(|| format!("{}_API_KEY", id_str.to_uppercase().replace('-', "_")));

        let credentials: Arc<dyn CredentialsProvider> = Arc::new(EnvCredentialProvider::new());
        let rate_limiter = Arc::new(RateLimiter::new(5));

        let model_configs: Vec<ModelConfig> = models
            .iter()
            .map(|(mid, dname, max_t, max_o, caps)| ModelConfig {
                id: ModelId::from(mid.clone()),
                display_name: dname.clone(),
                max_tokens: *max_t,
                max_output_tokens: *max_o,
                capabilities: caps.clone(),
            })
            .collect();

        let config = ProviderConfig {
            id: ProviderId::new(&id_str),
            name: ProviderName::new(&name_str),
            api_url: api_url_str.clone(),
            api_key_env: Some(env_key),
            models: model_configs,
            rate_limit: None,
            retry: None,
            extra_headers: HashMap::new(),
            max_request_body_bytes: None,
        };

        Self {
            client,
            credentials,
            rate_limiter,
            config,
            model_specs: models,
        }
    }

    /// 设置自定义凭证提供者。
    #[must_use]
    pub fn with_credentials(mut self, credentials: Arc<dyn CredentialsProvider>) -> Self {
        self.credentials = credentials;
        self
    }

    /// 返回所有模型的原始实例列表。
    #[must_use]
    pub fn provided_models_raw(&self) -> Vec<Arc<OpenAiCompatibleModel>> {
        self.model_specs
            .iter()
            .map(|(mid, dname, max_t, max_o, caps)| {
                Arc::new(OpenAiCompatibleModel::new(
                    mid,
                    dname,
                    self.config.id.clone(),
                    self.config.name.clone(),
                    *max_t,
                    *max_o,
                    caps.clone(),
                    self.client.clone(),
                    self.config.api_url.clone(),
                    self.credentials.clone(),
                    self.config.id.to_string(),
                    self.rate_limiter.clone(),
                    self.config.extra_headers.clone(),
                    self.config.max_request_body_bytes,
                ))
            })
            .collect()
    }

    /// 设置额外的 HTTP 请求头。
    #[must_use]
    pub fn with_extra_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.config.extra_headers = headers;
        self
    }

    /// 设置请求体最大字节数限制。
    #[must_use]
    pub fn with_max_request_body_bytes(mut self, max_bytes: usize) -> Self {
        self.config.max_request_body_bytes = Some(max_bytes);
        self
    }
}

#[async_trait]
impl LanguageModelProvider for OpenAiCompatibleProvider {
    fn id(&self) -> &ProviderId {
        &self.config.id
    }
    fn name(&self) -> &ProviderName {
        &self.config.name
    }

    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
        self.model_specs
            .iter()
            .map(|(mid, dname, max_t, max_o, caps)| {
                Arc::new(OpenAiCompatibleModel::new(
                    mid,
                    dname,
                    self.config.id.clone(),
                    self.config.name.clone(),
                    *max_t,
                    *max_o,
                    caps.clone(),
                    self.client.clone(),
                    self.config.api_url.clone(),
                    self.credentials.clone(),
                    self.config.id.to_string(),
                    self.rate_limiter.clone(),
                    self.config.extra_headers.clone(),
                    self.config.max_request_body_bytes,
                )) as Arc<dyn LanguageModel>
            })
            .collect()
    }

    fn is_authenticated(&self) -> bool {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(self.credentials.get_api_key(self.config.id.as_ref()))
        })
        .is_ok()
    }

    async fn authenticate(&self) -> Result<(), LlmError> {
        self.credentials
            .get_api_key(self.config.id.as_ref())
            .await?;
        Ok(())
    }

    async fn reset_credentials(&self) -> Result<(), LlmError> {
        self.credentials.invalidate(self.config.id.as_ref()).await;
        Ok(())
    }

    fn configuration(&self) -> &ProviderConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credential::ConfigCredentialProvider;
    use crate::provider::LanguageModelProvider;
    use crate::provider::types::{Message, Role};
    use crate::tool::ToolDefinition;

    fn make_test_model() -> OpenAiCompatibleModel {
        let client = Client::builder().timeout(DEFAULT_TIMEOUT).build().unwrap();
        let creds: Arc<dyn CredentialsProvider> = Arc::new(ConfigCredentialProvider::new(
            vec![("test-provider".to_string(), "test-key".to_string())]
                .into_iter()
                .collect(),
        ));
        OpenAiCompatibleModel::new(
            "test-model",
            "Test Model",
            ProviderId::new("test-provider"),
            ProviderName::new("Test Provider"),
            4096,
            Some(1024),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 4096,
            },
            client,
            "https://api.test.com/v1".to_string(),
            creds,
            "test-provider".to_string(),
            Arc::new(RateLimiter::new(5)),
            HashMap::new(),
            None,
        )
    }

    #[test]
    fn test_normalize_object_schema_adds_properties() {
        let mut schema = serde_json::json!({"type": "object"});
        normalize_object_schema(&mut schema);
        assert!(schema.get("properties").is_some());
        assert!(schema["properties"].as_object().unwrap().is_empty());
    }

    #[test]
    fn test_normalize_object_schema_already_has_properties() {
        let mut schema = serde_json::json!({"type": "object", "properties": {"a": 1}});
        normalize_object_schema(&mut schema);
        assert_eq!(schema["properties"]["a"], 1);
    }

    #[test]
    fn test_normalize_object_schema_non_object_type() {
        let mut schema = serde_json::json!({"type": "string"});
        normalize_object_schema(&mut schema);
        assert!(!schema.as_object().unwrap().contains_key("properties"));
    }

    #[test]
    fn test_normalize_object_schema_nested() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "nested": {"type": "object"}
            }
        });
        normalize_object_schema(&mut schema);
        assert!(
            schema["properties"]["nested"]
                .as_object()
                .unwrap()
                .contains_key("properties")
        );
    }

    #[test]
    fn test_normalize_object_schema_array() {
        let mut schema = serde_json::json!([{"type": "object"}]);
        normalize_object_schema(&mut schema);
        assert!(schema[0].as_object().unwrap().contains_key("properties"));
    }

    #[test]
    fn test_sanitize_tool_message_pairing_no_missing() {
        let mut messages = vec![
            serde_json::json!({"role": "assistant", "tool_calls": [{"id": "c1"}]}),
            serde_json::json!({"role": "tool", "tool_call_id": "c1", "content": "ok"}),
        ];
        sanitize_tool_message_pairing(&mut messages);
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_sanitize_tool_message_pairing_missing_results() {
        let mut messages = vec![
            serde_json::json!({"role": "assistant", "tool_calls": [{"id": "c1"}, {"id": "c2"}]}),
            serde_json::json!({"role": "tool", "tool_call_id": "c1", "content": "ok"}),
        ];
        sanitize_tool_message_pairing(&mut messages);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "missing_0");
    }

    #[test]
    fn test_sanitize_tool_message_pairing_no_tool_calls() {
        let mut messages = vec![
            serde_json::json!({"role": "user", "content": "hello"}),
            serde_json::json!({"role": "assistant", "content": "hi"}),
        ];
        sanitize_tool_message_pairing(&mut messages);
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_sanitize_tool_message_pairing_all_missing() {
        let mut messages =
            vec![serde_json::json!({"role": "assistant", "tool_calls": [{"id": "c1"}]})];
        sanitize_tool_message_pairing(&mut messages);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1]["role"], "tool");
    }

    #[test]
    fn test_build_request_body_basic() {
        let _model = make_test_model();
        let request = LanguageModelRequest::new("test-model", vec![Message::user("hello")]);
        let body = OpenAiCompatibleModel::build_request_body(&request);
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["stream"], true);
        assert!(body["messages"].is_array());
    }

    #[test]
    fn test_build_request_body_with_temperature() {
        let _model = make_test_model();
        let request = LanguageModelRequest::new("test-model", vec![Message::user("hi")])
            .with_temperature(0.7);
        let body = OpenAiCompatibleModel::build_request_body(&request);
        let temp = body["temperature"].as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_build_request_body_with_max_tokens() {
        let _model = make_test_model();
        let request =
            LanguageModelRequest::new("test-model", vec![Message::user("hi")]).with_max_tokens(512);
        let body = OpenAiCompatibleModel::build_request_body(&request);
        assert_eq!(body["max_tokens"], 512);
    }

    #[test]
    fn test_build_request_body_with_top_p() {
        let _model = make_test_model();
        let request =
            LanguageModelRequest::new("test-model", vec![Message::user("hi")]).with_top_p(0.9);
        let body = OpenAiCompatibleModel::build_request_body(&request);
        let top_p = body["top_p"].as_f64().unwrap();
        assert!((top_p - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_build_request_body_with_stop() {
        let _model = make_test_model();
        let request = LanguageModelRequest::new("test-model", vec![Message::user("hi")])
            .with_stop(vec!["END".to_string()]);
        let body = OpenAiCompatibleModel::build_request_body(&request);
        assert_eq!(body["stop"][0], "END");
    }

    #[test]
    fn test_build_request_body_with_tools() {
        let _model = make_test_model();
        let tool = ToolDefinition::new(
            "my_tool",
            "A test tool",
            serde_json::json!({"type": "object", "properties": {"city": {"type": "string"}}}),
        )
        .unwrap();
        let request = LanguageModelRequest::new("test-model", vec![Message::user("hi")])
            .with_tools(vec![tool]);
        let body = OpenAiCompatibleModel::build_request_body(&request);
        assert!(body["tools"].is_array());
        assert_eq!(body["tools"][0]["function"]["name"], "my_tool");
    }

    #[test]
    fn test_build_request_body_with_name_field() {
        let _model = make_test_model();
        let msg = Message::user("hello").with_name("Alice");
        let request = LanguageModelRequest::new("test-model", vec![msg]);
        let body = OpenAiCompatibleModel::build_request_body(&request);
        assert_eq!(body["messages"][0]["name"], "Alice");
    }

    #[test]
    fn test_build_request_body_with_tool_call_id() {
        let _model = make_test_model();
        let msg = Message {
            role: Role::Tool,
            content: crate::provider::content_block::MessageContent::text("result"),
            name: None,
            tool_call_id: Some("call_123".into()),
        };
        let request = LanguageModelRequest::new("test-model", vec![msg]);
        let body = OpenAiCompatibleModel::build_request_body(&request);
        assert_eq!(body["messages"][0]["tool_call_id"], "call_123");
    }

    #[test]
    fn test_model_accessors() {
        let model = make_test_model();
        assert_eq!(model.id().as_str(), "test-model");
        assert_eq!(model.name().as_str(), "Test Model");
        assert_eq!(model.provider_id().as_ref(), "test-provider");
        assert_eq!(model.provider_name().as_ref(), "Test Provider");
        assert!(model.supports_tools());
        assert!(model.supports_streaming_tools());
        assert!(!model.supports_images());
        assert!(!model.supports_thinking());
        assert_eq!(model.max_token_count(), 4096);
        assert_eq!(model.max_output_tokens(), Some(1024));
        assert_eq!(model.provider_id_str(), "test-provider");
        let _creds = model.credentials();
    }

    #[tokio::test]
    async fn test_count_tokens_estimate() {
        let model = make_test_model();
        let request = LanguageModelRequest::new("test-model", vec![Message::user("hello world")]);
        let count = model.count_tokens(&request).await.unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_provider_new() {
        let provider = OpenAiCompatibleProvider::new(
            "custom",
            "Custom Provider",
            "https://api.custom.com/v1",
            Some("CUSTOM_API_KEY".to_string()),
            vec![(
                "model-1".into(),
                "Model 1".into(),
                8192,
                Some(4096),
                ModelCapabilities::default(),
            )],
        );
        assert_eq!(provider.id().as_ref(), "custom");
        assert_eq!(provider.name().as_ref(), "Custom Provider");
    }

    #[test]
    fn test_provider_provided_models() {
        let provider = OpenAiCompatibleProvider::new(
            "test",
            "Test",
            "https://api.test.com/v1",
            None,
            vec![(
                "m1".into(),
                "M1".into(),
                4096,
                None,
                ModelCapabilities::default(),
            )],
        );
        let models = provider.provided_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id().as_str(), "m1");
    }

    #[test]
    fn test_provider_provided_models_raw() {
        let provider = OpenAiCompatibleProvider::new(
            "test",
            "Test",
            "https://api.test.com/v1",
            None,
            vec![(
                "m1".into(),
                "M1".into(),
                4096,
                None,
                ModelCapabilities::default(),
            )],
        );
        let models = provider.provided_models_raw();
        assert_eq!(models.len(), 1);
    }

    #[test]
    fn test_provider_with_extra_headers() {
        let provider =
            OpenAiCompatibleProvider::new("test", "Test", "https://api.test.com/v1", None, vec![])
                .with_extra_headers(HashMap::from([(
                    "X-Custom".to_string(),
                    "value".to_string(),
                )]));
        assert_eq!(
            provider.configuration().extra_headers.get("X-Custom"),
            Some(&"value".to_string())
        );
    }

    #[test]
    fn test_provider_with_max_request_body_bytes() {
        let provider =
            OpenAiCompatibleProvider::new("test", "Test", "https://api.test.com/v1", None, vec![])
                .with_max_request_body_bytes(1024);
        assert_eq!(provider.configuration().max_request_body_bytes, Some(1024));
    }

    #[test]
    fn test_provider_with_credentials() {
        let creds: Arc<dyn CredentialsProvider> = Arc::new(ConfigCredentialProvider::new(
            vec![("test".to_string(), "key".to_string())]
                .into_iter()
                .collect(),
        ));
        let provider = OpenAiCompatibleProvider::new(
            "test",
            "Test",
            "https://api.test.com/v1",
            None,
            vec![(
                "model-1".to_string(),
                "Model 1".to_string(),
                4096,
                None,
                ModelCapabilities::default(),
            )],
        )
        .with_credentials(creds);
        assert!(!provider.provided_models().is_empty());
    }

    #[test]
    fn test_provider_configuration() {
        let provider = OpenAiCompatibleProvider::new(
            "test",
            "Test",
            "https://api.test.com/v1",
            Some("TEST_API_KEY".to_string()),
            vec![],
        );
        let config = provider.configuration();
        assert_eq!(config.id.as_ref(), "test");
        assert_eq!(config.api_url, "https://api.test.com/v1");
    }

    #[test]
    fn test_provider_default_api_key_env() {
        let provider = OpenAiCompatibleProvider::new(
            "my-provider",
            "My Provider",
            "https://api.test.com/v1",
            None,
            vec![],
        );
        let config = provider.configuration();
        assert_eq!(config.api_key_env, Some("MY_PROVIDER_API_KEY".to_string()));
    }

    #[tokio::test]
    async fn test_provider_reset_credentials() {
        let provider =
            OpenAiCompatibleProvider::new("test", "Test", "https://api.test.com/v1", None, vec![]);
        let result = provider.reset_credentials().await;
        assert!(result.is_ok());
    }
}
