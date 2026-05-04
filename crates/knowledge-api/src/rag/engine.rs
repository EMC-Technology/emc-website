//! RAG 核心引擎
//!
//! 整合检索、重排序和 LLM 生成为端到端的知识问答系统。

use error_core::Result;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

/// LLM 客户端 trait（抽象大语言模型接口）
///
/// 支持多种后端：
/// - `` `OpenAI` `` `` `GPT-4` `` / `` `GPT-3.5` ``
/// - `` `Anthropic` `` `` `Claude` ``
/// - 本地 `` `Ollama` `` / `` `vLLM` ``
/// - 自定义 `` `API` `` 兼容服务
#[async_trait::async_trait]
pub trait LLMClient: Send + Sync {
    /// 同步生成完整回复
    ///
    /// # 参数
    ///
    /// * `messages` - 对话消息列表
    /// * `options` - 生成选项（`temperature`、`max_tokens` 等）
    async fn generate(
        &self,
        messages: &[LLMMessage],
        options: &GenerateOptions,
    ) -> Result<LLMResponse>;

    /// 流式生成（逐 token 返回）
    ///
    /// 返回 Token 流，适用于实时展示。
    async fn generate_stream(
        &self,
        messages: &[LLMMessage],
        options: &GenerateOptions,
    ) -> Result<Box<dyn futures::Stream<Item = Result<StreamChunk>> + Send + Unpin>>;
}

/// LLM 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMMessage {
    /// 角色：system/user/assistant
    pub role: MessageRole,
    /// 消息内容
    pub content: String,
}

/// 消息角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    /// 系统指令
    System,
    /// 用户输入
    User,
    /// 模型回复
    Assistant,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
        }
    }
}

/// 生成选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateOptions {
    /// 温度参数 [0, 2]，越高越随机（默认: 0.7）
    pub temperature: f64,
    /// 最大生成长度（token 数）
    pub max_tokens: usize,
    /// Top-P 采样参数（默认: 1.0）
    pub top_p: f64,
    /// 频率惩罚（默认: 0.0）
    pub frequency_penalty: f64,
    /// 存在惩罚（默认: 0.0）
    pub presence_penalty: f64,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 2048,
            top_p: 1.0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
        }
    }
}

/// LLM 完整响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    /// 生成的文本内容
    pub content: String,
    /// 使用的 token 数（输入 + 输出）
    pub usage: TokenUsage,
    /// 模型名称
    pub model: String,
    /// 结束原因（`` `stop` ``/`` `length` ``/`` `content_filter` ``）
    pub finish_reason: String,
}

/// Token 使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// 输入 prompt token 数
    pub prompt_tokens: usize,
    /// 输出 completion token 数
    pub completion_tokens: usize,
    /// 总计
    pub total_tokens: usize,
}

/// 流式 Token 块
#[derive(Debug, Clone)]
pub struct StreamChunk {
    /// Token 内容
    pub content: String,
    /// 是否为最后一个块
    pub is_final: bool,
    /// Token 使用统计（仅在最终块 `is_final == true` 时填充）
    pub usage: Option<TokenUsage>,
    /// 模型名称（仅在最终块 `is_final == true` 时填充）
    pub model: Option<String>,
}

/// 检索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    /// 检索结果唯一标识
    pub id: Uuid,
    /// 检索到的文档内容片段
    pub content: String,
    /// 相似度得分（0.0 ~ 1.0）
    pub score: f64,
    /// 附加元数据
    pub metadata: serde_json::Value,
}

/// 检索器 trait（抽象文档检索接口）
#[async_trait::async_trait]
pub trait Retriever: Send + Sync {
    /// 根据查询文本检索相关文档
    ///
    /// # Arguments
    ///
    /// * `query` - 用户查询字符串
    /// * `top_k` - 返回的最相关文档数量
    ///
    /// # Errors
    ///
    /// 当检索过程失败时返回错误（如数据库连接问题、嵌入计算错误等）。
    async fn retrieve(&self, query: &str, top_k: usize)
    -> error_core::Result<Vec<RetrievalResult>>;
}

