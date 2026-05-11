#![cfg(test)]
//! Tool 模块形式化验证测试
//!
//! ## 不变量 (Invariants)
//!
//! 1. `ToolDefinition::new` 拒绝空名称
//! 2. `ToolCallLoop::find_executor` 正确查找
//! 3. `ToolResult` 正确区分成功/错误

use crate::tool::{ToolCall, ToolCallLoop, ToolDefinition, ToolExecutor, ToolResult};
use crate::error::LlmError;
use async_trait::async_trait;
use std::sync::Arc;

struct MockExecutor {
    tools: Vec<ToolDefinition>,
}

#[async_trait]
impl ToolExecutor for MockExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, LlmError> {
        Ok(ToolResult::success(&call.id, format!("executed {}", call.name)))
    }

    fn list_tools(&self) -> Vec<ToolDefinition> {
        self.tools.clone()
    }
}

fn make_tool(name: &str) -> ToolDefinition {
    ToolDefinition::new(
        name,
        format!("{} description", name),
        serde_json::json!({"type": "object"}),
    )
    .unwrap()
}

#[test]
fn test_miri_tool_definition_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ToolDefinition>();
    assert_send_sync::<ToolCall>();
    assert_send_sync::<ToolResult>();
    assert_send_sync::<ToolCallLoop>();
}

#[test]
fn test_miri_tool_result_success_vs_error() {
    let success = ToolResult::success("call_1", "result");
    let error = ToolResult::error("call_1", "error message");

    assert!(
        !success.is_error,
        "THEOREM: success result MUST have is_error = false"
    );
    assert!(
        error.is_error,
        "THEOREM: error result MUST have is_error = true"
    );
}

#[test]
fn test_kani_tool_definition_empty_name_rejected() {
    let result = ToolDefinition::new(
        "",
        "description",
        serde_json::json!({"type": "object"}),
    );
    assert!(
        result.is_err(),
        "THEOREM: ToolDefinition::new MUST reject empty name"
    );
}

#[test]
fn test_kani_tool_definition_from_json_schema() {
    let schema = r#"{"type":"object","properties":{"city":{"type":"string"}}}"#;
    let result = ToolDefinition::from_json_schema("get_weather", "Get weather", schema);

    assert!(
        result.is_ok(),
        "THEOREM: valid JSON schema MUST succeed"
    );

    let def = result.unwrap();
    assert_eq!(
        def.name, "get_weather",
        "THEOREM: name MUST be set correctly"
    );
    assert!(
        def.input_schema.is_object(),
        "THEOREM: input_schema MUST be valid JSON object"
    );
}

#[test]
fn test_kani_tool_call_loop_find_executor() {
    let executor = MockExecutor {
        tools: vec![make_tool("read_file"), make_tool("write_file")],
    };
    let loop_ = ToolCallLoop::new(vec![Box::new(executor)], 5);

    assert!(
        loop_.find_executor("read_file").is_some(),
        "THEOREM: find_executor MUST find registered tool"
    );
    assert!(
        loop_.find_executor("write_file").is_some(),
        "THEOREM: find_executor MUST find second registered tool"
    );
    assert!(
        loop_.find_executor("unknown_tool").is_none(),
        "THEOREM: find_executor MUST return None for unknown tool"
    );
}

#[test]
fn test_kani_tool_call_loop_execute_found_tool() {
    let executor = MockExecutor {
        tools: vec![make_tool("echo")],
    };
    let loop_ = ToolCallLoop::new(vec![Box::new(executor)], 5);

    let call = ToolCall {
        id: "call_1".into(),
        name: "echo".into(),
        arguments: "{}".into(),
    };

    let result = loop_.execute_tool(&call);
    assert!(
        result.is_ok(),
        "THEOREM: execute_tool MUST succeed for registered tool"
    );
}

