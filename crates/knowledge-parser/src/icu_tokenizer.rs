//! ICU Unicode 分词器（Block 内容 → icu_segmenter Unicode 词级分词 → Token 流）
//!
//! 使用 ICU4X 官方项目的 NLP 分词规则集，输出连续的词元。
//! 遵循 Unicode Standard Annex #29 (UAX#29) 规范。
//!
//! # 设计要点
//!
//! - **词级分割**：使用 `WordSegmenter` 进行 Unicode 标准词级分词，
//!   确保对 CJK 文本、英文单词、缩写等的正确处理。
//!   对于 CJK 文本，`WordSegmenter` 基于 ICU 的词典进行分词。
//! - **词元类型自动分类**：根据 Unicode 属性自动判断 Token 类型
//! - **全局偏移追踪**：支持跨块的连续 global_offset 计算
//!
//! # 确定性保证
//!
//! `WordSegmenter` 基于 Unicode 标准规则集，输出完全确定性。
//! 相同输入在不同硬件/运行时环境下产生字节级一致的输出。

use icu_segmenter::WordSegmenter;
use knowledge_core::model::{RecordIdType, Token, TokenStatus, TokenType};
use surrealdb::sql::Thing;

/// ICU Unicode 分词器
///
/// 对单个 Block 内容进行词级分词，生成符合 UAX#29 规范的 Token 流。
/// 使用 `WordSegmenter`（而非 `GraphemeClusterSegmenter`）确保输出粒度为词级，
/// 而非字符级，这是语义处理的基本单位。
pub struct IcuTokenizer {
    segmenter: WordSegmenter,
}

impl Default for IcuTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl IcuTokenizer {
    /// 创建新的 ICU 分词器实例
    #[must_use]
    pub fn new() -> Self {
        let segmenter = WordSegmenter::new_auto();
        Self { segmenter }
    }

    /// 对单个 Block 内容进行分词
    ///
    /// # 参数
    ///
    /// * `block_content` - 要分词的块内容文本
    /// * `base_global_offset` - 全局绝对偏移的基础值
    /// * `block_id` - 所属 Block 的 `RecordId`
    ///
    /// # Panics
    ///
    /// 此函数不会 panic。空字符串返回空 Vec。
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn tokenize_block(
        &self,
        block_content: &str,
        base_global_offset: u64,
        block_id: &RecordIdType,
    ) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut char_offset: u32 = 0;

        let breakpoints: Vec<usize> = self.segmenter.segment_str(block_content).collect();
        let mut prev = 0usize;
        for &bp in &breakpoints {
            if bp == 0 {
                continue;
            }
            let word = &block_content[prev..bp];
            if word.is_empty() {
                prev = bp;
                continue;
            }
            let token_type = classify_token_type(word);

            let token = Token {
                id: None,
                block_id: block_id.clone(),
                content: word.to_string(),
                token_type,
                start_char: char_offset,
                global_offset: base_global_offset + u64::from(char_offset),
                status: TokenStatus::Created,
            };

            tokens.push(token);
            char_offset += word.chars().count() as u32;
            prev = bp;
        }

        tokens
    }

    /// 批量分词多个 Block 并计算连续的全局偏移
    #[must_use]
    pub fn tokenize_blocks_batch(&self, blocks_contents: &[&str]) -> Vec<Vec<Token>> {
        let temp_block_id: RecordIdType = Thing::from(("block".to_string(), "temp".to_string()));
        let mut all_tokens = Vec::with_capacity(blocks_contents.len());
        let mut running_offset: u64 = 0;

        for content in blocks_contents {
            let tokens = self.tokenize_block(content, running_offset, &temp_block_id);
            if let Some(last_token) = tokens.last() {
                running_offset = last_token.global_offset + last_token.char_len() as u64;
            }
            all_tokens.push(tokens);
        }

        all_tokens
    }
}

