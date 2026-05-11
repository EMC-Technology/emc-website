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
/// E2002 — 网络连接丢失
pub const E2002: &str = CONNECTION_LOST;
/// E2003 — I/O 操作失败
pub const E2003: &str = IO_FAILED;
/// E2004 — 文件解析失败
pub const E2004: &str = PARSE_FAILED;
/// E3001 — 不支持的格式
pub const E3001: &str = UNSUPPORTED_FORMAT;
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
/// E4001 — 序列化/反序列化失败
pub const E4001: &str = SERDE_FAILED;
/// E4002 — 令牌无效或过期
pub const E4002: &str = TOKEN_INVALID;
/// E4003 — 加密/解密失败
pub const E4003: &str = CRYPTO_FAILED;
/// E5001 — 认证失败
pub const E5001: &str = AUTH_FAILED;
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
/// EL001 — LLM 网络错误
pub const EL001: &str = ULLM_NETWORK;
/// EL002 — LLM 超时
pub const EL002: &str = ULLM_TIMEOUT;
/// EL003 — LLM 速率限制
pub const EL003: &str = ULLM_RATE_LIMIT;
/// EL004 — LLM 认证失败
pub const EL004: &str = ULLM_AUTH;
/// EL005 — LLM 权限错误
pub const EL005: &str = ULLM_PERMISSION;
/// EL006 — LLM 上下文窗口超限
pub const EL006: &str = ULLM_CONTEXT_WINDOW;
/// EL007 — LLM Prompt 过大
pub const EL007: &str = ULLM_PROMPT_TOO_LARGE;
/// EL008 — LLM 模型不可用
pub const EL008: &str = ULLM_MODEL_UNAVAILABLE;
/// EL009 — LLM 服务端过载
pub const EL009: &str = ULLM_SERVER_OVERLOADED;
/// EL010 — LLM 无效请求
pub const EL010: &str = ULLM_INVALID_REQUEST;
/// EL011 — LLM 流错误
pub const EL011: &str = ULLM_STREAM;
/// EL012 — LLM 工具调用错误
pub const EL012: &str = ULLM_TOOL_CALL;
/// EL013 — LLM 缺少凭证
pub const EL013: &str = ULLM_MISSING_CREDENTIALS;
/// EL014 — LLM Token 过期
pub const EL014: &str = ULLM_EXPIRED_TOKEN;
/// EL015 — LLM JSON 解析失败
pub const EL015: &str = ULLM_JSON_PARSE;
/// EL016 — LLM 重试耗尽
pub const EL016: &str = ULLM_RETRIES_EXHAUSTED;
/// EL017 — LLM 配置错误
pub const EL017: &str = ULLM_CONFIG;
/// EL018 — LLM 请求体过大
pub const EL018: &str = ULLM_REQUEST_BODY_SIZE;
/// EL019 — LLM 已取消
pub const EL019: &str = ULLM_CANCELLED;
/// EL020 — LLM 未知错误
pub const EL020: &str = ULLM_OTHER;
/// EL021 — LLM HTTP 客户端初始化失败
pub const EL021: &str = ULLM_HTTP_CLIENT_INIT;
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

// USR — 用户层

/// 无效 `RecordID` (USR/ERR/OPERATION)
pub const INVALID_RECORD_ID: &str = "ERR-USR-VAL-001_ERR_O";
/// 资源未找到 (USR/ERR/OPERATION)
pub const NOT_FOUND: &str = "ERR-USR-VAL-002_ERR_O";
/// 输入验证失败 (USR/WRN/OPERATION)
pub const VALIDATION_FAILED: &str = "ERR-USR-VAL-003_WRN_O";
/// 不支持的文件格式 (USR/WRN/OPERATION)
pub const UNSUPPORTED_FORMAT: &str = "ERR-USR-PARSE-002_WRN_O";
/// 前端/UI 层错误 (USR/ERR/OPERATION)
pub const FRONTEND_UI_ERROR: &str = "ERR-USR-UI-001_ERR_O";

