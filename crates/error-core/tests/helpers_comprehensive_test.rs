use error_core::classification::{ErrorSource, ImpactScope, Recoverability, Severity};
use error_core::error_code::registry;
use error_core::helpers;

#[test]
fn test_llm_api_error_code_matches_registry() {
    let err = helpers::llm_api_error("模型调用超时", "generate");
    assert_eq!(err.code(), registry::LLM_API_FAILED);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.impact_scope(), ImpactScope::OPERATION);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("模型调用超时"));
    assert_eq!(err.user_message(), "LLM 调用失败，请稍后重试");
    assert_eq!(err.module_path(), "llm");
    assert_eq!(err.operation(), "generate");
}

#[test]
fn test_llm_judgment_error_code_matches_registry() {
    let err = helpers::llm_judgment_error("判决服务不可用", "judge");
    assert_eq!(err.code(), registry::LLM_JUDGMENT_FAILED);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("判决服务不可用"));
    assert_eq!(err.operation(), "judge");
}

#[test]
fn test_metric_calc_error_code_matches_registry() {
    let err = helpers::metric_calc_error("除零错误");
    assert_eq!(err.code(), registry::METRIC_CALC_FAILED);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("除零错误"));
    assert_eq!(err.module_path(), "metrics");
    assert_eq!(err.operation(), "calculate");
}

#[test]
fn test_dataset_load_error_code_matches_registry() {
    let err = helpers::dataset_load_error("文件不存在");
    assert_eq!(err.code(), registry::DATASET_LOAD_FAILED);
    assert_eq!(err.source(), ErrorSource::FS);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.impact_scope(), ImpactScope::SESSION);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("文件不存在"));
    assert_eq!(err.module_path(), "dataset");
    assert_eq!(err.operation(), "load");
}

#[test]
fn test_eval_timeout_error_code_matches_registry() {
    let err = helpers::eval_timeout_error("评估超时 30s");
    assert_eq!(err.code(), registry::EVAL_TIMEOUT);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("评估超时 30s"));
    assert_eq!(err.module_path(), "evaluator");
    assert_eq!(err.operation(), "evaluate");
}

#[test]
fn test_io_error_code_matches_registry() {
    let err = helpers::io_error("磁盘读取失败");
    assert_eq!(err.code(), registry::IO_FAILED);
    assert_eq!(err.source(), ErrorSource::FS);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("磁盘读取失败"));
    assert!(err.message().contains("I/O 操作失败"));
    assert_eq!(err.module_path(), "io");
    assert_eq!(err.operation(), "io_operation");
}

#[test]
fn test_token_invalid_error_code_matches_registry() {
    let err = helpers::token_invalid_error("JWT 过期");
    assert_eq!(err.code(), registry::TOKEN_INVALID);
    assert_eq!(err.source(), ErrorSource::SEC);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("JWT 过期"));
    assert!(err.message().contains("令牌无效或已过期"));
    assert_eq!(err.module_path(), "auth");
    assert_eq!(err.operation(), "token_validate");
}

#[test]
fn test_general_fallback_error_code_matches_registry() {
    let err = helpers::general_fallback_error("未知错误");
    assert_eq!(err.code(), registry::GENERAL_FALLBACK);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("未知错误"));
    assert_eq!(err.module_path(), "system");
    assert_eq!(err.operation(), "fallback");
}

#[test]
fn test_frontend_ui_error_code_matches_registry() {
    let err = helpers::frontend_ui_error("渲染失败");
    assert_eq!(err.code(), registry::FRONTEND_UI_ERROR);
    assert_eq!(err.source(), ErrorSource::USR);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("渲染失败"));
    assert_eq!(err.module_path(), "ui");
    assert_eq!(err.operation(), "render");
}

#[test]
fn test_gateway_error_code_matches_registry() {
    let err = helpers::gateway_error("上游服务不可用");
    assert_eq!(err.code(), registry::GATEWAY_ERROR);
    assert_eq!(err.source(), ErrorSource::NET);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.impact_scope(), ImpactScope::SESSION);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("上游服务不可用"));
    assert_eq!(err.module_path(), "gateway");
    assert_eq!(err.operation(), "proxy");
}