/// 根据 Unicode 属性判断词元类型
fn classify_token_type(text: &str) -> TokenType {
    let mut has_letter_or_number = false;
    let mut has_punct_only = true;

    for ch in text.chars() {
        if ch.is_alphanumeric() || is_cjk_ideographic(ch) {
            has_letter_or_number = true;
            has_punct_only = false;
        } else if !ch.is_ascii_punctuation() && !is_unicode_punctuation(ch) {
            has_punct_only = false;
        }
    }

    if has_letter_or_number {
        TokenType::Word
    } else if has_punct_only && !text.is_empty() {
        TokenType::Punct
    } else {
        TokenType::Symbol
    }
}

const fn is_unicode_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation()
        || matches!(ch,
            '!' | '"' | '#' | '$' | '%' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | '-' | '.'
            | '/' | ':' | ';' | '<' | '=' | '>' | '?' | '@' | '[' | '\\' | ']' | '^' | '_'
            | '`' | '{' | '|' | '}' | '~'
            | '\u{2010}'..='\u{2027}' | '\u{2030}'..='\u{205E}' | '\u{3001}'..='\u{3003}'
            | '\u{3008}'..='\u{3011}' | '\u{3014}'..='\u{301F}'
            | '\u{FF01}'..='\u{FF0F}' | '\u{FF1A}'..='\u{FF20}' | '\u{FF3B}'..='\u{FF40}'
            | '\u{FF5B}'..='\u{FF65}'
        )
}