// FS — 文件系统/存储层

/// 数据库查询失败 (FS/ERR/OPERATION)
pub const DB_QUERY_FAILED: &str = "ERR-FS-DB-001_ERR_O";
/// I/O 操作失败 (FS/ERR/OPERATION)
pub const IO_FAILED: &str = "ERR-FS-IO-001_ERR_O";
/// 文件解析失败 (FS/ERR/OPERATION)
pub const PARSE_FAILED: &str = "ERR-FS-PARSE-001_ERR_O";
/// 数据集加载失败 (FS/ERR/SESSION)
pub const DATASET_LOAD_FAILED: &str = "ERR-FS-DATA-001_ERR_S";

// NET — 网络层

/// 网络连接丢失 (NET/ERR/SESSION)
pub const CONNECTION_LOST: &str = "ERR-NET-NET-001_ERR_S";
/// 网络 API 响应错误 (NET/ERR/OPERATION)
pub const NET_API_ERROR: &str = "ERR-NET-API-001_ERR_O";
/// 网关/API 层错误 (NET/ERR/SESSION)
pub const GATEWAY_ERROR: &str = "ERR-NET-GW-001_ERR_S";
/// WebSocket 客户端错误 (NET/ERR/OPERATION)
pub const WS_CLIENT_ERROR: &str = "ERR-NET-WSCL-001_ERR_O";
/// WebSocket 发送失败 (NET/WRN/OPERATION)
pub const WS_SEND_FAILED: &str = "ERR-NET-WS-001_WRN_O";
/// WebSocket 接收失败 (NET/WRN/OPERATION)
pub const WS_RECEIVE_FAILED: &str = "ERR-NET-WS-002_WRN_O";
/// WebSocket 订阅失败 (SESS/ERR/OPERATION)
pub const WS_SUBSCRIBE_FAILED: &str = "ERR-SESS-SUB-001_ERR_O";

// SEC — 安全层

/// 认证失败 (SEC/ERR/OPERATION)
pub const AUTH_FAILED: &str = "ERR-SEC-AUTH-001_ERR_O";
/// 令牌无效或过期 (SEC/WRN/OPERATION)
pub const TOKEN_INVALID: &str = "ERR-SEC-AUTH-002_WRN_O";
/// 加密/解密失败 (SEC/CRI/OPERATION)
pub const CRYPTO_FAILED: &str = "ERR-SEC-CRYPTO-001_CRI_O";

// INT — 内部/基础设施层

/// 序列化/反序列化失败 (INT/ERR/OPERATION)
pub const SERDE_FAILED: &str = "ERR-INT-SERDE-001_ERR_O";
/// 内部系统错误 (INT/CRI/GLOBAL)
pub const INTERNAL_ERROR: &str = "ERR-INT-SYS-001_CRI_G";
/// 通用回退错误 (INT/ERR/OPERATION)
pub const GENERAL_FALLBACK: &str = "ERR-INT-GEN-001_ERR_O";
/// 业务逻辑错误 (INT/ERR/MODULE)
pub const BUSINESS_LOGIC_ERROR: &str = "ERR-INT-BL-001_ERR_M";
/// 基础设施层错误 (SYS/ERR/GLOBAL)
pub const INFRASTRUCTURE_ERROR: &str = "ERR-SYS-IF-001_ERR_G";
/// API 反序列化错误 (INT/ERR/OPERATION)
pub const API_DESERIALIZE_ERROR: &str = "ERR-INT-API-002_ERR_O";
/// API 请求构建错误 (INT/ERR/OPERATION)
pub const API_REQUEST_ERROR: &str = "ERR-INT-API-003_ERR_O";
/// WebSocket 序列化错误 (INT/ERR/OPERATION)
pub const WS_SERIALIZE_ERROR: &str = "ERR-INT-WSCL-002_ERR_O";
/// 错误码内部错误 (INT/ERR/OPERATION)
pub const EC_INTERNAL: &str = "ERR-INT-EC-001_ERR_O";
/// 评估超时 (INT/WRN/OPERATION)
pub const EVAL_TIMEOUT: &str = "ERR-INT-TOUT-001_WRN_O";