/// RAG 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGRequest {
    /// 用户查询
    pub query: String,
    /// 会话 ID（用于多轮对话上下文）
    pub session_id: Option<String>,
    /// 历史消息（多轮对话）
    pub history: Vec<LLMMessage>,
    /// 是否启用流式返回（默认: false）
    pub stream: bool,
    /// 覆盖默认配置的选项
    pub options: Option<RAGRequestOptions>,
}

/// RAG 请求选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGRequestOptions {
    /// 检索 Top-K（默认: 10）
    pub top_k: Option<usize>,
    /// 相似度阈值（默认: 0.7）
    pub score_threshold: Option<f64>,
    /// 最大上下文长度（字符数，默认: 8000）
    pub max_context_length: Option<usize>,
    /// LLM 温度覆盖
    pub temperature: Option<f64>,
}

/// RAG 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGResponse {
    /// 请求唯一标识符
    pub request_id: Uuid,
    /// 生成的回答文本
    pub answer: String,
    /// 检索到的参考来源
    pub sources: Vec<SourceReference>,
    /// 处理耗时（毫秒）
    pub duration_ms: u128,
    /// RAG 统计信息
    pub stats: RAGStats,
}

/// 参考来源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceReference {
    /// 来源文档/块 ID
    pub id: Uuid,
    /// 相关性分数
    pub relevance_score: f64,
    /// 内容预览
    pub content_preview: String,
    /// 元数据
    pub metadata: serde_json::Value,
}

/// RAG 统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGStats {
    /// 检索阶段候选数
    pub retrieval_candidates: usize,
    /// 重排序后保留数
    pub reranked_count: usize,
    /// 最终使用的上下文数
    pub context_used: usize,
    /// 上下文总长度（字符数）
    pub context_length: usize,
    /// LLM 输入 token 数
    pub llm_input_tokens: usize,
    /// LLM 输出 token 数
    pub llm_output_tokens: usize,
}

/// RAG 引擎配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGConfig {
    /// 默认检索 Top-K
    pub default_top_k: usize,
    /// 默认相似度阈值
    pub default_score_threshold: f64,
    /// 最大上下文长度（字符数）
    pub max_context_length: usize,
    /// 系统提示词模板
    pub system_prompt_template: String,
    /// 上下文格式化模板
    pub context_template: String,
    /// 是否启用查询扩展（使用 LLM 改写查询）
    pub enable_query_expansion: bool,
    /// 缓存 TTL（秒）
    pub cache_ttl_secs: u64,
}

impl Default for RAGConfig {
    fn default() -> Self {
        Self {
            default_top_k: 10,
            default_score_threshold: 0.7,
            max_context_length: 8000,
            system_prompt_template: include_str!("system_prompt.txt").to_string(),
            context_template: "【来源 {index}】{content}\n".to_string(),
            enable_query_expansion: false,
            cache_ttl_secs: 300,
        }
    }
}

/// RAG 引擎（核心结构体）
///
/// 整合所有组件提供端到端的 RAG 服务。
///
/// # 示例
///
/// ```ignore
/// use knowledge_api::rag::{RAGEngine, RAGConfig, MockLLMClient, MockRetriever};
///
/// let config = RAGConfig::default();
/// let llm = Arc::new(MockLLMClient::new());
/// let retriever = Arc::new(MockRetriever::new());
/// let engine = RAGEngine::new(config, llm, retriever);
/// ```
pub struct RAGEngine {
    config: RAGConfig,
    llm_client: Arc<dyn LLMClient>,
    retriever: Arc<dyn Retriever>,
}

