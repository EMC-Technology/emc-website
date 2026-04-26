//! Error capture module formal verification tests using Kani

use error_core::error_capture::*;
use error_core::classification::*;
use std::collections::HashMap;
use serde_json::json;

// Define a test error type
#[derive(Debug, thiserror::Error)]
#[error("Test error")]
struct TestError;

// Test FrontendErrorCapture
#[kani::proof]
fn test_frontend_error_capture() {
    let capture = FrontendErrorCapture::new("user_dashboard");
    let error = TestError;
    let captured = capture.capture_error(&error);
    
    assert_eq!(captured.code(), "ERR-USR-UI-001_ERR_O");
    assert_eq!(captured.source(), ErrorSource::USR);
    assert_eq!(captured.severity(), Severity::ERROR);
    assert_eq!(captured.impact_scope(), ImpactScope::OPERATION);
    assert_eq!(captured.module_path(), "user_dashboard");
    assert_eq!(captured.operation(), "ui_operation");
}

// Test FrontendErrorCapture::add_context
#[kani::proof]
fn test_frontend_error_capture_add_context() {
    let capture = FrontendErrorCapture::new("user_dashboard");
    let error = TestError;
    let mut captured = capture.capture_error(&error);
    
    let mut context = HashMap::new();
    context.insert("component".to_string(), json!("login_form"));
    capture.add_context(&mut captured, context);
    
    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "user_dashboard");
}

// Test GatewayErrorCapture
#[kani::proof]
fn test_gateway_error_capture() {
    let capture = GatewayErrorCapture::new("auth_service", "login");
    let error = TestError;
    let captured = capture.capture_error(&error);
    
    assert_eq!(captured.code(), "ERR-NET-GW-001_ERR_S");
    assert_eq!(captured.source(), ErrorSource::NET);
    assert_eq!(captured.severity(), Severity::ERROR);
    assert_eq!(captured.impact_scope(), ImpactScope::SESSION);
    assert_eq!(captured.module_path(), "auth_service");
    assert_eq!(captured.operation(), "login");
}

// Test GatewayErrorCapture::add_context
#[kani::proof]
fn test_gateway_error_capture_add_context() {
    let capture = GatewayErrorCapture::new("auth_service", "login");
    let error = TestError;
    let mut captured = capture.capture_error(&error);
    
    let mut context = HashMap::new();
    context.insert("request_id".to_string(), json!("req_123"));
    capture.add_context(&mut captured, context);
    
    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "auth_service");
}

// Test BusinessErrorCapture
#[kani::proof]
fn test_business_error_capture() {
    let capture = BusinessErrorCapture::new("order_service", "process_payment");
    let error = TestError;
    let captured = capture.capture_error(&error);
    
    assert_eq!(captured.code(), "ERR-INT-BL-001_ERR_M");
    assert_eq!(captured.source(), ErrorSource::INT);
    assert_eq!(captured.severity(), Severity::ERROR);
    assert_eq!(captured.impact_scope(), ImpactScope::MODULE);
    assert_eq!(captured.module_path(), "order_service");
    assert_eq!(captured.operation(), "process_payment");
}

// Test BusinessErrorCapture::add_context
#[kani::proof]
fn test_business_error_capture_add_context() {
    let capture = BusinessErrorCapture::new("order_service", "process_payment");
    let error = TestError;
    let mut captured = capture.capture_error(&error);
    
    let mut context = HashMap::new();
    context.insert("order_id".to_string(), json!("ord_456"));
    capture.add_context(&mut captured, context);
    
    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "order_service");
}

// Test InfrastructureErrorCapture
#[kani::proof]
fn test_infrastructure_error_capture() {
    let capture = InfrastructureErrorCapture::new("database", "connection");
    let error = TestError;
    let captured = capture.capture_error(&error);
    
    assert_eq!(captured.code(), "ERR-SYS-IF-001_ERR_G");
    assert_eq!(captured.source(), ErrorSource::SYS);
    assert_eq!(captured.severity(), Severity::ERROR);
    assert_eq!(captured.impact_scope(), ImpactScope::GLOBAL);
    assert_eq!(captured.module_path(), "database");
    assert_eq!(captured.operation(), "connection");
}

// Test InfrastructureErrorCapture::add_context
#[kani::proof]
fn test_infrastructure_error_capture_add_context() {
    let capture = InfrastructureErrorCapture::new("database", "connection");
    let error = TestError;
    let mut captured = capture.capture_error(&error);
    
    let mut context = HashMap::new();
    context.insert("db_instance".to_string(), json!("prod_db"));
    capture.add_context(&mut captured, context);
    
    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "database");
}

// Test ErrorPropagation::append_context
#[kani::proof]
fn test_error_propagation_append_context() {
    let capture = BusinessErrorCapture::new("user_service", "create_user");
    let error = TestError;
    let captured = capture.capture_error(&error);
    
    let mut context = HashMap::new();
    context.insert("user_id".to_string(), json!("12345"));
    let propagated = ErrorPropagation::append_context(captured, "api_gateway", context);
    
    assert_eq!(propagated.context_chain().len(), 1);
    assert_eq!(propagated.context_chain()[0].source(), "api_gateway");
}

// Test ErrorPropagation::wrap_error
#[kani::proof]
fn test_error_propagation_wrap_error() {
    let inner_capture = InfrastructureErrorCapture::new("database", "connection");
    let inner_error = TestError;
    let inner_captured = inner_capture.capture_error(&inner_error);
    
    let outer_capture = BusinessErrorCapture::new("order_service", "process_payment");
    let outer_error = TestError;
    let outer_captured = outer_capture.capture_error(&outer_error);
    
    let wrapped = ErrorPropagation::wrap_error(outer_captured, inner_captured);
    assert!(wrapped.cause().is_some());
    assert_eq!(wrapped.cause().unwrap().code(), "ERR-SYS-IF-001_ERR_G");
}

// Test ErrorPropagation::strip_internal_details
#[kani::proof]
fn test_error_propagation_strip_internal_details() {
    let capture = BusinessErrorCapture::new("user_service", "create_user");
    let error = TestError;
    let mut captured = capture.capture_error(&error);
    
    // This should not panic
    ErrorPropagation::strip_internal_details(&mut captured);
    // Verify the error still has its basic properties
    assert_eq!(captured.code(), "ERR-INT-BL-001_ERR_M");
    assert_eq!(captured.source(), ErrorSource::INT);
}

// Test FrontendErrorCapture::get_user_friendly_message
#[kani::proof]
fn test_frontend_error_capture_get_user_friendly_message() {
    let capture = FrontendErrorCapture::new("user_dashboard");
    let error = TestError;
    let message = capture.get_user_friendly_message(&error);
    assert!(message.contains("Test error"));
}