// CFG — 配置层

/// 配置错误 (CFG/ERR/OPERATION)
pub const CONFIG_FAILED: &str = "ERR-CFG-CONF-001_ERR_O";

// AIM — AI/模型层

/// LLM 判决失败 (AIM/ERR/OPERATION)
pub const LLM_JUDGMENT_FAILED: &str = "ERR-AIM-JUDGE-001_ERR_O";
/// 指标计算失败 (AIM/ERR/OPERATION)
pub const METRIC_CALC_FAILED: &str = "ERR-AIM-METRIC-001_ERR_O";
/// LLM API 调用失败 (AIM/ERR/OPERATION)
pub const LLM_API_FAILED: &str = "ERR-AIM-LLM-001_ERR_O";
/// 实体/关系抽取失败 (AIM/ERR/OPERATION)
pub const EXTRACTION_FAILED: &str = "ERR-AIM-EXTT-001_ERR_O";
/// 实体消歧失败 (AIM/WRN/OPERATION)
pub const DISAMBIGUATION_FAILED: &str = "ERR-AIM-AMBIG-001_WRN_O";

// ── 嵌入模型错误码 ──────────────────────────────────────────

/// 嵌入配置错误 (AIM/ERR/OPERATION)
pub const EMBEDDING_CONFIG_ERROR: &str = "ERR-AIM-EMBD-001_ERR_O";
/// 嵌入模型未加载 (AIM/ERR/OPERATION)
pub const EMBEDDING_MODEL_NOT_LOADED: &str = "ERR-AIM-EMBD-002_ERR_O";
/// 嵌入模型加载失败 (AIM/CRI/OPERATION)
pub const EMBEDDING_MODEL_LOAD_FAILED: &str = "ERR-AIM-EMBD-003_CRI_O";
/// 嵌入推理失败 (AIM/ERR/OPERATION)
pub const EMBEDDING_INFERENCE_FAILED: &str = "ERR-AIM-EMBD-004_ERR_O";
/// 嵌入空输入 (AIM/WRN/OPERATION)
pub const EMBEDDING_EMPTY_INPUT: &str = "ERR-AIM-EMBD-005_WRN_O";
/// 嵌入分词器错误 (AIM/ERR/OPERATION)
pub const EMBEDDING_TOKENIZER_ERROR: &str = "ERR-AIM-EMBD-006_ERR_O";
/// 嵌入 I/O 错误 (AIM/ERR/OPERATION)
pub const EMBEDDING_IO_ERROR: &str = "ERR-AIM-EMBD-007_ERR_O";

// ── Agent 错误码 ────────────────────────────────────────────

