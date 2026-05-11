use crate::ProviderKind;

/// LLM 请求生命周期事件。
#[derive(Debug, Clone, PartialEq)]
pub enum LlmRequestEvent {
    /// 请求已创建
    Created {
        /// 触发源
        triggered_by: LlmTriggeredBy,
        /// 请求唯一标识
        request_id: String,
        /// 供应商类型
        provider: ProviderKind,
        /// 模型名称
        model: String,
    },
    /// Token 已发送
    TokenSent {
        /// 请求唯一标识
        request_id: String,
        /// 发送的 Token 数量
        tokens: u32,
    },
    /// 部分响应已到达
    PartialResponse {
        /// 请求唯一标识
        request_id: String,
        /// 响应文本片段
        chunk: String,
    },
    /// 请求已完成
    Completed {
        /// 请求唯一标识
        request_id: String,
        /// 总消耗 Token 数
        total_tokens: u32,
    },
    /// 请求失败
    Failed {
        /// 请求唯一标识
        request_id: String,
        /// 错误码
        error_code: String,
    },
    /// 请求已取消
    Cancelled {
        /// 请求唯一标识
        request_id: String,
    },
    /// 正在重试
    Retrying {
        /// 请求唯一标识
        request_id: String,
        /// 当前重试次数
        attempt: u32,
    },
}

/// LLM 请求状态机状态。
#[derive(Debug, Clone, PartialEq)]
pub enum LlmRequestState {
    /// 空闲
    Idle,
    /// 进行中
    InProgress {
        /// 供应商类型
        provider: ProviderKind,
        /// 模型名称
        model: String,
        /// 已发送 Token 数
        tokens_sent: u32,
    },
    /// 已完成
    Completed {
        /// 总消耗 Token 数
        total_tokens: u32,
    },
    /// 已失败
    Failed {
        /// 错误码
        error_code: String,
    },
    /// 已取消
    Cancelled,
}

/// 供应商定义标识（`def:` 前缀）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderDefId(String);

impl ProviderDefId {
    /// 创建供应商定义标识。
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self(format!("def:{id}"))
    }

    /// 返回标识的字符串引用。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 供应商实例标识（`inst:` 前缀）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderInstId(String);

impl ProviderInstId {
    /// 创建供应商实例标识。
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self(format!("inst:{id}"))
    }

    /// 返回标识的字符串引用。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 供应商定义，描述一个供应商的静态配置。
#[derive(Debug, Clone)]
pub struct ProviderDefinition {
    id: ProviderDefId,
    kind: ProviderKind,
    model: String,
}

impl ProviderDefinition {
    /// 创建供应商定义。
    #[must_use]
    pub fn new(id: ProviderDefId, kind: ProviderKind, model: String) -> Self {
        Self { id, kind, model }
    }

    /// 返回定义标识。
    #[must_use]
    pub fn id(&self) -> &ProviderDefId {
        &self.id
    }

    /// 返回供应商类型。
    #[must_use]
    pub fn kind(&self) -> &ProviderKind {
        &self.kind
    }

    /// 返回模型名称。
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }
}

/// 供应商实例，关联定义与运行时标识。
#[derive(Debug, Clone)]
pub struct ProviderInstance {
    id: ProviderInstId,
    definition: ProviderDefinition,
}

impl ProviderInstance {
    /// 创建供应商实例。
    #[must_use]
    pub fn new(id: ProviderInstId, definition: ProviderDefinition) -> Self {
        Self { id, definition }
    }

    /// 返回实例标识。
    #[must_use]
    pub fn id(&self) -> &ProviderInstId {
        &self.id
    }

    /// 返回关联的供应商定义。
    #[must_use]
    pub fn definition(&self) -> &ProviderDefinition {
        &self.definition
    }
}

/// LLM KCP（Knowledge Change Protocol）证明，验证数据变更的合法性。
#[derive(Debug, Clone)]
pub struct LlmKcpProof {
    /// 活动标识
    pub activity_id: String,
    /// 是否产生数据变更
    pub produces_data_change: bool,
    /// 是否影响可观测状态
    pub affects_observable_state: bool,
    /// 活动名称长度是否 ≥ 3
    pub name_length_gte_3: bool,
    /// 是否具有角色授权
    pub has_role_authorization: bool,
}

impl LlmKcpProof {
    /// 检查所有 KCP 条件是否满足。
    #[must_use]
    pub fn all_conditions_met(&self) -> bool {
        self.produces_data_change
            && self.affects_observable_state
            && self.name_length_gte_3
            && self.has_role_authorization
    }
}

/// LLM 数据变更事件触发源
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmTriggeredBy {
    /// 用户请求触发
    UserRequest(String),
    /// 系统内部调用触发
    Internal(String),
    /// 自动重试触发
    AutoRetry,
}

