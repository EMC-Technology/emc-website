use crate::error::LlmError;

use super::StreamEvent;

/// SSE 帧解析器，将字节流切分为独立的 SSE 事件帧
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    #[allow(dead_code)]
    provider: Option<String>,
    #[allow(dead_code)]
    model: Option<String>,
}

impl SseParser {
    /// 创建新的 SSE 解析器
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置供应商和模型上下文（用于错误信息）
    #[must_use]
    pub fn with_context(mut self, provider: impl Into<String>, model: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self.model = Some(model.into());
        self
    }

    /// 将数据块推入缓冲区并解析出所有完整的 SSE 事件。
    ///
    /// # Errors
    ///
    /// 当事件映射器处理帧失败时返回 `LlmError`。
    pub fn push(
        &mut self,
        chunk: &[u8],
        mapper: &dyn SseEventMapper,
    ) -> Result<Vec<StreamEvent>, LlmError> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(frame) = self.next_frame() {
            if !frame.trim().is_empty() {
                events.extend(mapper.map_frame(&frame)?);
            }
        }
        Ok(events)
    }

    /// 处理缓冲区中剩余的不完整数据。
    ///
    /// # Errors
    ///
    /// 当事件映射器处理尾部帧失败时返回 `LlmError`。
    pub fn finish(&mut self, mapper: &dyn SseEventMapper) -> Result<Vec<StreamEvent>, LlmError> {
        if self.buffer.is_empty() {
            return Ok(Vec::new());
        }
        let trailing = std::mem::take(&mut self.buffer);
        let frame = String::from_utf8_lossy(&trailing).into_owned();
        if frame.trim().is_empty() {
            return Ok(Vec::new());
        }
        mapper.map_frame(&frame)
    }

    fn next_frame(&mut self) -> Option<String> {
        let separator = self
            .buffer
            .windows(2)
            .position(|w| w == b"\n\n")
            .map(|pos| (pos, 2))
            .or_else(|| {
                self.buffer
                    .windows(4)
                    .position(|w| w == b"\r\n\r\n")
                    .map(|pos| (pos, 4))
            })?;

        let (position, separator_len) = separator;
        let frame: Vec<u8> = self.buffer.drain(..position + separator_len).collect();
        let frame_len = frame.len().saturating_sub(separator_len);
        Some(String::from_utf8_lossy(&frame[..frame_len]).into_owned())
    }
}

/// SSE 事件映射器 trait，将 SSE 帧转换为流事件
pub trait SseEventMapper: Send + Sync {
    /// 将 SSE 帧映射为流事件。
    ///
    /// # Errors
    ///
    /// 当帧解析或数据提取失败时返回 `LlmError`。
    fn map_frame(&self, frame: &str) -> Result<Vec<StreamEvent>, LlmError>;
}