impl RAGEngine {
    /// 创建新的 RAG 引擎实例
    ///
    /// # Arguments
    ///
    /// * `config` - RAG 配置参数（如温度、top-k 等）
    /// * `llm_client` - LLM 客户端，用于生成回答
    /// * `retriever` - 文档检索器，用于获取相关上下文
    pub fn new(
        config: RAGConfig,
        llm_client: Arc<dyn LLMClient>,
        retriever: Arc<dyn Retriever>,
    ) -> Self {
        Self {
            config,
            llm_client,
            retriever,
        }
    }

    /// 执行同步 RAG 查询
    ///
    /// 完整流程：
    /// 1. 查询预处理（可选扩展）
    /// 2. 上下文检索（Hybrid Search + Rerank）
    /// 3. Prompt 构建（注入检索到的上下文）
    /// 4. LLM 生成
    /// 5. 后处理（引用标注、格式化）
    ///
    /// # 参数
    ///
    /// * `request` - RAG 请求
    ///
    /// # 返回
    ///
    /// 包含回答、来源引用和统计信息的完整响应
    #[instrument(skip(self, request))]
    /// # Errors
    ///
    /// 当 LLM 调用失败或后端不可用时返回错误。
    pub async fn query(&self, request: &RAGRequest) -> Result<RAGResponse> {
        let start_time = std::time::Instant::now();
        let request_id = Uuid::new_v4();

        info!(
            request_id = %request_id,
            query_preview = &request.query[..request.query.len().min(50)],
            is_stream = request.stream,
            "执行 RAG 查询"
        );

        let top_k = request
            .options
            .as_ref()
            .and_then(|o| o.top_k)
            .unwrap_or(self.config.default_top_k);

        let score_threshold = request
            .options
            .as_ref()
            .and_then(|o| o.score_threshold)
            .unwrap_or(self.config.default_score_threshold);

        let max_context_length = request
            .options
            .as_ref()
            .and_then(|o| o.max_context_length)
            .unwrap_or(self.config.max_context_length);

        let candidates = self.retriever.retrieve(&request.query, top_k).await?;
        let retrieval_candidates = candidates.len();

        let filtered: Vec<&RetrievalResult> = candidates
            .iter()
            .filter(|r| r.score >= score_threshold)
            .collect();

        let (context_text, context_used) = self.build_context(&filtered, max_context_length);
        let context_length = context_text.len();

        let system_prompt = self.build_system_prompt();

        let mut messages = vec![LLMMessage {
            role: MessageRole::System,
            content: system_prompt,
        }];

        if !request.history.is_empty() {
            messages.extend(request.history.clone());
        }

        let user_content = if context_text.is_empty() {
            request.query.clone()
        } else {
            format!("{context_text}\n\n问题：{}", request.query)
        };

        messages.push(LLMMessage {
            role: MessageRole::User,
            content: user_content,
        });

        let options = Self::resolve_options(request);
        let response = self.llm_client.generate(&messages, &options).await?;

        let sources: Vec<SourceReference> = filtered
            .iter()
            .map(|r| SourceReference {
                id: r.id,
                relevance_score: r.score,
                content_preview: r.content.chars().take(200).collect(),
                metadata: r.metadata.clone(),
            })
            .collect();

        let duration_ms = start_time.elapsed().as_millis();

        info!(
            request_id = %request_id,
            duration_ms = duration_ms,
            output_len = response.content.len(),
            sources_count = sources.len(),
            "RAG 查询完成"
        );

        Ok(RAGResponse {
            request_id,
            answer: response.content,
            sources,
            duration_ms,
            stats: RAGStats {
                retrieval_candidates,
                reranked_count: retrieval_candidates,
                context_used,
                context_length,
                llm_input_tokens: response.usage.prompt_tokens,
                llm_output_tokens: response.usage.completion_tokens,
            },
        })
    }