/// LLM 数据变更事件，用于 IPC 通知。
#[derive(Debug, Clone)]
pub enum LlmDataChangeEvent {
    /// 模型响应已接收
    ModelResponseReceived {
        /// 触发源
        triggered_by: LlmTriggeredBy,
        /// 请求唯一标识
        request_id: String,
        /// 模型名称
        model: String,
        /// 输入 Token 数
        input_tokens: u32,
        /// 输出 Token 数
        output_tokens: u32,
    },
    /// 模型调用出错
    ModelErrorOccurred {
        /// 触发源
        triggered_by: LlmTriggeredBy,
        /// 请求唯一标识
        request_id: String,
        /// 模型名称
        model: String,
        /// 错误码
        error_code: String,
    },
    /// 模型调用正在重试
    ModelRetryAttempted {
        /// 触发源
        triggered_by: LlmTriggeredBy,
        /// 请求唯一标识
        request_id: String,
        /// 模型名称
        model: String,
        /// 当前重试次数
        attempt: u32,
    },
}

/// 恢复状态机状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryState {
    /// 就绪
    Ready,
    /// 请求中
    Requesting,
    /// 恢复中
    Recovering,
    /// 已失败
    Failed,
}

/// 恢复状态机动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// 发起请求
    StartRequest,
    /// 收到响应
    ReceiveResponse,
    /// 遇到错误
    EncounterError,
    /// 重试
    Retry,
    /// 放弃
    GiveUp,
}

/// 恢复有限状态机（FSM）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryFsm {
    state: RecoveryState,
}

impl RecoveryFsm {
    /// 创建初始状态为 `Ready` 的恢复 FSM。
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: RecoveryState::Ready,
        }
    }

    /// 返回当前状态。
    #[must_use]
    pub fn state(&self) -> &RecoveryState {
        &self.state
    }

    /// 根据动作执行状态转移。
    ///
    /// 非法转换保持原状态并记录警告日志。
    pub fn transition(&mut self, action: RecoveryAction) {
        self.state = match (&self.state, action) {
            (RecoveryState::Ready, RecoveryAction::StartRequest)
            | (RecoveryState::Recovering, RecoveryAction::Retry) => RecoveryState::Requesting,
            (RecoveryState::Requesting, RecoveryAction::ReceiveResponse) => RecoveryState::Ready,
            (RecoveryState::Requesting, RecoveryAction::EncounterError) => {
                RecoveryState::Recovering
            }
            (RecoveryState::Recovering, RecoveryAction::GiveUp) => RecoveryState::Failed,
            (
                RecoveryState::Ready,
                RecoveryAction::ReceiveResponse
                | RecoveryAction::EncounterError
                | RecoveryAction::GiveUp
                | RecoveryAction::Retry,
            ) => {
                tracing::warn!("Invalid FSM transition: Ready + {:?} — no-op", action);
                self.state.clone()
            }
            (
                RecoveryState::Requesting,
                RecoveryAction::StartRequest | RecoveryAction::Retry | RecoveryAction::GiveUp,
            ) => {
                tracing::warn!("Invalid FSM transition: Requesting + {:?} — no-op", action);
                self.state.clone()
            }
            (
                RecoveryState::Recovering,
                RecoveryAction::StartRequest
                | RecoveryAction::ReceiveResponse
                | RecoveryAction::EncounterError,
            ) => {
                tracing::warn!("Invalid FSM transition: Recovering + {:?} — no-op", action);
                self.state.clone()
            }
            (RecoveryState::Failed, _) => {
                tracing::warn!(
                    "Invalid FSM transition: Failed (terminal) + {:?} — no-op",
                    action
                );
                self.state.clone()
            }
        };
    }
}

impl Default for RecoveryFsm {
    fn default() -> Self {
        Self::new()
    }
}

/// LLM 事件日志，按序记录请求生命周期事件。
#[derive(Debug, Clone)]
pub struct LlmEventLog {
    events: Vec<LlmRequestEvent>,
}

impl LlmEventLog {
    /// 创建空事件日志。
    #[must_use]
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// 追加事件到日志。
    pub fn append(&mut self, event: LlmRequestEvent) {
        self.events.push(event);
    }

    /// 返回所有事件的切片。
    #[must_use]
    pub fn events(&self) -> &[LlmRequestEvent] {
        &self.events
    }
}

impl Default for LlmEventLog {
    fn default() -> Self {
        Self::new()
    }
}