#[test]
fn test_kani_tool_call_loop_execute_unknown_tool() {
    let executor = MockExecutor {
        tools: vec![make_tool("known_tool")],
    };
    let loop_ = ToolCallLoop::new(vec![Box::new(executor)], 5);

    let call = ToolCall {
        id: "call_1".into(),
        name: "unknown_tool".into(),
        arguments: "{}".into(),
    };

    let result = loop_.execute_tool(&call);
    assert!(
        result.is_err(),
        "THEOREM: execute_tool MUST fail for unknown tool"
    );
    assert!(
        matches!(result.unwrap_err(), LlmError::ToolCallError { .. }),
        "THEOREM: unknown tool MUST return ToolCallError"
    );
}

#[test]
fn test_kani_tool_call_parse_arguments() {
    let call = ToolCall {
        id: "call_1".into(),
        name: "test".into(),
        arguments: r#"{"key":"value","num":42}"#.into(),
    };

    let parsed = call.parse_arguments();
    assert!(
        parsed.is_ok(),
        "THEOREM: valid JSON arguments MUST parse successfully"
    );

    let value = parsed.unwrap();
    assert_eq!(value["key"], "value");
    assert_eq!(value["num"], 42);
}

#[test]
fn test_kani_tool_call_parse_invalid_arguments() {
    let call = ToolCall {
        id: "call_1".into(),
        name: "test".into(),
        arguments: "{invalid json}".into(),
    };

    let parsed = call.parse_arguments();
    assert!(
        parsed.is_err(),
        "THEOREM: invalid JSON arguments MUST return error"
    );
}

#[test]
fn test_deductive_tool_call_loop_all_tool_definitions() {
    let executor1 = MockExecutor {
        tools: vec![make_tool("tool_a")],
    };
    let executor2 = MockExecutor {
        tools: vec![make_tool("tool_b"), make_tool("tool_c")],
    };
    let loop_ = ToolCallLoop::new(vec![Box::new(executor1), Box::new(executor2)], 5);

    let defs = loop_.all_tool_definitions();
    assert_eq!(
        defs.len(),
        3,
        "THEOREM: all_tool_definitions MUST return tools from ALL executors"
    );
}

#[test]
fn test_deductive_tool_call_loop_max_rounds() {
    let loop_ = ToolCallLoop::new(vec![], 10);
    assert_eq!(
        loop_.max_rounds(),
        10,
        "THEOREM: max_rounds MUST return configured value"
    );
}

#[test]
fn test_deductive_tool_result_fields() {
    let success = ToolResult::success("call_123", "output text");
    assert_eq!(
        success.tool_use_id, "call_123",
        "THEOREM: success result MUST have correct tool_use_id"
    );
    assert_eq!(
        success.content, "output text",
        "THEOREM: success result MUST have correct content"
    );
    assert!(
        !success.is_error,
        "THEOREM: success result MUST have is_error = false"
    );

    let error = ToolResult::error("call_456", "error occurred");
    assert_eq!(
        error.tool_use_id, "call_456",
        "THEOREM: error result MUST have correct tool_use_id"
    );
    assert_eq!(
        error.content, "error occurred",
        "THEOREM: error result MUST have correct content"
    );
    assert!(
        error.is_error,
        "THEOREM: error result MUST have is_error = true"
    );
}

#[test]
fn test_deductive_tool_definition_clone_independence() {
    let def1 = make_tool("test_tool");
    let def2 = def1.clone();

    assert_eq!(
        def1.name, def2.name,
        "THEOREM: cloned ToolDefinition MUST have same name"
    );
    assert_eq!(
        def1.description, def2.description,
        "THEOREM: cloned ToolDefinition MUST have same description"
    );
}

#[test]
fn test_deductive_tool_call_loop_empty_executors() {
    let loop_ = ToolCallLoop::new(vec![], 5);

    assert!(
        loop_.find_executor("any_tool").is_none(),
        "THEOREM: empty executor list MUST return None for any tool lookup"
    );

    assert!(
        loop_.all_tool_definitions().is_empty(),
        "THEOREM: empty executor list MUST return empty tool definitions"
    );
}