//! RAG 流式响应模块
//!
//! 提供 Server-Sent Events (SSE) 格式的流式输出支持，
//! 实现实时的 Token 级别响应推送。
//!
//! # SSE 格式规范
//!
//! ```text
//! data: {"id":"uuid","chunk_type":"Context","content":"..."}
//! data: {"id":"uuid","chunk_type":"Content","content":"Hello"}
//! data: {"id":"uuid","chunk_type":"Content","content":" world"}
//! data: {"id":"uuid","chunk_type":"Citation","content":{"source":...}}
//! data: {"id":"uuid","chunk_type":"Done","content":""}
//! ```

use crate::rag::engine::StreamChunk;
use error_core::Result;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use uuid::Uuid;

/// 流式响应块类型
///
/// 定义 RAG 流式输出的不同数据类型：
/// - **Context**: 检索到的上下文片段（可选发送）
/// - **Content**: LLM 生成的文本内容（主要输出）
/// - **Citation**: 引用来源信息（在结束时发送）
/// - **Done**: 流结束标记
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChunkType {
    /// 检索到的上下文片段
    Context,
    /// LLM 生成的文本内容
    Content,
    /// 引用来源
    Citation,
    /// 流完成标记
    Done,
}

impl std::fmt::Display for ChunkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Context => write!(f, "Context"),
            Self::Content => write!(f, "Content"),
            Self::Citation => write!(f, "Citation"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// RAG 流式响应块
///
/// 每个 SSE 事件的数据载荷，包含类型、内容和元数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGStreamChunk {
    /// 块唯一标识符（与 `request_id` 关联）
    pub id: Uuid,
    /// 块类型
    #[serde(rename = "chunkType")]
    pub chunk_type: ChunkType,
    /// 块内容（根据类型不同含义不同）
    pub content: String,
    /// 可选元数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl RAGStreamChunk {
    /// 创建新的 Context 块
    #[must_use]
    pub fn context(id: Uuid, content: impl Into<String>) -> Self {
        Self {
            id,
            chunk_type: ChunkType::Context,
            content: content.into(),
            metadata: None,
        }
    }

    /// 创建 Content 块（LLM 生成的文本）
    #[must_use]
    pub fn content(id: Uuid, text: impl Into<String>) -> Self {
        Self {
            id,
            chunk_type: ChunkType::Content,
            content: text.into(),
            metadata: None,
        }
    }

    /// 创建 Citation 块（引用来源）
    #[must_use]
    pub const fn citation(id: Uuid, source_info: serde_json::Value) -> Self {
        Self {
            id,
            chunk_type: ChunkType::Citation,
            content: String::new(),
            metadata: Some(source_info),
        }
    }

    /// 创建 Done 块（流结束）
    #[must_use]
    pub const fn done(id: Uuid) -> Self {
        Self {
            id,
            chunk_type: ChunkType::Done,
            content: String::new(),
            metadata: None,
        }
    }

    /// 序列化为 SSE 数据行
    ///
    /// 格式：`data: {json}\n\n`
    ///
    /// # Errors
    ///
    /// 当 JSON 序列化失败时返回错误。
    pub fn to_sse_data(&self) -> Result<String> {
        let json = serde_json::to_string(self)?;
        Ok(format!("data: {json}\n\n"))
    }
}

/// SSE 格式化器
///
/// 将 `RAGStreamChunk` 序列化为符合 SSE 规范的文本格式。
pub struct SSEFormatter;

impl SSEFormatter {
    /// 将单个块格式化为 SSE 行
    ///
    /// # Errors
    ///
    /// 当块序列化失败时返回错误。
    pub fn format_chunk(chunk: &RAGStreamChunk) -> Result<String> {
        chunk.to_sse_data()
    }

    /// 格式化流结束标记
    #[must_use]
    pub const fn format_end() -> &'static str {
        "data: [DONE]\n\n"
    }

    /// 设置 SSE 响应头（用于 Axum handler）
    ///
    /// 返回正确的 `Content-Type` 和 `Cache-Control` 头。
    #[must_use]
    pub fn response_headers() -> [(String, String); 2] {
        [
            ("Content-Type".to_string(), "text/event-stream".to_string()),
            ("Cache-Control".to_string(), "no-cache".to_string()),
        ]
    }
}