/// Agent LLM 调用错误 (AIM/ERR/OPERATION)
pub const AGENT_LLM_ERROR: &str = "ERR-AIM-AGENT-001_ERR_O";
/// Agent 工具执行错误 (AIM/ERR/OPERATION)
pub const AGENT_TOOL_ERROR: &str = "ERR-AIM-AGENT-002_ERR_O";
/// Agent 工具未找到 (AIM/WRN/OPERATION)
pub const AGENT_TOOL_NOT_FOUND: &str = "ERR-AIM-AGENT-003_WRN_O";
/// Agent 达到最大迭代次数 (AIM/WRN/OPERATION)
pub const AGENT_MAX_ITERATIONS: &str = "ERR-AIM-AGENT-004_WRN_O";
/// Agent 执行超时 (AIM/WRN/SESSION)
pub const AGENT_TIMEOUT: &str = "ERR-AIM-AGENT-005_WRN_S";
/// Agent 需要用户澄清 (AIM/WRN/OPERATION)
pub const AGENT_CLARIFICATION: &str = "ERR-AIM-AGENT-006_WRN_O";
/// Agent 动作解析失败 (AIM/ERR/OPERATION)
pub const AGENT_PARSE_ACTION: &str = "ERR-AIM-AGENT-007_ERR_O";
/// Agent 记忆系统错误 (AIM/ERR/OPERATION)
pub const AGENT_MEMORY_ERROR: &str = "ERR-AIM-AGENT-008_ERR_O";
/// Agent 无效状态转换 (AIM/ERR/OPERATION)
pub const AGENT_INVALID_TRANSITION: &str = "ERR-AIM-AGENT-009_ERR_O";
/// Agent 工作流错误 (AIM/ERR/OPERATION)
pub const AGENT_WORKFLOW_ERROR: &str = "ERR-AIM-AGENT-010_ERR_O";
/// Agent 序列化错误 (AIM/ERR/OPERATION)
pub const AGENT_SERIALIZATION_ERROR: &str = "ERR-AIM-AGENT-011_ERR_O";
/// Agent 权限被拒 (SEC/ERR/OPERATION)
pub const AGENT_PERMISSION_DENIED: &str = "ERR-SEC-AGENT-012_ERR_O";
/// Agent 安全检查失败 (SEC/CRI/OPERATION)
pub const AGENT_SAFETY_CHECK: &str = "ERR-SEC-AGENT-013_CRI_O";

// ── CQRS 聚合错误码 ─────────────────────────────────────────

/// 聚合状态无效 (INT/ERR/OPERATION)
pub const AGGREGATE_INVALID_STATE: &str = "ERR-INT-AGG-001_ERR_O";
/// 聚合业务规则违反 (INT/ERR/OPERATION)
pub const AGGREGATE_BUSINESS_RULE: &str = "ERR-INT-AGG-002_ERR_O";
/// 聚合未找到 (INT/WRN/OPERATION)
pub const AGGREGATE_NOT_FOUND: &str = "ERR-INT-AGG-003_WRN_O";
/// 聚合版本冲突 (INT/ERR/OPERATION)
pub const AGGREGATE_VERSION_CONFLICT: &str = "ERR-INT-AGG-004_ERR_O";
/// 聚合序列化失败 (INT/ERR/OPERATION)
pub const AGGREGATE_SERIALIZATION: &str = "ERR-INT-AGG-005_ERR_O";

// ── 缓存错误码 ──────────────────────────────────────────────

/// L2 缓存连接失败 (FS/ERR/SESSION)
pub const CACHE_L2_CONNECTION: &str = "ERR-FS-CACHE-001_ERR_S";
/// L2 缓存序列化失败 (INT/ERR/OPERATION)
pub const CACHE_L2_SERIALIZATION: &str = "ERR-INT-CACHE-002_ERR_O";
/// L2 缓存反序列化失败 (INT/ERR/OPERATION)
pub const CACHE_L2_DESERIALIZATION: &str = "ERR-INT-CACHE-003_ERR_O";
/// L2 缓存操作失败 (FS/ERR/OPERATION)
pub const CACHE_L2_OPERATION: &str = "ERR-FS-CACHE-004_ERR_O";
/// 缓存管理器 L2 错误 (FS/ERR/OPERATION)
pub const CACHE_MANAGER_L2: &str = "ERR-FS-CACHE-005_ERR_O";
/// 缓存管理器序列化失败 (INT/ERR/OPERATION)
pub const CACHE_MANAGER_SERIALIZATION: &str = "ERR-INT-CACHE-006_ERR_O";

// ── 模型加载器错误码 ────────────────────────────────────────

