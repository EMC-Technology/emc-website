use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::LlmError;

/// 工具定义（Function Calling 的工具描述）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 工具输入参数的 JSON Schema
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// 创建新的工具定义。
    ///
    /// # Errors
    ///
    /// 当工具名称为空时返回 `LlmError::InvalidRequest`。
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Result<Self, LlmError> {
        let name = name.into();
        if name.is_empty() {
            return Err(LlmError::InvalidRequest {
                message: "tool name must not be empty".into(),
            });
        }
        Ok(Self {
            name,
            description: description.into(),
            input_schema,
        })
    }

    /// 从 JSON Schema 字符串创建工具定义。
    ///
    /// # Errors
    ///
    /// 当工具名称为空或 JSON Schema 格式无效时返回 `LlmError::InvalidRequest`。
    pub fn from_json_schema(
        name: impl Into<String> + AsRef<str>,
        description: impl Into<String>,
        schema: &str,
    ) -> Result<Self, LlmError> {
        let name_str = name.as_ref();
        if name_str.is_empty() {
            return Err(LlmError::InvalidRequest {
                message: "tool name must not be empty".into(),
            });
        }
        let input_schema: serde_json::Value =
            serde_json::from_str(schema).map_err(|e| LlmError::InvalidRequest {
                message: format!("invalid JSON schema for tool '{name_str}': {e}"),
            })?;
        Self::new(name, description, input_schema)
    }
}

/// 工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 调用唯一标识
    pub id: String,
    /// 工具名称
    pub name: String,
    /// 工具参数（JSON 字符串）
    pub arguments: String,
}

impl ToolCall {
    /// 将 `arguments` 字符串解析为 JSON 值。
    ///
    /// # Errors
    ///
    /// 当参数字符串不是合法 JSON 时返回 `serde_json::Error`。
    pub fn parse_arguments(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.arguments)
    }
}

/// 工具执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 对应的工具调用标识
    pub tool_use_id: String,
    /// 结果内容
    pub content: String,
    /// 是否为错误结果
    pub is_error: bool,
}

impl ToolResult {
    /// 创建成功结果
    pub fn success(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    /// 创建错误结果
    pub fn error(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: true,
        }
    }
}

/// 工具执行器 trait
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// 执行工具调用
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, LlmError>;
    /// 列出该执行器支持的所有工具
    fn list_tools(&self) -> Vec<ToolDefinition>;
}

/// 工具调用循环，管理多个工具执行器并按名称分发调用
pub struct ToolCallLoop {
    executors: Vec<Box<dyn ToolExecutor>>,
    max_rounds: u32,
}

impl ToolCallLoop {
    /// 创建工具调用循环
    #[must_use]
    pub fn new(executors: Vec<Box<dyn ToolExecutor>>, max_rounds: u32) -> Self {
        Self {
            executors,
            max_rounds,
        }
    }

    /// 按工具名称查找对应的执行器
    pub fn find_executor(&self, tool_name: &str) -> Option<&dyn ToolExecutor> {
        self.executors
            .iter()
            .find(|e| e.list_tools().iter().any(|t| t.name == tool_name))
            .map(std::convert::AsRef::as_ref)
    }

    /// 执行工具调用。
    ///
    /// # Errors
    ///
    /// 当未找到匹配的工具执行器或工具执行本身失败时返回 `LlmError`。
    pub async fn execute_tool(&self, call: &ToolCall) -> Result<ToolResult, LlmError> {
        let executor = self
            .find_executor(&call.name)
            .ok_or_else(|| LlmError::ToolCallError {
                message: format!("no executor found for tool '{}'", call.name),
                tool_name: Some(call.name.clone()),
            })?;
        executor.execute(call).await
    }

    /// 获取最大循环轮次
    #[must_use]
    pub fn max_rounds(&self) -> u32 {
        self.max_rounds
    }

    /// 聚合所有执行器的工具定义
    #[must_use]
    pub fn all_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.executors.iter().flat_map(|e| e.list_tools()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("call_123", "output text");
        assert_eq!(result.tool_use_id, "call_123");
        assert_eq!(result.content, "output text");
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("call_456", "something went wrong");
        assert_eq!(result.tool_use_id, "call_456");
        assert_eq!(result.content, "something went wrong");
        assert!(result.is_error);
    }

