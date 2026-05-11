/// 修复流式传输中不完整的 JSON（补齐缺失的引号、括号等）
#[must_use]
pub fn fix_streamed_json(partial_json: &str) -> String {
    let json = strip_trailing_incomplete_escape(partial_json);
    fix_json(json)
}

fn strip_trailing_incomplete_escape(json: &str) -> &str {
    let trailing_backslashes = json
        .as_bytes()
        .iter()
        .rev()
        .take_while(|&&b| b == b'\\')
        .count();
    if trailing_backslashes % 2 == 1 {
        &json[..json.len() - 1]
    } else {
        json
    }
}

fn fix_json(json: &str) -> String {
    if serde_json::from_str::<serde_json::Value>(json).is_ok() {
        return json.to_string();
    }

    let mut result = json.to_string();
    let mut open_stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escape_next = false;

    for ch in result.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if ch == '\\' {
            escape_next = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match ch {
            '{' | '[' => open_stack.push(ch),
            '}' if open_stack.last() == Some(&'{') => {
                open_stack.pop();
            }
            ']' if open_stack.last() == Some(&'[') => {
                open_stack.pop();
            }
            _ => {}
        }
    }

    if in_string {
        result.push('"');
    }
    while let Some(opener) = open_stack.pop() {
        result.push(match opener {
            '{' => '}',
            '[' => ']',
            _ => unreachable!(),
        });
    }
    result
}

