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

// ── E 短码映射 ──────────────────────────────────────────────

/// E1001 — 无效 `RecordID`
pub const E1001: &str = INVALID_RECORD_ID;
/// E1002 — 资源未找到
pub const E1002: &str = NOT_FOUND;
/// E1003 — 输入验证失败
pub const E1003: &str = VALIDATION_FAILED;
/// E2001 — 数据库查询失败
pub const E2001: &str = DB_QUERY_FAILED;
/// E2002 — I/O 操作失败
pub const E2002: &str = IO_FAILED;
/// E2003 — 文件解析失败
pub const E2003: &str = PARSE_FAILED;
/// E2004 — 不支持的格式
pub const E2004: &str = UNSUPPORTED_FORMAT;
/// E3001 — 网络连接丢失
pub const E3001: &str = CONNECTION_LOST;
/// E3002 — 网络 API 响应错误
pub const E3002: &str = NET_API_ERROR;
/// E3003 — 网关/API 层错误
pub const E3003: &str = GATEWAY_ERROR;
/// E3004 — WebSocket 客户端错误
pub const E3004: &str = WS_CLIENT_ERROR;
/// E3005 — WebSocket 发送失败
pub const E3005: &str = WS_SEND_FAILED;
/// E3006 — WebSocket 接收失败
pub const E3006: &str = WS_RECEIVE_FAILED;
/// E4001 — 认证失败
pub const E4001: &str = AUTH_FAILED;
/// E4002 — 令牌无效或过期
pub const E4002: &str = TOKEN_INVALID;
/// E4003 — 加密/解密失败
pub const E4003: &str = CRYPTO_FAILED;
/// E5001 — 序列化/反序列化失败
pub const E5001: &str = SERDE_FAILED;
/// E5002 — 内部系统错误
pub const E5002: &str = INTERNAL_ERROR;
/// E5003 — 通用回退错误
pub const E5003: &str = GENERAL_FALLBACK;
/// E5004 — 业务逻辑错误
pub const E5004: &str = BUSINESS_LOGIC_ERROR;
/// E5005 — 基础设施层错误
pub const E5005: &str = INFRASTRUCTURE_ERROR;
/// E5006 — 评估超时
pub const E5006: &str = EVAL_TIMEOUT;
/// E6001 — 配置错误
pub const E6001: &str = CONFIG_FAILED;
/// E7001 — 嵌入模型配置错误
pub const E7001: &str = EMBEDDING_CONFIG_ERROR;
/// E7002 — 嵌入模型未加载
pub const E7002: &str = EMBEDDING_MODEL_NOT_LOADED;
/// E7003 — 嵌入模型加载失败
pub const E7003: &str = EMBEDDING_MODEL_LOAD_FAILED;
/// E7004 — 嵌入推理失败
pub const E7004: &str = EMBEDDING_INFERENCE_FAILED;
/// E7005 — 嵌入输入为空
pub const E7005: &str = EMBEDDING_EMPTY_INPUT;
/// E7006 — 嵌入 Tokenizer 错误
pub const E7006: &str = EMBEDDING_TOKENIZER_ERROR;
/// E7007 — 嵌入 I/O 错误
pub const E7007: &str = EMBEDDING_IO_ERROR;
/// E8001 — Agent LLM 调用失败
pub const E8001: &str = AGENT_LLM_ERROR;
/// E8002 — Agent 工具调用失败
pub const E8002: &str = AGENT_TOOL_ERROR;
/// E8003 — Agent 工具不存在
pub const E8003: &str = AGENT_TOOL_NOT_FOUND;
/// E8004 — Agent 达到最大迭代
pub const E8004: &str = AGENT_MAX_ITERATIONS;
/// E8005 — Agent 执行超时
pub const E8005: &str = AGENT_TIMEOUT;
/// E8006 — Agent 需要澄清
pub const E8006: &str = AGENT_CLARIFICATION;
/// E8007 — Agent 动作解析失败
pub const E8007: &str = AGENT_PARSE_ACTION;
/// E8008 — Agent 记忆系统错误
pub const E8008: &str = AGENT_MEMORY_ERROR;
/// E8009 — Agent 状态转换无效
pub const E8009: &str = AGENT_INVALID_TRANSITION;
/// E8010 — Agent 工作流错误
pub const E8010: &str = AGENT_WORKFLOW_ERROR;
/// E8011 — Agent 序列化错误
pub const E8011: &str = AGENT_SERIALIZATION_ERROR;
/// E8012 — Agent 权限不足
pub const E8012: &str = AGENT_PERMISSION_DENIED;
/// E8013 — Agent 安全检查失败
pub const E8013: &str = AGENT_SAFETY_CHECK;
/// E9001 — 聚合状态无效
pub const E9001: &str = AGGREGATE_INVALID_STATE;
/// E9002 — 聚合业务规则违反
pub const E9002: &str = AGGREGATE_BUSINESS_RULE;
/// E9003 — 聚合未找到
pub const E9003: &str = AGGREGATE_NOT_FOUND;
/// E9004 — 聚合版本冲突
pub const E9004: &str = AGGREGATE_VERSION_CONFLICT;
/// E9005 — 聚合序列化错误
pub const E9005: &str = AGGREGATE_SERIALIZATION;
/// EA001 — L2 缓存连接失败
pub const EA001: &str = CACHE_L2_CONNECTION;
/// EA002 — L2 缓存序列化失败
pub const EA002: &str = CACHE_L2_SERIALIZATION;
/// EA003 — L2 缓存反序列化失败
pub const EA003: &str = CACHE_L2_DESERIALIZATION;
/// EA004 — L2 缓存操作失败
pub const EA004: &str = CACHE_L2_OPERATION;
/// EA005 — 缓存管理器 L2 错误
pub const EA005: &str = CACHE_MANAGER_L2;
/// EA006 — 缓存管理器序列化错误
pub const EA006: &str = CACHE_MANAGER_SERIALIZATION;
/// EB001 — 模型不支持
pub const EB001: &str = MODEL_LOADER_UNSUPPORTED_MODEL;
/// EB002 — 推理后端不支持
pub const EB002: &str = MODEL_LOADER_UNSUPPORTED_BACKEND;
/// EB003 — 模型文件未找到
pub const EB003: &str = MODEL_LOADER_NOT_FOUND;
/// EB004 — 模型加载失败
pub const EB004: &str = MODEL_LOADER_LOAD_FAILED;
/// EB005 — Tokenizer 加载失败
pub const EB005: &str = MODEL_LOADER_TOKENIZER;
/// EB006 — 模型推理失败
pub const EB006: &str = MODEL_LOADER_INFERENCE;
/// EB007 — 设备初始化失败
pub const EB007: &str = MODEL_LOADER_DEVICE;
/// EB008 — 模型配置错误
pub const EB008: &str = MODEL_LOADER_CONFIG;
/// EB009 — 模型 I/O 错误
pub const EB009: &str = MODEL_LOADER_IO;
/// EB010 — 模型序列化错误
pub const EB010: &str = MODEL_LOADER_SERIALIZATION;
/// EB011 — 模型下载失败
pub const EB011: &str = MODEL_DOWNLOAD_FAILED;
/// EB012 — 模型下载网络错误
pub const EB012: &str = MODEL_DOWNLOAD_NETWORK;
/// EB013 — 模型下载 I/O 错误
pub const EB013: &str = MODEL_DOWNLOAD_IO;
/// EB014 — 模型下载 JSON 解析错误
pub const EB014: &str = MODEL_DOWNLOAD_JSON;
/// EB015 — 模型不存在
pub const EB015: &str = MODEL_DOWNLOAD_NOT_FOUND;
/// EB016 — 模型校验和不匹配
pub const EB016: &str = MODEL_DOWNLOAD_CHECKSUM;
/// EB017 — 缓存目录创建失败
pub const EB017: &str = MODEL_DOWNLOAD_CACHE_DIR;
/// EB018 — 模型下载被取消
pub const EB018: &str = MODEL_DOWNLOAD_CANCELLED;
/// EC001 — 事件类型不匹配
pub const EC001: &str = EVENT_TYPE_MISMATCH;
/// EC002 — 事件处理失败
pub const EC002: &str = EVENT_PROCESSING_FAILED;
/// EC003 — 事件无订阅者
pub const EC003: &str = EVENT_NO_SUBSCRIBERS;
/// EC004 — 事件总线已关闭
pub const EC004: &str = EVENT_BUS_SHUTDOWN;
/// EC005 — 事件发布超时
pub const EC005: &str = EVENT_PUBLISH_TIMEOUT;
/// ED001 — 前端/UI 层错误
pub const ED001: &str = FRONTEND_UI_ERROR;
/// ED002 — 可观测性 Forbidden 错误
pub const ED002: &str = OBSERVABILITY_FORBIDDEN;
/// EE001 — LLM 判决失败
pub const EE001: &str = LLM_JUDGMENT_FAILED;
/// EE002 — 指标计算失败
pub const EE002: &str = METRIC_CALC_FAILED;
/// EE003 — 数据集加载失败
pub const EE003: &str = DATASET_LOAD_FAILED;
/// EE004 — LLM API 调用失败
pub const EE004: &str = LLM_API_FAILED;
/// EE005 — 实体/关系抽取失败
pub const EE005: &str = EXTRACTION_FAILED;
/// EE006 — 实体消歧失败
pub const EE006: &str = DISAMBIGUATION_FAILED;

