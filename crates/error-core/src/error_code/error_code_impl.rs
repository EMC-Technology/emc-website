//! Error code generation and validation
//! 
//! This module defines the `ErrorCode` struct for generating and validating error codes
//! according to the specified format: ERR-[SOURCE]-[MODULE]-[SEQ]_[SEVERITY][IMPACT_SCOPE].

use crate::classification::{ErrorSource, Severity, ImpactScope};
use crate::error_object::ErrorObject;
use crate::classification::Recoverability;

#[cfg(feature = "regex")]
use std::sync::OnceLock;
#[cfg(feature = "regex")]
use regex::Regex;

#[cfg(feature = "regex")]
static ERROR_CODE_REGEX: OnceLock<Regex> = OnceLock::new();

#[cfg(feature = "regex")]
fn error_code_regex() -> &'static Regex {
    ERROR_CODE_REGEX.get_or_init(|| {
        Regex::new(r"^ERR-([A-Z]{2,5})-([A-Z]{2,5})-(\d{3})_(CRI|ERR|WRN|INF)_([GSMO])$")
            .expect("ERROR_CODE_REGEX: 正则编译不应失败")
    })
}

pub(crate) fn ec_error(message: &str) -> ErrorObject {
    ErrorObject::builder()
        .code(super::registry::EC_INTERNAL)
        .source(ErrorSource::INT)
        .severity(Severity::ERROR)
        .impact_scope(ImpactScope::OPERATION)
        .recoverability(Recoverability::NonRecoverable)
        .message(message)
        .user_message("错误码处理失败")
        .module_path("error_code")
        .operation("parse")
        .build()
}

/// `Error` code struct
/// 
/// Represents a standardized error code with validation and parsing capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ErrorCode {
    /// The full error code string
    code: String,
    /// Error source
    source: ErrorSource,
    /// Module identifier
    module: String,
    /// Sequence number
    sequence: u32,
    /// Severity level
    severity: Severity,
    /// Impact scope
    impact_scope: ImpactScope,
}

impl ErrorCode {
    /// Create a new `ErrorCode` from components
    ///
    /// # Errors
    ///
    /// Returns an error if the module string is empty or contains invalid characters.
    pub fn new(
        source: ErrorSource,
        module: &str,
        sequence: u32,
        severity: Severity,
        impact_scope: ImpactScope,
    ) -> crate::Result<Self> {
        let source_str = source.as_str();
        let severity_str = severity.as_str();
        let impact_str = impact_scope.as_str();
        
        let code = format!("ERR-{source_str}-{module}-{sequence:03}_{severity_str}_{impact_str}");
        
        Self::parse(&code)
    }
    
    /// Parse an error code string
    /// 
    /// # Errors
    /// 
    /// Returns an error if the code doesn't match the expected format or contains invalid fields.
    pub fn parse(code: &str) -> crate::Result<Self> {
        #[cfg(feature = "regex")]
        {
            let regex = error_code_regex();
            let captures = regex.captures(code)
                .ok_or_else(|| ec_error(&format!("错误码格式无效: {code}")))?;
            let source_str = captures.get(1).ok_or_else(|| ec_error("正则捕获组1缺失"))?.as_str();
            let module = captures.get(2).ok_or_else(|| ec_error("正则捕获组2缺失"))?.as_str().to_string();
            let sequence_str = captures.get(3).ok_or_else(|| ec_error("正则捕获组3缺失"))?.as_str();
            let severity_str = captures.get(4).ok_or_else(|| ec_error("正则捕获组4缺失"))?.as_str();
            let impact_str = captures.get(5).ok_or_else(|| ec_error("正则捕获组5缺失"))?.as_str();
            let source = source_str.parse::<ErrorSource>()
                .map_err(|e| ec_error(&format!("无效的错误来源: {e}")))?;
            let sequence = sequence_str.parse::<u32>()
                .map_err(|e| ec_error(&format!("无效的序号: {e}")))?;
            let severity = severity_str.parse::<Severity>()
                .map_err(|e| ec_error(&format!("无效的严重级别: {e}")))?;
            let impact_scope = impact_str.parse::<ImpactScope>()
                .map_err(|e| ec_error(&format!("无效的影响范围: {e}")))?;
            Ok(Self { code: code.to_string(), source, module, sequence, severity, impact_scope })
        }
        #[cfg(not(feature = "regex"))]
        {
            let parts: Vec<&str> = code.splitn(2, '-').collect();
            if parts.len() != 2 || parts[0] != "ERR" {
                return Err(ec_error(&format!("错误码格式无效: {code}")));
            }
            let rest = parts[1];
            let parts: Vec<&str> = rest.splitn(3, '-').collect();
            if parts.len() < 3 { return Err(ec_error(&format!("错误码格式无效: {code}"))); }
            let source = parts[0].parse::<ErrorSource>()
                .map_err(|e| ec_error(&format!("无效的错误来源: {e}")))?;
            let module = parts[1].to_string();
            let seq_and_suffix = parts[2];
            let seq_str_end = seq_and_suffix.find('_').ok_or_else(|| ec_error("错误码缺少下划线分隔符"))?;
            let sequence = seq_and_suffix[..seq_str_end].parse::<u32>()
                .map_err(|e| ec_error(&format!("无效的序号: {e}")))?;
            let suffix = &seq_and_suffix[seq_str_end + 1..];
            let parts: Vec<&str> = suffix.split('_').collect();
            if parts.len() < 2 { return Err(ec_error("错误码缺少严重级别或影响范围")); }
            let severity = parts[0].parse::<Severity>()
                .map_err(|e| ec_error(&format!("无效的严重级别: {e}")))?;
            let impact_scope = parts[1].parse::<ImpactScope>()
                .map_err(|e| ec_error(&format!("无效的影响范围: {e}")))?;
            Ok(Self { code: code.to_string(), source, module, sequence, severity, impact_scope })
        }
    }
    
