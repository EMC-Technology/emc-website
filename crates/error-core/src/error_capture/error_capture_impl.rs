//! Error capture and propagation mechanisms
//! 
//! This module defines error capture mechanisms for different layers of the application,
//! including frontend/UI, gateway/API, business logic, and infrastructure layers.

use std::collections::HashMap;
use crate::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
use crate::error_object::ErrorObject;
use crate::propagation::ContextFrame;
use crate::error_code::registry;

/// Error capture trait
/// 
/// Defines a common interface for error capture across different layers.
pub trait ErrorCapture {
    /// Capture an error and convert it to an `ErrorObject`
    fn capture_error(&self, error: &dyn std::error::Error) -> ErrorObject;
    
    /// Add context to an existing error
    fn add_context(&self, error: &mut ErrorObject, context: HashMap<String, serde_json::Value>);
}

/// Frontend/UI layer error capture
/// 
/// Handles errors at the UI level, focusing on user-friendly messages and presentation.
pub struct FrontendErrorCapture {
    component: String,
}

impl FrontendErrorCapture {
    /// Create a new `FrontendErrorCapture`
    #[must_use]
    pub fn new(component: &str) -> Self {
        Self {
            component: component.to_string(),
        }
    }
}

impl ErrorCapture for FrontendErrorCapture {
    fn capture_error(&self, error: &dyn std::error::Error) -> ErrorObject {
        let message = format!("Frontend error: {error}");
        let user_message = Self::get_user_friendly_message(error);
        ErrorObject::builder()
            .code(registry::FRONTEND_UI_ERROR)
            .source(ErrorSource::USR)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::OPERATION)
            .recoverability(Recoverability::SemiAuto)
            .message(&message)
            .user_message(&user_message)
            .module_path(&self.component)
            .operation("ui_operation")
            .build()
    }
    
    fn add_context(&self, error: &mut ErrorObject, context: HashMap<String, serde_json::Value>) {
        let frame = ContextFrame::new(&self.component, context);
        error.add_context_frame(frame);
    }
}

impl FrontendErrorCapture {
    /// Get a user-friendly message based on the error
    fn get_user_friendly_message(error: &dyn std::error::Error) -> String {
        format!("An error occurred: {error}")
    }
}

/// Gateway/API layer error capture
/// 
/// Handles errors at the API gateway level, focusing on request processing and response formatting.
pub struct GatewayErrorCapture {
    service: String,
    endpoint: String,
}

impl GatewayErrorCapture {
    /// Create a new `GatewayErrorCapture`
    #[must_use]
    pub fn new(service: &str, endpoint: &str) -> Self {
        Self {
            service: service.to_string(),
            endpoint: endpoint.to_string(),
        }
    }
}

impl ErrorCapture for GatewayErrorCapture {
    fn capture_error(&self, error: &dyn std::error::Error) -> ErrorObject {
        let message = format!("Gateway error: {error}");
        ErrorObject::builder()
            .code(registry::GATEWAY_ERROR)
            .source(ErrorSource::NET)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::SESSION)
            .recoverability(Recoverability::AutoRecoverable)
            .message(&message)
            .user_message("Service temporarily unavailable")
            .module_path(&self.service)
            .operation(&self.endpoint)
            .build()
    }
    
    fn add_context(&self, error: &mut ErrorObject, context: HashMap<String, serde_json::Value>) {
        let frame = ContextFrame::new(&self.service, context);
        error.add_context_frame(frame);
    }
}

/// Business logic layer error capture
/// 
/// Handles errors at the business logic level, focusing on domain-specific error handling.
pub struct BusinessErrorCapture {
    domain: String,
    operation: String,
}

impl BusinessErrorCapture {
    /// Create a new `BusinessErrorCapture`
    #[must_use]
    pub fn new(domain: &str, operation: &str) -> Self {
        Self {
            domain: domain.to_string(),
            operation: operation.to_string(),
        }
    }
}