// ── ERR 长码常量 ────────────────────────────────────────────
//
// 每个常量的语义已在对应的 E 短码映射和 helper 函数中体现，
// 此处按域分组声明，常量名本身即为自文档化。

#[allow(missing_docs)]
// USR — 用户层
pub const INVALID_RECORD_ID: &str = "ERR-USR-VAL-001_ERR_O";
pub const NOT_FOUND: &str = "ERR-USR-VAL-002_ERR_O";
pub const VALIDATION_FAILED: &str = "ERR-USR-VAL-003_WRN_O";
pub const UNSUPPORTED_FORMAT: &str = "ERR-USR-PARSE-002_WRN_O";
pub const FRONTEND_UI_ERROR: &str = "ERR-USR-UI-001_ERR_O";

// FS — 文件系统/存储层
pub const DB_QUERY_FAILED: &str = "ERR-FS-DB-001_ERR_O";
pub const IO_FAILED: &str = "ERR-FS-IO-001_ERR_O";
pub const PARSE_FAILED: &str = "ERR-FS-PARSE-001_ERR_O";
pub const DATASET_LOAD_FAILED: &str = "ERR-FS-DATA-001_ERR_S";

// NET — 网络层
pub const CONNECTION_LOST: &str = "ERR-NET-NET-001_ERR_S";
pub const NET_API_ERROR: &str = "ERR-NET-API-001_ERR_O";
pub const GATEWAY_ERROR: &str = "ERR-NET-GW-001_ERR_S";
pub const WS_CLIENT_ERROR: &str = "ERR-NET-WSCL-001_ERR_O";
pub const WS_SEND_FAILED: &str = "ERR-NET-WS-001_WRN_O";
pub const WS_RECEIVE_FAILED: &str = "ERR-NET-WS-002_WRN_O";
pub const WS_SUBSCRIBE_FAILED: &str = "ERR-SESS-SUB-001_ERR_O";

