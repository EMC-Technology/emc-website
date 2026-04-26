//! Error capture fuzz tests
//! 
//! This module contains fuzz tests for the error_capture module to ensure coverage of all possible cases.

use error_core::error_capture::{ErrorCapture, FrontendErrorCapture, GatewayErrorCapture, BusinessErrorCapture, InfrastructureErrorCapture, ErrorPropagation};
use std::collections::HashMap;
use serde_json::json;

// Test error types for fuzzing
#[derive(Debug, thiserror::Error)]
#[error("Simple test error")]
struct SimpleError;

#[derive(Debug, thiserror::Error)]
#[error("Network error: {0}")]
struct NetworkError(String);

#[derive(Debug, thiserror::Error)]
#[error("Database error: {0}")]
struct DatabaseError(String);

#[derive(Debug, thiserror::Error)]
#[error("Validation error: {0}")]
struct ValidationError(String);

#[derive(Debug, thiserror::Error)]
#[error("Authorization error")]
struct AuthError;

#[test]
fn test_frontend_error_capture() {
    // Test FrontendErrorCapture with different component names
    let components = ["", "user_dashboard", "login_form", "settings_page", "dashboard", "profile"];
    let errors: &[&dyn std::error::Error] = &[
        &SimpleError,
        &NetworkError("Connection timeout".to_string()),
        &DatabaseError("Connection failed".to_string()),
        &ValidationError("Invalid input".to_string()),
        &AuthError,
    ];
    
    for component in components {
        let capture = FrontendErrorCapture::new(component);
        for error in errors {
            let captured = capture.capture_error(error);
            assert_eq!(captured.code(), "ERR-USR-UI-001_ERR_O");
            assert_eq!(captured.module_path(), component);
            assert!(captured.message().contains("Frontend error"));
        }
    }
}

#[test]
fn test_frontend_error_capture_add_context() {
    // Test add_context with different contexts
    let capture = FrontendErrorCapture::new("user_dashboard");
    let error = SimpleError;
    let mut captured = capture.capture_error(&error);
    
    // Test with empty context
    let empty_context = HashMap::new();
    capture.add_context(&mut captured, empty_context);
    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "user_dashboard");
    
    // Test with various context values
    let mut context1 = HashMap::new();
    context1.insert("component".to_string(), json!("login_form"));
    context1.insert("action".to_string(), json!("submit"));
    context1.insert("user_id".to_string(), json!(12345));
    
    let mut captured2 = capture.capture_error(&error);
    capture.add_context(&mut captured2, context1);
    assert_eq!(captured2.context_chain().len(), 1);
    
    // Test with nested context
    let mut context2 = HashMap::new();
    context2.insert("form_data".to_string(), json!({"username": "test", "password": "****"}));
    context2.insert("errors".to_string(), json!(vec!["Invalid password"]));
    
    let mut captured3 = capture.capture_error(&error);
    capture.add_context(&mut captured3, context2);
    assert_eq!(captured3.context_chain().len(), 1);
}

#[test]
fn test_gateway_error_capture() {
    // Test GatewayErrorCapture with different service and endpoint names
    let service_endpoint_pairs = [
        ("", ""),
        ("auth_service", "login"),
        ("user_service", "get_user"),
        ("order_service", "create_order"),
        ("payment_service", "process_payment"),
    ];
    let errors: &[&dyn std::error::Error] = &[
        &SimpleError,
        &NetworkError("Connection timeout".to_string()),
        &DatabaseError("Connection failed".to_string()),
        &ValidationError("Invalid input".to_string()),
        &AuthError,
    ];
    
    for (service, endpoint) in service_endpoint_pairs {
        let capture = GatewayErrorCapture::new(service, endpoint);
        for error in errors {
            let captured = capture.capture_error(error);
            assert_eq!(captured.code(), "ERR-NET-GW-001_ERR_S");
            assert_eq!(captured.module_path(), service);
            assert_eq!(captured.operation(), endpoint);
            assert!(captured.message().contains("Gateway error"));
        }
    }
}

#[test]
fn test_gateway_error_capture_add_context() {
    // Test add_context with different contexts
    let capture = GatewayErrorCapture::new("auth_service", "login");
    let error = SimpleError;
    let mut captured = capture.capture_error(&error);
    
    // Test with empty context
    let empty_context = HashMap::new();
    capture.add_context(&mut captured, empty_context);
    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "auth_service");
    
    // Test with various context values
    let mut context1 = HashMap::new();
    context1.insert("request_id".to_string(), json!("req_123"));
    context1.insert("client_ip".to_string(), json!("192.168.1.1"));
    context1.insert("method".to_string(), json!("POST"));
    
    let mut captured2 = capture.capture_error(&error);
    capture.add_context(&mut captured2, context1);
    assert_eq!(captured2.context_chain().len(), 1);
}

