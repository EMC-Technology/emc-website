#![deny(missing_docs)]
#![allow(clippy::result_large_err)] // ErrorObject 含因果链+上下文帧+恢复提示，体积较大但语义完整
//! Error handling core library for `InfinityEvolutionIDE`
//! 
//! This crate provides a comprehensive error handling system for `InfinityEvolutionIDE`,
//! covering the entire error lifecycle from detection to recovery.
//! 
//! ## Design Philosophy
//! 
//! The error handling system is designed with the following principles:
//! 
//! 1. **Semantic Consistency**: All errors have a unified classification fingerprint
//! 2. **Traceability**: Complete error chain with context preservation
//! 3. **User Experience**: Technical errors translated to user-friendly messages
//! 4. **System Resilience**: Hierarchical recovery capabilities
//! 5. **Operational Observability**: Structured logging and audit trails
//! 
//! ## Quick Start
//! 
//! ```rust
//! use error_core::helpers;
//!
//! // Create an error object via helpers (recommended)
//! let _error = helpers::internal_error("Something went wrong");
//! ```
//! 
//! ## Feature Matrix
//! 
//! | Feature | Description | Dependencies | Default |
//! |---------|-------------|--------------|---------|
//! | `std` | Standard library support | None | Yes |
//! | `serde` | Serialization/deserialization | serde | No |
//! | `uuid` | UUID error identifiers | uuid | No |
//! | `chrono` | Timestamp support | chrono | No |
//! | `wasm` | WASM target support | serde + uuid + serde-json | No |
//! | `db` | SurrealDB error conversion | surrealdb | No |
//! | `serde-json` | JSON serialization + serde_json::Error conversion | serde | No |
//! | `jsonwebtoken` | JWT error conversion | jsonwebtoken | No |
//! | `logging` | Structured logging (tracing) | tracing + tracing-subscriber | Yes |
//! | `regex` | Regex-based error code validation | regex | No |
//! | `full` | All features enabled | All above | No |

/// Error classification system
/// 
/// Defines error classification enums and their string representation.
pub mod classification;

/// Error code generation and validation
/// 
/// Provides utilities for generating and validating error codes.
pub mod error_code;

/// Error object core data model
/// 
/// Defines the core `ErrorObject` struct and its builder.
pub mod error_object;

/// Error capture mechanisms
/// 
/// Implements error capture for different layers of the application.
pub mod error_capture;

/// Error propagation structures
/// 
/// Defines context frames and recovery hints for error propagation.
pub mod propagation;

/// Error logging strategies
/// 
/// Implements structured logging and log file management.
pub mod logging;

/// Error recovery mechanisms
/// 
/// Implements recovery state machine and retry strategies.
pub mod recovery;

/// User-friendly prompt design
/// 
/// Provides utilities for generating user-friendly error messages.
pub mod user_prompt;

/// Utility functions
/// 
/// Provides general utility functions for error handling.
pub mod utils;

/// Error conversion implementations
///
/// Provides `From` implementations for converting external error types into ErrorObject.
pub mod conversions;

/// Unified error construction helpers
///
/// All business crates MUST use these helpers to construct errors.
pub mod helpers;

/// Prelude module
/// 
/// Exports commonly used types and functions.
pub mod prelude;

pub use prelude::*;


