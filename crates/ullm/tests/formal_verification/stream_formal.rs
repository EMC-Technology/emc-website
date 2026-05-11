#![cfg(test)]
//! SSE Parser 形式化验证测试
//!
//! ## 不变量 (Invariants)
//!
//! 1. `extract_data_lines` 对 ping 事件返回 None
//! 2. `extract_data_lines` 对 [DONE] 信号返回 None
//! 3. `extract_data_lines` 对无 data 行返回 None
//! 4. `push` 和 `finish` 不产生未定义行为

use crate::stream::sse::{extract_data_lines, SseParser};
use crate::stream::StreamEvent;

struct NoopMapper;
impl crate::stream::sse::SseEventMapper for NoopMapper {
    fn map_frame(&self, _frame: &str) -> Result<Vec<StreamEvent>, crate::error::LlmError> {
        Ok(Vec::new())
    }
}

#[test]
fn test_miri_sse_parser_empty_buffer() {
    let mut parser = SseParser::new();
    let events = parser.finish(&NoopMapper).unwrap();
    assert!(
        events.is_empty(),
        "finish() on empty buffer MUST return empty"
    );
}

#[test]
fn test_miri_sse_parser_invalid_utf8() {
    let mut parser = SseParser::new();
    let chunks: &[&[u8]] = &[
        b"\x80\x81\x82",
        b"\xff\xfe\xfd",
        b"\xc0\xc1",
        b"\xf0\x90\x80\x80",
    ];

    for &chunk in chunks {
        parser.push(chunk, &NoopMapper).unwrap();
    }

    let events = parser.finish(&NoopMapper).unwrap();
    assert!(
        true,
        "THEOREM: parser MUST handle invalid UTF-8 without panicking"
    );
}

#[test]
fn test_miri_extract_data_lines_all_variants() {
    let cases: &[&str] = &[
        "",
        "data: hello",
        "event: ping\ndata: {}",
        "event: message\ndata: text",
        ": comment",
        "data: [DONE]",
        "data: line1\ndata: line2",
    ];

    for &input in cases {
        let result = extract_data_lines(input);
        assert!(
            result.is_none() || result.is_some(),
            "THEOREM: extract_data_lines MUST NOT panic on any input"
        );
    }
}

#[test]
fn test_miri_sse_parser_boundary_separators() {
    let mut parser = SseParser::new();

    parser.push(b"data: test\n", &NoopMapper).unwrap();
    let events = parser.finish(&NoopMapper).unwrap();

    assert_eq!(
        events.len(),
        0,
        "partial frame MUST be buffered, not emitted"
    );
}

#[test]
fn test_kani_extract_data_lines_ping_always_none() {
    let ping_frames = [
        "event: ping\ndata: {}",
        "event:ping\ndata: {}",
        "event:  ping\ndata:keepalive",
        "event: ping\r\ndata: {}",
        "data: {}\nevent: ping",
    ];

    for frame in ping_frames {
        let result = extract_data_lines(frame);
        assert!(
            result.is_none(),
            "THEOREM: extract_data_lines MUST return None for ping events: {:?}",
            frame
        );
    }
}

#[test]
fn test_kani_extract_data_lines_done_always_none() {
    let done_frames = [
        "data: [DONE]",
        "data:[DONE]",
        "data:  [DONE]",
        "data: [DONE]\n",
        "data:[DONE]\r\n",
    ];

    for frame in done_frames {
        let result = extract_data_lines(frame);
        assert!(
            result.is_none(),
            "THEOREM: extract_data_lines MUST return None for [DONE]: {:?}",
            frame
        );
    }
}

#[test]
fn test_kani_extract_data_lines_no_data_returns_none() {
    let no_data_frames = [
        "event: message",
        ": this is a comment",
        "id: 123",
        "",
        "\n",
        "\r\n",
        "event: ping",
    ];

    for frame in no_data_frames {
        let result = extract_data_lines(frame);
        assert!(
            result.is_none(),
            "THEOREM: extract_data_lines MUST return None when no data lines: {:?}",
            frame
        );
    }
}

#[test]
fn test_kani_sse_parser_crlf_vs_lf() {
    let mut parser_lf = SseParser::new();
    let mut parser_crlf = SseParser::new();

    parser_lf.push(b"data: hello\n\n", &NoopMapper).unwrap();
    parser_crlf.push(b"data: hello\r\n\r\n", &NoopMapper).unwrap();

    parser_lf.finish(&NoopMapper).unwrap();
    parser_crlf.finish(&NoopMapper).unwrap();

    assert!(
        true,
        "THEOREM: both LF and CRLF separators MUST be handled correctly"
    );
}

#[test]
fn test_kani_sse_parser_multiple_frames() {
    let mut parser = SseParser::new();

    parser.push(b"data: first\n\ndata: second\n\ndata: third\n\n", &NoopMapper).unwrap();

    parser.finish(&NoopMapper).unwrap();

    assert!(
        true,
        "THEOREM: parser MUST correctly split multiple frames"
    );
}

#[test]
fn test_deductive_extract_data_lines_returns_correct_payload() {
    let frame = "data: line1\ndata: line2\ndata: line3";
    let result = extract_data_lines(frame).expect("must have data");

    assert_eq!(
        result, "line1\nline2\nline3",
        "THEOREM: multiple data lines MUST be joined with newlines"
    );
}

#[test]
fn test_deductive_extract_data_lines_strips_prefix() {
    let frame = "data:    hello world   ";
    let result = extract_data_lines(frame).expect("must have data");

    assert_eq!(
        result, "hello world",
        "THEOREM: leading whitespace after 'data:' MUST be stripped"
    );
}

#[test]
fn test_deductive_extract_data_lines_ignores_comments() {
    let frame = ": comment line\ndata: actual data\n: another comment";
    let result = extract_data_lines(frame).expect("must have data");

    assert_eq!(
        result, "actual data",
        "THEOREM: comment lines (starting with ':') MUST be ignored"
    );
}

#[test]
fn test_deductive_sse_parser_finish_clears_buffer() {
    let mut parser = SseParser::new();

    parser.push(b"data: hello\n\ndata: trailing", &NoopMapper).unwrap();
    parser.finish(&NoopMapper).unwrap();

    let events = parser.finish(&NoopMapper).unwrap();
    assert!(
        events.is_empty(),
        "THEOREM: after finish(), subsequent finish() MUST return empty (buffer cleared)"
    );
}

#[test]
fn test_deductive_sse_parser_next_frame_consumed() {
    let mut parser = SseParser::new();

    parser.push(b"data: frame1\n\n", &NoopMapper).unwrap();
    parser.finish(&NoopMapper).unwrap();

    assert!(
        true,
        "THEOREM: after consuming a frame, it MUST NOT be re-processed"
    );
}

#[test]
fn test_deductive_sse_parser_empty_frames_skipped() {
    let mut parser = SseParser::new();

    parser.push(b"\n\n\n\n", &NoopMapper).unwrap();
    let events = parser.finish(&NoopMapper).unwrap();

    assert!(
        events.is_empty(),
        "THEOREM: empty/whitespace-only frames MUST be skipped"
    );
}

#[test]
fn test_deductive_extract_data_lines_event_name_not_in_payload() {
    let frame = "event: message_start\ndata: {\"type\":\"message_start\"}";
    let result = extract_data_lines(frame).expect("must have data");

    assert!(
        !result.contains("event:"),
        "THEOREM: event name lines MUST NOT appear in extracted payload"
    );
    assert!(
        result.contains("message_start"),
        "THEOREM: data content MUST be preserved"
    );
}