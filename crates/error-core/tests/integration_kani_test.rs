//! Integration formal verification tests using Kani

use error_core::error_capture::*;
use error_core::error_object::*;
use error_core::logging::*;
use error_core::propagation::*;
use error_core::recovery::*;
use error_core::user_prompt::*;
use error_core::utils::*;
use std::collections::HashMap;
use serde_json::json;

// Define test error types
#[derive(Debug, thiserror::Error)]
#[error("Test error")]
struct TestError;

#[derive(Debug, thiserror::Error)]
#[error("Network error")]
struct NetworkError;

#[derive(Debug, thiserror::Error)]
#[error("Database error")]
struct DatabaseError;

// Test complete error handling flow
#[kani::proof]
fn test_complete_error_handling_flow() {
    // 1. Capture an error using BusinessErrorCapture
    let business_capture = BusinessErrorCapture::new("order_service", "process_payment");
    let error = TestError;
    let mut captured_error = business_capture.capture_error(&error);
    
    // 2. Add context to the error
    let mut context = HashMap::new();
    context.insert("order_id".to_string(), json!("ord_123"));
    business_capture.add_context(&mut captured_error, context);
    
    // 3. Propagate the error with additional context
    let mut propagation_context = HashMap::new();
    propagation_context.insert("user_id".to_string(), json!("user_456"));
    let propagated_error = ErrorPropagation::append_context(captured_error, "api_gateway", propagation_context);
    
    // 4. Log the error
    LoggingUtils::log_error(&propagated_error);
    
    // 5. Generate a user-friendly prompt
    let prompt_manager = UserPromptManager::new();
    let user_prompt = prompt_manager.get_error_prompt(propagated_error.severity(), propagated_error.message(), None);
    assert!(!user_prompt.is_empty());
    
    // 6. Create a recovery state machine
    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);
    let mut recovery_machine = RecoveryStateMachine::new(3, retry_config);
    
    // 7. Test recovery process
    recovery_machine.start_recovery();
    assert_eq!(*recovery_machine.state(), RecoveryState::Recovering);
    
    // 8. Calculate retry delay
    let retry_delay = recovery_machine.calculate_retry_delay();
    assert!(retry_delay >= std::time::Duration::from_millis(1000));
    
    // 9. Test circuit breaker
    let reset_timeout = std::time::Duration::from_millis(100);
    let mut circuit_breaker = CircuitBreaker::new(2, reset_timeout);
    assert_eq!(*circuit_breaker.state(), CircuitBreakerState::Closed);
    assert!(circuit_breaker.allow_request());
}

// Test error chaining and root cause extraction
#[kani::proof]
fn test_error_chaining_and_root_cause() {
    // 1. Create nested errors
    let db_error = DatabaseError;
    let network_error = NetworkError;
    
    // 2. Capture errors
    let infra_capture = InfrastructureErrorCapture::new("database", "connection");
    let db_captured = infra_capture.capture_error(&db_error);
    
    let gateway_capture = GatewayErrorCapture::new("api_service", "request");
    let network_captured = gateway_capture.capture_error(&network_error);
    
    // 3. Create error chain
    let mut business_error = ErrorObject::builder()
        .code("ERR-BIZ-ORD-001_ERR_S")
        .source(crate::classification::ErrorSource::INT)
        .severity(crate::classification::Severity::ERROR)
        .impact_scope(crate::classification::ImpactScope::SESSION)
        .recoverability(crate::classification::Recoverability::SemiAuto)
        .message("Order processing failed")
        .user_message("Failed to process your order")
        .module_path("order::service")
        .operation("process_order")
        .build();
    
    // Set network error as cause
    business_error.set_cause(network_captured);
    
    // 4. Extract root cause
    let root_cause = ErrorUtils::get_root_cause(&business_error);
    assert!(!root_cause.to_string().is_empty());
    
    // 5. Format error chain
    let error_chain = ErrorUtils::format_error_chain(&business_error);
    assert!(!error_chain.is_empty());
    assert!(error_chain.contains("Order processing failed"));
}

