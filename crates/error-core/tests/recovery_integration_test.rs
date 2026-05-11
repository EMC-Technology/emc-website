use error_core::error_capture::GatewayErrorCapture;
use error_core::prelude::*;
use error_core::propagation::RetryConfig;
use error_core::recovery::CircuitBreaker;
use error_core::recovery::ExponentialBackoff;
use error_core::recovery::RecoveryStateMachine;
use std::time::Duration;

#[test]
fn test_recovery_integration() {
    // 测试恢复机制集成
    #[derive(Debug, thiserror::Error)]
    #[error("External service temporary unavailable")]
    struct GatewayError;

    let gateway_capture = GatewayErrorCapture::new("api_gateway", "external_service");
    let _error = gateway_capture.capture_error(&GatewayError);

    // 测试恢复状态机
    let retry_config = RetryConfig::new(3, 100, 1000, 2.0, true);
    let mut recovery_state = RecoveryStateMachine::new(3, retry_config.clone());
    assert!(recovery_state.start_recovery().is_ok());
    assert_eq!(
        *recovery_state.state(),
        error_core::recovery::RecoveryState::Recovering
    );

    // 测试指数退避
    let mut backoff = ExponentialBackoff::new(retry_config);
    let first_delay = backoff.next_delay().unwrap();
    let second_delay = backoff.next_delay().unwrap();
    assert!(second_delay > first_delay);

    // 测试断路器
    let mut circuit_breaker = CircuitBreaker::new(5, Duration::from_secs(5));
    assert!(circuit_breaker.allow_request());

    // 模拟失败
    for _ in 0..5 {
        circuit_breaker.record_failure();
    }
    assert!(!circuit_breaker.allow_request());
}
