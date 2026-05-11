//! Capture property tests
#![allow(clippy::items_after_statements)]

use error_core::classification::{ErrorSource, ImpactScope, Recoverability, Severity};
use error_core::error_capture::*;

#[test]
fn test_frontend_error_capture() {
    let capture = FrontendErrorCapture::new("test_component");

    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    let error = TestError;
    let captured = capture.capture_error(&error);

    assert_eq!(captured.code(), "ERR-USR-UI-001_ERR_O");
    assert_eq!(captured.source(), ErrorSource::USR);
    assert_eq!(captured.severity(), Severity::ERROR);
    assert_eq!(captured.impact_scope(), ImpactScope::OPERATION);
    assert_eq!(captured.recoverability(), Recoverability::SemiAuto);
    assert_eq!(captured.module_path(), "test_component");
    assert_eq!(captured.operation(), "ui_operation");
    assert!(captured.message().contains("Frontend error"));
    assert!(captured.user_message().contains("An error occurred"));
}

#[test]
fn test_gateway_error_capture() {
    let capture = GatewayErrorCapture::new("test_service", "test_endpoint");

    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    let error = TestError;
    let captured = capture.capture_error(&error);

    assert_eq!(captured.code(), "ERR-NET-GW-001_ERR_S");
    assert_eq!(captured.source(), ErrorSource::NET);
    assert_eq!(captured.severity(), Severity::ERROR);
    assert_eq!(captured.impact_scope(), ImpactScope::SESSION);
    assert_eq!(captured.recoverability(), Recoverability::AutoRecoverable);
    assert_eq!(captured.module_path(), "test_service");
    assert_eq!(captured.operation(), "test_endpoint");
    assert!(captured.message().contains("Gateway error"));
    assert_eq!(captured.user_message(), "Service temporarily unavailable");
}

#[test]
fn test_business_error_capture() {
    let capture = BusinessErrorCapture::new("test_domain", "test_operation");

    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    let error = TestError;
    let captured = capture.capture_error(&error);

    assert_eq!(captured.code(), "ERR-INT-BL-001_ERR_M");
    assert_eq!(captured.source(), ErrorSource::INT);
    assert_eq!(captured.severity(), Severity::ERROR);
    assert_eq!(captured.impact_scope(), ImpactScope::MODULE);
    assert_eq!(
        captured.recoverability(),
        Recoverability::ManualIntervention
    );
    assert_eq!(captured.module_path(), "test_domain");
    assert_eq!(captured.operation(), "test_operation");
    assert!(captured.message().contains("Business logic error"));
    assert_eq!(captured.user_message(), "An internal error occurred");
}

#[test]
fn test_infrastructure_error_capture() {
    let capture = InfrastructureErrorCapture::new("test_service", "test_resource");

    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    let error = TestError;
    let captured = capture.capture_error(&error);

    assert_eq!(captured.code(), "ERR-SYS-IF-001_ERR_G");
    assert_eq!(captured.source(), ErrorSource::SYS);
    assert_eq!(captured.severity(), Severity::ERROR);
    assert_eq!(captured.impact_scope(), ImpactScope::GLOBAL);
    assert_eq!(captured.recoverability(), Recoverability::SemiAuto);
    assert_eq!(captured.module_path(), "test_service");
    assert_eq!(captured.operation(), "test_resource");
    assert!(captured.message().contains("Infrastructure error"));
    assert_eq!(captured.user_message(), "System resource unavailable");
}

#[test]
fn test_error_propagation_append_context() {
    let capture = FrontendErrorCapture::new("test");

    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    let error = TestError;
    let captured = capture.capture_error(&error);

    let mut context = std::collections::HashMap::new();
    context.insert("key".to_string(), serde_json::json!("value"));

    let propagated = ErrorPropagation::append_context(captured, "test_source", context);

    assert_eq!(propagated.context_chain().len(), 1);
    assert_eq!(propagated.context_chain()[0].source(), "test_source");
}

#[test]
fn test_error_propagation_wrap_error() {
    let inner_capture = InfrastructureErrorCapture::new("database", "connection");
    let outer_capture = BusinessErrorCapture::new("order_service", "process_payment");

    #[derive(Debug, thiserror::Error)]
    #[error("Inner error")]
    struct InnerError;

    #[derive(Debug, thiserror::Error)]
    #[error("Outer error")]
    struct OuterError;

    let inner_error = InnerError;
    let inner_captured = inner_capture.capture_error(&inner_error);

    let outer_error = OuterError;
    let outer_captured = outer_capture.capture_error(&outer_error);

    let wrapped = ErrorPropagation::wrap_error(outer_captured, inner_captured);
    assert!(wrapped.cause().is_some());
}

#[test]
fn test_error_propagation_strip_internal_details() {
    let capture = BusinessErrorCapture::new("user_service", "create_user");

    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    let error = TestError;
    let captured = capture.capture_error(&error);

    let captured = ErrorPropagation::strip_internal_details(captured);
    let _ = captured;
}

// Test for add_context method for all capture types
#[test]
fn test_frontend_error_capture_add_context() {
    let capture = FrontendErrorCapture::new("test_component");

    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    let error = TestError;
    let captured = capture.capture_error(&error);

    let mut context = std::collections::HashMap::new();
    context.insert("key".to_string(), serde_json::json!("value"));

    let captured = capture.add_context(captured, context);

    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "test_component");
}

#[test]
fn test_gateway_error_capture_add_context() {
    let capture = GatewayErrorCapture::new("test_service", "test_endpoint");

    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    let error = TestError;
    let captured = capture.capture_error(&error);

    let mut context = std::collections::HashMap::new();
    context.insert("key".to_string(), serde_json::json!("value"));

    let captured = capture.add_context(captured, context);

    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "test_service");
}

#[test]
fn test_business_error_capture_add_context() {
    let capture = BusinessErrorCapture::new("test_domain", "test_operation");

    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    let error = TestError;
    let captured = capture.capture_error(&error);

    let mut context = std::collections::HashMap::new();
    context.insert("key".to_string(), serde_json::json!("value"));

    let captured = capture.add_context(captured, context);

    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "test_domain");
}

#[test]
fn test_infrastructure_error_capture_add_context() {
    let capture = InfrastructureErrorCapture::new("test_service", "test_resource");

    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    let error = TestError;
    let captured = capture.capture_error(&error);

    let mut context = std::collections::HashMap::new();
    context.insert("key".to_string(), serde_json::json!("value"));

    let captured = capture.add_context(captured, context);

    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "test_service");
}