#[test]
fn test_business_logic_error_code_matches_registry() {
    let err = helpers::business_logic_error("余额不足", "withdraw");
    assert_eq!(err.code(), registry::BUSINESS_LOGIC_ERROR);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.impact_scope(), ImpactScope::MODULE);
    assert_eq!(err.recoverability(), Recoverability::ManualIntervention);
    assert!(err.message().contains("余额不足"));
    assert_eq!(err.operation(), "withdraw");
}

#[test]
fn test_infrastructure_error_code_matches_registry() {
    let err = helpers::infrastructure_error("DNS 解析失败");
    assert_eq!(err.code(), registry::INFRASTRUCTURE_ERROR);
    assert_eq!(err.source(), ErrorSource::SYS);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.impact_scope(), ImpactScope::GLOBAL);
    assert_eq!(err.recoverability(), Recoverability::ManualIntervention);
    assert!(err.message().contains("DNS 解析失败"));
    assert_eq!(err.module_path(), "infrastructure");
    assert_eq!(err.operation(), "system");
}

#[test]
fn test_extraction_error_code_matches_registry() {
    let err = helpers::extraction_error("实体抽取超时");
    assert_eq!(err.code(), registry::EXTRACTION_FAILED);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("实体抽取超时"));
    assert_eq!(err.module_path(), "extractor");
    assert_eq!(err.operation(), "extract");
}

#[test]
fn test_disambiguation_error_code_matches_registry() {
    let err = helpers::disambiguation_error("同名实体无法区分");
    assert_eq!(err.code(), registry::DISAMBIGUATION_FAILED);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("同名实体无法区分"));
    assert_eq!(err.module_path(), "disambiguator");
    assert_eq!(err.operation(), "disambiguate");
}

#[test]
fn test_embedding_config_error_code_matches_registry() {
    let err = helpers::embedding_config_error("维度不匹配");
    assert_eq!(err.code(), registry::EMBEDDING_CONFIG_ERROR);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("维度不匹配"));
    assert!(err.message().contains("嵌入模型配置错误"));
    assert_eq!(err.module_path(), "embedding");
    assert_eq!(err.operation(), "configure");
}

#[test]
fn test_embedding_model_not_loaded_code_matches_registry() {
    let err = helpers::embedding_model_not_loaded("GEMMA4 未加载");
    assert_eq!(err.code(), registry::EMBEDDING_MODEL_NOT_LOADED);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("GEMMA4 未加载"));
    assert!(err.message().contains("嵌入模型未加载"));
    assert_eq!(err.module_path(), "embedding");
    assert_eq!(err.operation(), "load");
}

#[test]
fn test_embedding_model_load_failed_code_matches_registry() {
    let err = helpers::embedding_model_load_failed("权重文件损坏");
    assert_eq!(err.code(), registry::EMBEDDING_MODEL_LOAD_FAILED);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::CRITICAL);
    assert_eq!(err.recoverability(), Recoverability::ManualIntervention);
    assert!(err.message().contains("权重文件损坏"));
    assert!(err.message().contains("嵌入模型加载失败"));
    assert_eq!(err.module_path(), "embedding");
    assert_eq!(err.operation(), "load");
}

#[test]
fn test_embedding_inference_failed_code_matches_registry() {
    let err = helpers::embedding_inference_failed("CUDA OOM");
    assert_eq!(err.code(), registry::EMBEDDING_INFERENCE_FAILED);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("CUDA OOM"));
    assert!(err.message().contains("嵌入推理失败"));
    assert_eq!(err.module_path(), "embedding");
    assert_eq!(err.operation(), "infer");
}

#[test]
fn test_embedding_empty_input_code_matches_registry() {
    let err = helpers::embedding_empty_input();
    assert_eq!(err.code(), registry::EMBEDDING_EMPTY_INPUT);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("嵌入计算输入为空"));
    assert_eq!(err.module_path(), "embedding");
    assert_eq!(err.operation(), "infer");
}

#[test]
fn test_embedding_tokenizer_error_code_matches_registry() {
    let err = helpers::embedding_tokenizer_error("词表加载失败");
    assert_eq!(err.code(), registry::EMBEDDING_TOKENIZER_ERROR);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("词表加载失败"));
    assert!(err.message().contains("Tokenizer 错误"));
    assert_eq!(err.module_path(), "embedding");
    assert_eq!(err.operation(), "tokenize");
}