// SEC — 安全层
pub const AUTH_FAILED: &str = "ERR-SEC-AUTH-001_ERR_O";
pub const TOKEN_INVALID: &str = "ERR-SEC-AUTH-002_WRN_O";
pub const CRYPTO_FAILED: &str = "ERR-SEC-CRYPTO-001_CRI_O";

// INT — 内部/基础设施层
pub const SERDE_FAILED: &str = "ERR-INT-SERDE-001_ERR_O";
pub const INTERNAL_ERROR: &str = "ERR-INT-SYS-001_CRI_G";
pub const GENERAL_FALLBACK: &str = "ERR-INT-GEN-001_ERR_O";
pub const BUSINESS_LOGIC_ERROR: &str = "ERR-INT-BL-001_ERR_M";
pub const INFRASTRUCTURE_ERROR: &str = "ERR-SYS-IF-001_ERR_G";
pub const API_DESERIALIZE_ERROR: &str = "ERR-INT-API-002_ERR_O";
pub const API_REQUEST_ERROR: &str = "ERR-INT-API-003_ERR_O";
pub const WS_SERIALIZE_ERROR: &str = "ERR-INT-WSCL-002_ERR_O";
pub const EC_INTERNAL: &str = "ERR-INT-EC-001_ERR_O";
pub const EVAL_TIMEOUT: &str = "ERR-INT-TIMEOUT-001_WRN_O";

