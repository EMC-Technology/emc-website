//! Unified error construction helpers
//!
//! All business crates MUST use these helpers to construct errors.
//! Direct use of `ErrorObject::builder()` is reserved for `From` trait implementations only.

use crate::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
use crate::error_code::registry;
use crate::error_object::ErrorObject;

/// Construct a validation error (USR/WARNING/OPERATION)
#[must_use]
pub fn validation_error(message: &str, operation: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::VALIDATION_FAILED)
        .source(ErrorSource::USR)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(message)
        .user_message("输入验证失败，请检查后重试")
        .module_path("validation")
        .operation(operation)
        .build()
}

/// Construct an invalid `RecordID` error (USR/ERROR/OPERATION)
#[must_use]
pub fn invalid_record_id(id: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::INVALID_RECORD_ID)
        .source(ErrorSource::USR)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("无效的 RecordID 格式: {id}"))
        .user_message("输入的 ID 格式无效，请检查后重试")
        .module_path("validation")
        .operation("validate_record_id")
        .build()
}

/// Construct a not-found error (USR/ERROR/OPERATION)
#[must_use]
pub fn not_found(entity: &str, id: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::NOT_FOUND)
        .source(ErrorSource::USR)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("资源不存在: {entity}:{id}"))
        .user_message("请求的资源未找到")
        .module_path("validation")
        .operation("lookup")
        .build()
}

/// Construct a database error (FS/ERROR/OPERATION)
#[must_use]
pub fn db_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::DB_QUERY_FAILED)
        .source(ErrorSource::FS)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::SemiAuto)
        .message(message)
        .user_message("数据库操作失败，请稍后重试")
        .module_path("db")
        .operation("query")
        .build()
}

/// Construct a parse error (FS/ERROR/OPERATION)
#[must_use]
pub fn parse_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::PARSE_FAILED)
        .source(ErrorSource::FS)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(message)
        .user_message("文件内容无法解析，请检查文件格式")
        .module_path("parser")
        .operation("parse")
        .build()
}

/// Construct an unsupported-format error (USR/WARNING/OPERATION)
#[must_use]
pub fn unsupported_format(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::UNSUPPORTED_FORMAT)
        .source(ErrorSource::USR)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("不支持的文件格式: {message}"))
        .user_message("该文件格式不受支持")
        .module_path("parser")
        .operation("format_check")
        .build()
}

/// Construct a configuration error (CFG/ERROR/OPERATION)
#[must_use]
pub fn config_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::CONFIG_FAILED)
        .source(ErrorSource::CFG)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::ManualIntervention)
        .message(message)
        .user_message("配置错误，请检查系统配置")
        .module_path("config")
        .operation("validate")
        .build()
}

/// Construct an authentication error (SEC/ERROR/OPERATION)
#[must_use]
pub fn auth_error(message: &str, operation: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AUTH_FAILED)
        .source(ErrorSource::SEC)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(message)
        .user_message("认证失败，请检查凭据")
        .module_path("auth")
        .operation(operation)
        .build()
}

/// Construct an internal system error (INT/CRITICAL/GLOBAL)
#[must_use]
pub fn internal_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::INTERNAL_ERROR)
        .source(ErrorSource::INT)
        .severity(Severity::CRITICAL)
        .impact_scope(ImpactScope::GLOBAL)
        .recoverability(Recoverability::ManualIntervention)
        .message(message)
        .user_message("系统发生内部错误，请联系管理员")
        .module_path("system")
        .operation("internal")
        .build()
}

/// Construct a connection-lost error (NET/ERROR/SESSION)
#[must_use]
pub fn connection_lost() -> ErrorObject {
    ErrorObject::builder()
        .code(registry::CONNECTION_LOST)
        .source(ErrorSource::NET)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("网络连接中断")
        .user_message("网络连接已断开，系统将自动尝试重连")
        .module_path("network")
        .operation("connection")
        .build()
}

/// Construct a crypto/encryption error (SEC/CRITICAL/OPERATION)
#[must_use]
pub fn crypto_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::CRYPTO_FAILED)
        .source(ErrorSource::SEC)
        .severity(Severity::CRITICAL)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("加密/解密失败: {message}"))
        .user_message("安全操作失败")
        .module_path("crypto")
        .operation("encrypt_decrypt")
        .build()
}

/// Construct a serialization/deserialization error (INT/ERROR/OPERATION)
#[must_use]
pub fn serde_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::SERDE_FAILED)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("序列化/反序列化失败: {message}"))
        .user_message("数据处理失败，请检查输入格式")
        .module_path("serde")
        .operation("serialize")
        .build()
}

/// Construct a network API response error (NET/ERROR/OPERATION)
#[must_use]
pub fn net_api_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::NET_API_ERROR)
        .source(ErrorSource::NET)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("网络请求失败，请稍后重试")
        .module_path("api")
        .operation("response")
        .build()
}

/// Construct an LLM API call error (AIM/ERROR/OPERATION)
#[must_use]
pub fn llm_api_error(message: &str, operation: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::LLM_API_FAILED)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("LLM 调用失败，请稍后重试")
        .module_path("llm")
        .operation(operation)
        .build()
}

