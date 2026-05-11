//! Error classification system
//!
//! This module defines the core enums for error classification, including error source,
//! severity level, impact scope, and recoverability.

use std::fmt;
use std::str::FromStr;

/// 无效的错误来源解析错误
#[derive(Debug, Clone, thiserror::Error)]
#[error("无效的错误来源: {0}")]
pub struct InvalidErrorSource(String);

/// 无效的严重级别解析错误
#[derive(Debug, Clone, thiserror::Error)]
#[error("无效的严重级别: {0}")]
pub struct InvalidSeverity(String);

/// 无效的影响范围解析错误
#[derive(Debug, Clone, thiserror::Error)]
#[error("无效的影响范围: {0}")]
pub struct InvalidImpactScope(String);

/// 无效的可恢复性解析错误
#[derive(Debug, Clone, thiserror::Error)]
#[error("无效的可恢复性: {0}")]
pub struct InvalidRecoverability(String);
// proptest 导入位于 #[cfg(test)] mod tests 块内

/// Error source classification
///
/// Represents the origin of an error, such as user input, AI model, file system, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ErrorSource {
    /// User input error
    USR,
    /// AI model error
    AIM,
    /// File system error
    FS,
    /// Network error
    NET,
    /// Configuration error
    CFG,
    /// Security error
    SEC,
    /// Tool execution error
    TOOL,
    /// Session management error
    SESS,
    /// State management error
    STATE,
    /// Extension/plugin error
    EXT,
    /// LSP integration error
    LSP,
    /// MCP integration error
    MCP,
    /// System resource error
    SYS,
    /// Internal logic error
    INT,
    /// Unknown error
    UNK,
}

impl ErrorSource {
    /// Get the string representation of the error source
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::USR => "USR",
            Self::AIM => "AIM",
            Self::FS => "FS",
            Self::NET => "NET",
            Self::CFG => "CFG",
            Self::SEC => "SEC",
            Self::TOOL => "TOOL",
            Self::SESS => "SESS",
            Self::STATE => "STATE",
            Self::EXT => "EXT",
            Self::LSP => "LSP",
            Self::MCP => "MCP",
            Self::SYS => "SYS",
            Self::INT => "INT",
            Self::UNK => "UNK",
        }
    }
}

impl FromStr for ErrorSource {
    type Err = InvalidErrorSource;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "USR" => Ok(Self::USR),
            "AIM" => Ok(Self::AIM),
            "FS" => Ok(Self::FS),
            "NET" => Ok(Self::NET),
            "CFG" => Ok(Self::CFG),
            "SEC" => Ok(Self::SEC),
            "TOOL" => Ok(Self::TOOL),
            "SESS" => Ok(Self::SESS),
            "STATE" => Ok(Self::STATE),
            "EXT" => Ok(Self::EXT),
            "LSP" => Ok(Self::LSP),
            "MCP" => Ok(Self::MCP),
            "SYS" => Ok(Self::SYS),
            "INT" => Ok(Self::INT),
            "UNK" => Ok(Self::UNK),
            _ => Err(InvalidErrorSource(s.to_string())),
        }
    }
}

impl fmt::Display for ErrorSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Severity level classification
///
/// Represents the severity of an error, from critical to info.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Severity {
    /// Critical error: system cannot continue to run
    CRITICAL,
    /// Error: current operation failed but system is still available
    ERROR,
    /// Warning: operation completed but with potential issues
    WARNING,
    /// Info: important notification in normal operation
    INFO,
}

impl Severity {
    /// Get the string representation of the severity
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CRITICAL => "CRI",
            Self::ERROR => "ERR",
            Self::WARNING => "WRN",
            Self::INFO => "INF",
        }
    }
}

impl FromStr for Severity {
    type Err = InvalidSeverity;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "CRI" => Ok(Self::CRITICAL),
            "ERR" => Ok(Self::ERROR),
            "WRN" => Ok(Self::WARNING),
            "INF" => Ok(Self::INFO),
            _ => Err(InvalidSeverity(s.to_string())),
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Impact scope classification
///
/// Represents the scope of impact of an error, from global to module-level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImpactScope {
    /// Global impact: affects the entire system or all users
    GLOBAL,
    /// Session impact: affects only the current session
    SESSION,
    /// Operation impact: affects only the current operation
    OPERATION,
    /// Module impact: affects only a specific module
    MODULE,
}

impl ImpactScope {
    /// Get the string representation of the impact scope
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::GLOBAL => "G",
            Self::SESSION => "S",
            Self::OPERATION => "O",
            Self::MODULE => "M",
        }
    }
}