// CFG — 配置层
pub const CONFIG_FAILED: &str = "ERR-CFG-CONF-001_ERR_O";

// AIM — AI/模型层
pub const LLM_JUDGMENT_FAILED: &str = "ERR-AIM-JUDGE-001_ERR_O";
pub const METRIC_CALC_FAILED: &str = "ERR-AIM-METRIC-001_ERR_O";
pub const LLM_API_FAILED: &str = "ERR-AIM-LLM-001_ERR_O";
pub const EXTRACTION_FAILED: &str = "ERR-AIM-EXTRACT-001_ERR_O";
pub const DISAMBIGUATION_FAILED: &str = "ERR-AIM-AMBIG-001_WRN_O";

// ── 嵌入模型错误码 ──────────────────────────────────────────
pub const EMBEDDING_CONFIG_ERROR: &str = "ERR-AIM-EMBD-001_ERR_O";
pub const EMBEDDING_MODEL_NOT_LOADED: &str = "ERR-AIM-EMBD-002_ERR_O";
pub const EMBEDDING_MODEL_LOAD_FAILED: &str = "ERR-AIM-EMBD-003_CRI_O";
pub const EMBEDDING_INFERENCE_FAILED: &str = "ERR-AIM-EMBD-004_ERR_O";
pub const EMBEDDING_EMPTY_INPUT: &str = "ERR-AIM-EMBD-005_WRN_O";
pub const EMBEDDING_TOKENIZER_ERROR: &str = "ERR-AIM-EMBD-006_ERR_O";
pub const EMBEDDING_IO_ERROR: &str = "ERR-AIM-EMBD-007_ERR_O";

// ── Agent 错误码 ────────────────────────────────────────────
pub const AGENT_LLM_ERROR: &str = "ERR-AIM-AGENT-001_ERR_O";
pub const AGENT_TOOL_ERROR: &str = "ERR-AIM-AGENT-002_ERR_O";
pub const AGENT_TOOL_NOT_FOUND: &str = "ERR-AIM-AGENT-003_WRN_O";
pub const AGENT_MAX_ITERATIONS: &str = "ERR-AIM-AGENT-004_WRN_O";
pub const AGENT_TIMEOUT: &str = "ERR-AIM-AGENT-005_WRN_S";
pub const AGENT_CLARIFICATION: &str = "ERR-AIM-AGENT-006_WRN_O";
pub const AGENT_PARSE_ACTION: &str = "ERR-AIM-AGENT-007_ERR_O";
pub const AGENT_MEMORY_ERROR: &str = "ERR-AIM-AGENT-008_ERR_O";
pub const AGENT_INVALID_TRANSITION: &str = "ERR-AIM-AGENT-009_ERR_O";
pub const AGENT_WORKFLOW_ERROR: &str = "ERR-AIM-AGENT-010_ERR_O";
pub const AGENT_SERIALIZATION_ERROR: &str = "ERR-AIM-AGENT-011_ERR_O";
pub const AGENT_PERMISSION_DENIED: &str = "ERR-SEC-AGENT-012_ERR_O";
pub const AGENT_SAFETY_CHECK: &str = "ERR-SEC-AGENT-013_CRI_O";

// ── CQRS 聚合错误码 ─────────────────────────────────────────
pub const AGGREGATE_INVALID_STATE: &str = "ERR-INT-AGG-001_ERR_O";
pub const AGGREGATE_BUSINESS_RULE: &str = "ERR-INT-AGG-002_ERR_O";
pub const AGGREGATE_NOT_FOUND: &str = "ERR-INT-AGG-003_WRN_O";
pub const AGGREGATE_VERSION_CONFLICT: &str = "ERR-INT-AGG-004_ERR_O";
pub const AGGREGATE_SERIALIZATION: &str = "ERR-INT-AGG-005_ERR_O";

