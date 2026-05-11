//! Error object comprehensive lifecycle tests
//!
//! This module tests the complete lifecycle of error objects, covering all edge cases and boundary values.
#![allow(clippy::uninlined_format_args, clippy::too_many_lines)]

use error_core::classification::{ErrorSource, ImpactScope, Recoverability, Severity};
use error_core::error_object::{ErrorObject, ErrorObjectBuilder};
use error_core::propagation::{ContextFrame, RecoveryAction, RecoveryHint, RetryConfig};
use std::collections::HashMap;

#[test]
fn test_error_object_complete_lifecycle() {
    // Test all possible combinations of error sources, severities, impact scopes, and recoverability
    let sources = [
        ErrorSource::AIM,
        ErrorSource::EXT,
        ErrorSource::INT,
        ErrorSource::NET,
        ErrorSource::SYS,
        ErrorSource::USR,
    ];

    let severities = [
        Severity::CRITICAL,
        Severity::ERROR,
        Severity::WARNING,
        Severity::INFO,
    ];

    let impact_scopes = [
        ImpactScope::GLOBAL,
        ImpactScope::SESSION,
        ImpactScope::MODULE,
        ImpactScope::OPERATION,
    ];

    let recoverabilities = [
        Recoverability::AutoRecoverable,
        Recoverability::ManualIntervention,
        Recoverability::NonRecoverable,
    ];

    // Test all combinations
    for source in &sources {
        for severity in &severities {
            for impact_scope in &impact_scopes {
                for recoverability in &recoverabilities {
                    // Create error object
                    let error = ErrorObject::builder()
                        .code("ERR-AIM-LM-002_ERR_S")
                        .source(*source)
                        .severity(*severity)
                        .impact_scope(*impact_scope)
                        .recoverability(*recoverability)
                        .message("AI model call timed out")
                        .user_message("AI model service is temporarily unavailable")
                        .module_path("ai_model::lm_manager")
                        .operation("generate_code_completion")
                        .session_id("session_123")
                        .request_id("request_456")
                        .detail("model_name", serde_json::json!("gpt-4"))
                        .build();

                    assert_eq!(error.code(), "ERR-AIM-LM-002_ERR_S");
                    assert_eq!(error.source(), *source);
                    assert_eq!(error.severity(), *severity);
                    assert_eq!(error.impact_scope(), *impact_scope);
                    assert_eq!(error.recoverability(), *recoverability);
                    assert_eq!(error.message(), "AI model call timed out");
                    assert_eq!(
                        error.user_message(),
                        "AI model service is temporarily unavailable"
                    );
                    assert_eq!(error.module_path(), "ai_model::lm_manager");
                    assert_eq!(error.operation(), "generate_code_completion");
                    assert_eq!(error.session_id().unwrap(), "session_123");
                    assert_eq!(error.request_id().unwrap(), "request_456");
                    assert_eq!(
                        error.details().get("model_name").unwrap(),
                        &serde_json::json!("gpt-4")
                    );

                    let context_frame = ContextFrame::new("api_gateway", HashMap::new());
                    let error = error.with_context_frame(context_frame);
                    assert_eq!(error.context_chain().len(), 1);
                    assert_eq!(error.context_chain()[0].source(), "api_gateway");

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

                    let error = error.with_cause(cause);
                    assert!(error.cause().is_some());
                    assert_eq!(error.cause().unwrap().code(), "ERR-NET-API-001_ERR_S");

                    // Test equality
                    let error_clone = error.clone();
                    assert_eq!(error, error_clone);

                    // Test debug formatting
                    let debug_str = format!("{:?}", error);
                    assert!(debug_str.contains("ErrorObject"));
                    assert!(debug_str.contains("ERR-AIM-LM-002_ERR_S"));
                }
            }
        }
    }
}

#[test]
fn test_error_object_builder_comprehensive() {
    // Test builder with all optional fields
    let context_frame = ContextFrame::new("api_gateway", HashMap::new());
    let recovery_hint =
        RecoveryHint::new(RecoveryAction::Retry, "Retry the operation", HashMap::new());
    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);

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
        .detail("timeout", serde_json::json!(30000))
        .context_frame(context_frame)
        .recovery_hint(recovery_hint)
        .retry_config(retry_config)
        .cause(cause)
        .build();

    // Test all fields
    assert_eq!(error.code(), "ERR-AIM-LM-002_ERR_S");
    assert_eq!(error.source(), ErrorSource::AIM);
    assert_eq!(error.severity(), Severity::ERROR);
    assert_eq!(error.impact_scope(), ImpactScope::SESSION);
    assert_eq!(error.recoverability(), Recoverability::AutoRecoverable);
    assert_eq!(error.message(), "AI model call timed out");
    assert_eq!(
        error.user_message(),
        "AI model service is temporarily unavailable"
    );
    assert_eq!(error.module_path(), "ai_model::lm_manager");
    assert_eq!(error.operation(), "generate_code_completion");
    assert_eq!(error.session_id().unwrap(), "session_123");
    assert_eq!(error.request_id().unwrap(), "request_456");
    assert_eq!(
        error.details().get("model_name").unwrap(),
        &serde_json::json!("gpt-4")
    );
    assert_eq!(
        error.details().get("timeout").unwrap(),
        &serde_json::json!(30000)
    );
    assert_eq!(error.context_chain().len(), 1);
    assert_eq!(error.recovery_hints().len(), 1);
    assert!(error.retry_config().is_some());
    assert!(error.cause().is_some());
}

#[test]
fn test_error_object_missing_required_fields() {
    // Test missing all required fields
    let builder = ErrorObject::builder();
    let result = builder.build_checked();
    assert!(result.is_err());

    // Test missing individual required fields
    // Missing code
    let result = ErrorObject::builder()
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(result.is_err());

    // Missing source
    let result = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(result.is_err());

    // Missing severity
    let result = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(result.is_err());

    // Missing impact_scope
    let result = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(result.is_err());

    // Missing recoverability
    let result = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(result.is_err());

    // Missing message
    let result = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(result.is_err());

    // Missing user_message
    let result = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .module_path("ai_model::lm_manager")
        .operation("generate_code_completion")
        .build_checked();
    assert!(result.is_err());

    // Missing module_path
    let result = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .operation("generate_code_completion")
        .build_checked();
    assert!(result.is_err());

    // Missing operation
    let result = ErrorObject::builder()
        .code("ERR-AIM-LM-002_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model call timed out")
        .user_message("AI model service is temporarily unavailable")
        .module_path("ai_model::lm_manager")
        .build_checked();
    assert!(result.is_err());
}

#[test]
fn test_error_object_default_builder() {
    // Test the Default implementation for ErrorObjectBuilder
    use std::default::Default;

    let builder = ErrorObjectBuilder::default();

    // Test that we can build an error object from the default builder
    let _result = builder
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
}