/// Construct an API deserialization error (INT/ERROR/OPERATION)
#[must_use]
pub fn api_deserialize_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::API_DESERIALIZE_ERROR)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("数据反序列化失败: {message}"))
        .user_message("数据处理失败，请检查输入格式")
        .module_path("api")
        .operation("deserialize")
        .build()
}

/// Construct an API request construction error (INT/ERROR/OPERATION)
#[must_use]
pub fn api_request_error(message: &str, operation: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::API_REQUEST_ERROR)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(message)
        .user_message("请求构建失败")
        .module_path("api")
        .operation(operation)
        .build()
}

/// Construct a WebSocket client error (NET/ERROR/OPERATION)
#[must_use]
pub fn ws_client_error(message: &str, operation: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::WS_CLIENT_ERROR)
        .source(ErrorSource::NET)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("实时通信异常，系统将自动重连")
        .module_path("ws")
        .operation(operation)
        .build()
}

/// Construct a WebSocket serialization error (INT/ERROR/OPERATION)
#[must_use]
pub fn ws_serialize_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::WS_SERIALIZE_ERROR)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("序列化失败: {message}"))
        .user_message("数据处理失败")
        .module_path("ws")
        .operation("serialize")
        .build()
}

/// Construct a WebSocket subscription limit error (SESS/ERROR/OPERATION)
#[must_use]
pub fn ws_subscribe_limit_error(limit: usize) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::WS_SUBSCRIBE_FAILED)
        .source(ErrorSource::SESS)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("单连接订阅数已达上限 ({limit})"))
        .user_message("订阅数量已达上限")
        .module_path("ws")
        .operation("subscribe")
        .build()
}

/// Construct a WebSocket send failed error (NET/WARNING/OPERATION)
#[must_use]
pub fn ws_send_failed_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::WS_SEND_FAILED)
        .source(ErrorSource::NET)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("消息发送失败，无活跃订阅者")
        .module_path("ws")
        .operation("send")
        .build()
}

/// Construct a WebSocket receive/broadcast target not found error (NET/WARNING/OPERATION)
#[must_use]
pub fn ws_receive_failed_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::WS_RECEIVE_FAILED)
        .source(ErrorSource::NET)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("广播目标未找到")
        .module_path("ws")
        .operation("receive")
        .build()
}

/// Construct an LLM judgment error (AIM/ERROR/OPERATION)
#[must_use]
pub fn llm_judgment_error(message: &str, operation: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::LLM_JUDGMENT_FAILED)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("LLM 判决失败")
        .module_path("llm_judge")
        .operation(operation)
        .build()
}

/// Construct a metric calculation error (AIM/ERROR/OPERATION)
#[must_use]
pub fn metric_calc_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::METRIC_CALC_FAILED)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("指标计算失败")
        .module_path("metrics")
        .operation("calculate")
        .build()
}

/// Construct a dataset load error (FS/ERROR/SESSION)
#[must_use]
pub fn dataset_load_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::DATASET_LOAD_FAILED)
        .source(ErrorSource::FS)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("数据集加载失败")
        .module_path("dataset")
        .operation("load")
        .build()
}

/// Construct an evaluation timeout error (INT/WARNING/OPERATION)
#[must_use]
pub fn eval_timeout_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EVAL_TIMEOUT)
        .source(ErrorSource::INT)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("评估操作超时")
        .module_path("evaluator")
        .operation("evaluate")
        .build()
}

// ── 孤儿错误码补充 helper ────────────────────────────────────

/// Construct an I/O operation error (FS/ERROR/OPERATION)
#[must_use]
pub fn io_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::IO_FAILED)
        .source(ErrorSource::FS)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("I/O 操作失败: {message}"))
        .user_message("文件操作失败，请稍后重试")
        .module_path("io")
        .operation("io_operation")
        .build()
}

/// Construct a token invalid/expired error (SEC/WARNING/OPERATION)
#[must_use]
pub fn token_invalid_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::TOKEN_INVALID)
        .source(ErrorSource::SEC)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("令牌无效或已过期: {message}"))
        .user_message("认证令牌已失效，请重新登录")
        .module_path("auth")
        .operation("token_validate")
        .build()
}

/// Construct a general fallback error (INT/ERROR/OPERATION)
#[must_use]
pub fn general_fallback_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::GENERAL_FALLBACK)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("操作失败，请稍后重试")
        .module_path("system")
        .operation("fallback")
        .build()
}

/// Construct a frontend/UI error (USR/ERROR/OPERATION)
#[must_use]
pub fn frontend_ui_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::FRONTEND_UI_ERROR)
        .source(ErrorSource::USR)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(message)
        .user_message("界面操作失败")
        .module_path("ui")
        .operation("render")
        .build()
}

/// Construct a gateway/API layer error (NET/ERROR/SESSION)
#[must_use]
pub fn gateway_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::GATEWAY_ERROR)
        .source(ErrorSource::NET)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("网关请求失败，请稍后重试")
        .module_path("gateway")
        .operation("proxy")
        .build()
}

/// Construct a business logic error (INT/ERROR/MODULE)
#[must_use]
pub fn business_logic_error(message: &str, operation: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::BUSINESS_LOGIC_ERROR)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::MODULE)
        .recoverability(Recoverability::ManualIntervention)
        .message(message)
        .user_message("业务处理失败")
        .module_path("business")
        .operation(operation)
        .build()
}