    /// 执行流式 RAG 查询（SSE）
    ///
    /// 返回异步 Token 流，支持 Server-Sent Events 协议。
    ///
    /// # 参数
    ///
    /// * `request` - RAG 请求（stream 字段应设为 true）
    ///
    /// # 返回
    ///
    /// 异步 `Stream`，每个元素为 `RAGStreamChunk`
    ///
    /// # Errors
    ///
    /// 当流式调用失败或连接中断时返回错误。
    #[instrument(skip(self, request))]
    pub async fn query_stream(
        &self,
        request: &RAGRequest,
    ) -> Result<impl Stream<Item = Result<super::RAGStreamChunk>>> {
        let request_id = Uuid::new_v4();

        info!(
            request_id = %request_id,
            query_preview = &request.query[..request.query.len().min(50)],
            "开始流式 RAG 查询"
        );

        let top_k = request
            .options
            .as_ref()
            .and_then(|o| o.top_k)
            .unwrap_or(self.config.default_top_k);

        let score_threshold = request
            .options
            .as_ref()
            .and_then(|o| o.score_threshold)
            .unwrap_or(self.config.default_score_threshold);

        let max_context_length = request
            .options
            .as_ref()
            .and_then(|o| o.max_context_length)
            .unwrap_or(self.config.max_context_length);

        let candidates = self.retriever.retrieve(&request.query, top_k).await?;
        let filtered: Vec<&RetrievalResult> = candidates
            .iter()
            .filter(|r| r.score >= score_threshold)
            .collect();

        let (context_text, _) = self.build_context(&filtered, max_context_length);

        let system_prompt = self.build_system_prompt();

        let mut messages = vec![LLMMessage {
            role: MessageRole::System,
            content: system_prompt,
        }];

        if !request.history.is_empty() {
            messages.extend(request.history.clone());
        }

        let user_content = if context_text.is_empty() {
            request.query.clone()
        } else {
            format!("{context_text}\n\n问题：{}", request.query)
        };

        messages.push(LLMMessage {
            role: MessageRole::User,
            content: user_content,
        });

        let options = Self::resolve_options(request);
        let stream = self.llm_client.generate_stream(&messages, &options).await?;

        let wrapped_stream = crate::rag::stream::wrap_llm_stream(stream, request_id);

        Ok(wrapped_stream)
    }

    // ====================================================================
    // 内部方法
    // ====================================================================

    /// 构建系统提示词
    fn build_system_prompt(&self) -> String {
        self.config.system_prompt_template.clone()
    }

    /// 合并请求选项与默认配置
    fn resolve_options(request: &RAGRequest) -> GenerateOptions {
        let mut opts = GenerateOptions::default();

        if let Some(ref req_opts) = request.options {
            if let Some(temp) = req_opts.temperature {
                opts.temperature = temp;
            }
        }

        opts
    }

    fn build_context(&self, results: &[&RetrievalResult], max_length: usize) -> (String, usize) {
        let mut context = String::new();
        let mut used = 0usize;

        for (index, result) in results.iter().enumerate() {
            let entry = self
                .config
                .context_template
                .replace("{index}", &(index + 1).to_string())
                .replace("{content}", &result.content);

            if context.len() + entry.len() > max_length {
                break;
            }

            context.push_str(&entry);
            used += 1;
        }

        (context, used)
    }
}

// ============================================================================
// Mock 实现（用于测试和开发）
// ============================================================================

/// Mock LLM 客户端（返回固定响应）
pub struct MockLLMClient {
    model_name: String,
}

impl MockLLMClient {
    /// 创建使用默认模型名称的 Mock LLM 客户端实例
    #[must_use]
    pub fn new() -> Self {
        Self {
            model_name: "mock-llm".to_string(),
        }
    }
    /// 创建使用指定模型名称的 Mock LLM 客户端实例
    ///
    /// # Arguments
    ///
    /// * `name` - 模型名称，将用于响应中的 `model` 字段
    #[must_use]
    pub fn with_model(name: impl Into<String>) -> Self {
        Self {
            model_name: name.into(),
        }
    }
}

