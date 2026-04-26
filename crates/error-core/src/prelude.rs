//! Prelude module for error-core
//! 
//! This module re-exports the most commonly used types and functions from the error-core library.

pub use crate::classification::{ErrorSource, Severity, ImpactScope, Recoverability};
pub use crate::error_code::ErrorCode;
pub use crate::error_object::ErrorObject;
pub use crate::error_capture::ErrorCapture;
pub use crate::propagation::{ContextFrame, RecoveryHint, RetryConfig};
pub use crate::logging::LogEntry;
pub use crate::recovery::{RecoveryStateMachine, ExponentialBackoff, CircuitBreaker};
pub use crate::user_prompt::UserPromptManager;
pub use crate::helpers;

/// Unified Result type alias for error-core
pub type Result<T> = std::result::Result<T, ErrorObject>;