/// Construct an infrastructure error (SYS/ERROR/GLOBAL)
#[must_use]
pub fn infrastructure_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::INFRASTRUCTURE_ERROR)
        .source(ErrorSource::SYS)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::GLOBAL)
        .recoverability(Recoverability::ManualIntervention)
        .message(message)
        .user_message("基础设施故障，请联系管理员")
        .module_path("infrastructure")
        .operation("system")
        .build()
}

/// Construct an extraction error (AIM/ERROR/OPERATION)
#[must_use]
pub fn extraction_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EXTRACTION_FAILED)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("实体/关系抽取失败")
        .module_path("extractor")
        .operation("extract")
        .build()
}

/// Construct a disambiguation error (AIM/WARNING/OPERATION)
#[must_use]
pub fn disambiguation_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::DISAMBIGUATION_FAILED)
        .source(ErrorSource::AIM)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(message)
        .user_message("实体消歧失败")
        .module_path("disambiguator")
        .operation("disambiguate")
        .build()
}

// ── 嵌入模型 helper ─────────────────────────────────────────

/// Construct an embedding config error (AIM/ERROR/OPERATION)
#[must_use]
pub fn embedding_config_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EMBEDDING_CONFIG_ERROR)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("嵌入模型配置错误: {message}"))
        .user_message("嵌入模型配置有误")
        .module_path("embedding")
        .operation("configure")
        .build()
}

/// Construct an embedding model not loaded error (AIM/ERROR/OPERATION)
#[must_use]
pub fn embedding_model_not_loaded(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EMBEDDING_MODEL_NOT_LOADED)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("嵌入模型未加载: {message}"))
        .user_message("嵌入模型尚未就绪，请稍后重试")
        .module_path("embedding")
        .operation("load")
        .build()
}

/// Construct an embedding model load failed error (AIM/CRITICAL/OPERATION)
#[must_use]
pub fn embedding_model_load_failed(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EMBEDDING_MODEL_LOAD_FAILED)
        .source(ErrorSource::AIM)
        .severity(Severity::CRITICAL)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::ManualIntervention)
        .message(&format!("嵌入模型加载失败: {message}"))
        .user_message("嵌入模型加载失败，请联系管理员")
        .module_path("embedding")
        .operation("load")
        .build()
}

/// Construct an embedding inference failed error (AIM/ERROR/OPERATION)
#[must_use]
pub fn embedding_inference_failed(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EMBEDDING_INFERENCE_FAILED)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("嵌入推理失败: {message}"))
        .user_message("嵌入计算失败，请稍后重试")
        .module_path("embedding")
        .operation("infer")
        .build()
}

/// Construct an embedding empty input error (AIM/WARNING/OPERATION)
#[must_use]
pub fn embedding_empty_input() -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EMBEDDING_EMPTY_INPUT)
        .source(ErrorSource::AIM)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message("嵌入计算输入为空")
        .user_message("输入内容为空，无法计算嵌入")
        .module_path("embedding")
        .operation("infer")
        .build()
}

/// Construct an embedding tokenizer error (AIM/ERROR/OPERATION)
#[must_use]
pub fn embedding_tokenizer_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EMBEDDING_TOKENIZER_ERROR)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("Tokenizer 错误: {message}"))
        .user_message("文本处理失败")
        .module_path("embedding")
        .operation("tokenize")
        .build()
}

/// Construct an embedding I/O error (AIM/ERROR/OPERATION)
#[must_use]
pub fn embedding_io_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EMBEDDING_IO_ERROR)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("嵌入模型 I/O 错误: {message}"))
        .user_message("模型文件操作失败")
        .module_path("embedding")
        .operation("io")
        .build()
}

// ── Agent helper ─────────────────────────────────────────────

/// Construct an agent LLM error (AIM/ERROR/OPERATION)
#[must_use]
pub fn agent_llm_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_LLM_ERROR)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("LLM 调用失败: {message}"))
        .user_message("AI 服务调用失败，请稍后重试")
        .module_path("agent")
        .operation("llm_call")
        .build()
}

/// Construct an agent tool error (AIM/ERROR/OPERATION)
#[must_use]
pub fn agent_tool_error(tool: &str, error: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_TOOL_ERROR)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("工具 '{tool}' 调用失败: {error}"))
        .user_message("工具执行失败")
        .module_path("agent")
        .operation("tool_call")
        .build()
}

/// Construct an agent tool not found error (AIM/WARNING/OPERATION)
#[must_use]
pub fn agent_tool_not_found(tool: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_TOOL_NOT_FOUND)
        .source(ErrorSource::AIM)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("工具 '{tool}' 不存在"))
        .user_message("请求的工具不可用")
        .module_path("agent")
        .operation("tool_lookup")
        .build()
}

/// Construct an agent max iterations error (AIM/WARNING/OPERATION)
#[must_use]
pub fn agent_max_iterations(max: usize) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_MAX_ITERATIONS)
        .source(ErrorSource::AIM)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("达到最大迭代次数: {max}"))
        .user_message("任务执行步数已达上限")
        .module_path("agent")
        .operation("execute")
        .build()
}