#[test]
fn test_embedding_io_error_code_matches_registry() {
    let err = helpers::embedding_io_error("safetensors 读取失败");
    assert_eq!(err.code(), registry::EMBEDDING_IO_ERROR);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("safetensors 读取失败"));
    assert!(err.message().contains("嵌入模型 I/O 错误"));
    assert_eq!(err.module_path(), "embedding");
    assert_eq!(err.operation(), "io");
}

#[test]
fn test_agent_llm_error_code_matches_registry() {
    let err = helpers::agent_llm_error("API 限流");
    assert_eq!(err.code(), registry::AGENT_LLM_ERROR);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("API 限流"));
    assert!(err.message().contains("LLM 调用失败"));
    assert_eq!(err.module_path(), "agent");
    assert_eq!(err.operation(), "llm_call");
}

#[test]
fn test_agent_tool_error_code_matches_registry() {
    let err = helpers::agent_tool_error("search", "超时");
    assert_eq!(err.code(), registry::AGENT_TOOL_ERROR);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("search"));
    assert!(err.message().contains("超时"));
    assert_eq!(err.operation(), "tool_call");
}

#[test]
fn test_agent_tool_not_found_code_matches_registry() {
    let err = helpers::agent_tool_not_found("calculator");
    assert_eq!(err.code(), registry::AGENT_TOOL_NOT_FOUND);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("calculator"));
    assert!(err.message().contains("工具"));
    assert_eq!(err.operation(), "tool_lookup");
}

#[test]
fn test_agent_max_iterations_code_matches_registry() {
    let err = helpers::agent_max_iterations(10);
    assert_eq!(err.code(), registry::AGENT_MAX_ITERATIONS);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("10"));
    assert!(err.message().contains("最大迭代次数"));
    assert_eq!(err.operation(), "execute");
}

#[test]
fn test_agent_timeout_code_matches_registry() {
    let err = helpers::agent_timeout();
    assert_eq!(err.code(), registry::AGENT_TIMEOUT);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.impact_scope(), ImpactScope::SESSION);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("Agent 执行超时"));
    assert_eq!(err.operation(), "execute");
}

#[test]
fn test_agent_clarification_code_matches_registry() {
    let err = helpers::agent_clarification("请确认操作意图");
    assert_eq!(err.code(), registry::AGENT_CLARIFICATION);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("请确认操作意图"));
    assert!(err.message().contains("需要澄清"));
    assert_eq!(err.operation(), "clarify");
}

#[test]
fn test_agent_parse_action_error_code_matches_registry() {
    let err = helpers::agent_parse_action_error("无效 JSON");
    assert_eq!(err.code(), registry::AGENT_PARSE_ACTION);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("无效 JSON"));
    assert!(err.message().contains("无法解析 LLM 输出的动作"));
    assert_eq!(err.operation(), "parse_action");
}

#[test]
fn test_agent_memory_error_code_matches_registry() {
    let err = helpers::agent_memory_error("上下文溢出");
    assert_eq!(err.code(), registry::AGENT_MEMORY_ERROR);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("上下文溢出"));
    assert!(err.message().contains("记忆系统错误"));
    assert_eq!(err.operation(), "memory");
}

#[test]
fn test_agent_invalid_transition_code_matches_registry() {
    let err = helpers::agent_invalid_transition("Running", "Draft");
    assert_eq!(err.code(), registry::AGENT_INVALID_TRANSITION);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("Running"));
    assert!(err.message().contains("Draft"));
    assert!(err.message().contains("无效的状态转换"));
    assert_eq!(err.operation(), "transition");
}

#[test]
fn test_agent_workflow_error_code_matches_registry() {
    let err = helpers::agent_workflow_error("DAG 循环检测");
    assert_eq!(err.code(), registry::AGENT_WORKFLOW_ERROR);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("DAG 循环检测"));
    assert!(err.message().contains("工作流错误"));
    assert_eq!(err.operation(), "workflow");
}

#[test]
fn test_agent_serialization_error_code_matches_registry() {
    let err = helpers::agent_serialization_error("状态快照失败");
    assert_eq!(err.code(), registry::AGENT_SERIALIZATION_ERROR);
    assert_eq!(err.source(), ErrorSource::AIM);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("状态快照失败"));
    assert!(err.message().contains("序列化错误"));
    assert_eq!(err.operation(), "serialize");
}

