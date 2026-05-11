#![allow(clippy::items_after_statements)]

use error_core::error_capture::{
    BusinessErrorCapture, FrontendErrorCapture, GatewayErrorCapture, InfrastructureErrorCapture,
};
use error_core::logging::{LogFileManager, LoggingUtils};
use error_core::prelude::*;
use error_core::propagation::{ContextFrame, RecoveryAction, RecoveryHint, RetryConfig};
use error_core::recovery::{CircuitBreaker, RecoveryStateMachine};
use error_core::user_prompt::{UserPromptConfig, UserPromptManager};
use error_core::utils::ErrorUtils;
use std::collections::HashMap;
use std::time::Duration;

/// 测试完整的错误处理端到端流程
#[test]
fn test_full_error_lifecycle_e2e() {
    // 步骤1: 模拟基础设施层错误（数据库连接失败）
    #[derive(Debug, thiserror::Error)]
    #[error("Database connection timeout")]
    struct DatabaseTimeoutError;

    let db_capture = InfrastructureErrorCapture::new("database", "connection");
    let db_error = db_capture.capture_error(&DatabaseTimeoutError);

    // 验证数据库错误的基本属性
    assert_eq!(db_error.source(), ErrorSource::SYS);
    assert_eq!(db_error.severity(), Severity::ERROR);

    // 步骤2: 模拟业务逻辑层错误（依赖数据库错误）
    #[derive(Debug, thiserror::Error)]
    #[error("Failed to retrieve user data")]
    struct UserDataError;

    let business_capture = BusinessErrorCapture::new("user_service", "get_user");
    let business_error = business_capture.capture_error(&UserDataError);

    // 步骤3: 传播错误，附加上下文
    let propagated_error =
        error_core::error_capture::ErrorPropagation::wrap_error(business_error, db_error);

    // 验证错误传播链
    assert!(propagated_error.cause().is_some());
    assert_eq!(propagated_error.source(), ErrorSource::INT);

    // 步骤4: 记录错误日志
    LoggingUtils::log_error(&propagated_error);

    // 步骤5: 测试用户提示
    let prompt_manager = UserPromptManager::default();
    let user_prompt = prompt_manager.get_error_prompt(
        Severity::ERROR,
        "Failed to retrieve user data",
        Some("zh-CN"),
    );
    assert!(!user_prompt.is_empty());

    // 步骤6: 测试错误恢复机制
    let retry_config = RetryConfig::new(3, 100, 1000, 2.0, true);
    let mut recovery_state = RecoveryStateMachine::new(3, retry_config);

    assert!(recovery_state.start_recovery().is_ok());
    assert_eq!(
        *recovery_state.state(),
        error_core::recovery::RecoveryState::Recovering
    );

    // 模拟重试失败，连续3次失败后状态应变为Failed
    assert!(recovery_state.recover_failed().is_ok());
    assert!(recovery_state.recover_failed().is_ok());
    assert!(recovery_state.recover_failed().is_ok());

    assert_eq!(
        *recovery_state.state(),
        error_core::recovery::RecoveryState::Failed
    );
}

/// 测试不同层级的错误捕获和处理
#[test]
fn test_multi_layer_error_capture_e2e() {
    // 前端错误
    #[derive(Debug, thiserror::Error)]
    #[error("User input validation failed")]
    struct FrontendValidationError;

    let frontend_capture = FrontendErrorCapture::new("ui");
    let frontend_error = frontend_capture.capture_error(&FrontendValidationError);

    // API网关错误
    #[derive(Debug, thiserror::Error)]
    #[error("API request timeout")]
    struct GatewayTimeoutError;

    let gateway_capture = GatewayErrorCapture::new("api_gateway", "external_service");
    let gateway_error = gateway_capture.capture_error(&GatewayTimeoutError);

    // 业务逻辑错误
    #[derive(Debug, thiserror::Error)]
    #[error("Business logic validation failed")]
    struct BusinessValidationError;

    let business_capture = BusinessErrorCapture::new("order_service", "validate_order");
    let business_error = business_capture.capture_error(&BusinessValidationError);

    // 基础设施错误
    #[derive(Debug, thiserror::Error)]
    #[error("Network connection failed")]
    struct NetworkError;

    let infra_capture = InfrastructureErrorCapture::new("network", "connection");
    let infra_error = infra_capture.capture_error(&NetworkError);

    // 验证所有错误都有正确的来源
    assert_eq!(frontend_error.source(), ErrorSource::USR);
    assert_eq!(gateway_error.source(), ErrorSource::NET);
    assert_eq!(business_error.source(), ErrorSource::INT);
    assert_eq!(infra_error.source(), ErrorSource::SYS);
}