impl Default for MockLLMClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LLMClient for MockLLMClient {
    async fn generate(
        &self,
        messages: &[LLMMessage],
        _options: &GenerateOptions,
    ) -> Result<LLMResponse> {
        let user_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::User)
            .map_or("", |m| m.content.as_str());

        let answer = format!(
            "这是对「{user_msg}」的模拟回答。在实际部署中，这里会调用真实的大语言模型API。"
        );

        Ok(LLMResponse {
            content: answer,
            usage: TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            },
            model: self.model_name.clone(),
            finish_reason: "stop".to_string(),
        })
    }

    async fn generate_stream(
        &self,
        messages: &[LLMMessage],
        _options: &GenerateOptions,
    ) -> Result<Box<dyn futures::Stream<Item = Result<StreamChunk>> + Send + Unpin>> {
        let user_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        let full_response = format!("这是对「{user_msg}」的模拟流式回答。");
        let final_chunk = Ok(StreamChunk {
            content: String::new(),
            is_final: true,
            usage: Some(TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            }),
            model: Some(self.model_name.clone()),
        });

        let all_chunks: Vec<Result<StreamChunk>> = full_response
            .chars()
            .map(|c| {
                Ok(StreamChunk {
                    content: c.to_string(),
                    is_final: false,
                    usage: None,
                    model: None,
                })
            })
            .chain(std::iter::once(final_chunk))
            .collect();
        Ok(Box::new(futures::stream::iter(all_chunks)))
    }
}

/// Mock 检索器（返回固定检索结果）
pub struct MockRetriever {
    results: Vec<RetrievalResult>,
}

impl MockRetriever {
    /// 创建包含预设默认检索结果的 Mock 检索器实例
    ///
    /// 默认结果包含三条关于 Rust 所有权系统的示例文档，适用于单元测试。
    #[must_use]
    pub fn new() -> Self {
        Self {
            results: vec![
                RetrievalResult {
                    id: Uuid::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
                    content: "Rust 的所有权系统确保内存安全，无需垃圾回收。".to_string(),
                    score: 0.95,
                    metadata: serde_json::json!({"source": "rust-book"}),
                },
                RetrievalResult {
                    id: Uuid::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]),
                    content: "借用规则要求同一时间只能有一个可变引用或多个不可变引用。".to_string(),
                    score: 0.88,
                    metadata: serde_json::json!({"source": "rust-book"}),
                },
                RetrievalResult {
                    id: Uuid::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3]),
                    content: "生命周期标注帮助编译器验证引用的有效性。".to_string(),
                    score: 0.60,
                    metadata: serde_json::json!({"source": "rust-reference"}),
                },
            ],
        }
    }

    /// 创建包含自定义检索结果的 Mock 检索器实例
    ///
    /// # Arguments
    ///
    /// * `results` - 自定义的检索结果列表
    #[must_use]
    pub fn with_results(results: Vec<RetrievalResult>) -> Self {
        Self { results }
    }
}

