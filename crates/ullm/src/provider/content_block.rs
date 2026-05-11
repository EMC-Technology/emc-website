use serde::{Deserialize, Serialize};

/// 消息内容块，表示 LLM 对话中一条消息内的结构化内容单元。
///
/// 采用 `#[serde(tag = "type")]` 进行外部标签序列化，
/// 与 `OpenAI` / Anthropic 等供应商的 JSON 协议兼容。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    /// 纯文本内容块
    #[serde(rename = "text")]
    Text {
        /// 文本内容
        text: String,
    },

    /// 图片内容块
    #[serde(rename = "image")]
    Image {
        /// 图片数据源（Base64 或 URL）
        source: ImageSource,
        /// 图片解析精度
        #[serde(default)]
        detail: ImageDetail,
    },

    /// 工具调用请求块
    #[serde(rename = "tool_use")]
    ToolUse {
        /// 工具调用唯一标识
        id: String,
        /// 工具名称
        name: String,
        /// 工具调用参数（JSON）
        input: serde_json::Value,
    },

    /// 工具调用结果块
    #[serde(rename = "tool_result")]
    ToolResult {
        /// 对应的工具调用标识
        tool_use_id: String,
        /// 工具返回内容
        content: String,
        /// 是否为错误结果
        is_error: bool,
    },

    /// 思维链内容块（可读）
    #[serde(rename = "thinking")]
    Thinking {
        /// 思维链文本
        text: String,
        /// 签名（用于验证思维链完整性，部分供应商支持）
        signature: Option<String>,
    },

    /// 已脱敏的思维链内容块
    #[serde(rename = "redacted_thinking")]
    RedactedThinking {
        /// 脱敏后的加密数据
        data: String,
    },
}

impl ContentBlock {
    /// 创建纯文本内容块。
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text {
            text: content.into(),
        }
    }

    /// 创建 Base64 编码的图片内容块。
    #[must_use]
    pub fn image_base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Image {
            source: ImageSource::Base64 {
                media_type: media_type.into(),
                data: data.into(),
            },
            detail: ImageDetail::default(),
        }
    }

    /// 创建 URL 引用的图片内容块。
    #[must_use]
    pub fn image_url(url: impl Into<String>) -> Self {
        Self::Image {
            source: ImageSource::Url { url: url.into() },
            detail: ImageDetail::default(),
        }
    }

    /// 创建工具调用请求块。
    #[must_use]
    pub fn tool_use(
        id: impl Into<String>,
        name: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        Self::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }
    }

    /// 创建工具调用成功结果块。
    #[must_use]
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    /// 创建工具调用错误结果块。
    #[must_use]
    pub fn tool_result_error(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: true,
        }
    }

    /// 创建无签名的思维链内容块。
    #[must_use]
    pub fn thinking(text: impl Into<String>) -> Self {
        Self::Thinking {
            text: text.into(),
            signature: None,
        }
    }

    /// 创建带签名的思维链内容块。
    #[must_use]
    pub fn thinking_with_signature(text: impl Into<String>, signature: impl Into<String>) -> Self {
        Self::Thinking {
            text: text.into(),
            signature: Some(signature.into()),
        }
    }

    /// 创建已脱敏的思维链内容块。
    #[must_use]
    pub fn redacted_thinking(data: impl Into<String>) -> Self {
        Self::RedactedThinking { data: data.into() }
    }

    /// 判断是否为文本内容块。
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self, ContentBlock::Text { .. })
    }

    /// 判断是否为图片内容块。
    #[must_use]
    pub fn is_image(&self) -> bool {
        matches!(self, ContentBlock::Image { .. })
    }

    /// 判断是否为工具调用请求块。
    #[must_use]
    pub fn is_tool_use(&self) -> bool {
        matches!(self, ContentBlock::ToolUse { .. })
    }

    /// 判断是否为工具调用结果块。
    #[must_use]
    pub fn is_tool_result(&self) -> bool {
        matches!(self, ContentBlock::ToolResult { .. })
    }

    /// 判断是否为思维链内容块（含已脱敏变体）。
    #[must_use]
    pub fn is_thinking(&self) -> bool {
        matches!(
            self,
            ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. }
        )
    }

    /// 若为文本块，返回其文本内容的引用；否则返回 `None`。
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentBlock::Text { text } => Some(text),
            _ => None,
        }
    }

    /// 若为工具调用请求块，返回 `(id, name, input)` 的引用元组；否则返回 `None`。
    #[must_use]
    pub fn as_tool_use(&self) -> Option<(&str, &str, &serde_json::Value)> {
        match self {
            ContentBlock::ToolUse { id, name, input } => Some((id, name, input)),
            _ => None,
        }
    }
}

/// 图片数据源，支持 Base64 内嵌和 URL 引用两种方式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageSource {
    /// Base64 编码的图片数据
    Base64 {
        /// MIME 类型（如 `image/png`）
        media_type: String,
        /// Base64 编码数据
        data: String,
    },
    /// 图片 URL 引用
    Url {
        /// 图片地址
        url: String,
    },
}

/// 图片解析精度级别。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ImageDetail {
    /// 自动选择精度
    #[default]
    Auto,
    /// 低精度（节省 Token）
    Low,
    /// 高精度（消耗更多 Token）
    High,
}

/// 消息内容，支持纯文本和多内容块两种形式。
///
/// 采用 `#[serde(untagged)]` 序列化，纯文本直接输出字符串，
/// 多内容块输出为数组。标记 `#[non_exhaustive]` 以预留扩展。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum MessageContent {
    /// 纯文本消息
    Text(String),
    /// 多内容块消息
    Blocks(Vec<ContentBlock>),
}