#[test]
fn test_agent_permission_denied_code_matches_registry() {
    let err = helpers::agent_permission_denied("无权访问资源");
    assert_eq!(err.code(), registry::AGENT_PERMISSION_DENIED);
    assert_eq!(err.source(), ErrorSource::SEC);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("无权访问资源"));
    assert!(err.message().contains("权限不足"));
    assert_eq!(err.operation(), "authorize");
}

#[test]
fn test_agent_safety_check_failed_code_matches_registry() {
    let err = helpers::agent_safety_check_failed("输出包含敏感信息");
    assert_eq!(err.code(), registry::AGENT_SAFETY_CHECK);
    assert_eq!(err.source(), ErrorSource::SEC);
    assert_eq!(err.severity(), Severity::CRITICAL);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("输出包含敏感信息"));
    assert!(err.message().contains("安全检查未通过"));
    assert_eq!(err.operation(), "safety_check");
}

#[test]
fn test_aggregate_invalid_state_code_matches_registry() {
    let err = helpers::aggregate_invalid_state("已删除的聚合");
    assert_eq!(err.code(), registry::AGGREGATE_INVALID_STATE);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("已删除的聚合"));
    assert!(err.message().contains("聚合状态无效"));
    assert_eq!(err.module_path(), "cqrs");
    assert_eq!(err.operation(), "aggregate");
}

#[test]
fn test_aggregate_business_rule_code_matches_registry() {
    let err = helpers::aggregate_business_rule("不能删除已发布文档");
    assert_eq!(err.code(), registry::AGGREGATE_BUSINESS_RULE);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("不能删除已发布文档"));
    assert!(err.message().contains("业务规则违反"));
    assert_eq!(err.module_path(), "cqrs");
    assert_eq!(err.operation(), "validate");
}

#[test]
fn test_aggregate_not_found_code_matches_registry() {
    let err = helpers::aggregate_not_found("doc-42");
    assert_eq!(err.code(), registry::AGGREGATE_NOT_FOUND);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("doc-42"));
    assert!(err.message().contains("聚合未找到"));
    assert_eq!(err.module_path(), "cqrs");
    assert_eq!(err.operation(), "load");
}

#[test]
fn test_aggregate_version_conflict_code_matches_registry() {
    let err = helpers::aggregate_version_conflict(3, 5);
    assert_eq!(err.code(), registry::AGGREGATE_VERSION_CONFLICT);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains('3'));
    assert!(err.message().contains('5'));
    assert!(err.message().contains("版本冲突"));
    assert_eq!(err.module_path(), "cqrs");
    assert_eq!(err.operation(), "commit");
}

#[test]
fn test_aggregate_serialization_error_code_matches_registry() {
    let err = helpers::aggregate_serialization_error("事件序列化失败");
    assert_eq!(err.code(), registry::AGGREGATE_SERIALIZATION);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("事件序列化失败"));
    assert!(err.message().contains("聚合序列化错误"));
    assert_eq!(err.module_path(), "cqrs");
    assert_eq!(err.operation(), "serialize");
}

#[test]
fn test_cache_l2_connection_error_code_matches_registry() {
    let err = helpers::cache_l2_connection_error("Redis 连接拒绝");
    assert_eq!(err.code(), registry::CACHE_L2_CONNECTION);
    assert_eq!(err.source(), ErrorSource::FS);
    assert_eq!(err.impact_scope(), ImpactScope::SESSION);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("Redis 连接拒绝"));
    assert!(err.message().contains("缓存连接失败"));
    assert_eq!(err.module_path(), "cache");
    assert_eq!(err.operation(), "connect");
}

#[test]
fn test_cache_l2_serialization_error_code_matches_registry() {
    let err = helpers::cache_l2_serialization_error("bincode 编码失败");
    assert_eq!(err.code(), registry::CACHE_L2_SERIALIZATION);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("bincode 编码失败"));
    assert!(err.message().contains("缓存序列化失败"));
    assert_eq!(err.module_path(), "cache");
    assert_eq!(err.operation(), "serialize");
}