/// 不支持的模型类型 (AIM/ERR/OPERATION)
pub const MODEL_LOADER_UNSUPPORTED_MODEL: &str = "ERR-AIM-MODEL-001_ERR_O";
/// 不支持的推理后端 (AIM/ERR/OPERATION)
pub const MODEL_LOADER_UNSUPPORTED_BACKEND: &str = "ERR-AIM-MODEL-002_ERR_O";
/// 模型文件未找到 (FS/WRN/OPERATION)
pub const MODEL_LOADER_NOT_FOUND: &str = "ERR-FS-MODEL-003_WRN_O";
/// 模型加载失败 (AIM/CRI/OPERATION)
pub const MODEL_LOADER_LOAD_FAILED: &str = "ERR-AIM-MODEL-004_CRI_O";
/// 模型分词器错误 (AIM/ERR/OPERATION)
pub const MODEL_LOADER_TOKENIZER: &str = "ERR-AIM-MODEL-005_ERR_O";
/// 模型推理失败 (AIM/ERR/OPERATION)
pub const MODEL_LOADER_INFERENCE: &str = "ERR-AIM-MODEL-006_ERR_O";
/// 模型设备错误 (AIM/CRI/OPERATION)
pub const MODEL_LOADER_DEVICE: &str = "ERR-AIM-MODEL-007_CRI_O";
/// 模型配置错误 (AIM/ERR/OPERATION)
pub const MODEL_LOADER_CONFIG: &str = "ERR-AIM-MODEL-008_ERR_O";
/// 模型 I/O 错误 (FS/ERR/OPERATION)
pub const MODEL_LOADER_IO: &str = "ERR-FS-MODEL-009_ERR_O";
/// 模型序列化失败 (INT/ERR/OPERATION)
pub const MODEL_LOADER_SERIALIZATION: &str = "ERR-INT-MODEL-010_ERR_O";
/// 模型下载失败 (NET/ERR/OPERATION)
pub const MODEL_DOWNLOAD_FAILED: &str = "ERR-NET-DOWN-001_ERR_O";
/// 模型下载网络错误 (NET/ERR/SESSION)
pub const MODEL_DOWNLOAD_NETWORK: &str = "ERR-NET-DOWN-002_ERR_S";
/// 模型下载 I/O 错误 (FS/ERR/OPERATION)
pub const MODEL_DOWNLOAD_IO: &str = "ERR-FS-DOWN-003_ERR_O";
/// 模型下载 JSON 解析错误 (INT/ERR/OPERATION)
pub const MODEL_DOWNLOAD_JSON: &str = "ERR-INT-DOWN-004_ERR_O";
/// 模型下载资源未找到 (NET/WRN/OPERATION)
pub const MODEL_DOWNLOAD_NOT_FOUND: &str = "ERR-NET-DOWN-005_WRN_O";
/// 模型下载校验和不匹配 (SEC/CRI/OPERATION)
pub const MODEL_DOWNLOAD_CHECKSUM: &str = "ERR-SEC-DOWN-006_CRI_O";
/// 模型下载缓存目录错误 (FS/ERR/OPERATION)
pub const MODEL_DOWNLOAD_CACHE_DIR: &str = "ERR-FS-DOWN-007_ERR_O";
/// 模型下载已取消 (NET/WRN/OPERATION)
pub const MODEL_DOWNLOAD_CANCELLED: &str = "ERR-NET-DOWN-008_WRN_O";

// ── 事件系统错误码 ──────────────────────────────────────────

/// 事件类型不匹配 (INT/ERR/OPERATION)
pub const EVENT_TYPE_MISMATCH: &str = "ERR-INT-EVNT-001_ERR_O";
/// 事件处理失败 (INT/ERR/OPERATION)
pub const EVENT_PROCESSING_FAILED: &str = "ERR-INT-EVNT-002_ERR_O";
/// 事件无订阅者 (INT/WRN/OPERATION)
pub const EVENT_NO_SUBSCRIBERS: &str = "ERR-INT-EVNT-003_WRN_O";
/// 事件总线已关闭 (INT/WRN/SESSION)
pub const EVENT_BUS_SHUTDOWN: &str = "ERR-INT-EVNT-004_WRN_S";
/// 事件发布超时 (INT/WRN/OPERATION)
pub const EVENT_PUBLISH_TIMEOUT: &str = "ERR-INT-EVNT-005_WRN_O";

// ── 可观测性错误码 ──────────────────────────────────────────