impl ErrorCapture for BusinessErrorCapture {
    fn capture_error(&self, error: &dyn std::error::Error) -> ErrorObject {
        let message = format!("Business logic error: {error}");
        ErrorObject::builder()
            .code(registry::BUSINESS_LOGIC_ERROR)
            .source(ErrorSource::INT)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::MODULE)
            .recoverability(Recoverability::ManualIntervention)
            .message(&message)
            .user_message("An internal error occurred")
            .module_path(&self.domain)
            .operation(&self.operation)
            .build()
    }
    
    fn add_context(&self, error: &mut ErrorObject, context: HashMap<String, serde_json::Value>) {
        let frame = ContextFrame::new(&self.domain, context);
        error.add_context_frame(frame);
    }
}

/// Infrastructure layer error capture
/// 
/// Handles errors at the infrastructure level, focusing on external system errors and resource issues.
pub struct InfrastructureErrorCapture {
    service: String,
    resource: String,
}

impl InfrastructureErrorCapture {
    /// Create a new `InfrastructureErrorCapture`
    #[must_use]
    pub fn new(service: &str, resource: &str) -> Self {
        Self {
            service: service.to_string(),
            resource: resource.to_string(),
        }
    }
}

impl ErrorCapture for InfrastructureErrorCapture {
    fn capture_error(&self, error: &dyn std::error::Error) -> ErrorObject {
        let message = format!("Infrastructure error: {error}");
        ErrorObject::builder()
            .code(registry::INFRASTRUCTURE_ERROR)
            .source(ErrorSource::SYS)
            .severity(Severity::ERROR)
            .impact_scope(ImpactScope::GLOBAL)
            .recoverability(Recoverability::SemiAuto)
            .message(&message)
            .user_message("System resource unavailable")
            .module_path(&self.service)
            .operation(&self.resource)
            .build()
    }
    
    fn add_context(&self, error: &mut ErrorObject, context: HashMap<String, serde_json::Value>) {
        let frame = ContextFrame::new(&self.service, context);
        error.add_context_frame(frame);
    }
}

/// Error propagation utilities
/// 
/// Provides functions for propagating errors through different layers of the application.
pub struct ErrorPropagation;

impl ErrorPropagation {
    /// Append context to an error as it propagates up the call stack
    #[must_use]
    pub fn append_context(
        error: ErrorObject,
        source: &str,
        context: HashMap<String, serde_json::Value>,
    ) -> ErrorObject {
        let mut error = error;
        let frame = ContextFrame::new(source, context);
        error.add_context_frame(frame);
        error
    }
    
    /// Preserve the cause chain when wrapping errors
    #[must_use]
    pub fn wrap_error(
        outer_error: ErrorObject,
        inner_error: ErrorObject,
    ) -> ErrorObject {
        let mut outer_error = outer_error;
        outer_error.set_cause(inner_error);
        outer_error
    }
    
    /// Strip internal details from an error before returning it to external clients
    pub fn strip_internal_details(error: &mut ErrorObject) {
        error.clear_details();
        error.clear_context_chain();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_frontend_error_capture() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;
        
        let capture = FrontendErrorCapture::new("user_dashboard");
        let error = TestError;
        let captured = capture.capture_error(&error);
        
        assert_eq!(captured.code(), "ERR-USR-UI-001_ERR_O");
        assert_eq!(captured.source(), ErrorSource::USR);
        assert_eq!(captured.severity(), Severity::ERROR);
        assert_eq!(captured.impact_scope(), ImpactScope::OPERATION);
    }
    
    #[test]
    fn test_error_propagation() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;
        
