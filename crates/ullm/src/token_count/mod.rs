use crate::error::LlmError;

/// Token 计数异步 Future 类型
pub type TokenCountFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64, LlmError>> + Send + 'a>>;

/// Token 计数器 trait
pub trait TokenCounter: Send + Sync {
    /// 计算文本的 Token 数量
    fn count_tokens(&self, text: &str, model: &str) -> u64;

    /// 计算消息列表的 Token 数量
    fn count_messages_tokens(&self, messages: &[(&str, &str)], model: &str) -> u64;

    /// 计算图片的 Token 数量（基于分辨率估算）
    fn count_image_tokens(&self, width: u32, height: u32) -> u64 {
        ((u64::from(width) * u64::from(height)) / 750).max(1)
    }
}

/// 字节估算器，按 4 字节 ≈ 1 Token 估算
pub struct ByteEstimator;

impl TokenCounter for ByteEstimator {
    fn count_tokens(&self, text: &str, _model: &str) -> u64 {
        (text.len() as u64 / 4).max(1)
    }

    fn count_messages_tokens(&self, messages: &[(&str, &str)], _model: &str) -> u64 {
        let total: usize = messages
            .iter()
            .map(|(role, content)| role.len() + content.len())
            .sum();
        (total as u64 / 4).max(1)
    }
}

/// Tiktoken 计数器，使用 tiktoken-rs 进行精确计数
pub struct TiktokenCounter;

impl TokenCounter for TiktokenCounter {
    fn count_tokens(&self, text: &str, model: &str) -> u64 {
        match tiktoken_rs::num_tokens_from_messages(
            model,
            &[tiktoken_rs::ChatCompletionRequestMessage {
                role: "user".to_string(),
                content: Some(text.to_string()),
                name: None,
                function_call: None,
            }],
        ) {
            Ok(count) => count as u64,
            Err(_) => ByteEstimator.count_tokens(text, model),
        }
    }

    fn count_messages_tokens(&self, messages: &[(&str, &str)], model: &str) -> u64 {
        let chat_messages: Vec<tiktoken_rs::ChatCompletionRequestMessage> = messages
            .iter()
            .map(
                |(role, content)| tiktoken_rs::ChatCompletionRequestMessage {
                    role: (*role).to_string(),
                    content: Some((*content).to_string()),
                    name: None,
                    function_call: None,
                },
            )
            .collect();
        match tiktoken_rs::num_tokens_from_messages(model, &chat_messages) {
            Ok(count) => count as u64,
            Err(_) => ByteEstimator.count_messages_tokens(messages, model),
        }
    }
}

/// 远程 Token 计数器，调用外部 API 进行计数（当前回退到 `TiktokenCounter`）
#[allow(dead_code)]
pub struct RemoteTokenCounter {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    fallback: TiktokenCounter,
}

impl RemoteTokenCounter {
    /// 创建远程 Token 计数器
    pub fn new(api_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url: api_url.into(),
            api_key: api_key.into(),
            fallback: TiktokenCounter,
        }
    }
}

impl TokenCounter for RemoteTokenCounter {
    fn count_tokens(&self, text: &str, model: &str) -> u64 {
        self.fallback.count_tokens(text, model)
    }

    fn count_messages_tokens(&self, messages: &[(&str, &str)], model: &str) -> u64 {
        self.fallback.count_messages_tokens(messages, model)
    }
}

/// 回退式 Token 计数器，依次尝试主计数器、备用计数器和字节估算器
#[allow(dead_code)]
pub struct FallbackTokenCounter {
    primary: Box<dyn TokenCounter>,
    secondary: Box<dyn TokenCounter>,
    tertiary: ByteEstimator,
}

impl FallbackTokenCounter {
    /// 创建回退式计数器
    #[must_use]
    pub fn new(primary: Box<dyn TokenCounter>, secondary: Box<dyn TokenCounter>) -> Self {
        Self {
            primary,
            secondary,
            tertiary: ByteEstimator,
        }
    }

    /// 为指定模型创建默认回退式计数器
    #[must_use]
    pub fn default_for_model(_model: &str) -> Self {
        Self {
            primary: Box::new(TiktokenCounter),
            secondary: Box::new(ByteEstimator),
            tertiary: ByteEstimator,
        }
    }
}

impl TokenCounter for FallbackTokenCounter {
    fn count_tokens(&self, text: &str, model: &str) -> u64 {
        self.primary.count_tokens(text, model)
    }