/// 解析工具调用的参数字符串为 JSON 值。
///
/// # Errors
///
/// 当参数字符串不是合法 JSON 时返回 `serde_json::Error`。
pub fn parse_tool_arguments(arguments: &str) -> Result<serde_json::Value, serde_json::Error> {
    if arguments.is_empty() {
        Ok(serde_json::Value::Object(serde_json::Map::default()))
    } else {
        serde_json::from_str(arguments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_streamed_json_valid_json() {
        let result = fix_streamed_json(r#"{"key":"value"}"#);
        assert_eq!(result, r#"{"key":"value"}"#);
    }

    #[test]
    fn test_fix_streamed_json_missing_closing_brace() {
        let result = fix_streamed_json(r#"{"key":"value"#);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn test_fix_streamed_json_multiple_missing_braces() {
        let result = fix_streamed_json(r#"{"outer":{"inner":"val"#);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["outer"]["inner"], "val");
    }

    #[test]
    fn test_strip_trailing_incomplete_escape_odd() {
        let result = strip_trailing_incomplete_escape(r"test\");
        assert_eq!(result, "test");
    }

    #[test]
    fn test_strip_trailing_incomplete_escape_even() {
        let result = strip_trailing_incomplete_escape(r"test\\");
        assert_eq!(result, "test\\\\");
    }

    #[test]
    fn test_strip_trailing_incomplete_escape_none() {
        let result = strip_trailing_incomplete_escape("test");
        assert_eq!(result, "test");
    }

    #[test]
    fn test_parse_tool_arguments_empty() {
        let result = parse_tool_arguments("").unwrap();
        assert!(result.is_object());
        assert!(result.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_parse_tool_arguments_valid() {
        let result = parse_tool_arguments(r#"{"city":"SF"}"#).unwrap();
        assert_eq!(result["city"], "SF");
    }

    #[test]
    fn test_parse_tool_arguments_invalid() {
        let result = parse_tool_arguments("{invalid}");
        assert!(result.is_err());
    }

    #[test]
    fn test_fix_json_already_valid() {
        let result = fix_json(r#"{"a":1}"#);
        assert_eq!(result, r#"{"a":1}"#);
    }

    #[test]
    fn test_fix_json_unclosed_string() {
        let result = fix_json(r#"{"key":"value"#);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn test_fix_json_unclosed_bracket() {
        let result = fix_json(r#"{"arr":[1,2"#);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["arr"][0], 1);
        assert_eq!(parsed["arr"][1], 2);
    }

    #[test]
    fn test_fix_json_mixed_unclosed_brackets_and_braces() {
        let result = fix_json(r#"{"arr":[{"key":"val"#);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["arr"][0]["key"], "val");
    }

    #[test]
    fn test_miri_fix_streamed_json_no_ub() {
        let cases: &[&str] = &[
            r#"{""#,
            r"{",
            r#"{"key":"#,
            r#"{"key":#,
            r"[",
            r#"[1,2,"#,
            r#"{"a":\#,
            r#"{"a":\\#,
            r#"{"a":"\#,
            r#"{"a":"\\#,
            "",
            "{}",
            "[]",
            r#"{"k":"v"}"#,
            r#"{"a":[1,2,3"#,
            r#"{"a":{"b":"c"#,
            r#"{"a":"b\#,
            r#"{"a":"b\\#,
            r#"{"a":"b\\\"#,
        ];
        for &input in cases {
            let _ = fix_streamed_json(input);
        }
    }

    #[test]
    fn test_miri_strip_trailing_incomplete_escape_no_ub() {
        let cases: &[&str] = &[r"\", r"\\", r"\\\", r"\\\\", "", "a", r"a\", r"a\\"];
        for &input in cases {
            let _ = strip_trailing_incomplete_escape(input);
        }
    }

    #[test]
    fn test_miri_parse_tool_arguments_no_ub() {
        let cases: &[&str] = &[
            "",
            "{}",
            r#"{"a":1}"#,
            "{invalid}",
            r#"{"a","#,
            "null",
            "[]",
        ];
        for &input in cases {
            let _ = parse_tool_arguments(input);
        }
    }

    #[test]
    fn test_kani_fix_json_produces_valid_json() {
        let cases: &[&str] = &[
            r#"{"key":"value"#,
            r#"{"outer":{"inner":"val"#,
            r#"{"arr":[1,2"#,
            r#"{"arr":[{"key":"val"#,
            r#"{"a":"b"#,
            r"{",
            r"[",
            r#"{"k":"v"}"#,
            "[]",
            "{}",
        ];
        for &input in cases {
            let result = fix_json(input);
            assert!(
                serde_json::from_str::<serde_json::Value>(&result).is_ok(),
                "fix_json must always produce valid JSON for input {input:?}, got: {result}"
            );
        }
    }

    #[test]
    fn test_deductive_fix_json_idempotent_on_valid_input() {
        let valid_cases: &[&str] = &[
            r#"{"key":"value"}"#,
            "{}",
            "[]",
            r#"{"a":1,"b":2}"#,
            r#"{"arr":[1,2,3]}"#,
        ];
        for &input in valid_cases {
            let first = fix_json(input);
            let second = fix_json(&first);
            assert_eq!(first, second, "fix_json must be idempotent on valid JSON");
        }
    }

    #[test]
    fn test_deductive_fix_json_monotone_brace_balance() {
        let cases: &[&str] = &[
            r#"{"key":"value"#,
            r#"{"outer":{"inner":"val"#,
            r#"{"arr":[{"key":"val"#,
        ];
        for &input in cases {
            let result = fix_json(input);
            let open_braces = result.chars().filter(|&c| c == '{').count();
            let close_braces = result.chars().filter(|&c| c == '}').count();
            assert_eq!(
                open_braces, close_braces,
                "fix_json must balance braces: input={input:?}, result={result:?}"
            );
            let open_brackets = result.chars().filter(|&c| c == '[').count();
            let close_brackets = result.chars().filter(|&c| c == ']').count();
            assert_eq!(
                open_brackets, close_brackets,
                "fix_json must balance brackets: input={input:?}, result={result:?}"
            );
        }
    }

    #[test]
    fn test_deductive_strip_trailing_incomplete_escape_even_backslashes_unchanged() {
        let even_cases: &[&str] = &[r"test\\", r"test\\\\", "test", ""];
        for &input in even_cases {
            let result = strip_trailing_incomplete_escape(input);
            assert_eq!(result, input, "even backslashes must be unchanged");
        }
    }

    #[test]
    fn test_deductive_strip_trailing_incomplete_escape_odd_backslashes_stripped() {
        let odd_cases: &[&str] = &[r"test\", r"test\\\"];
        for &input in odd_cases {
            let result = strip_trailing_incomplete_escape(input);
            assert_ne!(result, input, "odd backslashes must be stripped");
            assert!(
                result.len() < input.len(),
                "stripped result must be shorter"
            );
        }
    }
}
