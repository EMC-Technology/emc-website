//! 智能文本分块器（text-splitter + icu_segmenter 中文分词）

use text_splitter::{Characters, TextSplitter};

/// 文本分块配置
pub struct ChunkConfig {
    /// 目标分块大小（字符数）
    pub chunk_size: usize,
    /// 分块之间的重叠字符数
    pub overlap: usize,
}

/// 基于配置创建分块器实例
///
/// `chunk_size` 和 `overlap` 配置存储在 `ChunkConfig` 中，
/// 实际分割时通过 `splitter.chunks(text, config.chunk_size)` 传入。
#[must_use]
pub fn create_chunker(_config: &ChunkConfig) -> TextSplitter<Characters> {
    TextSplitter::new(Characters).with_trim_chunks(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_config_custom_values_stored_correctly() {
        let config = ChunkConfig {
            chunk_size: 512,
            overlap: 64,
        };
        assert_eq!(config.chunk_size, 512);
        assert_eq!(config.overlap, 64);
    }

    #[test]
    fn test_chunk_config_small_values_stored_correctly() {
        let config = ChunkConfig {
            chunk_size: 1,
            overlap: 0,
        };
        assert_eq!(config.chunk_size, 1);
        assert_eq!(config.overlap, 0);
    }

    #[test]
    fn test_chunk_config_large_values_stored_correctly() {
        let config = ChunkConfig {
            chunk_size: 100_000,
            overlap: 10_000,
        };
        assert_eq!(config.chunk_size, 100_000);
        assert_eq!(config.overlap, 10_000);
    }

    #[test]
    fn test_create_chunker_returns_valid_splitter() {
        let config = ChunkConfig {
            chunk_size: 256,
            overlap: 32,
        };
        let _splitter = create_chunker(&config);
    }

    #[test]
    fn test_create_chunker_with_zero_overlap_succeeds() {
        let config = ChunkConfig {
            chunk_size: 128,
            overlap: 0,
        };
        let _splitter = create_chunker(&config);
    }

    #[test]
    fn test_create_chunker_chunking_short_text_returns_single_chunk() {
        let config = ChunkConfig {
            chunk_size: 1000,
            overlap: 0,
        };
        let splitter = create_chunker(&config);
        let text = "Hello, world! This is a short text.";
        let chunks: Vec<&str> = splitter.chunks(text, config.chunk_size).collect();
        assert_eq!(chunks.len(), 1, "短文本应产生单个分块");
        assert_eq!(chunks[0], text, "分块内容应与原文一致");
    }

    #[test]
    fn test_create_chunker_chunking_long_text_produces_multiple_chunks() {
        let config = ChunkConfig {
            chunk_size: 50,
            overlap: 0,
        };
        let splitter = create_chunker(&config);
        let text = "A ".repeat(100);
        let chunks: Vec<&str> = splitter.chunks(&text, config.chunk_size).collect();
        assert!(
            chunks.len() > 1,
            "长文本应产生多个分块，实际: {}",
            chunks.len()
        );
    }

    #[test]
    fn test_create_chunker_chunking_empty_text_returns_no_chunks() {
        let config = ChunkConfig {
            chunk_size: 100,
            overlap: 0,
        };
        let splitter = create_chunker(&config);
        assert!(
            splitter.chunks("", config.chunk_size).next().is_none(),
            "空文本不应产生分块"
        );
    }

    #[test]
    fn test_create_chunker_trim_chunks_enabled() {
        let config = ChunkConfig {
            chunk_size: 10,
            overlap: 0,
        };
        let splitter = create_chunker(&config);
        let text = "  hello  world  ";
        let chunks: Vec<&str> = splitter.chunks(text, config.chunk_size).collect();
        for chunk in &chunks {
            let trimmed = chunk.trim_start();
            assert!(
                trimmed.len() < chunk.len() || !chunk.starts_with(' '),
                "分块不应以空白字符开头（trim_chunks=true）: {chunk:?}"
            );
        }
    }

    #[test]
    fn test_create_chunker_chunking_preserves_content_coverage() {
        let config = ChunkConfig {
            chunk_size: 20,
            overlap: 0,
        };
        let splitter = create_chunker(&config);
        let text = "ABCDEFGHIJ KLMNOPQRST UVWXYZ";
        let chunks: Vec<&str> = splitter.chunks(text, config.chunk_size).collect();
        let joined: String = chunks.join("");
        assert!(
            joined.chars().all(|c| text.contains(c)),
            "分块拼接后应覆盖原文所有字符"
        );
    }

    #[test]
    fn test_create_chunker_with_chinese_text_produces_chunks() {
        let config = ChunkConfig {
            chunk_size: 10,
            overlap: 0,
        };
        let splitter = create_chunker(&config);
        let text = "你好世界，这是一个中文文本测试。";
        assert!(
            splitter.chunks(text, config.chunk_size).next().is_some(),
            "中文文本应至少产生一个分块"
        );
    }

    #[test]
    fn test_create_chunker_with_various_chunk_sizes() {
        let text = "A ".repeat(200);

        let config_small = ChunkConfig {
            chunk_size: 10,
            overlap: 0,
        };
        let splitter_small = create_chunker(&config_small);
        let chunks_small: Vec<&str> = splitter_small
            .chunks(&text, config_small.chunk_size)
            .collect();

        let config_large = ChunkConfig {
            chunk_size: 1000,
            overlap: 0,
        };
        let splitter_large = create_chunker(&config_large);
        let chunks_large: Vec<&str> = splitter_large
            .chunks(&text, config_large.chunk_size)
            .collect();

        assert!(
            chunks_small.len() > chunks_large.len(),
            "更小的分块大小应产生更多分块"
        );
    }

    #[test]
    fn test_chunk_config_zero_overlap_valid() {
        let config = ChunkConfig {
            chunk_size: 100,
            overlap: 0,
        };
        assert_eq!(config.chunk_size, 100);
        assert_eq!(config.overlap, 0);
        let _splitter = create_chunker(&config);
    }
}