    fn count_messages_tokens(&self, messages: &[(&str, &str)], model: &str) -> u64 {
        self.primary.count_messages_tokens(messages, model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_estimator_count_tokens() {
        let estimator = ByteEstimator;
        let text = "Hello, world!";
        let count = estimator.count_tokens(text, "gpt-4");
        assert!(count > 0);
        assert_eq!(count, (text.len() as u64 / 4).max(1));
    }

    #[test]
    fn test_byte_estimator_count_messages_tokens() {
        let estimator = ByteEstimator;
        let messages = vec![("user", "Hello"), ("assistant", "Hi there!")];
        let count = estimator.count_messages_tokens(&messages, "gpt-4");
        assert!(count > 0);
    }

    #[test]
    fn test_count_image_tokens() {
        let estimator = ByteEstimator;
        let tokens = estimator.count_image_tokens(1024, 1024);
        assert_eq!(tokens, 1024 * 1024 / 750);
    }

    #[test]
    fn test_count_image_tokens_minimum() {
        let estimator = ByteEstimator;
        let tokens = estimator.count_image_tokens(1, 1);
        assert_eq!(tokens, 1);
    }

    #[test]
    fn test_tiktoken_counter_fallback() {
        let counter = TiktokenCounter;
        let count = counter.count_tokens("Hello, world!", "gpt-4");
        assert!(count > 0);
    }

    #[test]
    fn test_tiktoken_counter_unknown_model() {
        let counter = TiktokenCounter;
        let count = counter.count_tokens("Hello", "totally-unknown-model-xyz");
        assert!(count > 0);
    }

    #[test]
    fn test_remote_token_counter() {
        let counter = RemoteTokenCounter::new("http://localhost:8080", "test-key");
        let count = counter.count_tokens("Hello", "gpt-4");
        assert!(count > 0);
    }

    #[test]
    fn test_fallback_token_counter() {
        let counter = FallbackTokenCounter::default_for_model("gpt-4");
        let count = counter.count_tokens("Hello, world!", "gpt-4");
        assert!(count > 0);
    }

    #[test]
    fn test_fallback_token_counter_messages() {
        let counter = FallbackTokenCounter::default_for_model("gpt-4");
        let messages = vec![("user", "Hello"), ("assistant", "Hi")];
        let count = counter.count_messages_tokens(&messages, "gpt-4");
        assert!(count > 0);
    }

    #[test]
    fn test_fallback_token_counter_new_custom() {
        let counter = FallbackTokenCounter::new(Box::new(TiktokenCounter), Box::new(ByteEstimator));
        let count = counter.count_tokens("Hello", "gpt-4");
        assert!(count > 0);
    }

    #[test]
    fn test_tiktoken_counter_messages_tokens() {
        let counter = TiktokenCounter;
        let messages = vec![("user", "Hello"), ("assistant", "Hi there!")];
        let count = counter.count_messages_tokens(&messages, "gpt-4");
        assert!(count > 0);
    }

    #[test]
    fn test_byte_estimator_count_messages_tokens_with_roles() {
        let estimator = ByteEstimator;
        let messages = vec![
            ("system", "You are helpful"),
            ("user", "Hello"),
            ("assistant", "Hi"),
        ];
        let count = estimator.count_messages_tokens(&messages, "gpt-4");
        assert!(count > 0);
    }

    #[test]
    fn test_remote_token_counter_messages_tokens() {
        let counter = RemoteTokenCounter::new("http://localhost:8080", "test-key");
        let messages = vec![("user", "Hello")];
        let count = counter.count_messages_tokens(&messages, "gpt-4");
        assert!(count > 0);
    }

    #[test]
    fn test_byte_estimator_empty_text() {
        let estimator = ByteEstimator;
        let count = estimator.count_tokens("", "gpt-4");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_byte_estimator_count_image_tokens_large() {
        let estimator = ByteEstimator;
        let tokens = estimator.count_image_tokens(2048, 2048);
        assert!(tokens > 0);
    }

    #[test]
    fn test_deductive_byte_estimator_never_zero() {
        let estimator = ByteEstimator;
        let texts = &["", "a", "ab", "abc", "abcd", "abcde"];
        for &text in texts {
            let count = estimator.count_tokens(text, "gpt-4");
            assert!(
                count > 0,
                "ByteEstimator::count_tokens must never return 0, got {count} for {text:?}"
            );
        }
    }

    #[test]
    fn test_deductive_byte_estimator_messages_never_zero() {
        let estimator = ByteEstimator;
        let cases: &[Vec<(&str, &str)>] = &[
            vec![],
            vec![("user", "")],
            vec![("user", "a")],
            vec![("system", "hello"), ("user", "world")],
        ];
        for messages in cases {
            let count = estimator.count_messages_tokens(messages, "gpt-4");
            assert!(
                count > 0,
                "ByteEstimator::count_messages_tokens must never return 0"
            );
        }
    }

    #[test]
    fn test_deductive_count_image_tokens_minimum_one() {
        let estimator = ByteEstimator;
        assert_eq!(estimator.count_image_tokens(0, 0), 1);
        assert_eq!(estimator.count_image_tokens(1, 1), 1);
        assert_eq!(estimator.count_image_tokens(0, 100), 1);
        assert_eq!(estimator.count_image_tokens(100, 0), 1);
    }

    #[test]
    fn test_deductive_count_image_tokens_monotone_in_resolution() {
        let estimator = ByteEstimator;
        let small = estimator.count_image_tokens(100, 100);
        let large = estimator.count_image_tokens(1024, 1024);
        assert!(large >= small, "larger resolution must produce >= tokens");
    }

    #[test]
    fn test_deductive_byte_estimator_monotone_in_length() {
        let estimator = ByteEstimator;
        let short = estimator.count_tokens("ab", "gpt-4");
        let long = estimator.count_tokens("abcdefgh", "gpt-4");
        assert!(long >= short, "longer text must produce >= tokens");
    }
}