impl FromStr for ImpactScope {
    type Err = InvalidImpactScope;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "G" => Ok(Self::GLOBAL),
            "S" => Ok(Self::SESSION),
            "O" => Ok(Self::OPERATION),
            "M" => Ok(Self::MODULE),
            _ => Err(InvalidImpactScope(s.to_string())),
        }
    }
}

impl fmt::Display for ImpactScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Recoverability classification
///
/// Represents the recoverability of an error, from auto-recoverable to non-recoverable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Recoverability {
    /// Auto-recoverable: can be recovered without human intervention
    AutoRecoverable,
    /// Semi-auto recoverable: requires user confirmation
    SemiAuto,
    /// Manual intervention: requires developer/operator intervention
    ManualIntervention,
    /// Non-recoverable: cannot be recovered
    NonRecoverable,
}

impl Recoverability {
    /// Get the string representation of the recoverability
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::AutoRecoverable => "AUTO",
            Self::SemiAuto => "SEMI",
            Self::ManualIntervention => "MANUAL",
            Self::NonRecoverable => "NON",
        }
    }
}

impl FromStr for Recoverability {
    type Err = InvalidRecoverability;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AUTO" => Ok(Self::AutoRecoverable),
            "SEMI" => Ok(Self::SemiAuto),
            "MANUAL" => Ok(Self::ManualIntervention),
            "NON" => Ok(Self::NonRecoverable),
            _ => Err(InvalidRecoverability(s.to_string())),
        }
    }
}

impl fmt::Display for Recoverability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
use proptest::prelude::*;

#[cfg(test)]
impl Arbitrary for ErrorSource {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            Just(Self::USR),
            Just(Self::AIM),
            Just(Self::FS),
            Just(Self::NET),
            Just(Self::CFG),
            Just(Self::SEC),
            Just(Self::TOOL),
            Just(Self::SESS),
            Just(Self::STATE),
            Just(Self::EXT),
            Just(Self::LSP),
            Just(Self::MCP),
            Just(Self::SYS),
            Just(Self::INT),
            Just(Self::UNK),
        ]
        .boxed()
    }
}

#[cfg(test)]
impl Arbitrary for Severity {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            Just(Self::CRITICAL),
            Just(Self::ERROR),
            Just(Self::WARNING),
            Just(Self::INFO),
        ]
        .boxed()
    }
}

#[cfg(test)]
impl Arbitrary for ImpactScope {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            Just(Self::GLOBAL),
            Just(Self::SESSION),
            Just(Self::MODULE),
            Just(Self::OPERATION),
        ]
        .boxed()
    }
}