/// Construct an agent timeout error (AIM/WARNING/SESSION)
#[must_use]
pub fn agent_timeout() -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_TIMEOUT)
        .source(ErrorSource::AIM)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Agent 执行超时")
        .user_message("任务执行超时，请稍后重试")
        .module_path("agent")
        .operation("execute")
        .build()
}

/// Construct an agent clarification needed error (AIM/WARNING/OPERATION)
#[must_use]
pub fn agent_clarification(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_CLARIFICATION)
        .source(ErrorSource::AIM)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("需要澄清: {message}"))
        .user_message("请提供更多信息以继续")
        .module_path("agent")
        .operation("clarify")
        .build()
}

/// Construct an agent parse action error (AIM/ERROR/OPERATION)
#[must_use]
pub fn agent_parse_action_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_PARSE_ACTION)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("无法解析 LLM 输出的动作: {message}"))
        .user_message("AI 响应解析失败")
        .module_path("agent")
        .operation("parse_action")
        .build()
}

/// Construct an agent memory error (AIM/ERROR/OPERATION)
#[must_use]
pub fn agent_memory_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_MEMORY_ERROR)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("记忆系统错误: {message}"))
        .user_message("记忆系统异常")
        .module_path("agent")
        .operation("memory")
        .build()
}

/// Construct an agent invalid state transition error (AIM/ERROR/OPERATION)
#[must_use]
pub fn agent_invalid_transition(from: &str, to: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_INVALID_TRANSITION)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("无效的状态转换: 从 {from} 到 {to}"))
        .user_message("任务状态异常")
        .module_path("agent")
        .operation("transition")
        .build()
}

/// Construct an agent workflow error (AIM/ERROR/OPERATION)
#[must_use]
pub fn agent_workflow_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_WORKFLOW_ERROR)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("工作流错误: {message}"))
        .user_message("工作流执行失败")
        .module_path("agent")
        .operation("workflow")
        .build()
}

/// Construct an agent serialization error (AIM/ERROR/OPERATION)
#[must_use]
pub fn agent_serialization_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_SERIALIZATION_ERROR)
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("序列化错误: {message}"))
        .user_message("数据处理失败")
        .module_path("agent")
        .operation("serialize")
        .build()
}

/// Construct an agent permission denied error (SEC/ERROR/OPERATION)
#[must_use]
pub fn agent_permission_denied(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_PERMISSION_DENIED)
        .source(ErrorSource::SEC)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("权限不足: {message}"))
        .user_message("权限不足，无法执行此操作")
        .module_path("agent")
        .operation("authorize")
        .build()
}

/// Construct an agent safety check failed error (SEC/CRITICAL/OPERATION)
#[must_use]
pub fn agent_safety_check_failed(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGENT_SAFETY_CHECK)
        .source(ErrorSource::SEC)
        .severity(Severity::CRITICAL)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("安全检查未通过: {message}"))
        .user_message("操作被安全策略拦截")
        .module_path("agent")
        .operation("safety_check")
        .build()
}

// ── CQRS 聚合 helper ────────────────────────────────────────

/// Construct an aggregate invalid state error (INT/ERROR/OPERATION)
#[must_use]
pub fn aggregate_invalid_state(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGGREGATE_INVALID_STATE)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("聚合状态无效: {message}"))
        .user_message("数据状态异常")
        .module_path("cqrs")
        .operation("aggregate")
        .build()
}

/// Construct an aggregate business rule violation error (INT/ERROR/OPERATION)
#[must_use]
pub fn aggregate_business_rule(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGGREGATE_BUSINESS_RULE)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("业务规则违反: {message}"))
        .user_message("操作违反业务规则")
        .module_path("cqrs")
        .operation("validate")
        .build()
}

/// Construct an aggregate not found error (INT/WARNING/OPERATION)
#[must_use]
pub fn aggregate_not_found(id: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGGREGATE_NOT_FOUND)
        .source(ErrorSource::INT)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("聚合未找到: {id}"))
        .user_message("请求的数据未找到")
        .module_path("cqrs")
        .operation("load")
        .build()
}

/// Construct an aggregate version conflict error (INT/ERROR/OPERATION)
#[must_use]
pub fn aggregate_version_conflict(expected: u64, actual: u64) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGGREGATE_VERSION_CONFLICT)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("版本冲突: 期望 {expected}, 实际 {actual}"))
        .user_message("数据已被其他操作修改，请刷新后重试")
        .module_path("cqrs")
        .operation("commit")
        .build()
}

/// Construct an aggregate serialization error (INT/ERROR/OPERATION)
#[must_use]
pub fn aggregate_serialization_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::AGGREGATE_SERIALIZATION)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("聚合序列化错误: {message}"))
        .user_message("数据处理失败")
        .module_path("cqrs")
        .operation("serialize")
        .build()
}

// ── 缓存 helper ─────────────────────────────────────────────

/// Construct a cache L2 connection error (FS/ERROR/SESSION)
#[must_use]
pub fn cache_l2_connection_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::CACHE_L2_CONNECTION)
        .source(ErrorSource::FS)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("缓存连接失败: {message}"))
        .user_message("缓存服务暂时不可用")
        .module_path("cache")
        .operation("connect")
        .build()
}

