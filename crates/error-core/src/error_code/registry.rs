//! Error code central registry
//!
//! All error codes used across the system are defined here as constants.
//! This ensures uniqueness, consistency, and discoverability.
//!
//! Format: `ERR-{SOURCE}-{MODULE}-{SEQ}_{SEVERITY}_{IMPACT}`
//!
//! - SOURCE: 2-5 uppercase letters (see `ErrorSource`)
//! - MODULE: 2-5 uppercase letters
//! - SEQ: 3-digit zero-padded sequence number
//! - SEVERITY: CRI | ERR | WRN | INF
//! - IMPACT: G | S | O | M
//!
//! # 设计文档短码映射
//!
//! 设计文档使用 E 短码（如 E1001），注册表使用 ERR 长码。
//! 两套体系通过常量别名保持映射关系：

/// E1001 — 无效 `RecordID`
pub const E1001: &str = INVALID_RECORD_ID;
/// E1002 — 资源未找到
pub const E1002: &str = NOT_FOUND;
/// E2001 — 数据库查询失败
pub const E2001: &str = DB_QUERY_FAILED;
/// E2002 — 网络连接丢失
pub const E2002: &str = CONNECTION_LOST;
/// E3001 — 解析失败
pub const E3001: &str = PARSE_FAILED;
/// E3002 — 不支持的格式
pub const E3002: &str = UNSUPPORTED_FORMAT;
/// E4001 — 序列化/反序列化失败
pub const E4001: &str = SERDE_FAILED;
/// E5001 — 加密/解密失败
pub const E5001: &str = CRYPTO_FAILED;

/// Invalid `RecordID` format
pub const INVALID_RECORD_ID: &str = "ERR-USR-VAL-001_ERR_O";
/// Resource not found
pub const NOT_FOUND: &str = "ERR-USR-VAL-002_WRN_O";
/// Input validation failed
pub const VALIDATION_FAILED: &str = "ERR-USR-VAL-003_ERR_O";

/// Database query failed
pub const DB_QUERY_FAILED: &str = "ERR-FS-DB-001_ERR_O";
/// I/O operation failed
pub const IO_FAILED: &str = "ERR-FS-IO-001_ERR_O";
/// File parsing failed
pub const PARSE_FAILED: &str = "ERR-FS-PARSE-001_ERR_O";

/// Unsupported file format
pub const UNSUPPORTED_FORMAT: &str = "ERR-USR-PARSE-002_WRN_O";

/// Network connection lost
pub const CONNECTION_LOST: &str = "ERR-NET-NET-001_ERR_S";

/// Authentication failed
pub const AUTH_FAILED: &str = "ERR-SEC-AUTH-001_ERR_O";
/// Token invalid or expired
pub const TOKEN_INVALID: &str = "ERR-SEC-AUTH-002_WRN_O";
/// Encryption/decryption failed
pub const CRYPTO_FAILED: &str = "ERR-SEC-CRYPTO-001_CRI_O";

/// Serialization/deserialization failed
pub const SERDE_FAILED: &str = "ERR-INT-SERDE-001_ERR_O";
/// Internal system error
pub const INTERNAL_ERROR: &str = "ERR-INT-SYS-001_CRI_G";
/// General fallback error
pub const GENERAL_FALLBACK: &str = "ERR-INT-GEN-001_ERR_O";

/// Configuration error
pub const CONFIG_FAILED: &str = "ERR-CFG-CONF-001_ERR_O";

/// Error code module internal error
pub const EC_INTERNAL: &str = "ERR-INT-EC-001_ERR_O";

/// WebSocket subscription limit reached
pub const WS_SUBSCRIBE_FAILED: &str = "ERR-SESS-SUB-001_ERR_O";
/// WebSocket send failed (no active subscribers)
pub const WS_SEND_FAILED: &str = "ERR-NET-WS-001_WRN_O";
/// WebSocket broadcast target not found
pub const WS_RECEIVE_FAILED: &str = "ERR-NET-WS-002_WRN_O";

/// Frontend/UI layer error
pub const FRONTEND_UI_ERROR: &str = "ERR-USR-UI-001_ERR_O";
/// Gateway/API layer error
pub const GATEWAY_ERROR: &str = "ERR-NET-GW-001_ERR_S";
/// Business logic layer error
pub const BUSINESS_LOGIC_ERROR: &str = "ERR-INT-BL-001_ERR_M";
/// Infrastructure layer error
pub const INFRASTRUCTURE_ERROR: &str = "ERR-SYS-IF-001_ERR_G";
/// Network API response error
pub const NET_API_ERROR: &str = "ERR-NET-API-001_ERR_O";
/// Deserialization error (API layer)
pub const API_DESERIALIZE_ERROR: &str = "ERR-INT-API-002_ERR_O";
/// API request construction error
pub const API_REQUEST_ERROR: &str = "ERR-INT-API-003_ERR_O";
/// WebSocket client error
pub const WS_CLIENT_ERROR: &str = "ERR-NET-WSCL-001_ERR_O";
/// WebSocket serialization error
pub const WS_SERIALIZE_ERROR: &str = "ERR-INT-WSCL-002_ERR_O";