/// 从 SSE 帧中提取 data 行内容，忽略 ping 事件和 [DONE] 信号
#[must_use]
pub fn extract_data_lines(frame: &str) -> Option<String> {
    let mut data_lines = Vec::new();
    let mut event_name: Option<&str> = None;

    for line in frame.trim().lines() {
        if line.starts_with(':') {
            continue;
        }
        if let Some(name) = line.strip_prefix("event:") {
            event_name = Some(name.trim());
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.trim_start());
        }
    }

    if matches!(event_name, Some("ping")) {
        return None;
    }
    if data_lines.is_empty() {
        return None;
    }

    let payload = data_lines.join("\n");
    if payload == "[DONE]" {
        return None;
    }

    Some(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopMapper;
    impl SseEventMapper for NoopMapper {
        fn map_frame(&self, _frame: &str) -> Result<Vec<StreamEvent>, LlmError> {
            Ok(Vec::new())
        }
    }

    struct EchoMapper;
    impl SseEventMapper for EchoMapper {
        fn map_frame(&self, frame: &str) -> Result<Vec<StreamEvent>, LlmError> {
            Ok(vec![StreamEvent::Text(frame.to_string())])
        }
    }

    #[test]
    fn test_sse_parser_push_single_frame() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data: hello\n\n", &EchoMapper).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_sse_parser_push_multiple_frames() {
        let mut parser = SseParser::new();
        let chunk = b"data: hello\n\ndata: world\n\n";
        let events = parser.push(chunk, &EchoMapper).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_sse_parser_push_partial_frame() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data: hel", &EchoMapper).unwrap();
        assert!(events.is_empty());
        let events = parser.push(b"lo\n\n", &EchoMapper).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_sse_parser_push_empty_frame_skipped() {
        let mut parser = SseParser::new();
        let events = parser.push(b"\n\n", &EchoMapper).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_sse_parser_finish_empty_buffer() {
        let mut parser = SseParser::new();
        let events = parser.finish(&EchoMapper).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_sse_parser_finish_trailing_data() {
        let mut parser = SseParser::new();
        parser
            .push(b"data: hello\n\ndata: trail", &NoopMapper)
            .unwrap();
        let events = parser.finish(&EchoMapper).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_sse_parser_finish_trailing_whitespace() {
        let mut parser = SseParser::new();
        parser.push(b"   \n   ", &NoopMapper).unwrap();
        let events = parser.finish(&NoopMapper).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_sse_parser_with_context() {
        let parser = SseParser::new().with_context("openai", "gpt-4o");
        assert!(parser.provider.is_some());
        assert!(parser.model.is_some());
    }

    #[test]
    fn test_sse_parser_crlf_separator() {
        let mut parser = SseParser::new();
        let events = parser.push(b"data: hello\r\n\r\n", &EchoMapper).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_extract_data_lines_basic() {
        let frame = "data: hello world\n";
        let result = extract_data_lines(frame);
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[test]
    fn test_extract_data_lines_multiple_data() {
        let frame = "data: line1\ndata: line2\n";
        let result = extract_data_lines(frame);
        assert_eq!(result, Some("line1\nline2".to_string()));
    }

    #[test]
    fn test_extract_data_lines_done_signal() {
        let frame = "data: [DONE]\n";
        let result = extract_data_lines(frame);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_data_lines_ping_event() {
        let frame = "event: ping\ndata: {}\n";
        let result = extract_data_lines(frame);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_data_lines_no_data() {
        let frame = "event: message\n";
        let result = extract_data_lines(frame);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_data_lines_comment_ignored() {
        let frame = ": this is a comment\ndata: hello\n";
        let result = extract_data_lines(frame);
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn test_extract_data_lines_event_name() {
        let frame = "event: message_start\ndata: {\"type\":\"message_start\"}\n";
        let result = extract_data_lines(frame);
        assert!(result.is_some());
    }

    #[test]
    fn test_miri_sse_parser_no_ub_on_malformed_input() {
        let cases: &[&[u8]] = &[
            b"",
            b"\n",
            b"\r\n",
            b"\n\n",
            b"\r\n\r\n",
            b"\x00\x00\xFF\xFE",
            b"data: \xC0\xC0\n\n",
            b"data: hello\r\n",
            b"data: hello\n\r\n",
        ];
        for &chunk in cases {
            let mut parser = SseParser::new();
            let _ = parser.push(chunk, &NoopMapper);
            let _ = parser.finish(&NoopMapper);
        }
    }

    #[test]
    fn test_miri_extract_data_lines_no_ub() {
        let cases: &[&str] = &[
            "",
            "\n",
            "\r\n",
            "data:",
            "event:",
            ":",
            "data: \x00\x01",
            "event: ping\ndata: {}",
            "data: [DONE]",
            "event: message\ndata: hello",
        ];
        for &input in cases {
            let _ = extract_data_lines(input);
        }
    }

    #[test]
    fn test_deductive_extract_data_lines_ping_always_none() {
        let ping_variants: &[&str] = &[
            "event: ping\ndata: {}",
            "event:ping\ndata: {}",
            "event: ping\ndata: keepalive",
        ];
        for &frame in ping_variants {
            assert!(
                extract_data_lines(frame).is_none(),
                "ping events MUST always return None: frame={frame:?}"
            );
        }
    }

    #[test]
    fn test_deductive_extract_data_lines_done_always_none() {
        assert!(
            extract_data_lines("data: [DONE]").is_none(),
            "[DONE] signal MUST return None"
        );
    }

    #[test]
    fn test_deductive_extract_data_lines_no_data_implies_none() {
        let no_data_frames: &[&str] = &["event: message", ": comment only", "id: 123", ""];
        for &frame in no_data_frames {
            assert!(
                extract_data_lines(frame).is_none(),
                "frames without data lines MUST return None: frame={frame:?}"
            );
        }
    }

    #[test]
    fn test_deductive_sse_parser_finish_clears_buffer() {
        let mut parser = SseParser::new();
        parser
            .push(b"data: hello\n\ndata: trail", &NoopMapper)
            .unwrap();
        let _ = parser.finish(&NoopMapper);
        let events = parser.finish(&NoopMapper).unwrap();
        assert!(
            events.is_empty(),
            "finish() on already-finished parser MUST return empty"
        );
    }

    #[test]
    fn test_deductive_sse_parser_next_frame_consumed() {
        let mut parser = SseParser::new();
        let events1 = parser.push(b"data: hello\n\n", &EchoMapper).unwrap();
        assert_eq!(events1.len(), 1);
        let events2 = parser.push(b"", &EchoMapper).unwrap();
        assert!(
            events2.is_empty(),
            "pushing empty chunk after consumed frame MUST yield no events"
        );
    }
}
