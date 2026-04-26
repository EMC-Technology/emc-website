//! Error propagation structures
//! 
//! This module defines structures related to error propagation, including context frames,
//! recovery hints, and retry configurations.

use std::collections::HashMap;
#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

/// Context frame for error propagation
/// 
/// Represents a frame of context information added to an error as it propagates through the system.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContextFrame {
    /// The source module that added this context
    source: String,
    /// The timestamp when this context was added
    #[cfg(feature = "chrono")]
    timestamp: DateTime<Utc>,
    /// Key-value pairs of context information
    data: HashMap<String, serde_json::Value>,
}

impl ContextFrame {
    /// Create a new `ContextFrame`
    #[must_use]
    pub fn new(source: &str, data: HashMap<String, serde_json::Value>) -> Self {
        Self {
            source: source.to_string(),
            #[cfg(feature = "chrono")]
            timestamp: Utc::now(),
            data,
        }
    }
    
    /// Get the source module
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
    
    /// Get the timestamp
    #[cfg(feature = "chrono")]
    #[must_use]
    pub const fn timestamp(&self) -> &DateTime<Utc> {
        &self.timestamp
    }
    
    /// Get the context data
    #[must_use]
    pub const fn data(&self) -> &HashMap<String, serde_json::Value> {
        &self.data
    }
}

/// Recovery hint for error handling
/// 
/// Provides suggestions for how to recover from an error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RecoveryHint {
    /// The type of recovery action suggested
    action: String,
    /// Description of the recovery action
    description: String,
    /// Parameters for the recovery action
    params: HashMap<String, serde_json::Value>,
}

impl RecoveryHint {
    /// Create a new `RecoveryHint`
    #[must_use]
    pub fn new(action: &str, description: &str, params: HashMap<String, serde_json::Value>) -> Self {
        Self {
            action: action.to_string(),
            description: description.to_string(),
            params,
        }
    }
    
    /// Get the recovery action
    #[must_use]
    pub fn action(&self) -> &str {
        &self.action
    }
    
    /// Get the recovery description
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
    
    /// Get the recovery parameters
    #[must_use]
    pub const fn params(&self) -> &HashMap<String, serde_json::Value> {
        &self.params
    }
}

/// Retry configuration for error recovery
/// 
/// Defines the retry strategy for recoverable errors.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    max_attempts: u32,
    /// Initial delay in milliseconds
    initial_delay_ms: u64,
    /// Maximum delay in milliseconds
    max_delay_ms: u64,
    /// Backoff multiplier
    backoff_multiplier: f64,
    /// Whether to use jitter in the retry delays
    use_jitter: bool,
}

impl RetryConfig {
    /// Create a new `RetryConfig`
    #[must_use]
    pub const fn new(
        max_attempts: u32,
        initial_delay_ms: u64,
        max_delay_ms: u64,
        backoff_multiplier: f64,
        use_jitter: bool,
    ) -> Self {
        Self {
            max_attempts,
            initial_delay_ms,
            max_delay_ms,
            backoff_multiplier,
            use_jitter,
        }
    }
    
    /// Get the maximum number of attempts
    #[must_use]
    pub const fn max_attempts(&self) -> u32 {
        self.max_attempts
    }
    
    /// Get the initial delay in milliseconds
    #[must_use]
    pub const fn initial_delay_ms(&self) -> u64 {
        self.initial_delay_ms
    }
    
    /// Get the maximum delay in milliseconds
    #[must_use]
    pub const fn max_delay_ms(&self) -> u64 {
        self.max_delay_ms
    }
    
    /// Get the backoff multiplier
    #[must_use]
    pub const fn backoff_multiplier(&self) -> f64 {
        self.backoff_multiplier
    }
    
    /// Get whether to use jitter
    #[must_use]
    pub const fn use_jitter(&self) -> bool {
        self.use_jitter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_context_frame() {
        let mut data = HashMap::new();
        data.insert("user_id".to_string(), json!("12345"));
        data.insert("operation".to_string(), json!("generate_code"));
        
        let frame = ContextFrame::new("ai_model::lm_manager", data);
        assert_eq!(frame.source(), "ai_model::lm_manager");
        assert_eq!(frame.data().get("user_id").unwrap(), &json!("12345"));
        assert_eq!(frame.data().get("operation").unwrap(), &json!("generate_code"));
    }
    
    #[test]
    fn test_recovery_hint() {
        let mut params = HashMap::new();
        params.insert("retry_delay_ms".to_string(), json!(1000));
        params.insert("max_attempts".to_string(), json!(3));
        
        let hint = RecoveryHint::new(
            "retry",
            "Retry the AI model call with exponential backoff",
            params,
        );
        assert_eq!(hint.action(), "retry");
        assert_eq!(hint.description(), "Retry the AI model call with exponential backoff");
        assert_eq!(hint.params().get("retry_delay_ms").unwrap(), &json!(1000));
        assert_eq!(hint.params().get("max_attempts").unwrap(), &json!(3));
    }
    
    #[test]
    #[allow(clippy::float_cmp)]
    fn test_retry_config() {
        let config = RetryConfig::new(3, 1000, 10000, 2.0, true);
        assert_eq!(config.max_attempts(), 3);
        assert_eq!(config.initial_delay_ms(), 1000);
        assert_eq!(config.max_delay_ms(), 10000);
        assert_eq!(config.backoff_multiplier(), 2.0);
        assert!(config.use_jitter());
    }
}