#[test]
fn test_cache_l2_deserialization_error_code_matches_registry() {
    let err = helpers::cache_l2_deserialization_error("数据格式不兼容");
    assert_eq!(err.code(), registry::CACHE_L2_DESERIALIZATION);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("数据格式不兼容"));
    assert!(err.message().contains("缓存反序列化失败"));
    assert_eq!(err.module_path(), "cache");
    assert_eq!(err.operation(), "deserialize");
}

#[test]
fn test_cache_l2_operation_error_code_matches_registry() {
    let err = helpers::cache_l2_operation_error("TTL 设置失败");
    assert_eq!(err.code(), registry::CACHE_L2_OPERATION);
    assert_eq!(err.source(), ErrorSource::FS);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("TTL 设置失败"));
    assert!(err.message().contains("缓存操作失败"));
    assert_eq!(err.module_path(), "cache");
    assert_eq!(err.operation(), "operate");
}

#[test]
fn test_cache_manager_l2_error_code_matches_registry() {
    let err = helpers::cache_manager_l2_error("L2 层初始化失败");
    assert_eq!(err.code(), registry::CACHE_MANAGER_L2);
    assert_eq!(err.source(), ErrorSource::FS);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("L2 层初始化失败"));
    assert!(err.message().contains("缓存管理器 L2 错误"));
    assert_eq!(err.module_path(), "cache");
    assert_eq!(err.operation(), "manage");
}

#[test]
fn test_cache_manager_serialization_error_code_matches_registry() {
    let err = helpers::cache_manager_serialization_error("缓存项编码失败");
    assert_eq!(err.code(), registry::CACHE_MANAGER_SERIALIZATION);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("缓存项编码失败"));
    assert!(err.message().contains("缓存管理器序列化错误"));
    assert_eq!(err.module_path(), "cache");
    assert_eq!(err.operation(), "serialize");
}

#[test]
fn test_event_type_mismatch_code_matches_registry() {
    let err = helpers::event_type_mismatch();
    assert_eq!(err.code(), registry::EVENT_TYPE_MISMATCH);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("事件类型不匹配"));
    assert_eq!(err.module_path(), "event");
    assert_eq!(err.operation(), "dispatch");
}

#[test]
fn test_event_processing_failed_code_matches_registry() {
    let err = helpers::event_processing_failed("处理器异常");
    assert_eq!(err.code(), registry::EVENT_PROCESSING_FAILED);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("处理器异常"));
    assert!(err.message().contains("事件处理失败"));
    assert_eq!(err.module_path(), "event");
    assert_eq!(err.operation(), "process");
}

#[test]
fn test_event_no_subscribers_code_matches_registry() {
    let err = helpers::event_no_subscribers("evt-123");
    assert_eq!(err.code(), registry::EVENT_NO_SUBSCRIBERS);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("evt-123"));
    assert!(err.message().contains("没有活跃的订阅者"));
    assert_eq!(err.module_path(), "event");
    assert_eq!(err.operation(), "publish");
}

#[test]
fn test_event_bus_shutdown_code_matches_registry() {
    let err = helpers::event_bus_shutdown();
    assert_eq!(err.code(), registry::EVENT_BUS_SHUTDOWN);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.impact_scope(), ImpactScope::SESSION);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("事件总线已关闭"));
    assert_eq!(err.module_path(), "event");
    assert_eq!(err.operation(), "publish");
}

#[test]
fn test_event_publish_timeout_code_matches_registry() {
    let err = helpers::event_publish_timeout(5000);
    assert_eq!(err.code(), registry::EVENT_PUBLISH_TIMEOUT);
    assert_eq!(err.source(), ErrorSource::INT);
    assert_eq!(err.severity(), Severity::WARNING);
    assert_eq!(err.recoverability(), Recoverability::AutoRecoverable);
    assert!(err.message().contains("5000"));
    assert!(err.message().contains("事件发布超时"));
    assert_eq!(err.module_path(), "event");
    assert_eq!(err.operation(), "publish");
}