impl Default for MockRetriever {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Retriever for MockRetriever {
    async fn retrieve(
        &self,
        _query: &str,
        top_k: usize,
    ) -> error_core::Result<Vec<RetrievalResult>> {
        Ok(self.results.iter().take(top_k).cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rag_request_serialization() {
        let request = RAGRequest {
            query: "What is Rust?".to_string(),
            session_id: Some("session-123".to_string()),
            history: vec![],
            stream: false,
            options: None,
        };

        let serialized = serde_json::to_string(&request).expect("序列化失败");
        let deserialized: RAGRequest = serde_json::from_str(&serialized).expect("反序列化失败");

        assert_eq!(deserialized.query, "What is Rust?");
        assert_eq!(deserialized.session_id, Some("session-123".to_string()));
    }

    #[test]
    fn test_rag_config_default() {
        let config = RAGConfig::default();
        assert_eq!(config.default_top_k, 10);
        assert!((config.default_score_threshold - 0.7).abs() < f64::EPSILON);
        assert_eq!(config.max_context_length, 8000);
        assert!(!config.enable_query_expansion);
    }

    #[test]
    fn test_generate_options_default() {
        let opts = GenerateOptions::default();
        assert!((opts.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(opts.max_tokens, 2048);
    }

    #[test]
    fn test_message_role_display() {
        assert_eq!(MessageRole::System.to_string(), "system");
        assert_eq!(MessageRole::User.to_string(), "user");
        assert_eq!(MessageRole::Assistant.to_string(), "assistant");
    }

    #[tokio::test]
    async fn test_mock_llm_client_sync_generation() {
        let client = MockLLMClient::with_model("test-model");

        let messages = vec![LLMMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
        }];

        let response = client
            .generate(&messages, &GenerateOptions::default())
            .await
            .expect("生成失败");

        assert!(!response.content.is_empty());
        assert_eq!(response.model, "test-model");
        assert_eq!(response.finish_reason, "stop");
    }

    #[tokio::test]
    async fn test_mock_llm_client_stream() {
        use futures::StreamExt;

        let client = MockLLMClient::new();

        let messages = vec![LLMMessage {
            role: MessageRole::User,
            content: "Stream test".to_string(),
        }];

        let mut stream = client
            .generate_stream(&messages, &GenerateOptions::default())
            .await
            .expect("流式生成失败");

        let mut received_chunks = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.expect("流式块错误");
            received_chunks += 1;
            if chunk.is_final {
                break;
            }
        }

        assert!(received_chunks > 0, "应接收到至少一个非最终块");
    }

    #[tokio::test]
    async fn test_rag_engine_query() {
        let engine = RAGEngine::new(
            RAGConfig::default(),
            Arc::new(MockLLMClient::new()),
            Arc::new(MockRetriever::new()),
        );

        let request = RAGRequest {
            query: "Explain ownership in Rust".to_string(),
            session_id: None,
            history: vec![],
            stream: false,
            options: None,
        };

        let response = engine.query(&request).await.expect("RAG 查询失败");

        assert!(!response.answer.is_empty());
    }

    #[test]
    fn test_rag_stats_serialization() {
        let stats = RAGStats {
            retrieval_candidates: 50,
            reranked_count: 20,
            context_used: 5,
            context_length: 4000,
            llm_input_tokens: 500,
            llm_output_tokens: 200,
        };

        let serialized = serde_json::to_string(&stats).expect("序列化失败");
        assert!(serialized.contains("retrieval_candidates"));
    }

    #[test]
    fn test_source_reference_creation() {
        let source = SourceReference {
            id: Uuid::new_v4(),
            relevance_score: 0.95,
            content_preview: "preview text...".to_string(),
            metadata: serde_json::json!({"doc_id": "123"}),
        };

        assert!((source.relevance_score - 0.95).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_rag_engine_sources_populated() {
        let engine = RAGEngine::new(
            RAGConfig::default(),
            Arc::new(MockLLMClient::new()),
            Arc::new(MockRetriever::new()),
        );

        let request = RAGRequest {
            query: "What is ownership?".to_string(),
            session_id: None,
            history: vec![],
            stream: false,
            options: None,
        };

        let response = engine.query(&request).await.expect("RAG 查询失败");

        assert!(!response.sources.is_empty(), "sources 应被填充");
        assert!(
            (response.sources[0].relevance_score - 0.95).abs() < f64::EPSILON,
            "第一个来源的分数应为 0.95"
        );
        assert!(
            !response.sources[0].content_preview.is_empty(),
            "content_preview 不应为空"
        );
    }

    #[tokio::test]
    async fn test_rag_engine_stats_populated() {
        let engine = RAGEngine::new(
            RAGConfig::default(),
            Arc::new(MockLLMClient::new()),
            Arc::new(MockRetriever::new()),
        );

        let request = RAGRequest {
            query: "What is ownership?".to_string(),
            session_id: None,
            history: vec![],
            stream: false,
            options: None,
        };

        let response = engine.query(&request).await.expect("RAG 查询失败");

        assert_eq!(
            response.stats.retrieval_candidates, 3,
            "retrieval_candidates 应为 3"
        );
        assert_eq!(
            response.stats.reranked_count, 3,
            "reranked_count 当前等于候选数"
        );
        assert!(
            response.stats.context_used >= 2,
            "context_used 应至少为 2（阈值过滤后）"
        );
        assert!(response.stats.context_length > 0, "context_length 应大于 0");
    }

    #[tokio::test]
    async fn test_rag_engine_score_threshold_filtering() {
        let config = RAGConfig {
            default_score_threshold: 0.7,
            ..RAGConfig::default()
        };

        let engine = RAGEngine::new(
            config,
            Arc::new(MockLLMClient::new()),
            Arc::new(MockRetriever::new()),
        );

        let request = RAGRequest {
            query: "What is ownership?".to_string(),
            session_id: None,
            history: vec![],
            stream: false,
            options: None,
        };

        let response = engine.query(&request).await.expect("RAG 查询失败");

        assert_eq!(
            response.sources.len(),
            2,
            "阈值 0.7 应过滤掉分数为 0.60 的结果"
        );
        for source in &response.sources {
            assert!(source.relevance_score >= 0.7, "所有来源分数应 >= 0.7");
        }
    }

    #[tokio::test]
    async fn test_rag_engine_empty_retriever() {
        let engine = RAGEngine::new(
            RAGConfig::default(),
            Arc::new(MockLLMClient::new()),
            Arc::new(MockRetriever::with_results(vec![])),
        );

        let request = RAGRequest {
            query: "What is ownership?".to_string(),
            session_id: None,
            history: vec![],
            stream: false,
            options: None,
        };

        let response = engine.query(&request).await.expect("RAG 查询失败");

        assert!(response.sources.is_empty(), "空检索器应返回空 sources");
        assert_eq!(response.stats.retrieval_candidates, 0);
        assert_eq!(response.stats.context_used, 0);
        assert_eq!(response.stats.context_length, 0);
    }

    #[test]
    fn test_retrieval_result_serialization() {
        let result = RetrievalResult {
            id: Uuid::new_v4(),
            content: "test content".to_string(),
            score: 0.9,
            metadata: serde_json::json!({"key": "value"}),
        };

        let serialized = serde_json::to_string(&result).expect("序列化失败");
        let deserialized: RetrievalResult =
            serde_json::from_str(&serialized).expect("反序列化失败");

        assert!((deserialized.score - 0.9).abs() < f64::EPSILON);
        assert_eq!(deserialized.content, "test content");
    }

    #[tokio::test]
    async fn test_mock_retriever_respects_top_k() {
        let retriever = MockRetriever::new();

        let results = retriever.retrieve("test query", 2).await.expect("检索失败");

        assert_eq!(results.len(), 2, "top_k=2 应返回 2 个结果");
    }

    #[test]
    fn test_build_context_respects_max_length() {
        let config = RAGConfig::default();
        let engine = RAGEngine::new(
            config,
            Arc::new(MockLLMClient::new()),
            Arc::new(MockRetriever::new()),
        );

        let results = [
            RetrievalResult {
                id: Uuid::new_v4(),
                content: "A".repeat(100),
                score: 0.9,
                metadata: serde_json::Value::Null,
            },
            RetrievalResult {
                id: Uuid::new_v4(),
                content: "B".repeat(100),
                score: 0.8,
                metadata: serde_json::Value::Null,
            },
        ];
        let refs: Vec<&RetrievalResult> = results.iter().collect();

        let (context, used) = engine.build_context(&refs, 50);

        assert!(context.len() <= 50, "上下文不应超过最大长度");
        assert!(used < 2, "应只使用部分结果");
    }
}
