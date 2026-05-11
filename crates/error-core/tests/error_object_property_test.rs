//! Error object property tests
//!
//! This module contains property tests for the error object module to ensure coverage of all possible cases.
#![allow(clippy::uninlined_format_args, clippy::cast_sign_loss)]

use error_core::classification::{ErrorSource, ImpactScope, Recoverability, Severity};
use error_core::error_object::{ErrorObject, ErrorObjectBuilder};
use error_core::propagation::{ContextFrame, RecoveryAction, RecoveryHint, RetryConfig};
use std::collections::HashMap;

#[test]
fn test_error_object_all_combinations() {
    // Test all combinations of ErrorSource, Severity, ImpactScope, and Recoverability
    let sources = [
        ErrorSource::USR,
        ErrorSource::AIM,
        ErrorSource::FS,
        ErrorSource::NET,
        ErrorSource::CFG,
        ErrorSource::SEC,
        ErrorSource::TOOL,
        ErrorSource::SESS,
        ErrorSource::STATE,
        ErrorSource::EXT,
        ErrorSource::LSP,
        ErrorSource::MCP,
        ErrorSource::SYS,
        ErrorSource::INT,
        ErrorSource::UNK,
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
        Recoverability::SemiAuto,
        Recoverability::ManualIntervention,
        Recoverability::NonRecoverable,
    ];

    let modules = ["LM", "API", "DB", "FS"];
    let operations = ["generate", "validate", "process", "connect"];

    for source in &sources {
        for severity in &severities {
            for impact_scope in &impact_scopes {
                for recoverability in &recoverabilities {
                    for module in &modules {
                        for operation in &operations {
                            let code = format!(
                                "ERR-{}-{}-001_{}_{}",
                                source.as_str(),
                                module,
                                severity.as_str(),
                                impact_scope.as_str()
                            );
                            let message = format!("Error in {} operation", operation);
                            let user_message = format!("An error occurred during {}", operation);
                            let module_path = format!("{}.{}", module, operation);

                            let _result = ErrorObject::builder()
                                .code(&code)
                                .source(*source)
                                .severity(*severity)
                                .impact_scope(*impact_scope)
                                .recoverability(*recoverability)
                                .message(&message)
                                .user_message(&user_message)
                                .module_path(&module_path)
                                .operation(operation)
                                .build();
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn test_error_object_with_context_frames() {
    // Test error object with different numbers of context frames
    let frame_counts = [0, 1, 3, 5];

    for frame_count in &frame_counts {
        let mut builder = ErrorObject::builder()
            .code("ERR-AIM-LM-001_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Test error")
            .user_message("Test user message")
            .module_path("ai_model.lm")
            .operation("generate");

        for i in 0..*frame_count {
            let source = format!("module_{}", i);
            let mut data = HashMap::new();
            data.insert("key".to_string(), serde_json::json!(i));
            let frame = ContextFrame::new(&source, data);
            builder = builder.context_frame(frame);
        }

        let error = builder.build();
        assert_eq!(error.context_chain().len(), *frame_count as usize);
    }
}

#[test]
fn test_error_object_with_recovery_hints() {
    // Test error object with different numbers of recovery hints
    let hint_counts = [0, 1, 3, 5];

    for hint_count in &hint_counts {
        let mut builder = ErrorObject::builder()
            .code("ERR-AIM-LM-001_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Test error")
            .user_message("Test user message")
            .module_path("ai_model.lm")
            .operation("generate");

        for i in 0..*hint_count {
            let description = format!("Description {}", i);
            let mut params = HashMap::new();
            params.insert("param".to_string(), serde_json::json!(i));
            let hint = RecoveryHint::new(RecoveryAction::Retry, &description, params);
            builder = builder.recovery_hint(hint);
        }

        let error = builder.build();
        assert_eq!(error.recovery_hints().len(), *hint_count as usize);
    }
}

#[test]
fn test_error_object_with_cause() {
    // Test error object with a cause
    let cause = ErrorObject::builder()
        .code("ERR-NET-API-001_ERR_S")
        .source(ErrorSource::NET)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Network error")
        .user_message("Network error occurred")
        .module_path("network.api")
        .operation("connect")
        .build();

    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model error")
        .user_message("AI model error occurred")
        .module_path("ai_model.lm")
        .operation("generate")
        .cause(cause)
        .build();
    assert!(error.cause().is_some());
    assert_eq!(error.cause().unwrap().code(), "ERR-NET-API-001_ERR_S");
}

#[test]
fn test_error_object_with_retry_config() {
    // Test error object with different retry configurations
    let retry_configs = [
        RetryConfig::new(1, 100, 1000, 2.0, false),
        RetryConfig::new(3, 1000, 10000, 2.0, true),
        RetryConfig::new(5, 500, 5000, 1.5, false),
    ];

    for config in &retry_configs {
        let error = ErrorObject::builder()
            .code("ERR-AIM-LM-001_ERR_S")
            .source(ErrorSource::AIM)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message("Test error")
            .user_message("Test user message")
            .module_path("ai_model.lm")
            .operation("generate")
            .retry_config(config.clone())
            .build();
        assert!(error.retry_config().is_some());
        assert_eq!(error.retry_config().unwrap(), config);
    }
}

#[test]
fn test_error_object_missing_required_fields() {
    // Test error object with missing required fields

    // Missing code
    let builder1 = ErrorObject::builder()
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("ai_model.lm")
        .operation("generate");
    let result1 = builder1.build_checked();
    assert!(result1.is_err());

    // Missing source
    let builder2 = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("ai_model.lm")
        .operation("generate");
    let result2 = builder2.build_checked();
    assert!(result2.is_err());

    // Missing severity
    let builder3 = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("ai_model.lm")
        .operation("generate");
    let result3 = builder3.build_checked();
    assert!(result3.is_err());

    // Missing impact_scope
    let builder4 = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("ai_model.lm")
        .operation("generate");
    let result4 = builder4.build_checked();
    assert!(result4.is_err());

    // Missing recoverability
    let builder5 = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .message("Test error")
        .user_message("Test user message")
        .module_path("ai_model.lm")
        .operation("generate");
    let result5 = builder5.build_checked();
    assert!(result5.is_err());

    // Missing message
    let builder6 = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .user_message("Test user message")
        .module_path("ai_model.lm")
        .operation("generate");
    let result6 = builder6.build_checked();
    assert!(result6.is_err());

    // Missing user_message
    let builder7 = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .module_path("ai_model.lm")
        .operation("generate");
    let result7 = builder7.build_checked();
    assert!(result7.is_err());

    // Missing module_path
    let builder8 = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .operation("generate");
    let result8 = builder8.build_checked();
    assert!(result8.is_err());

    // Missing operation
    let builder9 = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("ai_model.lm");
    let result9 = builder9.build_checked();
    assert!(result9.is_err());
}

#[test]
fn test_error_object_all_getters() {
    // Test all getter methods
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("ai_model.lm")
        .operation("generate")
        .session_id("session_123")
        .request_id("request_456")
        .detail("key1", serde_json::json!("value1"))
        .build();

    // Test all getter methods
    assert_eq!(error.code(), "ERR-AIM-LM-001_ERR_S");
    assert_eq!(error.source(), ErrorSource::AIM);
    assert_eq!(error.severity(), Severity::ERROR);
    assert_eq!(error.impact_scope(), ImpactScope::SESSION);
    assert_eq!(error.recoverability(), Recoverability::AutoRecoverable);
    assert_eq!(error.message(), "Test error");
    assert_eq!(error.user_message(), "Test user message");
    assert_eq!(error.module_path(), "ai_model.lm");
    assert_eq!(error.operation(), "generate");
    assert_eq!(error.session_id().unwrap(), "session_123");
    assert_eq!(error.request_id().unwrap(), "request_456");
    assert!(error.details().contains_key("key1"));
    assert!(error.context_chain().is_empty());
    assert!(error.recovery_hints().is_empty());
    assert!(error.retry_config().is_none());
    assert!(error.cause().is_none());
}

#[test]
fn test_error_object_add_context_frame() {
    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("ai_model.lm")
        .operation("generate")
        .build();

    let frame1 = ContextFrame::new("source1", HashMap::new());
    let error = error.with_context_frame(frame1);
    assert_eq!(error.context_chain().len(), 1);
    assert_eq!(error.context_chain()[0].source(), "source1");

    let mut data = HashMap::new();
    data.insert("key".to_string(), serde_json::json!("value"));
    let frame2 = ContextFrame::new("source2", data);
    let error = error.with_context_frame(frame2);
    assert_eq!(error.context_chain().len(), 2);
    assert_eq!(error.context_chain()[1].source(), "source2");
    let _ = error;
}

#[test]
fn test_error_object_set_cause() {
    let cause = ErrorObject::builder()
        .code("ERR-NET-API-001_ERR_S")
        .source(ErrorSource::NET)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Network error")
        .user_message("Network error occurred")
        .module_path("network.api")
        .operation("connect")
        .build();

    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("AI model error")
        .user_message("AI model error occurred")
        .module_path("ai_model.lm")
        .operation("generate")
        .build();

    let error = error.with_cause(cause);
    assert!(error.cause().is_some());
    assert_eq!(error.cause().unwrap().code(), "ERR-NET-API-001_ERR_S");
}

#[test]
fn test_error_object_with_optional_fields() {
    // Test error object with all optional fields
    let mut data = HashMap::new();
    data.insert("key1".to_string(), serde_json::json!("value1"));
    data.insert("key2".to_string(), serde_json::json!(42));

    let context_frame = ContextFrame::new("source", data);

    let recovery_hint =
        RecoveryHint::new(RecoveryAction::Retry, "Retry the operation", HashMap::new());

    let retry_config = RetryConfig::new(3, 1000, 10000, 2.0, true);

    let cause = ErrorObject::builder()
        .code("ERR-NET-API-001_ERR_S")
        .source(ErrorSource::NET)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Network error")
        .user_message("Network error occurred")
        .module_path("network.api")
        .operation("connect")
        .build();

    let error = ErrorObject::builder()
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("ai_model.lm")
        .operation("generate")
        .session_id("session_123")
        .request_id("request_456")
        .detail("test_key", serde_json::json!("test_value"))
        .context_frame(context_frame)
        .recovery_hint(recovery_hint)
        .retry_config(retry_config)
        .cause(cause)
        .build();

    // Verify all optional fields
    assert_eq!(error.session_id().unwrap(), "session_123");
    assert_eq!(error.request_id().unwrap(), "request_456");
    assert!(error.details().contains_key("test_key"));
    assert_eq!(error.context_chain().len(), 1);
    assert_eq!(error.recovery_hints().len(), 1);
    assert!(error.retry_config().is_some());
    assert!(error.cause().is_some());
}

#[test]
fn test_error_object_builder_default_impl() {
    // Test the Default implementation for ErrorObjectBuilder
    let builder = ErrorObjectBuilder::default();

    // Build with all required fields
    let _result = builder
        .code("ERR-AIM-LM-001_ERR_S")
        .source(ErrorSource::AIM)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::SESSION)
        .recoverability(Recoverability::AutoRecoverable)
        .message("Test error")
        .user_message("Test user message")
        .module_path("ai_model.lm")
        .operation("generate")
        .build();
}