        let capture = BusinessErrorCapture::new("user_service", "create_user");
        let error = TestError;
        let captured = capture.capture_error(&error);
        
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), json!("12345"));
        let propagated = ErrorPropagation::append_context(captured, "api_gateway", context);
        
        assert_eq!(propagated.context_chain().len(), 1);
        assert_eq!(propagated.context_chain()[0].source(), "api_gateway");
    }

    #[test]
    fn test_gateway_error_capture() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;
        
        let capture = GatewayErrorCapture::new("auth_service", "login");
        let error = TestError;
        let captured = capture.capture_error(&error);
        
        assert_eq!(captured.code(), "ERR-NET-GW-001_ERR_S");
        assert_eq!(captured.source(), ErrorSource::NET);
        assert_eq!(captured.severity(), Severity::ERROR);
        assert_eq!(captured.impact_scope(), ImpactScope::SESSION);
    }

    #[test]
    fn test_business_error_capture() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;
        
        let capture = BusinessErrorCapture::new("order_service", "process_payment");
        let error = TestError;
        let captured = capture.capture_error(&error);
        
        assert_eq!(captured.code(), "ERR-INT-BL-001_ERR_M");
        assert_eq!(captured.source(), ErrorSource::INT);
        assert_eq!(captured.severity(), Severity::ERROR);
        assert_eq!(captured.impact_scope(), ImpactScope::MODULE);
    }

    #[test]
    fn test_infrastructure_error_capture() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;
        
        let capture = InfrastructureErrorCapture::new("database", "connection");
        let error = TestError;
        let captured = capture.capture_error(&error);
        
        assert_eq!(captured.code(), "ERR-SYS-IF-001_ERR_G");
        assert_eq!(captured.source(), ErrorSource::SYS);
        assert_eq!(captured.severity(), Severity::ERROR);
        assert_eq!(captured.impact_scope(), ImpactScope::GLOBAL);
    }

    #[test]
    fn test_frontend_error_capture_add_context() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;
        
        let capture = FrontendErrorCapture::new("user_dashboard");
        let error = TestError;
        let mut captured = capture.capture_error(&error);
        
        let mut context = HashMap::new();
        context.insert("component".to_string(), json!("login_form"));
        capture.add_context(&mut captured, context);
        
        assert_eq!(captured.context_chain().len(), 1);
    }

    #[test]
    fn test_gateway_error_capture_add_context() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;
        
        let capture = GatewayErrorCapture::new("auth_service", "login");
        let error = TestError;
        let mut captured = capture.capture_error(&error);
        
        let mut context = HashMap::new();
        context.insert("request_id".to_string(), json!("req_123"));
        capture.add_context(&mut captured, context);
        
        assert_eq!(captured.context_chain().len(), 1);
    }

    #[test]
    fn test_business_error_capture_add_context() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;
        
        let capture = BusinessErrorCapture::new("order_service", "process_payment");
        let error = TestError;
        let mut captured = capture.capture_error(&error);
        
        let mut context = HashMap::new();
        context.insert("order_id".to_string(), json!("ord_456"));
        capture.add_context(&mut captured, context);
        
        assert_eq!(captured.context_chain().len(), 1);
    }

    #[test]
    fn test_infrastructure_error_capture_add_context() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;
        
        let capture = InfrastructureErrorCapture::new("database", "connection");
        let error = TestError;
        let mut captured = capture.capture_error(&error);
        
        let mut context = HashMap::new();
        context.insert("db_instance".to_string(), json!("prod_db"));
        capture.add_context(&mut captured, context);
        
        assert_eq!(captured.context_chain().len(), 1);
    }

    #[test]
    fn test_error_propagation_wrap_error() {
        #[derive(Debug, thiserror::Error)]
        #[error("Inner error")]
        struct InnerError;
        
        #[derive(Debug, thiserror::Error)]
        #[error("Outer error")]
        struct OuterError;
        
        let inner_capture = InfrastructureErrorCapture::new("database", "connection");
        let inner_error = InnerError;
        let inner_captured = inner_capture.capture_error(&inner_error);
        
        let outer_capture = BusinessErrorCapture::new("order_service", "process_payment");
        let outer_error = OuterError;
        let outer_captured = outer_capture.capture_error(&outer_error);
        
        let wrapped = ErrorPropagation::wrap_error(outer_captured, inner_captured);
        assert!(wrapped.cause().is_some());
    }

    #[test]
    fn test_error_propagation_strip_internal_details() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;
        
        let capture = BusinessErrorCapture::new("user_service", "create_user");
        let error = TestError;
        let mut captured = capture.capture_error(&error);
        
        ErrorPropagation::strip_internal_details(&mut captured);
    }

    #[test]
    fn test_frontend_error_capture_get_user_friendly_message() {
        #[derive(Debug, thiserror::Error)]
        #[error("Test error")]
        struct TestError;
        
        let _capture = FrontendErrorCapture::new("user_dashboard");
        let error = TestError;
        let message = FrontendErrorCapture::get_user_friendly_message(&error);
        assert!(message.contains("Test error"));
    }
}