    #[test]
    fn test_tool_call_serialize() {
        let call = ToolCall {
            id: "call_1".to_string(),
            name: "read_file".to_string(),
            arguments: r#"{"path": "/tmp/test"}"#.to_string(),
        };
        let json = serde_json::to_string(&call).unwrap();
        let deserialized: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "call_1");
        assert_eq!(deserialized.name, "read_file");
    }

    struct MockExecutor {
        tools: Vec<ToolDefinition>,
    }

    #[async_trait]
    impl ToolExecutor for MockExecutor {
        async fn execute(&self, call: &ToolCall) -> Result<ToolResult, LlmError> {
            Ok(ToolResult::success(
                &call.id,
                format!("executed {}", call.name),
            ))
        }

        fn list_tools(&self) -> Vec<ToolDefinition> {
            self.tools.clone()
        }
    }

    fn make_tool_def(name: &str) -> ToolDefinition {
        ToolDefinition::new(
            name,
            format!("{name} tool"),
            serde_json::json!({"type": "object"}),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_find_executor() {
        let executor = MockExecutor {
            tools: vec![make_tool_def("read_file"), make_tool_def("write_file")],
        };
        let loop_ = ToolCallLoop::new(vec![Box::new(executor)], 5);

        assert!(loop_.find_executor("read_file").is_some());
        assert!(loop_.find_executor("write_file").is_some());
        assert!(loop_.find_executor("unknown").is_none());
    }

    #[tokio::test]
    async fn test_execute_tool() {
        let executor = MockExecutor {
            tools: vec![make_tool_def("read_file")],
        };
        let loop_ = ToolCallLoop::new(vec![Box::new(executor)], 5);

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "read_file".to_string(),
            arguments: "{}".to_string(),
        };
        let result = loop_.execute_tool(&call).await.unwrap();
        assert_eq!(result.content, "executed read_file");
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_execute_tool_not_found() {
        let executor = MockExecutor {
            tools: vec![make_tool_def("read_file")],
        };
        let loop_ = ToolCallLoop::new(vec![Box::new(executor)], 5);

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "unknown_tool".to_string(),
            arguments: "{}".to_string(),
        };
        let result = loop_.execute_tool(&call).await;
        assert!(matches!(result, Err(LlmError::ToolCallError { .. })));
    }

    #[test]
    fn test_all_tool_definitions() {
        let executor1 = MockExecutor {
            tools: vec![make_tool_def("tool_a")],
        };
        let executor2 = MockExecutor {
            tools: vec![make_tool_def("tool_b"), make_tool_def("tool_c")],
        };
        let loop_ = ToolCallLoop::new(vec![Box::new(executor1), Box::new(executor2)], 5);

        let defs = loop_.all_tool_definitions();
        assert_eq!(defs.len(), 3);
    }

    #[test]
    fn test_max_rounds() {
        let loop_ = ToolCallLoop::new(vec![], 10);
        assert_eq!(loop_.max_rounds(), 10);
    }

    #[test]
    fn test_tool_definition_new() {
        let def = ToolDefinition::new(
            "my_tool",
            "A test tool",
            serde_json::json!({"type": "object"}),
        )
        .unwrap();
        assert_eq!(def.name, "my_tool");
        assert_eq!(def.description, "A test tool");
    }

    #[test]
    fn test_tool_definition_from_json_schema() {
        let schema = r#"{"type":"object","properties":{"city":{"type":"string"}}}"#;
        let def = ToolDefinition::from_json_schema("get_weather", "Get weather for a city", schema)
            .unwrap();
        assert_eq!(def.name, "get_weather");
        assert_eq!(def.description, "Get weather for a city");
        assert!(def.input_schema["properties"]["city"].is_object());
    }

    #[test]
    fn test_tool_definition_from_json_schema_invalid() {
        let result = ToolDefinition::from_json_schema("bad_tool", "desc", "{invalid}");
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_call_parse_arguments_valid() {
        let call = ToolCall {
            id: "call_1".into(),
            name: "tool".into(),
            arguments: r#"{"city":"SF"}"#.into(),
        };
        let result = call.parse_arguments();
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["city"], "SF");
    }

    #[test]
    fn test_tool_call_parse_arguments_empty() {
        let call = ToolCall {
            id: "call_1".into(),
            name: "tool".into(),
            arguments: String::new(),
        };
        let result = call.parse_arguments();
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_call_parse_arguments_invalid() {
        let call = ToolCall {
            id: "call_1".into(),
            name: "tool".into(),
            arguments: "{invalid}".into(),
        };
        let result = call.parse_arguments();
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_result_serde_roundtrip() {
        let result = ToolResult::success("call_123", "output");
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ToolResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tool_use_id, "call_123");
        assert_eq!(deserialized.content, "output");
        assert!(!deserialized.is_error);
    }

    #[test]
    fn test_tool_definition_new_empty_name() {
        let result = ToolDefinition::new("", "desc", serde_json::json!({"type": "object"}));
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_definition_serde_roundtrip() {
        let def =
            ToolDefinition::new("tool", "desc", serde_json::json!({"type": "object"})).unwrap();
        let json = serde_json::to_string(&def).unwrap();
        let deserialized: ToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "tool");
        assert_eq!(deserialized.description, "desc");
    }
}