    /// Get the full error code string
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }
    
    /// Get the error source
    #[must_use]
    pub const fn source(&self) -> ErrorSource {
        self.source
    }
    
    /// Get the module identifier
    #[must_use]
    pub fn module(&self) -> &str {
        &self.module
    }
    
    /// Get the sequence number
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }
    
    /// Get the severity level
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }
    
    /// Get the impact scope
    #[must_use]
    pub const fn impact_scope(&self) -> ImpactScope {
        self.impact_scope
    }
    

}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_code_parse() {
        let code = "ERR-AIM-LM-002_ERR_S";
        let result = ErrorCode::parse(code);
        assert!(result.is_ok());
        let error_code = result.unwrap();
        assert_eq!(error_code.code(), code);
        assert_eq!(error_code.source(), ErrorSource::AIM);
        assert_eq!(error_code.module(), "LM");
        assert_eq!(error_code.sequence(), 2);
        assert_eq!(error_code.severity(), Severity::ERROR);
        assert_eq!(error_code.impact_scope(), ImpactScope::SESSION);
    }
    
    #[test]
    fn test_error_code_new() {
        let result = ErrorCode::new(
            ErrorSource::AIM,
            "LM",
            2,
            Severity::ERROR,
            ImpactScope::SESSION,
        );
        assert!(result.is_ok());
        let error_code = result.unwrap();
        assert_eq!(error_code.code(), "ERR-AIM-LM-002_ERR_S");
    }
    
    #[test]
    fn test_error_code_invalid() {
        let invalid_codes = [
            "INVALID",
            "ERR-AIM-LM-002_INVALID",
            "ERR-AIM-LM-002_ERR_X",
        ];

        for code in invalid_codes {
            let result = ErrorCode::parse(code);
            assert!(result.is_err());
        }

        #[cfg(feature = "regex")]
        {
            let regex_only_invalid = ["ERR-AIM-LM-00_ERR_S"];
            for code in regex_only_invalid {
                let result = ErrorCode::parse(code);
                assert!(result.is_err());
            }
        }
    }
    
    #[test]
    fn test_error_code_invalid_source() {
        // Test with invalid error source
        let result = ErrorCode::parse("ERR-XXX-LM-002_ERR_S");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_error_code_invalid_sequence() {
        // Test with non-numeric sequence
        let result = ErrorCode::parse("ERR-AIM-LM-ABC_ERR_S");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_error_code_invalid_severity() {
        // Test with invalid severity
        let result = ErrorCode::parse("ERR-AIM-LM-002_INVALID_S");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_error_code_invalid_impact_scope() {
        // Test with invalid impact scope
        let result = ErrorCode::parse("ERR-AIM-LM-002_ERR_X");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_error_code_getters() {
        let result = ErrorCode::new(
            ErrorSource::AIM,
            "LM",
            2,
            Severity::ERROR,
            ImpactScope::SESSION,
        );
        assert!(result.is_ok());
        let error_code = result.unwrap();
        
        // Test all getter methods
        assert_eq!(error_code.code(), "ERR-AIM-LM-002_ERR_S");
        assert_eq!(error_code.source(), ErrorSource::AIM);
        assert_eq!(error_code.module(), "LM");
        assert_eq!(error_code.sequence(), 2);
        assert_eq!(error_code.severity(), Severity::ERROR);
        assert_eq!(error_code.impact_scope(), ImpactScope::SESSION);
    }
    
    #[test]
    fn test_error_code_new_with_invalid_module() {
        #[cfg(feature = "regex")]
        {
            let result = ErrorCode::new(
                ErrorSource::AIM,
                "LONGMODULE",
                2,
                Severity::ERROR,
                ImpactScope::SESSION,
            );
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_error_code_new_with_all_severities() {
        // Test with all severity levels
        let severities = [
            Severity::CRITICAL,
            Severity::ERROR,
            Severity::WARNING,
            Severity::INFO,
        ];

        for severity in severities {
            let result = ErrorCode::new(
                ErrorSource::AIM,
                "LM",
                1,
                severity,
                ImpactScope::SESSION,
            );
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_error_code_new_with_all_impact_scopes() {
        // Test with all impact scopes
        let impact_scopes = [
            ImpactScope::GLOBAL,
            ImpactScope::SESSION,
            ImpactScope::MODULE,
            ImpactScope::OPERATION,
        ];

        for impact_scope in impact_scopes {
            let result = ErrorCode::new(
                ErrorSource::AIM,
                "LM",
                1,
                Severity::ERROR,
                impact_scope,
            );
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_error_code_new_with_all_sources() {
        // Test with all error sources
        let sources = [
            ErrorSource::AIM,
            ErrorSource::EXT,
            ErrorSource::INT,
            ErrorSource::NET,
            ErrorSource::SYS,
            ErrorSource::USR,
        ];

        for source in sources {
            let result = ErrorCode::new(
                source,
                "LM",
                1,
                Severity::ERROR,
                ImpactScope::SESSION,
            );
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_error_code_parse_with_invalid_regex() {
        // This test is to ensure that the regex compilation error path is covered
        // We can't directly test the regex compilation failure, but we can test other error paths
        let invalid_codes = [
            "ERR-AIM-LM-002_ERR", // Missing impact scope
            "ERR-AIM-LM_ERR_S", // Missing sequence
            "ERR-AIM-002_ERR_S", // Missing module
            "ERR-002_ERR_S", // Missing source
            "ERR-AIM-LM-002_", // Missing severity and impact scope
        ];

        for code in invalid_codes {
            let result = ErrorCode::parse(code);
            assert!(result.is_err());
        }
    }
}  