/// Construct a cache L2 serialization error (INT/ERROR/OPERATION)
#[must_use]
pub fn cache_l2_serialization_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::CACHE_L2_SERIALIZATION)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("缓存序列化失败: {message}"))
        .user_message("缓存数据处理失败")
        .module_path("cache")
        .operation("serialize")
        .build()
}

/// Construct a cache L2 deserialization error (INT/ERROR/OPERATION)
#[must_use]
pub fn cache_l2_deserialization_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::CACHE_L2_DESERIALIZATION)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("缓存反序列化失败: {message}"))
        .user_message("缓存数据读取失败")
        .module_path("cache")
        .operation("deserialize")
        .build()
}

/// Construct a cache L2 operation error (FS/ERROR/OPERATION)
#[must_use]
pub fn cache_l2_operation_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::CACHE_L2_OPERATION)
        .source(ErrorSource::FS)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("缓存操作失败: {message}"))
        .user_message("缓存操作失败，请稍后重试")
        .module_path("cache")
        .operation("operate")
        .build()
}

/// Construct a cache manager L2 error (FS/ERROR/OPERATION)
#[must_use]
pub fn cache_manager_l2_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::CACHE_MANAGER_L2)
        .source(ErrorSource::FS)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("缓存管理器 L2 错误: {message}"))
        .user_message("缓存服务异常")
        .module_path("cache")
        .operation("manage")
        .build()
}

/// Construct a cache manager serialization error (INT/ERROR/OPERATION)
#[must_use]
pub fn cache_manager_serialization_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::CACHE_MANAGER_SERIALIZATION)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("缓存管理器序列化错误: {message}"))
        .user_message("缓存数据处理失败")
        .module_path("cache")
        .operation("serialize")
        .build()
}

// ── 事件系统 helper ─────────────────────────────────────────

/// Construct an event type mismatch error (INT/ERROR/OPERATION)
#[must_use]
pub fn event_type_mismatch() -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EVENT_TYPE_MISMATCH)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message("事件类型不匹配")
        .user_message("事件处理失败")
        .module_path("event")
        .operation("dispatch")
        .build()
}

/// Construct an event processing failed error (INT/ERROR/OPERATION)
#[must_use]
pub fn event_processing_failed(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EVENT_PROCESSING_FAILED)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("事件处理失败: {message}"))
        .user_message("事件处理异常")
        .module_path("event")
        .operation("process")
        .build()
}

/// Construct an event no subscribers error (INT/WARNING/OPERATION)
#[must_use]
pub fn event_no_subscribers(event_id: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EVENT_NO_SUBSCRIBERS)
        .source(ErrorSource::INT)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(&format!("没有活跃的订阅者接收事件 (event_id={event_id})"))
        .user_message("事件无接收方")
        .module_path("event")
        .operation("publish")
        .build()
}

/// Construct an event bus shutdown error (INT/WARNING/SESSION)
#[must_use]
pub fn event_bus_shutdown() -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EVENT_BUS_SHUTDOWN)
        .source(ErrorSource::INT)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::NonRecoverable)
        .message("事件总线已关闭")
        .user_message("事件服务已停止")
        .module_path("event")
        .operation("publish")
        .build()
}

/// Construct an event publish timeout error (INT/WARNING/OPERATION)
#[must_use]
pub fn event_publish_timeout(elapsed_ms: u64) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::EVENT_PUBLISH_TIMEOUT)
        .source(ErrorSource::INT)
        .severity(Severity::WARNING)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::AutoRecoverable)
        .message(&format!("事件发布超时 (elapsed={elapsed_ms}ms)"))
        .user_message("事件发布超时")
        .module_path("event")
        .operation("publish")
        .build()
}

// ── 可观测性 helper ─────────────────────────────────────────

