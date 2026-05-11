use error_core::error_capture::BusinessErrorCapture;
use error_core::error_capture::GatewayErrorCapture;
use error_core::error_capture::InfrastructureErrorCapture;
use error_core::logging::LoggingUtils;
use error_core::prelude::*;
use error_core::propagation::RetryConfig;
use error_core::recovery::RecoveryStateMachine;

#[test]
fn test_full_error_handling_flow() {
    // 测试完整的错误处理流程
    #[derive(Debug, thiserror::Error)]
    #[error("User not found")]
    struct TestError;

    let capture = BusinessErrorCapture::new("user_service", "login");
    let error = capture.capture_error(&TestError);

    // 测试错误日志
    LoggingUtils::log_error(&error);

    // 测试用户提示
    let prompt = UserPromptManager::default().get_error_prompt(
        Severity::ERROR,
        "User not found",
        Some("zh-CN"),
    );
    assert!(!prompt.is_empty());
}

#[test]
fn test_error_propagation_chain() {
    // 测试错误传播链
    #[derive(Debug, thiserror::Error)]
    #[error("Database connection failed")]
    struct DatabaseError;

    #[derive(Debug, thiserror::Error)]
    #[error("Failed to retrieve user data")]
    struct BusinessError;

    let db_capture = InfrastructureErrorCapture::new("database", "connection");
    let root_error = db_capture.capture_error(&DatabaseError);

    let business_capture = BusinessErrorCapture::new("user_service", "get_user");
    let business_error = business_capture.capture_error(&BusinessError);

    let propagated_error =
        error_core::error_capture::ErrorPropagation::wrap_error(business_error, root_error);
    assert!(propagated_error.cause().is_some());
}

#[test]
fn test_error_recovery_mechanism() {
    // 测试错误恢复机制
    #[derive(Debug, thiserror::Error)]
    #[error("External service temporary unavailable")]
    struct GatewayError;

    let gateway_capture = GatewayErrorCapture::new("api_gateway", "external_service");
    let _error = gateway_capture.capture_error(&GatewayError);

    let retry_config = RetryConfig::new(3, 100, 1000, 2.0, true);
    let mut recovery_state = RecoveryStateMachine::new(3, retry_config);
    assert!(recovery_state.start_recovery().is_ok());
    assert_eq!(
        *recovery_state.state(),
        error_core::recovery::RecoveryState::Recovering
    );
}