/// 将 LLM Token 流包装为 `RAGStreamChunk` 流
///
/// 内部函数，由 `RAGEngine::query_stream` 调用。
pub(crate) fn wrap_llm_stream(
    llm_stream: Box<dyn Stream<Item = Result<StreamChunk>> + Send + Unpin>,
    request_id: Uuid,
) -> Pin<Box<dyn Stream<Item = Result<RAGStreamChunk>> + Send>> {
    use futures::stream::StreamExt;

    let mapped = llm_stream.map(move |result| {
        result.map(|chunk| {
            if chunk.is_final {
                let mut done_chunk = RAGStreamChunk::done(request_id);
                let mut meta = serde_json::Map::new();
                if let Some(usage) = chunk.usage {
                    meta.insert(
                        "usage".to_string(),
                        serde_json::json!({
                            "promptTokens": usage.prompt_tokens,
                            "completionTokens": usage.completion_tokens,
                            "totalTokens": usage.total_tokens,
                        }),
                    );
                }
                if let Some(model) = chunk.model {
                    meta.insert("model".to_string(), serde_json::Value::String(model));
                }
                if !meta.is_empty() {
                    done_chunk.metadata = Some(serde_json::Value::Object(meta));
                }
                done_chunk
            } else {
                RAGStreamChunk::content(request_id, chunk.content)
            }
        })
    });

    Box::pin(mapped)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rag::engine::TokenUsage;
    use futures::StreamExt;

    #[test]
    fn test_chunk_type_display() {
        assert_eq!(ChunkType::Context.to_string(), "Context");
        assert_eq!(ChunkType::Content.to_string(), "Content");
        assert_eq!(ChunkType::Citation.to_string(), "Citation");
        assert_eq!(ChunkType::Done.to_string(), "Done");
    }

    #[test]
    fn test_rag_stream_chunk_constructors() {
        let id = Uuid::new_v4();

        let ctx = RAGStreamChunk::context(id, "context text");
        assert_eq!(ctx.chunk_type, ChunkType::Context);
        assert_eq!(ctx.content, "context text");

        let content = RAGStreamChunk::content(id, "hello");
        assert_eq!(content.chunk_type, ChunkType::Content);

        let citation = RAGStreamChunk::citation(id, serde_json::json!({"source": "doc1"}));
        assert_eq!(citation.chunk_type, ChunkType::Citation);
        assert!(citation.metadata.is_some());

        let done = RAGStreamChunk::done(id);
        assert_eq!(done.chunk_type, ChunkType::Done);
    }

    #[test]
    fn test_sse_formatting() {
        let id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let chunk = RAGStreamChunk::content(id, "Hello, world!");

        let sse_data = SSEFormatter::format_chunk(&chunk).expect("SSE 格式化失败");

        assert!(sse_data.starts_with("data: "));
        assert!(sse_data.ends_with("\n\n"));
        assert!(sse_data.contains("\"chunkType\":\"content\""));
        assert!(sse_data.contains("Hello, world!"));
    }

    #[test]
    fn test_sse_format_end() {
        let end_marker = SSEFormatter::format_end();
        assert_eq!(end_marker, "data: [DONE]\n\n");
    }

    #[test]
    fn test_sse_headers() {
        let headers = SSEFormatter::response_headers();
        assert_eq!(headers[0].0, "Content-Type");
        assert_eq!(headers[0].1, "text/event-stream");
        assert_eq!(headers[1].0, "Cache-Control");
    }

    #[tokio::test]
    async fn test_wrap_llm_stream() {
        let mock_chunks: Vec<Result<StreamChunk>> = vec![
            Ok(StreamChunk { content: "Hello ".to_string(), is_final: false, usage: None, model: None }),
            Ok(StreamChunk { content: "world!".to_string(), is_final: false, usage: None, model: None }),
            Ok(StreamChunk { content: String::new(), is_final: true, usage: Some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }), model: Some("mock-llm".to_string()) }),
        ];

        let mock_stream = futures::stream::iter(mock_chunks);
        let boxed: Box<dyn Stream<Item = Result<StreamChunk>> + Send + Unpin> = Box::new(mock_stream);

        let request_id = Uuid::new_v4();
        let wrapped = wrap_llm_stream(boxed, request_id);

        let mut collected: Vec<RAGStreamChunk> = Vec::new();
        futures::pin_mut!(wrapped);

        while let Some(result) = wrapped.next().await {
            match result {
                Ok(chunk) => collected.push(chunk),
                Err(e) => panic!("流式传输错误: {e}"),
            }
        }

        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0].chunk_type, ChunkType::Content);
        assert_eq!(collected[0].content, "Hello ");
        assert_eq!(collected[1].content, "world!");
        assert_eq!(collected[2].chunk_type, ChunkType::Done);
    }

    #[test]
    fn test_rag_stream_chunk_serialization() {
        let id = Uuid::new_v4();
        let chunk = RAGStreamChunk {
            id,
            chunk_type: ChunkType::Content,
            content: "test".to_string(),
            metadata: Some(serde_json::json!({"key": "value"})),
        };

        let json = serde_json::to_string(&chunk).expect("序列化失败");
        assert!(json.contains("\"chunkType\":\"content\""));
        assert!(json.contains("\"metadata\""));
    }

    #[test]
    fn test_multiple_chunks_sse_sequence() {
        let id = Uuid::new_v4();

        let chunks = [
            RAGStreamChunk::context(id, "Source document excerpt..."),
            RAGStreamChunk::content(id, "Based on the context, "),
            RAGStreamChunk::content(id, "the answer is..."),
            RAGStreamChunk::citation(id, serde_json::json!({"id": "doc-1", "score": 0.95})),
            RAGStreamChunk::done(id),
        ];

        let sse_output: std::result::Result<Vec<String>, _> = chunks
            .iter()
            .map(RAGStreamChunk::to_sse_data)
            .collect();

        assert!(sse_output.is_ok());
        let lines = sse_output.unwrap();
        assert_eq!(lines.len(), 5);

        for line in &lines {
            assert!(line.starts_with("data: "), "每行应以 'data: ' 开头: {line}");
            assert!(line.ends_with("\n\n"), "每行应以双换行结尾: {line}");
        }
    }
}