/// 根据当前状态和事件，投影计算新的请求状态。
#[must_use]
pub fn project_request_state(state: LlmRequestState, event: &LlmRequestEvent) -> LlmRequestState {
    match (&state, event) {
        (
            LlmRequestState::Idle,
            LlmRequestEvent::Created {
                provider, model, ..
            },
        ) => LlmRequestState::InProgress {
            provider: provider.clone(),
            model: model.clone(),
            tokens_sent: 0,
        },
        (
            LlmRequestState::InProgress {
                provider,
                model,
                tokens_sent,
            },
            LlmRequestEvent::TokenSent { tokens, .. },
        ) => LlmRequestState::InProgress {
            provider: provider.clone(),
            model: model.clone(),
            tokens_sent: tokens_sent + tokens,
        },
        (LlmRequestState::InProgress { .. }, LlmRequestEvent::Completed { total_tokens, .. }) => {
            LlmRequestState::Completed {
                total_tokens: *total_tokens,
            }
        }
        (LlmRequestState::InProgress { .. }, LlmRequestEvent::Failed { error_code, .. }) => {
            LlmRequestState::Failed {
                error_code: error_code.clone(),
            }
        }
        (LlmRequestState::InProgress { .. }, LlmRequestEvent::Cancelled { .. }) => {
            LlmRequestState::Cancelled
        }
        _ => state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_request_event_created() {
        let event = LlmRequestEvent::Created {
            triggered_by: LlmTriggeredBy::UserRequest("user-1".into()),
            request_id: "req-1".into(),
            provider: ProviderKind::OpenAi,
            model: "gpt-4".into(),
        };
        assert!(matches!(event, LlmRequestEvent::Created { .. }));
    }

    #[test]
    fn test_llm_request_state_idle() {
        let state = LlmRequestState::Idle;
        assert_eq!(state, LlmRequestState::Idle);
    }

    #[test]
    fn test_project_request_state_created() {
        let state = LlmRequestState::Idle;
        let event = LlmRequestEvent::Created {
            triggered_by: LlmTriggeredBy::UserRequest("user-1".into()),
            request_id: "req-1".into(),
            provider: ProviderKind::OpenAi,
            model: "gpt-4".into(),
        };
        let new_state = project_request_state(state, &event);
        assert_eq!(
            new_state,
            LlmRequestState::InProgress {
                provider: ProviderKind::OpenAi,
                model: "gpt-4".into(),
                tokens_sent: 0,
            }
        );
    }

    #[test]
    fn test_project_request_state_completed() {
        let state = LlmRequestState::InProgress {
            provider: ProviderKind::OpenAi,
            model: "gpt-4".into(),
            tokens_sent: 100,
        };
        let event = LlmRequestEvent::Completed {
            request_id: "req-1".into(),
            total_tokens: 200,
        };
        let new_state = project_request_state(state, &event);
        assert_eq!(new_state, LlmRequestState::Completed { total_tokens: 200 });
    }

    #[test]
    fn test_project_request_state_failed() {
        let state = LlmRequestState::InProgress {
            provider: ProviderKind::OpenAi,
            model: "gpt-4".into(),
            tokens_sent: 50,
        };
        let event = LlmRequestEvent::Failed {
            request_id: "req-1".into(),
            error_code: "ERR-NET-LLM-001_ERR_O".into(),
        };
        let new_state = project_request_state(state, &event);
        assert_eq!(
            new_state,
            LlmRequestState::Failed {
                error_code: "ERR-NET-LLM-001_ERR_O".into(),
            }
        );
    }

    #[test]
    fn test_provider_def_id_new() {
        let id = ProviderDefId::new("openai-gpt4");
        assert_eq!(id.as_str(), "def:openai-gpt4");
    }

    #[test]
    fn test_provider_inst_id_new() {
        let id = ProviderInstId::new("inst-001");
        assert_eq!(id.as_str(), "inst:inst-001");
    }

    #[test]
    fn test_provider_def_inst_id_different_types() {
        let def_id = ProviderDefId::new("openai-gpt4");
        let inst_id = ProviderInstId::new("openai-gpt4");
        assert_ne!(def_id.as_str(), inst_id.as_str());
    }

    #[test]
    fn test_provider_definition_new() {
        let defn = ProviderDefinition::new(
            ProviderDefId::new("openai-gpt4"),
            ProviderKind::OpenAi,
            "gpt-4".into(),
        );
        assert_eq!(defn.id().as_str(), "def:openai-gpt4");
        assert_eq!(defn.kind(), &ProviderKind::OpenAi);
        assert_eq!(defn.model(), "gpt-4");
    }

    #[test]
    fn test_provider_instance_new() {
        let defn = ProviderDefinition::new(
            ProviderDefId::new("openai-gpt4"),
            ProviderKind::OpenAi,
            "gpt-4".into(),
        );
        let inst = ProviderInstance::new(ProviderInstId::new("inst-001"), defn);
        assert_eq!(inst.id().as_str(), "inst:inst-001");
        assert_eq!(inst.definition().kind(), &ProviderKind::OpenAi);
    }

    #[test]
    fn test_llm_kcp_proof_new() {
        let proof = LlmKcpProof {
            activity_id: "act-001".into(),
            produces_data_change: true,
            affects_observable_state: true,
            name_length_gte_3: true,
            has_role_authorization: true,
        };
        assert_eq!(proof.activity_id, "act-001");
        assert!(proof.all_conditions_met());
    }

    #[test]
    fn test_llm_data_change_event() {
        let event = LlmDataChangeEvent::ModelResponseReceived {
            triggered_by: LlmTriggeredBy::UserRequest("user-1".into()),
            request_id: "req-001".into(),
            model: "gpt-4".into(),
            input_tokens: 100,
            output_tokens: 50,
        };
        assert!(matches!(
            event,
            LlmDataChangeEvent::ModelResponseReceived { .. }
        ));
    }

    #[test]
    fn test_kcp_proof_all_conditions_met() {
        let proof = LlmKcpProof {
            activity_id: "act-001".into(),
            produces_data_change: true,
            affects_observable_state: true,
            name_length_gte_3: true,
            has_role_authorization: true,
        };
        assert!(proof.all_conditions_met());
    }

    #[test]
    fn test_kcp_proof_missing_condition() {
        let proof = LlmKcpProof {
            activity_id: "act-001".into(),
            produces_data_change: true,
            affects_observable_state: true,
            name_length_gte_3: false,
            has_role_authorization: true,
        };
        assert!(!proof.all_conditions_met());
    }

    #[test]
    fn test_kcp_proof_no_conditions() {
        let proof = LlmKcpProof {
            activity_id: "act-001".into(),
            produces_data_change: false,
            affects_observable_state: false,
            name_length_gte_3: false,
            has_role_authorization: false,
        };
        assert!(!proof.all_conditions_met());
    }

    #[test]
    fn test_recovery_fsm_happy_path() {
        let mut fsm = RecoveryFsm::new();
        assert_eq!(*fsm.state(), RecoveryState::Ready);

        fsm.transition(RecoveryAction::StartRequest);
        assert_eq!(*fsm.state(), RecoveryState::Requesting);

        fsm.transition(RecoveryAction::ReceiveResponse);
        assert_eq!(*fsm.state(), RecoveryState::Ready);
    }

    #[test]
    fn test_recovery_fsm_retry_path() {
        let mut fsm = RecoveryFsm::new();
        fsm.transition(RecoveryAction::StartRequest);
        assert_eq!(*fsm.state(), RecoveryState::Requesting);

        fsm.transition(RecoveryAction::EncounterError);
        assert_eq!(*fsm.state(), RecoveryState::Recovering);

        fsm.transition(RecoveryAction::Retry);
        assert_eq!(*fsm.state(), RecoveryState::Requesting);

        fsm.transition(RecoveryAction::ReceiveResponse);
        assert_eq!(*fsm.state(), RecoveryState::Ready);
    }

    #[test]
    fn test_recovery_fsm_exhausted() {
        let mut fsm = RecoveryFsm::new();
        fsm.transition(RecoveryAction::StartRequest);
        fsm.transition(RecoveryAction::EncounterError);
        fsm.transition(RecoveryAction::GiveUp);
        assert_eq!(*fsm.state(), RecoveryState::Failed);
    }

    #[test]
    fn test_event_log_append_immutability() {
        let mut log = LlmEventLog::new();
        log.append(LlmRequestEvent::Created {
            triggered_by: LlmTriggeredBy::UserRequest("user-1".into()),
            request_id: "r1".into(),
            provider: ProviderKind::OpenAi,
            model: "gpt-4".into(),
        });
        let first_snapshot = log.events().to_vec();

        log.append(LlmRequestEvent::TokenSent {
            request_id: "r1".into(),
            tokens: 10,
        });

        assert_eq!(log.events().len(), 2);
        assert_eq!(log.events()[0], first_snapshot[0]);
    }

    #[test]
    fn test_event_log_order_preserved() {
        let mut log = LlmEventLog::new();
        log.append(LlmRequestEvent::Created {
            triggered_by: LlmTriggeredBy::UserRequest("user-1".into()),
            request_id: "r1".into(),
            provider: ProviderKind::OpenAi,
            model: "gpt-4".into(),
        });
        log.append(LlmRequestEvent::TokenSent {
            request_id: "r1".into(),
            tokens: 10,
        });
        log.append(LlmRequestEvent::Completed {
            request_id: "r1".into(),
            total_tokens: 100,
        });

        assert!(matches!(log.events()[0], LlmRequestEvent::Created { .. }));
        assert!(matches!(log.events()[1], LlmRequestEvent::TokenSent { .. }));
        assert!(matches!(log.events()[2], LlmRequestEvent::Completed { .. }));
    }
}