// ── 缓存错误码 ──────────────────────────────────────────────
pub const CACHE_L2_CONNECTION: &str = "ERR-FS-CACHE-001_ERR_S";
pub const CACHE_L2_SERIALIZATION: &str = "ERR-INT-CACHE-002_ERR_O";
pub const CACHE_L2_DESERIALIZATION: &str = "ERR-INT-CACHE-003_ERR_O";
pub const CACHE_L2_OPERATION: &str = "ERR-FS-CACHE-004_ERR_O";
pub const CACHE_MANAGER_L2: &str = "ERR-FS-CACHE-005_ERR_O";
pub const CACHE_MANAGER_SERIALIZATION: &str = "ERR-INT-CACHE-006_ERR_O";

// ── 模型加载器错误码 ────────────────────────────────────────
pub const MODEL_LOADER_UNSUPPORTED_MODEL: &str = "ERR-AIM-MODEL-001_ERR_O";
pub const MODEL_LOADER_UNSUPPORTED_BACKEND: &str = "ERR-AIM-MODEL-002_ERR_O";
pub const MODEL_LOADER_NOT_FOUND: &str = "ERR-FS-MODEL-003_WRN_O";
pub const MODEL_LOADER_LOAD_FAILED: &str = "ERR-AIM-MODEL-004_CRI_O";
pub const MODEL_LOADER_TOKENIZER: &str = "ERR-AIM-MODEL-005_ERR_O";
pub const MODEL_LOADER_INFERENCE: &str = "ERR-AIM-MODEL-006_ERR_O";
pub const MODEL_LOADER_DEVICE: &str = "ERR-AIM-MODEL-007_CRI_O";
pub const MODEL_LOADER_CONFIG: &str = "ERR-AIM-MODEL-008_ERR_O";
pub const MODEL_LOADER_IO: &str = "ERR-FS-MODEL-009_ERR_O";
pub const MODEL_LOADER_SERIALIZATION: &str = "ERR-INT-MODEL-010_ERR_O";
pub const MODEL_DOWNLOAD_FAILED: &str = "ERR-NET-DOWN-001_ERR_O";
pub const MODEL_DOWNLOAD_NETWORK: &str = "ERR-NET-DOWN-002_ERR_S";
pub const MODEL_DOWNLOAD_IO: &str = "ERR-FS-DOWN-003_ERR_O";
pub const MODEL_DOWNLOAD_JSON: &str = "ERR-INT-DOWN-004_ERR_O";
pub const MODEL_DOWNLOAD_NOT_FOUND: &str = "ERR-NET-DOWN-005_WRN_O";
pub const MODEL_DOWNLOAD_CHECKSUM: &str = "ERR-SEC-DOWN-006_CRI_O";
pub const MODEL_DOWNLOAD_CACHE_DIR: &str = "ERR-FS-DOWN-007_ERR_O";
pub const MODEL_DOWNLOAD_CANCELLED: &str = "ERR-NET-DOWN-008_WRN_O";

// ── 事件系统错误码 ──────────────────────────────────────────
pub const EVENT_TYPE_MISMATCH: &str = "ERR-INT-EVNT-001_ERR_O";
pub const EVENT_PROCESSING_FAILED: &str = "ERR-INT-EVNT-002_ERR_O";
pub const EVENT_NO_SUBSCRIBERS: &str = "ERR-INT-EVNT-003_WRN_O";
pub const EVENT_BUS_SHUTDOWN: &str = "ERR-INT-EVNT-004_WRN_S";
pub const EVENT_PUBLISH_TIMEOUT: &str = "ERR-INT-EVNT-005_WRN_O";

// ── 可观测性错误码 ──────────────────────────────────────────
pub const OBSERVABILITY_FORBIDDEN: &str = "ERR-SEC-OBSV-001_ERR_O";
