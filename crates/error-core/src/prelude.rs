//! Prelude module for error-core
//!
//! This module re-exports the most commonly used types and functions from the error-core library.

pub use crate::classification::{ErrorSource, ImpactScope, Recoverability, Severity};
pub use crate::error_capture::ErrorCapture;
pub use crate::error_code::ErrorCode;
pub use crate::error_object::ErrorObject;
pub use crate::helpers;
#[cfg(feature = "logging")]
pub use crate::logging::LogEntry;
pub use crate::propagation::{ContextFrame, RecoveryAction, RecoveryHint, RetryConfig};
pub use crate::recovery::{CircuitBreaker, ExponentialBackoff, RecoveryStateMachine};
pub use crate::user_prompt::UserPromptManager;

/// Unified Result type alias for error-core
pub type Result<T> = std::result::Result<T, ErrorObject>;