#[test]
fn test_business_error_capture() {
    // Test BusinessErrorCapture with different domain and operation names
    let domain_operation_pairs = [
        ("", ""),
        ("user_service", "create_user"),
        ("order_service", "process_payment"),
        ("inventory_service", "check_stock"),
        ("billing_service", "generate_invoice"),
    ];
    let errors: &[&dyn std::error::Error] = &[
        &SimpleError,
        &NetworkError("Connection timeout".to_string()),
        &DatabaseError("Connection failed".to_string()),
        &ValidationError("Invalid input".to_string()),
        &AuthError,
    ];
    
    for (domain, operation) in domain_operation_pairs {
        let capture = BusinessErrorCapture::new(domain, operation);
        for error in errors {
            let captured = capture.capture_error(error);
            assert_eq!(captured.code(), "ERR-INT-BL-001_ERR_M");
            assert_eq!(captured.module_path(), domain);
            assert_eq!(captured.operation(), operation);
            assert!(captured.message().contains("Business logic error"));
        }
    }
}

#[test]
fn test_business_error_capture_add_context() {
    // Test add_context with different contexts
    let capture = BusinessErrorCapture::new("order_service", "process_payment");
    let error = SimpleError;
    let mut captured = capture.capture_error(&error);
    
    // Test with empty context
    let empty_context = HashMap::new();
    capture.add_context(&mut captured, empty_context);
    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "order_service");
    
    // Test with various context values
    let mut context1 = HashMap::new();
    context1.insert("order_id".to_string(), json!("ord_456"));
    context1.insert("amount".to_string(), json!(100.0));
    context1.insert("currency".to_string(), json!("USD"));
    
    let mut captured2 = capture.capture_error(&error);
    capture.add_context(&mut captured2, context1);
    assert_eq!(captured2.context_chain().len(), 1);
}

#[test]
fn test_infrastructure_error_capture() {
    // Test InfrastructureErrorCapture with different service and resource names
    let service_resource_pairs = [
        ("", ""),
        ("database", "connection"),
        ("cache", "redis"),
        ("storage", "s3"),
        ("message_queue", "rabbitmq"),
    ];
    let errors: &[&dyn std::error::Error] = &[
        &SimpleError,
        &NetworkError("Connection timeout".to_string()),
        &DatabaseError("Connection failed".to_string()),
        &ValidationError("Invalid input".to_string()),
        &AuthError,
    ];
    
    for (service, resource) in service_resource_pairs {
        let capture = InfrastructureErrorCapture::new(service, resource);
        for error in errors {
            let captured = capture.capture_error(error);
            assert_eq!(captured.code(), "ERR-SYS-IF-001_ERR_G");
            assert_eq!(captured.module_path(), service);
            assert_eq!(captured.operation(), resource);
            assert!(captured.message().contains("Infrastructure error"));
        }
    }
}

#[test]
fn test_infrastructure_error_capture_add_context() {
    // Test add_context with different contexts
    let capture = InfrastructureErrorCapture::new("database", "connection");
    let error = SimpleError;
    let mut captured = capture.capture_error(&error);
    
    // Test with empty context
    let empty_context = HashMap::new();
    capture.add_context(&mut captured, empty_context);
    assert_eq!(captured.context_chain().len(), 1);
    assert_eq!(captured.context_chain()[0].source(), "database");
    
    // Test with various context values
    let mut context1 = HashMap::new();
    context1.insert("db_instance".to_string(), json!("prod_db"));
    context1.insert("host".to_string(), json!("db.example.com"));
    context1.insert("port".to_string(), json!(5432));
    
    let mut captured2 = capture.capture_error(&error);
    capture.add_context(&mut captured2, context1);
    assert_eq!(captured2.context_chain().len(), 1);
}

#[test]
fn test_error_propagation_append_context() {
    // Test append_context with different sources and contexts
    let capture = FrontendErrorCapture::new("user_dashboard");
    let error = SimpleError;
    let captured = capture.capture_error(&error);
    
    // Test with empty context
    let empty_context = HashMap::new();
    let propagated = ErrorPropagation::append_context(captured, "api_gateway", empty_context);
    assert_eq!(propagated.context_chain().len(), 1);
    assert_eq!(propagated.context_chain()[0].source(), "api_gateway");
    
    // Test with various context values
    let mut context1 = HashMap::new();
    context1.insert("request_id".to_string(), json!("req_123"));
    context1.insert("client_ip".to_string(), json!("192.168.1.1"));
    
    let capture2 = FrontendErrorCapture::new("user_dashboard");
    let captured2 = capture2.capture_error(&error);
    let propagated2 = ErrorPropagation::append_context(captured2, "api_gateway", context1);
    assert_eq!(propagated2.context_chain().len(), 1);
    
    // Test with multiple context appends
    let capture3 = FrontendErrorCapture::new("user_dashboard");
    let captured3 = capture3.capture_error(&error);
    let propagated3 = ErrorPropagation::append_context(captured3, "api_gateway", HashMap::new());
    let propagated3 = ErrorPropagation::append_context(propagated3, "auth_service", HashMap::new());
    let propagated3 = ErrorPropagation::append_context(propagated3, "database", HashMap::new());
    assert_eq!(propagated3.context_chain().len(), 3);
}