/// 测试错误恢复和断路器模式
#[test]
fn test_recovery_mechanism_e2e() {
    // 创建重试配置
    let retry_config = RetryConfig::new(3, 100, 1000, 2.0, true);
    let mut recovery_state = RecoveryStateMachine::new(3, retry_config);

    // 测试恢复流程
    assert!(recovery_state.start_recovery().is_ok());
    assert_eq!(
        *recovery_state.state(),
        error_core::recovery::RecoveryState::Recovering
    );

    // 模拟第一次重试失败
    assert!(recovery_state.recover_failed().is_ok());
    let delay = recovery_state.calculate_retry_delay();
    assert!(delay > Duration::from_millis(0));

    // 模拟第二次重试成功
    assert!(recovery_state.recover_success().is_ok());
    assert_eq!(
        *recovery_state.state(),
        error_core::recovery::RecoveryState::Recovered
    );

    // 测试断路器
    let mut circuit_breaker = CircuitBreaker::new(3, Duration::from_secs(1));

    // 模拟失败率超过阈值
    for _ in 0..3 {
        circuit_breaker.record_failure();
    }

    // 断路器应该打开
    assert_eq!(
        *circuit_breaker.state(),
        error_core::recovery::CircuitBreakerState::Open
    );

    // 模拟时间过去，断路器应该半开
    std::thread::sleep(Duration::from_secs(2));
    assert!(circuit_breaker.allow_request());
}

/// 测试错误日志和文件管理
#[test]
fn test_error_logging_e2e() {
    // 创建测试错误
    #[derive(Debug, thiserror::Error)]
    #[error("Test error for logging")]
    struct TestLoggingError;

    let capture = BusinessErrorCapture::new("test_service", "test_operation");
    let error = capture.capture_error(&TestLoggingError);

    // 测试日志记录
    LoggingUtils::log_error(&error);

    // 测试日志文件管理
    let log_manager = LogFileManager::new("./logs");
    let error_log_path = log_manager.get_log_path(&tracing::Level::ERROR);
    assert!(!error_log_path.is_empty());
}

/// 测试用户提示和多语言支持
#[test]
fn test_user_prompt_e2e() {
    let mut prompt_config = UserPromptConfig::new();
    prompt_config.set_default_locale("en-US");

    let mut prompt_manager = UserPromptManager::new();
    prompt_manager.configure(prompt_config);

    // 测试英文提示
    let en_prompt =
        prompt_manager.get_error_prompt(Severity::ERROR, "Network error", Some("en-US"));
    assert!(!en_prompt.is_empty());

    // 测试中文提示
    let zh_prompt =
        prompt_manager.get_error_prompt(Severity::ERROR, "Network error", Some("zh-CN"));
    assert!(!zh_prompt.is_empty());

    // 测试默认提示
    let default_prompt = prompt_manager.get_error_prompt(Severity::ERROR, "Network error", None);
    assert!(!default_prompt.is_empty());
}

/// 测试错误工具函数
#[test]
fn test_error_utils_e2e() {
    // 创建嵌套错误
    #[derive(Debug, thiserror::Error)]
    #[error("Root error")]
    #[allow(dead_code)]
    struct RootError;

    #[derive(Debug, thiserror::Error)]
    #[error("Middle error")]
    #[allow(dead_code)]
    struct MiddleError;

    #[derive(Debug, thiserror::Error)]
    #[error("Top error")]
    struct TopError;

    // 测试获取根错误
    let root_cause = ErrorUtils::get_root_cause(&TopError);
    assert!(!root_cause.to_string().is_empty());

    // 测试格式化错误链
    let error_chain = ErrorUtils::format_error_chain(&TopError);
    assert!(!error_chain.is_empty());
}

/// 测试错误对象构建和验证
#[test]
fn test_error_object_builder_e2e() {
    use serde_json::json;

    // 构建完整的错误对象
    let mut context1 = HashMap::new();
    context1.insert("username".to_string(), json!("test"));

    let mut context2 = HashMap::new();
    context2.insert("token".to_string(), json!("invalid"));

    let mut hint_params = HashMap::new();
    hint_params.insert("action".to_string(), json!("RETRY"));

    let error = ErrorObject::builder()
        .code("ERR-INT-USER-001_ERR_S")
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("User not found")
        .user_message("The user account does not exist")
        .module_path("user_service")
        .operation("login")
        .context_frame(ContextFrame::new("user_service", context1))
        .context_frame(ContextFrame::new("auth_service", context2))
        .recovery_hint(RecoveryHint::new(
            RecoveryAction::Reauthenticate,
            "Please verify your username and password",
            hint_params,
        ))
        .build();

    // 验证错误对象属性
    assert_eq!(error.code(), "ERR-INT-USER-001_ERR_S");
    assert_eq!(error.source(), ErrorSource::INT);
    assert_eq!(error.severity(), Severity::ERROR);
    assert_eq!(error.impact_scope(), ImpactScope::SESSION);
    assert_eq!(error.recoverability(), Recoverability::AutoRecoverable);
    assert_eq!(error.message(), "User not found");
    assert_eq!(error.user_message(), "The user account does not exist");
    assert_eq!(error.context_chain().len(), 2);
    assert_eq!(error.recovery_hints().len(), 1);
}