/// 判断字符是否为 CJK 统一表意文字（包括扩展区）
#[inline]
const fn is_cjk_ideographic(ch: char) -> bool {
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}'
        | '\u{3400}'..='\u{4DBF}'
        | '\u{20000}'..='\u{2A6DF}'
        | '\u{2A700}'..='\u{2B73F}'
        | '\u{2B740}'..='\u{2B81F}'
        | '\u{2B820}'..='\u{2CEAF}'
        | '\u{2CEB0}'..='\u{2EBEF}'
        | '\u{30000}'..='\u{3134F}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_block_id() -> RecordIdType {
        Thing::from(("block".to_string(), "temp".to_string()))
    }

    #[test]
    fn test_tokenize_english_text() {
        let tokenizer = IcuTokenizer::new();
        let tokens = tokenizer.tokenize_block("Hello", 0, &temp_block_id());
        assert!(!tokens.is_empty(), "英文文本应生成 Token");
        assert_eq!(tokens[0].content, "Hello");
        assert_eq!(tokens[0].token_type, TokenType::Word);
    }

    #[test]
    fn test_tokenize_empty_content() {
        let tokenizer = IcuTokenizer::new();
        let tokens = tokenizer.tokenize_block("", 0, &temp_block_id());
        assert!(tokens.is_empty(), "空内容应返回空列表");
    }

    #[test]
    fn test_global_offset_calculation() {
        let tokenizer = IcuTokenizer::new();
        let base_offset: u64 = 1000;
        let tokens = tokenizer.tokenize_block("Hi", base_offset, &temp_block_id());
        if !tokens.is_empty() {
            assert_eq!(
                tokens[0].global_offset, base_offset,
                "第一个词元全局偏移应为 base_offset"
            );
        }
    }

    #[test]
    fn test_classify_word() {
        assert_eq!(classify_token_type("abc"), TokenType::Word);
        assert_eq!(classify_token_type("123"), TokenType::Word);
    }

    #[test]
    fn test_classify_punct() {
        assert_eq!(classify_token_type("."), TokenType::Punct);
        assert_eq!(classify_token_type(","), TokenType::Punct);
    }

    #[test]
    fn test_classify_symbol() {
        assert_eq!(classify_token_type("→"), TokenType::Symbol);
    }

    #[test]
    fn test_is_cjk_ideographic() {
        assert!(is_cjk_ideographic('你'));
        assert!(is_cjk_ideographic('世'));
        assert!(!is_cjk_ideographic('A'));
    }

    #[test]
    fn test_word_segmenter_produces_word_level_tokens() {
        let tokenizer = IcuTokenizer::new();
        let tokens = tokenizer.tokenize_block("Hello World", 0, &temp_block_id());
        let words: Vec<&str> = tokens.iter().map(|t| t.content.as_str()).collect();
        assert!(
            words.contains(&"Hello"),
            "词级分词应产生完整单词 'Hello'，实际: {words:?}"
        );
        assert!(
            words.contains(&"World"),
            "词级分词应产生完整单词 'World'，实际: {words:?}"
        );
    }

    #[test]
    fn test_default_creates_tokenizer() {
        let tokenizer = IcuTokenizer::default();
        let tokens = tokenizer.tokenize_block("test", 0, &temp_block_id());
        assert!(!tokens.is_empty(), "Default 创建的分词器应能正常工作");
    }

    #[test]
    fn test_tokenize_blocks_batch_multiple_blocks() {
        let tokenizer = IcuTokenizer::new();
        let blocks = &["Hello world", "Second block"];
        let result = tokenizer.tokenize_blocks_batch(blocks);
        assert_eq!(result.len(), 2, "应返回两个块的 Token 列表");
        assert!(!result[0].is_empty(), "第一个块应有 Token");
        assert!(!result[1].is_empty(), "第二个块应有 Token");
    }

    #[test]
    fn test_tokenize_blocks_batch_empty_blocks() {
        let tokenizer = IcuTokenizer::new();
        let blocks: &[&str] = &[];
        let result = tokenizer.tokenize_blocks_batch(blocks);
        assert!(result.is_empty(), "空块列表应返回空结果");
    }

    #[test]
    fn test_tokenize_blocks_batch_global_offset_continuity() {
        let tokenizer = IcuTokenizer::new();
        let blocks = &["Hello", "World"];
        let result = tokenizer.tokenize_blocks_batch(blocks);
        if !result[0].is_empty() && !result[1].is_empty() {
            let last_offset_first = result[0].last().unwrap().global_offset;
            let first_offset_second = result[1][0].global_offset;
            assert!(
                first_offset_second > last_offset_first,
                "第二个块的偏移应大于第一个块的偏移"
            );
        }
    }

    #[test]
    fn test_classify_cjk_as_word() {
        assert_eq!(classify_token_type("你好"), TokenType::Word);
        assert_eq!(classify_token_type("世界"), TokenType::Word);
    }

    #[test]
    fn test_classify_mixed_alphanumeric_as_word() {
        assert_eq!(classify_token_type("abc123"), TokenType::Word);
    }

    #[test]
    fn test_classify_unicode_punctuation() {
        assert_eq!(classify_token_type("。"), TokenType::Punct);
        assert_eq!(classify_token_type("，"), TokenType::Punct);
        assert_eq!(classify_token_type("！"), TokenType::Punct);
    }

    #[test]
    fn test_classify_empty_falls_to_symbol() {
        assert_eq!(classify_token_type(""), TokenType::Symbol);
    }

    #[test]
    fn test_is_unicode_punctuation_common_chars() {
        assert!(is_unicode_punctuation('。'));
        assert!(is_unicode_punctuation('，'));
        assert!(is_unicode_punctuation('！'));
        assert!(is_unicode_punctuation('？'));
        assert!(!is_unicode_punctuation('A'));
        assert!(!is_unicode_punctuation('你'));
    }

    #[test]
    fn test_is_cjk_ideographic_extended_ranges() {
        assert!(is_cjk_ideographic('一'));
        assert!(is_cjk_ideographic('龥'));
        assert!(is_cjk_ideographic('\u{3400}'));
        assert!(!is_cjk_ideographic('a'));
        assert!(!is_cjk_ideographic('1'));
    }

    #[test]
    fn test_tokenize_block_preserves_block_id() {
        let tokenizer = IcuTokenizer::new();
        let block_id = temp_block_id();
        let tokens = tokenizer.tokenize_block("test", 0, &block_id);
        for token in &tokens {
            assert_eq!(token.block_id, block_id, "Token 的 block_id 应与输入一致");
        }
    }

    #[test]
    fn test_tokenize_blocks_batch_single_block() {
        let tokenizer = IcuTokenizer::new();
        let blocks = &["Single block content"];
        let result = tokenizer.tokenize_blocks_batch(blocks);
        assert_eq!(result.len(), 1, "单个块应返回一个 Token 列表");
        assert!(!result[0].is_empty(), "单个块应有 Token");
    }
}
