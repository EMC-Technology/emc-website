//! Error propagation structures
//!
//! This module defines structures related to error propagation, including context frames,
//! recovery hints, and retry configurations.

#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Recovery action type (AP-B07 fix: replaces `String`-based action representation)
///
/// Represents the finite set of recovery strategies available in the system.
/// Using an enum instead of `String` ensures compile-time exhaustiveness checking
/// and prevents typos from silently producing invalid recovery hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum RecoveryAction {
    /// Retry the same operation (possibly with backoff)
    Retry,
    /// Fall back to an alternative implementation or data source
    Fallback,
    /// Redirect the request to another service or endpoint
    Redirect,
    /// Skip the failing operation and continue
    Skip,
    /// Escalate to a human operator or higher-level system
    Escalate,
    /// Activate a circuit breaker to prevent cascading failures
    CircuitBreak,
    /// Degrade gracefully to a reduced-functionality mode
    Degrade,
    /// Cache the result and serve stale data
    Cache,
    /// Re-authenticate and retry
    Reauthenticate,
    /// No recovery action available
    None,
}

impl RecoveryAction {
    /// Convert to a static string representation
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::Fallback => "fallback",
            Self::Redirect => "redirect",
            Self::Skip => "skip",
            Self::Escalate => "escalate",
            Self::CircuitBreak => "circuit_break",
            Self::Degrade => "degrade",
            Self::Cache => "cache",
            Self::Reauthenticate => "reauthenticate",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for RecoveryAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for RecoveryAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "retry" => Ok(Self::Retry),
            "fallback" => Ok(Self::Fallback),
            "redirect" => Ok(Self::Redirect),
            "skip" => Ok(Self::Skip),
            "escalate" => Ok(Self::Escalate),
            "circuit_break" => Ok(Self::CircuitBreak),
            "degrade" => Ok(Self::Degrade),
            "cache" => Ok(Self::Cache),
            "reauthenticate" => Ok(Self::Reauthenticate),
            "none" => Ok(Self::None),
            _ => Err(()),
        }
    }
}

/// Context frame for error propagation
///
/// Represents a frame of context information added to an error as it propagates through the system.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContextFrame {
    source: String,
    #[cfg(feature = "chrono")]
    timestamp: DateTime<Utc>,
    data: HashMap<String, serde_json::Value>,
}

impl PartialEq for ContextFrame {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.data == other.data
    }
}

impl Eq for ContextFrame {}

impl ContextFrame {
    /// Create a new `ContextFrame`
    #[must_use]
    pub fn new(source: &str, data: HashMap<String, serde_json::Value>) -> Self {
        Self {
            source: source.to_string(),
            #[cfg(feature = "chrono")]
            timestamp: Utc::now(), // 确定性例外：时间戳记录事件发生时刻，需真实时间而非可复现值
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
    action: RecoveryAction,
    /// Description of the recovery action
    description: String,
    /// Parameters for the recovery action
    params: HashMap<String, serde_json::Value>,
}

impl RecoveryHint {
    /// Create a new `RecoveryHint`
    #[must_use]
    pub fn new(
        action: RecoveryAction,
        description: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            action,
            description: description.to_string(),
            params,
        }
    }

    /// Get the recovery action
    #[must_use]
    pub const fn action(&self) -> &RecoveryAction {
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
        assert_eq!(
            frame.data().get("operation").unwrap(),
            &json!("generate_code")
        );
    }

    #[test]
    fn test_recovery_hint() {
        let mut params = HashMap::new();
        params.insert("retry_delay_ms".to_string(), json!(1000));
        params.insert("max_attempts".to_string(), json!(3));

        let hint = RecoveryHint::new(
            RecoveryAction::Retry,
            "Retry the AI model call with exponential backoff",
            params,
        );
        assert_eq!(hint.action(), &RecoveryAction::Retry);
        assert_eq!(
            hint.description(),
            "Retry the AI model call with exponential backoff"
        );
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