impl MessageContent {
    /// 创建纯文本消息内容。
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text(content.into())
    }

    /// 创建多内容块消息内容。
    #[must_use]
    pub fn blocks(blocks: Vec<ContentBlock>) -> Self {
        Self::Blocks(blocks)
    }

    /// 判断消息内容是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            MessageContent::Text(s) => s.is_empty(),
            MessageContent::Blocks(blocks) => blocks.is_empty(),
        }
    }

    /// 尝试获取首个文本块的内容引用。
    ///
    /// 对于 `Text` 变体直接返回字符串引用；
    /// 对于 `Blocks` 变体返回第一个 `ContentBlock::Text` 的文本。
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(s) => Some(s),
            MessageContent::Blocks(blocks) => {
                let mut result = None;
                for block in blocks {
                    if let Some(text) = block.as_text() {
                        result = Some(text);
                        break;
                    }
                }
                result
            }
        }
    }

    /// 将消息内容转换为纯文本，丢弃非文本块。
    ///
    /// 对于 `Blocks` 变体，将所有文本块拼接为一个字符串。
    #[must_use]
    pub fn to_text_lossy(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| b.as_text())
                .collect::<Vec<_>>()
                .join(""),
        }
    }

    /// 返回所有内容块的引用列表。
    ///
    /// 对于 `Text` 变体返回空列表。
    #[must_use]
    pub fn content_blocks(&self) -> Vec<&ContentBlock> {
        match self {
            MessageContent::Text(_) => Vec::new(),
            MessageContent::Blocks(blocks) => blocks.iter().collect(),
        }
    }
}

impl Default for MessageContent {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_block_text() {
        let block = ContentBlock::text("Hello");
        assert!(block.is_text());
        assert!(!block.is_image());
        assert_eq!(block.as_text(), Some("Hello"));
    }

    #[test]
    fn test_content_block_image_url() {
        let block = ContentBlock::image_url("https://example.com/img.png");
        assert!(block.is_image());
        assert!(!block.is_text());
    }

    #[test]
    fn test_content_block_image_base64() {
        let block = ContentBlock::image_base64("image/png", "iVBOR...");
        assert!(block.is_image());
    }

    #[test]
    fn test_content_block_tool_use() {
        let block =
            ContentBlock::tool_use("call_1", "get_weather", serde_json::json!({"city": "SF"}));
        assert!(block.is_tool_use());
        let (id, name, input) = block.as_tool_use().unwrap();
        assert_eq!(id, "call_1");
        assert_eq!(name, "get_weather");
        assert_eq!(input["city"], "SF");
    }

    #[test]
    fn test_content_block_tool_result() {
        let block = ContentBlock::tool_result("call_1", "72°F");
        assert!(block.is_tool_result());
        assert!(!block.is_tool_use());
    }

    #[test]
    fn test_content_block_tool_result_error() {
        let block = ContentBlock::tool_result_error("call_1", "API error");
        assert!(block.is_tool_result());
        match block {
            ContentBlock::ToolResult { is_error, .. } => assert!(is_error),
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn test_content_block_thinking() {
        let block = ContentBlock::thinking("Let me think...");
        assert!(block.is_thinking());
    }

    #[test]
    fn test_content_block_thinking_with_signature() {
        let block = ContentBlock::thinking_with_signature("Thought", "sig123");
        assert!(block.is_thinking());
    }

    #[test]
    fn test_content_block_redacted_thinking() {
        let block = ContentBlock::redacted_thinking("encrypted_data");
        assert!(block.is_thinking());
    }

    #[test]
    fn test_message_content_text() {
        let content = MessageContent::text("Hello");
        assert!(!content.is_empty());
        assert_eq!(content.as_text(), Some("Hello"));
        assert_eq!(content.to_text_lossy(), "Hello");
        assert!(content.content_blocks().is_empty());
    }

    #[test]
    fn test_message_content_blocks() {
        let content = MessageContent::blocks(vec![
            ContentBlock::text("Hello"),
            ContentBlock::image_url("https://example.com/img.png"),
        ]);
        assert!(!content.is_empty());
        assert_eq!(content.as_text(), Some("Hello"));
        assert_eq!(content.to_text_lossy(), "Hello");
        assert_eq!(content.content_blocks().len(), 2);
    }

    #[test]
    fn test_message_content_empty() {
        let content = MessageContent::text("");
        assert!(content.is_empty());
    }

    #[test]
    fn test_message_content_from_string() {
        let content: MessageContent = "Hello".into();
        assert_eq!(content.as_text(), Some("Hello"));
    }

    #[test]
    fn test_message_content_default() {
        let content = MessageContent::default();
        assert!(content.is_empty());
    }

    #[test]
    fn test_content_block_serde_roundtrip() {
        let block = ContentBlock::text("Hello");
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_text());
        assert_eq!(deserialized.as_text(), Some("Hello"));
    }

    #[test]
    fn test_message_content_serde_text() {
        let content = MessageContent::text("Hello");
        let json = serde_json::to_string(&content).unwrap();
        let deserialized: MessageContent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.as_text(), Some("Hello"));
    }

    #[test]
    fn test_message_content_serde_blocks() {
        let content = MessageContent::blocks(vec![ContentBlock::text("Hello")]);
        let json = serde_json::to_string(&content).unwrap();
        let deserialized: MessageContent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.content_blocks().len(), 1);
    }

    #[test]
    fn test_image_detail_default() {
        let detail = ImageDetail::default();
        assert!(matches!(detail, ImageDetail::Auto));
    }
}
