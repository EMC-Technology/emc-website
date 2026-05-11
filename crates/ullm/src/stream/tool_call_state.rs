use std::collections::BTreeMap;

use crate::tool::ToolCall;

use super::StreamEvent;

/// 单个工具调用的状态追踪器
#[derive(Debug, Default)]
pub struct ToolCallState {
    #[allow(dead_code)]
    openai_index: u32,
    id: Option<String>,
    name: Option<String>,
    arguments: String,
    #[allow(dead_code)]
    emitted_len: usize,
    started: bool,
}

impl ToolCallState {
    /// 摄入工具调用增量数据
    pub fn ingest_delta(
        &mut self,
        index: u32,
        id: Option<String>,
        name: Option<String>,
        arguments_delta: Option<String>,
    ) -> Option<StreamEvent> {
        if !self.started {
            self.started = true;
            self.openai_index = index;
        }
        if let Some(id) = id {
            self.id = Some(id);
        }
        if let Some(name) = name {
            self.name = Some(name);
        }
        if let Some(delta) = arguments_delta {
            self.arguments.push_str(&delta);
        }

        None
    }

    /// 完成工具调用，返回完整的 `ToolCall`
    #[must_use]
    pub fn finalize(&self) -> Option<ToolCall> {
        let id = self.id.clone()?;
        let name = self.name.clone()?;
        Some(ToolCall {
            id,
            name,
            arguments: self.arguments.clone(),
        })
    }
}

/// 多工具调用累积器，按索引追踪并发的工具调用
#[derive(Debug, Default)]
pub struct ToolCallAccumulator {
    tool_calls: BTreeMap<u32, ToolCallState>,
}

impl ToolCallAccumulator {
    /// 摄入工具调用增量数据到指定索引
    pub fn ingest(
        &mut self,
        index: u32,
        id: Option<String>,
        name: Option<String>,
        arguments_delta: Option<String>,
    ) {
        let state = self.tool_calls.entry(index).or_default();
        state.ingest_delta(index, id, name, arguments_delta);
    }