/// 可观测性访问被拒 (SEC/ERR/OPERATION)
pub const OBSERVABILITY_FORBIDDEN: &str = "ERR-SEC-OBSV-001_ERR_O";

// ── ULLM 模块错误码 ──────────────────────────────────────────

/// LLM 网络错误 (NET/ERR/OPERATION)
pub const ULLM_NETWORK: &str = "ERR-NET-LLM-001_ERR_O";
/// LLM 超时 (NET/WRN/OPERATION)
pub const ULLM_TIMEOUT: &str = "ERR-NET-LLM-002_WRN_O";
/// LLM 速率限制 (EXT/WRN/OPERATION)
pub const ULLM_RATE_LIMIT: &str = "ERR-EXT-LLM-003_WRN_O";
/// LLM 认证失败 (SEC/CRI/SESSION)
pub const ULLM_AUTH: &str = "ERR-SEC-LLM-004_CRI_S";
/// LLM 权限错误 (SEC/CRI/SESSION)
pub const ULLM_PERMISSION: &str = "ERR-SEC-LLM-005_CRI_S";
/// LLM 上下文窗口超限 (AIM/ERR/OPERATION)
pub const ULLM_CONTEXT_WINDOW: &str = "ERR-AIM-LLM-006_ERR_O";
/// LLM Prompt 过大 (AIM/WRN/OPERATION)
pub const ULLM_PROMPT_TOO_LARGE: &str = "ERR-AIM-LLM-007_WRN_O";
/// LLM 模型不可用 (EXT/ERR/OPERATION)
pub const ULLM_MODEL_UNAVAILABLE: &str = "ERR-EXT-LLM-008_ERR_O";
/// LLM 服务端过载 (EXT/WRN/OPERATION)
pub const ULLM_SERVER_OVERLOADED: &str = "ERR-EXT-LLM-009_WRN_O";
/// LLM 无效请求 (USR/INF/OPERATION)
pub const ULLM_INVALID_REQUEST: &str = "ERR-USR-LLM-010_INF_O";
/// LLM 流错误 (NET/WRN/OPERATION)
pub const ULLM_STREAM: &str = "ERR-NET-LLM-011_WRN_O";
/// LLM 工具调用错误 (TOOL/ERR/OPERATION)
pub const ULLM_TOOL_CALL: &str = "ERR-TOOL-LLM-012_ERR_O";
/// LLM 缺少凭证 (CFG/CRI/SESSION)
pub const ULLM_MISSING_CREDENTIALS: &str = "ERR-CFG-LLM-013_CRI_S";
/// LLM Token 过期 (SEC/WRN/SESSION)
pub const ULLM_EXPIRED_TOKEN: &str = "ERR-SEC-LLM-014_WRN_S";
/// LLM JSON 解析失败 (EXT/INF/OPERATION)
pub const ULLM_JSON_PARSE: &str = "ERR-EXT-LLM-015_INF_O";
/// LLM 重试耗尽 (SYS/ERR/OPERATION)
pub const ULLM_RETRIES_EXHAUSTED: &str = "ERR-SYS-LLM-016_ERR_O";
/// LLM 配置错误 (CFG/ERR/SESSION)
pub const ULLM_CONFIG: &str = "ERR-CFG-LLM-017_ERR_S";
/// LLM 请求体过大 (AIM/WRN/OPERATION)
pub const ULLM_REQUEST_BODY_SIZE: &str = "ERR-AIM-LLM-018_WRN_O";
/// LLM 已取消 (SYS/INF/OPERATION)
pub const ULLM_CANCELLED: &str = "ERR-SYS-LLM-019_INF_O";
/// LLM 未知错误 (UNK/WRN/OPERATION)
pub const ULLM_OTHER: &str = "ERR-UNK-LLM-020_WRN_O";
/// LLM HTTP 客户端初始化失败 (CFG/ERR/SESSION)
pub const ULLM_HTTP_CLIENT_INIT: &str = "ERR-CFG-LLM-021_ERR_S";
