use async_trait::async_trait;

use crate::Result;

/// 统一文本摘要器 trait — 整合 Agent Memory / Parser 两套 Summarizer
///
/// # 碎片化现状
///
/// | Trait | Crate | 输入类型 | 用途 |
/// |-------|-------|---------|------|
/// | `agent::memory::Summarizer` | knowledge-api | `&[MemoryEntry]` | Agent 记忆反思 |
/// | `parser::summarizer::CommunitySummarizer` | knowledge-parser | 结构体（非 trait） | 社区摘要 |
///
/// # 统一方案
///
/// `TextSummarizer` 提供通用的文本摘要接口，
/// 接受 `&str` 输入，返回摘要文本。
/// Agent Memory 的 `Summarizer` 和 Parser 的摘要逻辑
/// 均可通过适配器实现此 trait。
///
/// # Examples
///
/// ```ignore
/// use knowledge_api::plugins::summarizer_adapter::TextSummarizer;
/// use std::sync::Arc;
///
/// let summarizer: Arc<dyn TextSummarizer> = Arc::new(MySummarizer::new());
/// let summary = summarizer.summarize_text("长文本内容...").await?;
/// ```
#[async_trait]
pub trait TextSummarizer: Send + Sync {
    /// 对文本生成摘要
    ///
    /// # Arguments
    ///
    /// * `text` - 待摘要的原始文本
    ///
    /// # Errors
    ///
    /// 当 LLM 调用失败或文本处理异常时返回错误。
    async fn summarize_text(&self, text: &str) -> Result<String>;

    /// 对多个文本片段生成合并摘要
    ///
    /// 默认实现将多个片段拼接后调用 `summarize_text`。
    /// 实现者可覆盖此方法以提供更高效的批量摘要策略。
    async fn summarize_texts(&self, texts: &[&str]) -> Result<String> {
        let combined = texts.join("\n\n---\n\n");
        self.summarize_text(&combined).await
    }

    /// 健康检查
    async fn health_check(&self) -> Result<bool>;
}

/// Agent Memory `Summarizer` → `TextSummarizer` 的适配器
///
/// 将 `knowledge_api::agent::memory::Summarizer` 适配为 `TextSummarizer`。
/// 由于 `Summarizer` 接受 `&[MemoryEntry]` 而非 `&str`，
/// 此适配器将文本包装为 `MemoryEntry` 再委托给原始 `Summarizer`。
pub struct MemorySummarizerAdapter<T> {
    inner: T,
}

impl<T> MemorySummarizerAdapter<T> {
    /// 创建适配器
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<T: crate::agent::memory::Summarizer + 'static> TextSummarizer for MemorySummarizerAdapter<T> {
    async fn summarize_text(&self, text: &str) -> Result<String> {
        let entry =
            crate::agent::memory::MemoryEntry::new(crate::agent::memory::MemoryType::Fact, text);
        self.inner.summarize(&[entry]).await
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}

/// 基于 LLM 的文本摘要器（通用实现）
///
/// 使用 `LLMProvider` 生成文本摘要。
/// 这是最通用的 `TextSummarizer` 实现，
/// 可用于任何场景。
pub struct LlmTextSummarizer<T> {
    llm: T,
    max_length: usize,
}

impl<T> LlmTextSummarizer<T> {
    /// 创建基于 LLM 的文本摘要器
    pub fn new(llm: T) -> Self {
        Self {
            llm,
            max_length: 500,
        }
    }

    /// 设置最大摘要长度
    #[must_use]
    pub fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = max_length;
        self
    }
}

#[async_trait]
impl<T: super::llm_provider::LLMProvider + 'static> TextSummarizer for LlmTextSummarizer<T> {
    async fn summarize_text(&self, text: &str) -> Result<String> {
        let prompt = format!(
            "请对以下文本生成简洁的摘要，不超过 {} 字：\n\n{}",
            self.max_length, text
        );
        self.llm.generate(&prompt).await
    }

    async fn health_check(&self) -> Result<bool> {
        self.llm.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_summarizer_default_max_length() {
        // 验证默认最大长度
        struct MockLlm;
        let summarizer: LlmTextSummarizer<MockLlm> = LlmTextSummarizer::new(MockLlm);
        assert_eq!(summarizer.max_length, 500);
    }

    #[test]
    fn test_llm_summarizer_custom_max_length() {
        struct MockLlm;
        let summarizer: LlmTextSummarizer<MockLlm> =
            LlmTextSummarizer::new(MockLlm).with_max_length(200);
        assert_eq!(summarizer.max_length, 200);
    }
}