    /// 完成所有工具调用并清空内部状态
    pub fn finalize_all(&mut self) -> Vec<ToolCall> {
        let calls: Vec<ToolCall> = self
            .tool_calls
            .values()
            .filter_map(ToolCallState::finalize)
            .collect();
        self.tool_calls.clear();
        calls
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_call_state_ingest_delta_first_call() {
        let mut state = ToolCallState::default();
        assert!(!state.started);
        let result = state.ingest_delta(
            0,
            Some("call_1".into()),
            Some("get_weather".into()),
            Some(r#"{"city":"SF"}"#.into()),
        );
        assert!(result.is_none());
        assert!(state.started);
        assert_eq!(state.id, Some("call_1".into()));
        assert_eq!(state.name, Some("get_weather".into()));
        assert_eq!(state.arguments, r#"{"city":"SF"}"#);
    }

    #[test]
    fn test_tool_call_state_ingest_delta_subsequent() {
        let mut state = ToolCallState::default();
        state.ingest_delta(0, Some("call_1".into()), Some("tool".into()), None);
        state.ingest_delta(0, None, None, Some(r#"{"a":1}"#.into()));
        assert_eq!(state.arguments, r#"{"a":1}"#);
    }

    #[test]
    fn test_tool_call_state_finalize_complete() {
        let mut state = ToolCallState::default();
        state.ingest_delta(
            0,
            Some("call_1".into()),
            Some("tool".into()),
            Some("{}".into()),
        );
        let call = state.finalize().unwrap();
        assert_eq!(call.id, "call_1");
        assert_eq!(call.name, "tool");
        assert_eq!(call.arguments, "{}");
    }

    #[test]
    fn test_tool_call_state_finalize_missing_id() {
        let mut state = ToolCallState::default();
        state.ingest_delta(0, None, Some("tool".into()), Some("{}".into()));
        assert!(state.finalize().is_none());
    }

    #[test]
    fn test_tool_call_state_finalize_missing_name() {
        let mut state = ToolCallState::default();
        state.ingest_delta(0, Some("call_1".into()), None, Some("{}".into()));
        assert!(state.finalize().is_none());
    }

    #[test]
    fn test_tool_call_accumulator_single_tool() {
        let mut acc = ToolCallAccumulator::default();
        acc.ingest(
            0,
            Some("c1".into()),
            Some("tool_a".into()),
            Some("{}".into()),
        );
        let calls = acc.finalize_all();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "tool_a");
    }

    #[test]
    fn test_tool_call_accumulator_multiple_tools() {
        let mut acc = ToolCallAccumulator::default();
        acc.ingest(
            0,
            Some("c1".into()),
            Some("tool_a".into()),
            Some("{}".into()),
        );
        acc.ingest(
            1,
            Some("c2".into()),
            Some("tool_b".into()),
            Some("{}".into()),
        );
        let calls = acc.finalize_all();
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn test_tool_call_accumulator_incremental_args() {
        let mut acc = ToolCallAccumulator::default();
        acc.ingest(
            0,
            Some("c1".into()),
            Some("tool".into()),
            Some(r#"{"a":"#.into()),
        );
        acc.ingest(0, None, None, Some(r#""1"}"#.into()));
        let calls = acc.finalize_all();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].arguments, r#"{"a":"1"}"#);
    }

    #[test]
    fn test_tool_call_accumulator_finalize_clears() {
        let mut acc = ToolCallAccumulator::default();
        acc.ingest(0, Some("c1".into()), Some("tool".into()), Some("{}".into()));
        acc.finalize_all();
        let calls = acc.finalize_all();
        assert!(calls.is_empty());
    }

    #[test]
    fn test_deductive_finalize_requires_both_id_and_name() {
        let mut state = ToolCallState::default();
        state.ingest_delta(0, None, None, Some("{}".into()));
        assert!(
            state.finalize().is_none(),
            "finalize MUST return None without id and name"
        );

        state.ingest_delta(0, Some("id1".into()), None, None);
        assert!(
            state.finalize().is_none(),
            "finalize MUST return None without name"
        );

        state.ingest_delta(0, None, Some("tool".into()), None);
        assert!(
            state.finalize().is_some(),
            "finalize MUST return Some with both id and name"
        );
    }

    #[test]
    fn test_deductive_accumulator_finalize_all_clears_state() {
        let mut acc = ToolCallAccumulator::default();
        acc.ingest(0, Some("c1".into()), Some("t1".into()), Some("{}".into()));
        acc.ingest(1, Some("c2".into()), Some("t2".into()), Some("{}".into()));
        let first = acc.finalize_all();
        assert_eq!(first.len(), 2);
        let second = acc.finalize_all();
        assert!(
            second.is_empty(),
            "finalize_all MUST clear all internal state"
        );
    }

    #[test]
    fn test_deductive_ingest_delta_arguments_append_only() {
        let mut state = ToolCallState::default();
        state.ingest_delta(
            0,
            Some("c1".into()),
            Some("t".into()),
            Some(r#"{"a":"#.into()),
        );
        state.ingest_delta(0, None, None, Some(r#""1"}"#.into()));
        assert_eq!(state.arguments, r#"{"a":"1"}"#);
        assert!(state.arguments.contains(r#""1"}"#));
    }

    #[test]
    fn test_miri_tool_call_accumulator_no_ub() {
        let mut acc = ToolCallAccumulator::default();
        acc.ingest(0, None, None, None);
        acc.ingest(
            0,
            Some(String::new()),
            Some(String::new()),
            Some(String::new()),
        );
        acc.ingest(
            u32::MAX,
            Some("x".into()),
            Some("y".into()),
            Some("z".into()),
        );
        let _ = acc.finalize_all();
    }
}