#[test]
fn test_error_propagation_wrap_error() {
    // Test wrap_error with different error combinations
    let inner_capture = InfrastructureErrorCapture::new("database", "connection");
    let inner_error = SimpleError;
    let inner_captured = inner_capture.capture_error(&inner_error);
    
    let outer_capture = BusinessErrorCapture::new("order_service", "process_payment");
    let outer_error = SimpleError;
    let outer_captured = outer_capture.capture_error(&outer_error);
    
    let wrapped = ErrorPropagation::wrap_error(outer_captured, inner_captured);
    assert!(wrapped.cause().is_some());
    assert_eq!(wrapped.cause().unwrap().code(), "ERR-SYS-IF-001_ERR_G");
    
    // Test nested wrapping
    let outer_outer_capture = GatewayErrorCapture::new("api_gateway", "create_order");
    let outer_outer_error = SimpleError;
    let outer_outer_captured = outer_outer_capture.capture_error(&outer_outer_error);
    
    let nested_wrapped = ErrorPropagation::wrap_error(outer_outer_captured, wrapped);
    assert!(nested_wrapped.cause().is_some());
    assert!(nested_wrapped.cause().unwrap().cause().is_some());
}

#[test]
fn test_error_propagation_strip_internal_details() {
    // Test strip_internal_details with different error types
    let captures = vec![
        FrontendErrorCapture::new("user_dashboard").capture_error(&SimpleError),
        GatewayErrorCapture::new("auth_service", "login").capture_error(&SimpleError),
        BusinessErrorCapture::new("order_service", "process_payment").capture_error(&SimpleError),
        InfrastructureErrorCapture::new("database", "connection").capture_error(&SimpleError),
    ];
    
    for mut captured in captures {
        // This should not panic
        ErrorPropagation::strip_internal_details(&mut captured);
        // The error should still be valid
        assert!(!captured.code().is_empty());
    }
}

#[test]
fn test_error_capture_edge_cases() {
    // Test edge cases for all error capture implementations
    
    // Test with very long component names
    let long_name = "a".repeat(1000);
    let capture = FrontendErrorCapture::new(&long_name);
    let error = SimpleError;
    let captured = capture.capture_error(&error);
    assert_eq!(captured.module_path(), long_name);
    
    // Test with special characters in component names
    let special_chars = "component-with-special-chars_123!@#";
    let capture = FrontendErrorCapture::new(special_chars);
    let error = SimpleError;
    let captured = capture.capture_error(&error);
    assert_eq!(captured.module_path(), special_chars);
    
    // Test with empty error messages
    #[derive(Debug, thiserror::Error)]
    #[error("")]
    struct EmptyError;
    
    let capture = FrontendErrorCapture::new("test");
    let error = EmptyError;
    let captured = capture.capture_error(&error);
    assert!(captured.message().contains("Frontend error"));
}

#[test]
fn test_error_capture_all_combinations() {
    // Test all combinations of error capture implementations with different error types
    let captures: Vec<(&str, Box<dyn ErrorCapture>)> = vec![
        ("frontend", Box::new(FrontendErrorCapture::new("user_dashboard"))),
        ("gateway", Box::new(GatewayErrorCapture::new("auth_service", "login"))),
        ("business", Box::new(BusinessErrorCapture::new("order_service", "process_payment"))),
        ("infrastructure", Box::new(InfrastructureErrorCapture::new("database", "connection"))),
    ];
    
    let errors: &[&dyn std::error::Error] = &[
        &SimpleError,
        &NetworkError("Connection timeout".to_string()),
        &DatabaseError("Connection failed".to_string()),
        &ValidationError("Invalid input".to_string()),
        &AuthError,
    ];
    
    for (name, capture) in captures {
        for error in errors {
            let captured = capture.capture_error(error);
            assert!(!captured.code().is_empty(), "{} capture failed for error", name);
            
            // Test add_context
            let mut captured_mut = captured;
            let mut context = HashMap::new();
            context.insert("test_key".to_string(), json!("test_value"));
            capture.add_context(&mut captured_mut, context);
            assert_eq!(captured_mut.context_chain().len(), 1);
        }
    }
}