// Test recovery and retry mechanisms
#[kani::proof]
fn test_recovery_and_retry_mechanisms() {
    // 1. Create retry config
    let retry_config = RetryConfig::new(3, 500, 5000, 1.5, true);
    
    // 2. Create exponential backoff
    let mut backoff = ExponentialBackoff::new(retry_config);
    
    // 3. Test backoff delays
    let delay1 = backoff.next_delay().unwrap();
    assert!(delay1 >= std::time::Duration::from_millis(450)); // With jitter
    
    let delay2 = backoff.next_delay().unwrap();
    assert!(delay2 >= std::time::Duration::from_millis(675)); // With jitter
    
    let delay3 = backoff.next_delay().unwrap();
    assert!(delay3 >= std::time::Duration::from_millis(1012)); // With jitter
    
    let delay4 = backoff.next_delay();
    assert!(delay4.is_none());
    
    // 4. Reset backoff
    backoff.reset();
    assert_eq!(backoff.current_attempt(), 0);
    
    let reset_delay = backoff.next_delay().unwrap();
    assert!(reset_delay >= std::time::Duration::from_millis(450));
}

// Test user prompt generation with different locales
#[kani::proof]
fn test_user_prompt_with_locales() {
    // 1. Create and configure prompt manager
    let mut prompt_config = UserPromptConfig::new();
    prompt_config.set_template(crate::classification::Severity::ERROR, "zh-CN", "发生错误: {message}");
    prompt_config.set_template(crate::classification::Severity::ERROR, "fr-FR", "Erreur: {message}");
    
    let mut prompt_manager = UserPromptManager::new();
    prompt_manager.configure(prompt_config);
    
    // 2. Test prompts in different locales
    let en_prompt = prompt_manager.get_error_prompt(
        crate::classification::Severity::ERROR, 
        "Network timeout", 
        Some("en-US")
    );
    assert!(en_prompt.contains("An error occurred. Please try again later."));
    
    let zh_prompt = prompt_manager.get_error_prompt(
        crate::classification::Severity::ERROR, 
        "Network timeout", 
        Some("zh-CN")
    );
    assert_eq!(zh_prompt, "发生错误: Network timeout");
    
    let fr_prompt = prompt_manager.get_error_prompt(
        crate::classification::Severity::ERROR, 
        "Network timeout", 
        Some("fr-FR")
    );
    assert_eq!(fr_prompt, "Erreur: Network timeout");
}

// Test logging and log entry creation
#[kani::proof]
fn test_logging_and_log_entry() {
    // 1. Create an error object
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(crate::classification::ErrorSource::AIM)
        .severity(crate::classification::Severity::ERROR)
        .impact_scope(crate::classification::ImpactScope::SESSION)
        .recoverability(crate::classification::Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .detail("model_name", json!("gpt-4"))
        .session_id("session_123")
        .build();
    
    // 2. Create log entry
    let log_entry = LogEntry::from_error_object(&error);
    assert_eq!(log_entry.error_code, "ERR-AIM-LM-002_ERR_S");
    assert_eq!(log_entry.source, crate::classification::ErrorSource::AIM);
    assert_eq!(log_entry.severity, crate::classification::Severity::ERROR);
    assert_eq!(log_entry.impact_scope, crate::classification::ImpactScope::SESSION);
    assert_eq!(log_entry.recoverability, crate::classification::Recoverability::AutoRecoverable);
    assert_eq!(log_entry.message, "AI model call timed out");
    assert_eq!(log_entry.user_message, "AI model service is temporarily unavailable");
    assert_eq!(log_entry.module_path, "ai_model::lm_manager");
    assert_eq!(log_entry.operation, "generate_code_completion");
    assert!(log_entry.context.contains_key("model_name"));
    assert_eq!(log_entry.session_id, Some("session_123".to_string()));
    assert_eq!(log_entry.level, "ERROR");
}