#[test]
fn test_observability_forbidden_code_matches_registry() {
    let err = helpers::observability_forbidden("无权访问指标", "metrics_query");
    assert_eq!(err.code(), registry::OBSERVABILITY_FORBIDDEN);
    assert_eq!(err.source(), ErrorSource::SEC);
    assert_eq!(err.severity(), Severity::ERROR);
    assert_eq!(err.recoverability(), Recoverability::NonRecoverable);
    assert!(err.message().contains("无权访问指标"));
    assert_eq!(err.module_path(), "observability");
    assert_eq!(err.operation(), "metrics_query");
}

fn assert_error_code_format(cases: Vec<(String, ErrorSource)>) {
    for (code, expected_source) in cases {
        assert!(code.starts_with("ERR-"), "错误码 '{code}' 不以 'ERR-' 开头",);
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

#[test]
fn test_extended_helpers_error_code_format_aim_source() {
    let cases: Vec<(String, ErrorSource)> = vec![
        (
            helpers::llm_api_error("m", "op").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::llm_judgment_error("m", "op").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::metric_calc_error("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::extraction_error("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::disambiguation_error("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::embedding_config_error("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::embedding_model_not_loaded("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::embedding_model_load_failed("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::embedding_inference_failed("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::embedding_empty_input().code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::embedding_tokenizer_error("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::embedding_io_error("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::agent_llm_error("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::agent_tool_error("t", "e").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::agent_tool_not_found("t").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::agent_max_iterations(5).code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::agent_timeout().code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::agent_clarification("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::agent_parse_action_error("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::agent_memory_error("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::agent_invalid_transition("a", "b")
                .code()
                .to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::agent_workflow_error("m").code().to_string(),
            ErrorSource::AIM,
        ),
        (
            helpers::agent_serialization_error("m").code().to_string(),
            ErrorSource::AIM,
        ),
    ];
    assert_error_code_format(cases);
}

#[test]
fn test_extended_helpers_error_code_format_other_sources() {
    let cases: Vec<(String, ErrorSource)> = vec![
        (
            helpers::dataset_load_error("m").code().to_string(),
            ErrorSource::FS,
        ),
        (
            helpers::eval_timeout_error("m").code().to_string(),
            ErrorSource::INT,
        ),
        (helpers::io_error("m").code().to_string(), ErrorSource::FS),
        (
            helpers::token_invalid_error("m").code().to_string(),
            ErrorSource::SEC,
        ),
        (
            helpers::general_fallback_error("m").code().to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::frontend_ui_error("m").code().to_string(),
            ErrorSource::USR,
        ),
        (
            helpers::gateway_error("m").code().to_string(),
            ErrorSource::NET,
        ),
        (
            helpers::business_logic_error("m", "op").code().to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::infrastructure_error("m").code().to_string(),
            ErrorSource::SYS,
        ),
        (
            helpers::agent_permission_denied("m").code().to_string(),
            ErrorSource::SEC,
        ),
        (
            helpers::agent_safety_check_failed("m").code().to_string(),
            ErrorSource::SEC,
        ),
        (
            helpers::observability_forbidden("m", "c")
                .code()
                .to_string(),
            ErrorSource::SEC,
        ),
    ];
    assert_error_code_format(cases);
}

#[test]
fn test_extended_helpers_error_code_format_aggregate_cache_event() {
    let cases: Vec<(String, ErrorSource)> = vec![
        (
            helpers::aggregate_invalid_state("m").code().to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::aggregate_business_rule("m").code().to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::aggregate_not_found("id").code().to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::aggregate_version_conflict(1, 2).code().to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::aggregate_serialization_error("m")
                .code()
                .to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::cache_l2_connection_error("m").code().to_string(),
            ErrorSource::FS,
        ),
        (
            helpers::cache_l2_serialization_error("m")
                .code()
                .to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::cache_l2_deserialization_error("m")
                .code()
                .to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::cache_l2_operation_error("m").code().to_string(),
            ErrorSource::FS,
        ),
        (
            helpers::cache_manager_l2_error("m").code().to_string(),
            ErrorSource::FS,
        ),
        (
            helpers::cache_manager_serialization_error("m")
                .code()
                .to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::event_type_mismatch().code().to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::event_processing_failed("m").code().to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::event_no_subscribers("e").code().to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::event_bus_shutdown().code().to_string(),
            ErrorSource::INT,
        ),
        (
            helpers::event_publish_timeout(100).code().to_string(),
            ErrorSource::INT,
        ),
    ];
    assert_error_code_format(cases);
}