#[cfg(test)]
impl Arbitrary for Recoverability {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            Just(Self::AutoRecoverable),
            Just(Self::SemiAuto),
            Just(Self::ManualIntervention),
            Just(Self::NonRecoverable),
        ]
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_source_as_str() {
        assert_eq!(ErrorSource::USR.as_str(), "USR");
        assert_eq!(ErrorSource::AIM.as_str(), "AIM");
        assert_eq!(ErrorSource::FS.as_str(), "FS");
        assert_eq!(ErrorSource::NET.as_str(), "NET");
        assert_eq!(ErrorSource::CFG.as_str(), "CFG");
        assert_eq!(ErrorSource::SEC.as_str(), "SEC");
        assert_eq!(ErrorSource::TOOL.as_str(), "TOOL");
        assert_eq!(ErrorSource::SESS.as_str(), "SESS");
        assert_eq!(ErrorSource::STATE.as_str(), "STATE");
        assert_eq!(ErrorSource::EXT.as_str(), "EXT");
        assert_eq!(ErrorSource::LSP.as_str(), "LSP");
        assert_eq!(ErrorSource::MCP.as_str(), "MCP");
        assert_eq!(ErrorSource::SYS.as_str(), "SYS");
        assert_eq!(ErrorSource::INT.as_str(), "INT");
        assert_eq!(ErrorSource::UNK.as_str(), "UNK");
    }

    #[test]
    fn test_error_source_from_str() {
        assert_eq!(ErrorSource::from_str("USR").unwrap(), ErrorSource::USR);
        assert_eq!(ErrorSource::from_str("AIM").unwrap(), ErrorSource::AIM);
        assert_eq!(ErrorSource::from_str("FS").unwrap(), ErrorSource::FS);
        assert_eq!(ErrorSource::from_str("NET").unwrap(), ErrorSource::NET);
        assert_eq!(ErrorSource::from_str("CFG").unwrap(), ErrorSource::CFG);
        assert_eq!(ErrorSource::from_str("SEC").unwrap(), ErrorSource::SEC);
        assert_eq!(ErrorSource::from_str("TOOL").unwrap(), ErrorSource::TOOL);
        assert_eq!(ErrorSource::from_str("SESS").unwrap(), ErrorSource::SESS);
        assert_eq!(ErrorSource::from_str("STATE").unwrap(), ErrorSource::STATE);
        assert_eq!(ErrorSource::from_str("EXT").unwrap(), ErrorSource::EXT);
        assert_eq!(ErrorSource::from_str("LSP").unwrap(), ErrorSource::LSP);
        assert_eq!(ErrorSource::from_str("MCP").unwrap(), ErrorSource::MCP);
        assert_eq!(ErrorSource::from_str("SYS").unwrap(), ErrorSource::SYS);
        assert_eq!(ErrorSource::from_str("INT").unwrap(), ErrorSource::INT);
        assert_eq!(ErrorSource::from_str("UNK").unwrap(), ErrorSource::UNK);
        assert!(ErrorSource::from_str("INVALID").is_err());
    }

    #[test]
    fn test_error_source_display() {
        assert_eq!(format!("{}", ErrorSource::USR), "USR");
        assert_eq!(format!("{}", ErrorSource::AIM), "AIM");
    }

    #[test]
    fn test_severity_as_str() {
        assert_eq!(Severity::CRITICAL.as_str(), "CRI");
        assert_eq!(Severity::ERROR.as_str(), "ERR");
        assert_eq!(Severity::WARNING.as_str(), "WRN");
        assert_eq!(Severity::INFO.as_str(), "INF");
    }

    #[test]
    fn test_severity_from_str() {
        assert_eq!(Severity::from_str("CRI").unwrap(), Severity::CRITICAL);
        assert_eq!(Severity::from_str("ERR").unwrap(), Severity::ERROR);
        assert_eq!(Severity::from_str("WRN").unwrap(), Severity::WARNING);
        assert_eq!(Severity::from_str("INF").unwrap(), Severity::INFO);
        assert!(Severity::from_str("INVALID").is_err());
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(format!("{}", Severity::CRITICAL), "CRI");
        assert_eq!(format!("{}", Severity::ERROR), "ERR");
    }

    #[test]
    fn test_impact_scope_as_str() {
        assert_eq!(ImpactScope::GLOBAL.as_str(), "G");
        assert_eq!(ImpactScope::SESSION.as_str(), "S");
        assert_eq!(ImpactScope::OPERATION.as_str(), "O");
        assert_eq!(ImpactScope::MODULE.as_str(), "M");
    }

    #[test]
    fn test_impact_scope_from_str() {
        assert_eq!(ImpactScope::from_str("G").unwrap(), ImpactScope::GLOBAL);
        assert_eq!(ImpactScope::from_str("S").unwrap(), ImpactScope::SESSION);
        assert_eq!(ImpactScope::from_str("O").unwrap(), ImpactScope::OPERATION);
        assert_eq!(ImpactScope::from_str("M").unwrap(), ImpactScope::MODULE);
        assert!(ImpactScope::from_str("INVALID").is_err());
    }

    #[test]
    fn test_impact_scope_display() {
        assert_eq!(format!("{}", ImpactScope::GLOBAL), "G");
        assert_eq!(format!("{}", ImpactScope::SESSION), "S");
    }

    #[test]
    fn test_recoverability_as_str() {
        assert_eq!(Recoverability::AutoRecoverable.as_str(), "AUTO");
        assert_eq!(Recoverability::SemiAuto.as_str(), "SEMI");
        assert_eq!(Recoverability::ManualIntervention.as_str(), "MANUAL");
        assert_eq!(Recoverability::NonRecoverable.as_str(), "NON");
    }

    #[test]
    fn test_recoverability_from_str() {
        assert_eq!(
            Recoverability::from_str("AUTO").unwrap(),
            Recoverability::AutoRecoverable
        );
        assert_eq!(
            Recoverability::from_str("SEMI").unwrap(),
            Recoverability::SemiAuto
        );
        assert_eq!(
            Recoverability::from_str("MANUAL").unwrap(),
            Recoverability::ManualIntervention
        );
        assert_eq!(
            Recoverability::from_str("NON").unwrap(),
            Recoverability::NonRecoverable
        );
        assert!(Recoverability::from_str("INVALID").is_err());
    }

    #[test]
    fn test_recoverability_display() {
        assert_eq!(format!("{}", Recoverability::AutoRecoverable), "AUTO");
        assert_eq!(format!("{}", Recoverability::SemiAuto), "SEMI");
    }
}