/// Construct an observability forbidden error (SEC/ERROR/OPERATION)
#[must_use]
pub fn observability_forbidden(message: &str, code: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(registry::OBSERVABILITY_FORBIDDEN)
        .source(ErrorSource::SEC)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(message)
        .user_message("访问被拒绝")
        .module_path("observability")
        .operation(code)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classification::{ErrorSource, Severity, ImpactScope, Recoverability};

    #[test]
    fn test_validation_error_valid_input_returns_usr_source() {
        let err = validation_error("字段不能为空", "submit_form");
        assert_eq!(err.source(), ErrorSource::USR);
        assert!(err.code().starts_with("ERR-USR-VAL"));
        assert!(err.message().contains("字段不能为空"));
        assert_eq!(err.operation(), "submit_form");
    }

    #[test]
    fn test_validation_error_code_matches_registry() {
        let err = validation_error("测试消息", "test_op");
        assert_eq!(err.code(), registry::VALIDATION_FAILED);
    }

    #[test]
    fn test_validation_error_classification_fields() {
        let err = validation_error("测试", "op");
        assert_eq!(err.severity(), Severity::WARNING);
        assert_eq!(err.impact_scope(), ImpactScope::OPERATION);
        assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    }

    #[test]
    fn test_invalid_record_id_valid_input_returns_usr_source() {
        let err = invalid_record_id("abc-123");
        assert_eq!(err.source(), ErrorSource::USR);
        assert!(err.code().starts_with("ERR-USR-VAL"));
        assert!(err.message().contains("abc-123"));
        assert!(err.message().contains("无效的 RecordID 格式"));
    }

    #[test]
    fn test_invalid_record_id_code_matches_registry() {
        let err = invalid_record_id("bad-id");
        assert_eq!(err.code(), registry::INVALID_RECORD_ID);
    }

    #[test]
    fn test_invalid_record_id_operation_is_validate_record_id() {
        let err = invalid_record_id("bad-id");
        assert_eq!(err.operation(), "validate_record_id");
    }

    #[test]
    fn test_not_found_valid_input_returns_usr_source() {
        let err = not_found("Document", "doc-42");
        assert_eq!(err.source(), ErrorSource::USR);
        assert!(err.code().starts_with("ERR-USR-VAL"));
        assert!(err.message().contains("Document"));
        assert!(err.message().contains("doc-42"));
        assert!(err.message().contains("资源不存在"));
    }

    #[test]
    fn test_not_found_code_matches_registry() {
        let err = not_found("User", "u1");
        assert_eq!(err.code(), registry::NOT_FOUND);
    }

    #[test]
    fn test_not_found_operation_is_lookup() {
        let err = not_found("Item", "i1");
        assert_eq!(err.operation(), "lookup");
    }

    #[test]
    fn test_db_error_valid_input_returns_fs_source() {
        let err = db_error("连接超时");
        assert_eq!(err.source(), ErrorSource::FS);
        assert!(err.code().starts_with("ERR-FS-DB"));
        assert!(err.message().contains("连接超时"));
    }

    #[test]
    fn test_db_error_code_matches_registry() {
        let err = db_error("查询失败");
        assert_eq!(err.code(), registry::DB_QUERY_FAILED);
    }

    #[test]
    fn test_db_error_classification_fields() {
        let err = db_error("测试");
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.impact_scope(), ImpactScope::OPERATION);
        assert_eq!(err.recoverability(), Recoverability::SemiAuto);
    }

    #[test]
    fn test_parse_error_valid_input_returns_fs_source() {
        let err = parse_error("JSON 格式错误");
        assert_eq!(err.source(), ErrorSource::FS);
        assert!(err.code().starts_with("ERR-FS-PARSE"));
        assert!(err.message().contains("JSON 格式错误"));
    }

    #[test]
    fn test_parse_error_code_matches_registry() {
        let err = parse_error("解析失败");
        assert_eq!(err.code(), registry::PARSE_FAILED);
    }

    #[test]
    fn test_parse_error_operation_is_parse() {
        let err = parse_error("测试");
        assert_eq!(err.operation(), "parse");
    }

    #[test]
    fn test_unsupported_format_valid_input_returns_usr_source() {
        let err = unsupported_format("xlsx");
        assert_eq!(err.source(), ErrorSource::USR);
        assert!(err.code().starts_with("ERR-USR-PARSE"));
        assert!(err.message().contains("xlsx"));
        assert!(err.message().contains("不支持的文件格式"));
    }

    #[test]
    fn test_unsupported_format_code_matches_registry() {
        let err = unsupported_format("bmp");
        assert_eq!(err.code(), registry::UNSUPPORTED_FORMAT);
    }

    #[test]
    fn test_unsupported_format_classification_fields() {
        let err = unsupported_format("exe");
        assert_eq!(err.severity(), Severity::WARNING);
        assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    }

    #[test]
    fn test_config_error_valid_input_returns_cfg_source() {
        let err = config_error("端口配置缺失");
        assert_eq!(err.source(), ErrorSource::CFG);
        assert!(err.code().starts_with("ERR-CFG-CONF"));
        assert!(err.message().contains("端口配置缺失"));
    }

    #[test]
    fn test_config_error_code_matches_registry() {
        let err = config_error("配置项无效");
        assert_eq!(err.code(), registry::CONFIG_FAILED);
    }

    #[test]
    fn test_config_error_classification_fields() {
        let err = config_error("测试");
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.recoverability(), Recoverability::ManualIntervention);
    }

    #[test]
    fn test_auth_error_valid_input_returns_sec_source() {
        let err = auth_error("令牌过期", "login");
        assert_eq!(err.source(), ErrorSource::SEC);
        assert!(err.code().starts_with("ERR-SEC-AUTH"));
        assert!(err.message().contains("令牌过期"));
        assert_eq!(err.operation(), "login");
    }

    #[test]
    fn test_auth_error_code_matches_registry() {
        let err = auth_error("认证失败", "auth");
        assert_eq!(err.code(), registry::AUTH_FAILED);
    }

    #[test]
    fn test_auth_error_classification_fields() {
        let err = auth_error("测试", "op");
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    }

    #[test]
    fn test_internal_error_valid_input_returns_int_source() {
        let err = internal_error("空指针引用");
        assert_eq!(err.source(), ErrorSource::INT);
        assert!(err.code().starts_with("ERR-INT-SYS"));
        assert!(err.message().contains("空指针引用"));
    }

    #[test]
    fn test_internal_error_code_matches_registry() {
        let err = internal_error("内部错误");
        assert_eq!(err.code(), registry::INTERNAL_ERROR);
    }

    #[test]
    fn test_internal_error_classification_fields() {
        let err = internal_error("测试");
        assert_eq!(err.severity(), Severity::CRITICAL);
        assert_eq!(err.impact_scope(), ImpactScope::GLOBAL);
        assert_eq!(err.recoverability(), Recoverability::ManualIntervention);
    }

    #[test]
    fn test_connection_lost_returns_net_source() {
        let err = connection_lost();
        assert_eq!(err.source(), ErrorSource::NET);
        assert!(err.code().starts_with("ERR-NET-NET"));
        assert!(err.message().contains("网络连接中断"));
    }

    #[test]
    fn test_connection_lost_code_matches_registry() {
        let err = connection_lost();
        assert_eq!(err.code(), registry::CONNECTION_LOST);
    }

    #[test]
    fn test_connection_lost_classification_fields() {
        let err = connection_lost();
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.impact_scope(), ImpactScope::SESSION);
        assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    }

    #[test]
    fn test_crypto_error_valid_input_returns_sec_source() {
        let err = crypto_error("AES 解密失败");
        assert_eq!(err.source(), ErrorSource::SEC);
        assert!(err.code().starts_with("ERR-SEC-CRYPTO"));
        assert!(err.message().contains("AES 解密失败"));
        assert!(err.message().contains("加密/解密失败"));
    }

    #[test]
    fn test_crypto_error_code_matches_registry() {
        let err = crypto_error("加密失败");
        assert_eq!(err.code(), registry::CRYPTO_FAILED);
    }

    #[test]
    fn test_crypto_error_classification_fields() {
        let err = crypto_error("测试");
        assert_eq!(err.severity(), Severity::CRITICAL);
        assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    }

    #[test]
    fn test_serde_error_valid_input_returns_int_source() {
        let err = serde_error("JSON 反序列化失败");
        assert_eq!(err.source(), ErrorSource::INT);
        assert!(err.code().starts_with("ERR-INT-SERDE"));
        assert!(err.message().contains("JSON 反序列化失败"));
        assert!(err.message().contains("序列化/反序列化失败"));
    }

    #[test]
    fn test_serde_error_code_matches_registry() {
        let err = serde_error("序列化失败");
        assert_eq!(err.code(), registry::SERDE_FAILED);
    }

    #[test]
    fn test_serde_error_operation_is_serialize() {
        let err = serde_error("测试");
        assert_eq!(err.operation(), "serialize");
    }

    #[test]
    fn test_net_api_error_valid_input_returns_net_source() {
        let err = net_api_error("远程服务 503");
        assert_eq!(err.source(), ErrorSource::NET);
        assert!(err.code().starts_with("ERR-NET-API"));
        assert!(err.message().contains("远程服务 503"));
    }

    #[test]
    fn test_net_api_error_code_matches_registry() {
        let err = net_api_error("API 错误");
        assert_eq!(err.code(), registry::NET_API_ERROR);
    }

    #[test]
    fn test_net_api_error_classification_fields() {
        let err = net_api_error("测试");
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    }

    #[test]
    fn test_api_deserialize_error_valid_input_returns_int_source() {
        let err = api_deserialize_error("响应体格式错误");
        assert_eq!(err.source(), ErrorSource::INT);
        assert!(err.code().starts_with("ERR-INT-API"));
        assert!(err.message().contains("响应体格式错误"));
        assert!(err.message().contains("数据反序列化失败"));
    }

    #[test]
    fn test_api_deserialize_error_code_matches_registry() {
        let err = api_deserialize_error("反序列化失败");
        assert_eq!(err.code(), registry::API_DESERIALIZE_ERROR);
    }

    #[test]
    fn test_api_deserialize_error_operation_is_deserialize() {
        let err = api_deserialize_error("测试");
        assert_eq!(err.operation(), "deserialize");
    }

    #[test]
    fn test_api_request_error_valid_input_returns_int_source() {
        let err = api_request_error("请求构建失败", "create_user");
        assert_eq!(err.source(), ErrorSource::INT);
        assert!(err.code().starts_with("ERR-INT-API"));
        assert!(err.message().contains("请求构建失败"));
        assert_eq!(err.operation(), "create_user");
    }

    #[test]
    fn test_api_request_error_code_matches_registry() {
        let err = api_request_error("请求错误", "op");
        assert_eq!(err.code(), registry::API_REQUEST_ERROR);
    }

    #[test]
    fn test_api_request_error_classification_fields() {
        let err = api_request_error("测试", "op");
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    }

    #[test]
    fn test_ws_client_error_valid_input_returns_net_source() {
        let err = ws_client_error("连接被拒绝", "connect");
        assert_eq!(err.source(), ErrorSource::NET);
        assert!(err.code().starts_with("ERR-NET-WSCL"));
        assert!(err.message().contains("连接被拒绝"));
        assert_eq!(err.operation(), "connect");
    }

    #[test]
    fn test_ws_client_error_code_matches_registry() {
        let err = ws_client_error("WS 错误", "op");
        assert_eq!(err.code(), registry::WS_CLIENT_ERROR);
    }

    #[test]
    fn test_ws_client_error_classification_fields() {
        let err = ws_client_error("测试", "op");
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    }

    #[test]
    fn test_ws_serialize_error_valid_input_returns_int_source() {
        let err = ws_serialize_error("消息编码失败");
        assert_eq!(err.source(), ErrorSource::INT);
        assert!(err.code().starts_with("ERR-INT-WSCL"));
        assert!(err.message().contains("消息编码失败"));
        assert!(err.message().contains("序列化失败"));
    }

    #[test]
    fn test_ws_serialize_error_code_matches_registry() {
        let err = ws_serialize_error("序列化错误");
        assert_eq!(err.code(), registry::WS_SERIALIZE_ERROR);
    }

    #[test]
    fn test_ws_serialize_error_operation_is_serialize() {
        let err = ws_serialize_error("测试");
        assert_eq!(err.operation(), "serialize");
    }

    #[test]
    fn test_ws_subscribe_limit_error_valid_input_returns_sess_source() {
        let err = ws_subscribe_limit_error(100);
        assert_eq!(err.source(), ErrorSource::SESS);
        assert!(err.code().starts_with("ERR-SESS-SUB"));
        assert!(err.message().contains("100"));
        assert!(err.message().contains("单连接订阅数已达上限"));
    }

    #[test]
    fn test_ws_subscribe_limit_error_code_matches_registry() {
        let err = ws_subscribe_limit_error(50);
        assert_eq!(err.code(), registry::WS_SUBSCRIBE_FAILED);
    }

    #[test]
    fn test_ws_subscribe_limit_error_classification_fields() {
        let err = ws_subscribe_limit_error(10);
        assert_eq!(err.severity(), Severity::ERROR);
        assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    }

    #[test]
    fn test_ws_send_failed_error_valid_input_returns_net_source() {
        let err = ws_send_failed_error("无活跃订阅者");
        assert_eq!(err.source(), ErrorSource::NET);
        assert!(err.code().starts_with("ERR-NET-WS"));
        assert!(err.message().contains("无活跃订阅者"));
    }

    #[test]
    fn test_ws_send_failed_error_code_matches_registry() {
        let err = ws_send_failed_error("发送失败");
        assert_eq!(err.code(), registry::WS_SEND_FAILED);
    }

    #[test]
    fn test_ws_send_failed_error_classification_fields() {
        let err = ws_send_failed_error("测试");
        assert_eq!(err.severity(), Severity::WARNING);
        assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    }

    #[test]
    fn test_ws_receive_failed_error_valid_input_returns_net_source() {
        let err = ws_receive_failed_error("广播目标未找到");
        assert_eq!(err.source(), ErrorSource::NET);
        assert!(err.code().starts_with("ERR-NET-WS"));
        assert!(err.message().contains("广播目标未找到"));
    }

    #[test]
    fn test_ws_receive_failed_error_code_matches_registry() {
        let err = ws_receive_failed_error("接收失败");
        assert_eq!(err.code(), registry::WS_RECEIVE_FAILED);
    }

    #[test]
    fn test_ws_receive_failed_error_classification_fields() {
        let err = ws_receive_failed_error("测试");
        assert_eq!(err.severity(), Severity::WARNING);
        assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    }

    #[test]
    fn test_all_helpers_error_code_format_matches_pattern() {
        let cases: Vec<(String, ErrorSource)> = vec![
            (validation_error("m", "op").code().to_string(), ErrorSource::USR),
            (invalid_record_id("m").code().to_string(), ErrorSource::USR),
            (not_found("r", "id").code().to_string(), ErrorSource::USR),
            (db_error("m").code().to_string(), ErrorSource::FS),
            (parse_error("m").code().to_string(), ErrorSource::FS),
            (unsupported_format("m").code().to_string(), ErrorSource::USR),
            (config_error("m").code().to_string(), ErrorSource::CFG),
            (auth_error("m", "op").code().to_string(), ErrorSource::SEC),
            (internal_error("m").code().to_string(), ErrorSource::INT),
            (connection_lost().code().to_string(), ErrorSource::NET),
            (crypto_error("m").code().to_string(), ErrorSource::SEC),
            (serde_error("m").code().to_string(), ErrorSource::INT),
            (net_api_error("m").code().to_string(), ErrorSource::NET),
            (api_deserialize_error("m").code().to_string(), ErrorSource::INT),
            (api_request_error("m", "op").code().to_string(), ErrorSource::INT),
            (ws_client_error("m", "op").code().to_string(), ErrorSource::NET),
            (ws_serialize_error("m").code().to_string(), ErrorSource::INT),
            (ws_subscribe_limit_error(1).code().to_string(), ErrorSource::SESS),
            (ws_send_failed_error("m").code().to_string(), ErrorSource::NET),
            (ws_receive_failed_error("m").code().to_string(), ErrorSource::NET),
        ];
        for (code, expected_source) in cases {
            assert!(
                code.starts_with("ERR-"),
                "错误码 '{code}' 不以 'ERR-' 开头",
            );
            assert!(
                code.contains(&format!("-{expected_source}-")),
                "错误码 '{code}' 不包含来源 '-{expected_source}-'",
            );
            assert!(
                code.split('_').count() >= 2,
                "错误码 '{code}' 格式不正确，缺少 '_' 分隔的严重性/影响域",
            );
        }
    }
}
