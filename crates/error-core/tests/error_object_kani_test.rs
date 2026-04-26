//! Error object module formal verification tests using Kani

use error_core::error_object::*;
use error_core::classification::*;
use error_core::propagation::{ContextFrame, RecoveryHint, RetryConfig};
use std::collections::HashMap;

// Test ErrorObjectBuilder::build with all required fields
#[kani::proof]
fn test_error_object_build_valid() {
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build();

    assert_eq!(error.code(), "ERR-AIM-LM-002_ERR_S");
    assert_eq!(error.source(), ErrorSource::AIM);
    assert_eq!(error.severity(), Severity::ERROR);
    assert_eq!(error.impact_scope(), ImpactScope::SESSION);
    assert_eq!(error.recoverability(), Recoverability::AutoRecoverable);
    assert_eq!(error.message(), "AI model call timed out");
    assert_eq!(error.user_message(), "AI model service is temporarily unavailable");
    assert_eq!(error.module_path(), "ai_model::lm_manager");
    assert_eq!(error.operation(), "generate_code_completion");
}

// Test ErrorObjectBuilder::build_checked with missing required fields
#[kani::proof]
fn test_error_object_build_missing_required_fields() {
    // Test missing code
    let error = ErrorObject::builder()
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(error.is_err());

    // Test missing source
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(error.is_err());

    // Test missing severity
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(error.is_err());

    // Test missing impact_scope
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(error.is_err());

    // Test missing recoverability
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(error.is_err());

    // Test missing message
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(error.is_err());

    // Test missing user_message
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(error.is_err());

    // Test missing module_path
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .operation("generate_code_completion")
        .build_checked();
    assert!(error.is_err());

    // Test missing operation
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .build_checked();
    assert!(error.is_err());
}

// Test ErrorObject getter methods
#[kani::proof]
fn test_error_object_getters() {
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .session_id("session_123")
        .request_id("request_456")
        .detail("model_name", serde_json::json!("gpt-4"))
        .build();

    assert_eq!(error.code(), "ERR-AIM-LM-002_ERR_S");
    assert_eq!(error.source(), ErrorSource::AIM);
    assert_eq!(error.severity(), Severity::ERROR);
    assert_eq!(error.impact_scope(), ImpactScope::SESSION);
    assert_eq!(error.recoverability(), Recoverability::AutoRecoverable);
    assert_eq!(error.message(), "AI model call timed out");
    assert_eq!(error.user_message(), "AI model service is temporarily unavailable");
    assert_eq!(error.module_path(), "ai_model::lm_manager");
    assert_eq!(error.operation(), "generate_code_completion");
    assert_eq!(error.session_id().unwrap(), "session_123");
    assert_eq!(error.request_id().unwrap(), "request_456");
    assert_eq!(error.details().get("model_name").unwrap(), &serde_json::json!("gpt-4"));
    assert!(error.cause().is_none());
    assert!(error.retry_config().is_none());
    assert!(error.recovery_hints().is_empty());
    assert!(error.context_chain().is_empty());
}

// Test ErrorObject::add_context_frame
#[kani::proof]
fn test_error_object_add_context_frame() {
    let mut error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build();

    let context_frame = ContextFrame::new("api_gateway", HashMap::new());
    error.add_context_frame(context_frame);

    assert_eq!(error.context_chain().len(), 1);
    assert_eq!(error.context_chain()[0].source(), "api_gateway");
}

// Test ErrorObject::set_cause
#[kani::proof]
fn test_error_object_set_cause() {
    let cause = ErrorObject::builder()
        .code("ERR-NET-API-001_ERR_S")
        .source(ErrorSource::NET)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Network timeout")
        .user_message("Network service is temporarily unavailable")
        .module_path("network::api_client")
        .operation("send_request")
        .build();

    let mut error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build();

    error.set_cause(cause);
    assert!(error.cause().is_some());
    assert_eq!(error.cause().unwrap().code(), "ERR-NET-API-001_ERR_S");
}

// Test ErrorObjectBuilder with optional fields
#[kani::proof]
fn test_error_object_builder_optional_fields() {
    let context_frame = ContextFrame::new("api_gateway", HashMap::new());
    let recovery_hint = RecoveryHint::new("Retry", "Retry the operation", HashMap::new());
    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);

    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .session_id("session_123")
        .request_id("request_456")
        .detail("model_name", serde_json::json!("gpt-4"))
        .context_frame(context_frame)
        .recovery_hint(recovery_hint)
        .retry_config(retry_config)
        .build();

    assert_eq!(error.session_id().unwrap(), "session_123");
    assert_eq!(error.request_id().unwrap(), "request_456");
    assert_eq!(error.details().get("model_name").unwrap(), &serde_json::json!("gpt-4"));
    assert_eq!(error.context_chain().len(), 1);
    assert_eq!(error.recovery_hints().len(), 1);
    assert!(error.retry_config().is_some());
}

// Test ErrorObjectBuilder::default
#[kani::proof]
fn test_error_object_builder_default() {
    let builder = ErrorObjectBuilder::default();
    let error = builder
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build();

    assert_eq!(error.code(), "ERR-AIM-LM-002_ERR_S");
}

// Test ErrorObject with different severity levels
#[kani::proof]
fn test_error_object_with_different_severities() {
    let severities = vec![
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];

    for severity in severities {
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(severity)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();

        assert_eq!(error.severity(), severity);
    }
}

// Test ErrorObject with different impact scopes
#[kani::proof]
fn test_error_object_with_different_impact_scopes() {
    let impact_scopes = vec![
        ImpactScope::GLOBAL,
        ImpactScope::SESSION,
        ImpactScope::OPERATION,
        ImpactScope::MODULE,
    ];

    for impact_scope in impact_scopes {
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(impact_scope)
            .recoverability(Recoverability::AutoRecoverable)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();

        assert_eq!(error.impact_scope(), impact_scope);
    }
}

// Test ErrorObject with different recoverability levels
#[kani::proof]
fn test_error_object_with_different_recoverability() {
    let recoverabilities = vec![
        Recoverability::AutoRecoverable,
        Recoverability::SemiAuto,
        Recoverability::ManualIntervention,
        Recoverability::NonRecoverable,
    ];

    for recoverability in recoverabilities {
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-002_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(recoverability)
            .message("AI model call timed out")
            .user_message("AI model service is temporarily unavailable")
            .module_path("ai_model::lm_manager")
            .operation("generate_code_completion")
            .build();

        assert_eq!(error.recoverability(), recoverability);
    }
}
