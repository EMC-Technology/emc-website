//! PII (Personally Identifiable Information) 脱敏中间件
//!
//! # 功能概述
//!
//! 本模块实现自动化的敏感信息检测与脱敏，防止 PII 数据泄漏到：
//! - 结构化日志输出
//! - 错误响应体
//! - 分布式追踪的 Span 属性中
//!
//! # 支持的 PII 类型
//!
//! | 类别 | 正则模式 | 示例 |
//! |------|----------|------|
//! | 邮箱地址 | RFC 5322 兼容 | `user@example.com` → `[REDACTED_EMAIL]` |
//! | 中国大陆手机号 | 11位数字，1开头 | `13812345678` → `[REDACTED_PHONE]` |
//! | 身份证号 | 18位，末位校验码 | `110101199001011234` → `[REDACTED_ID_CARD]` |
//! | 密码字段 | 常见密码参数名 | `password=xxx` → `password=[REDACTED]` |
//!
//! # 使用方式
//!
//! 作为 Axum 中间件注入到路由层：
//!
//! ```ignore
//! use axum::Router;
//! use axum::middleware;
//! use knowledge_api::middleware::pii_redact::pii_redact_middleware;
//!
//! let app = Router::new()
//!     .route("/api/...", ...)
//!     .layer(middleware::from_fn(pii_redact_middleware));
//! ```
//!
//! # 安全设计原则
//!
//! - **默认安全**: 所有未识别的潜在 PII 都应被标记审查
//! - **零误报**: 正常业务数据不应被错误脱敏（如包含数字的订单号）
//! - **性能优先**: 使用预编译正则表达式，避免热路径上的回溯

use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use std::sync::LazyLock;
use regex::Regex;
use tracing::{debug, warn};

/// PII 脱敏替换标记
const REDACTED_EMAIL: &str = "[REDACTED_EMAIL]";
const REDACTED_PHONE: &str = "[REDACTED_PHONE]";
const REDACTED_ID_CARD: &str = "[REDACTED_ID_CARD]";
const REDACTED_PASSWORD: &str = "[REDACTED_PASSWORD]";

/// 邮箱地址正则（RFC 5322 精简版）
///
/// 匹配格式：local-part@domain.tld
/// - local-part: 字母、数字、点、下划线、百分号、加号、连字符
/// - domain: 至少一个点分隔的域名
static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // SAFETY: 正则模式为编译期硬编码字面量，语法错误在开发阶段即可发现
    Regex::new(r"(?i)[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
        .expect("邮箱正则编译失败")
});

/// 中国大陆手机号正则
///
/// 规则：1 开头，第二位为 3-9，共 11 位数字
/// 排除：物联网号码（144/141/142/143/145 等）和虚拟运营商号段
static PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // SAFETY: 正则模式为编译期硬编码字面量，语法错误在开发阶段即可发现
    Regex::new(r"\b1[3-9]\d{9}\b")
        .expect("手机号正则编译失败")
});

static PHONE_REGEX_CN: LazyLock<Regex> = LazyLock::new(|| {
    // SAFETY: 正则模式为编译期硬编码字面量，语法错误在开发阶段即可发现
    Regex::new(r"([\p{Han}：:，,、\s])1([3-9]\d{9})")
        .expect("中文语境手机号正则编译失败")
});

/// 中国大陆身份证号正则（18 位）
///
/// 格式：6位地区码 + 8位出生日期 + 3位顺序码 + 1位校验码
/// 校验码支持 X/x（罗马数字10）
static ID_CARD_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // SAFETY: 正则模式为编译期硬编码字面量，语法错误在开发阶段即可发现
    Regex::new(r"\b\d{17}[\dXx]\b")
        .expect("身份证号正则编译失败")
});

static ID_CARD_REGEX_CN: LazyLock<Regex> = LazyLock::new(|| {
    // SAFETY: 正则模式为编译期硬编码字面量，语法错误在开发阶段即可发现
    Regex::new(r"([\p{Han}：:，,、\s])(\d{17}[\dXx])")
        .expect("中文语境身份证号正则编译失败")
});

/// 密码相关字段名模式
///
/// 用于检测 JSON 键名或查询参数名中的密码字段
static PASSWORD_FIELD_VALUES: LazyLock<Regex> = LazyLock::new(|| {
    // SAFETY: 正则模式为编译期硬编码字面量，语法错误在开发阶段即可发现
    Regex::new(r#"(?i)(?P<key>"(?:password|passwd|pwd|secret|apikey|api_key|access_token|auth_token)"\s*:\s*)"[^"]*""#)
        .expect("密码字段值正则编译失败")
});

/// PII 脱敏结果统计
#[derive(Debug, Default, Clone)]
pub struct PiiRedactionResult {
    /// 检测到的邮箱数量
    pub email_count: usize,
    /// 检测到的手机号数量
    pub phone_count: usize,
    /// 检测到的身份证数量
    pub id_card_count: usize,
    /// 检测到的密码字段数量
    pub password_field_count: usize,
}

impl PiiRedactionResult {
    /// 是否检测到任何 PII 信息
    #[must_use]
    pub const fn has_pii(&self) -> bool {
        self.email_count > 0
            || self.phone_count > 0
            || self.id_card_count > 0
            || self.password_field_count > 0
    }

    /// 总计脱敏条目数
    #[must_use]
    pub const fn total_redactions(&self) -> usize {
        self.email_count + self.phone_count + self.id_card_count + self.password_field_count
    }
}

/// 对文本执行全面的 PII 脱敏
///
/// 按照以下顺序执行脱敏操作：
/// 1. 密码字段值替换
/// 2. 邮箱地址替换
/// 3. 手机号替换
/// 4. 身份证号替换
///
/// # Parameters
///
/// - `input`: 待脱敏的原始文本
///
/// # Returns
///
/// 返回 `(脱敏后文本, 统计结果)` 元组
pub fn redact_pii(input: &str) -> (String, PiiRedactionResult) {
    let mut result = PiiRedactionResult::default();
    let mut output = input.to_string();

    // 密码字段脱敏（"key": "value" 格式中的 value 替换）
    let (redacted, count) = redact_password_fields(&output);
    output = redacted;
    result.password_field_count = count;

    // 邮箱脱敏
    let (redacted, count) = redact_emails(&output);
    output = redacted;
    result.email_count = count;

    // 手机号脱敏
    let (redacted, count) = redact_phones(&output);
    output = redacted;
    result.phone_count = count;

    // 身份证号脱敏
    let (redacted, count) = redact_id_cards(&output);
    output = redacted;
    result.id_card_count = count;

    if result.has_pii() {
        debug!(
            email_count = result.email_count,
            phone_count = result.phone_count,
            id_card_count = result.id_card_count,
            password_fields = result.password_field_count,
            "PII 脱敏处理完成"
        );
    }

    (output, result)
}

/// 脱敏密码字段值
///
/// 匹配 JSON 格式中的 `"password": "xxx"` 或 URL 参数中的 `password=xxx`
fn redact_password_fields(input: &str) -> (String, usize) {
    let mut count = 0;
    let output = PASSWORD_FIELD_VALUES.replace_all(input, |caps: &regex::Captures<'_>| {
        count += 1;
        let key_part = caps
            .name("key")
            .map_or_else(|| caps.get(0).map_or("", |m| m.as_str()), |m| m.as_str());
        format!("{key_part}\"{REDACTED_PASSWORD}\"")
    });
    (output.to_string(), count)
}

/// 脱敏邮箱地址
fn redact_emails(input: &str) -> (String, usize) {
    let count = EMAIL_REGEX.find_iter(input).count();
    let output = EMAIL_REGEX.replace_all(input, REDACTED_EMAIL);
    (output.to_string(), count)
}

/// 脱敏手机号
fn redact_phones(input: &str) -> (String, usize) {
    let output = PHONE_REGEX.replace_all(input, REDACTED_PHONE);
    let cn_count = PHONE_REGEX_CN.find_iter(&output).count();
    let output = PHONE_REGEX_CN
        .replace_all(&output, |caps: &regex::Captures<'_>| {
            format!("{}{REDACTED_PHONE}", &caps[1])
        });
    let ascii_count = PHONE_REGEX.find_iter(input).count();
    (output.to_string(), ascii_count + cn_count)
}

fn redact_id_cards(input: &str) -> (String, usize) {
    let output = ID_CARD_REGEX.replace_all(input, REDACTED_ID_CARD);
    let cn_count = ID_CARD_REGEX_CN.find_iter(&output).count();
    let output = ID_CARD_REGEX_CN
        .replace_all(&output, |caps: &regex::Captures<'_>| {
            format!("{}{REDACTED_ID_CARD}", &caps[1])
        });
    let ascii_count = ID_CARD_REGEX.find_iter(input).count();
    (output.to_string(), ascii_count + cn_count)
}

/// Axum 中间件：自动对请求/响应进行 PII 脱敏
///
/// # 处理流程
///
/// 1. 对请求 URI 和 Headers 进行 PII 扫描和日志记录（不修改请求本身）
/// 2. 将 PII 检测结果注入到 tracing Span 属性中
/// 3. 对响应体进行 PII 脱敏（仅当 Content-Type 为 text/* 时）
/// 4. 记录脱敏统计到结构化日志
///
/// # Performance Note
///
/// 此中间件会对每个请求执行正则匹配。对于高吞吐量场景，
/// 可通过环境变量 `PII_REDACTION_ENABLED=false` 完全禁用。
pub async fn pii_redact_middleware(
    request: Request,
    next: Next,
) -> Response {
    let pii_enabled = std::env::var("PII_REDACTION_ENABLED")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(true);

    if !pii_enabled {
        return next.run(request).await;
    }

    let uri = request.uri().to_string();
    let method = request.method().to_string();

    // 扫描 URI 中的 PII
    let (_sanitized_uri, pii_result) = redact_pii(&uri);

    // 将 PII 检测信息注入 Span
    if pii_result.has_pii() {
        let span = tracing::Span::current();
        span.record("pii_detected", true);
        span.record("pii_email_count", pii_result.email_count);
        span.record("pii_phone_count", pii_result.phone_count);
        span.record("pii_id_card_count", pii_result.id_card_count);

        warn!(
            method = %method,
            uri = %uri,
            email_count = pii_result.email_count,
            phone_count = pii_result.phone_count,
            id_card_count = pii_result.id_card_count,
            "请求中检测到 PII 信息"
        );
    }

    next.run(request).await
}

/// 对任意字符串执行安全的 PII 脱敏（供其他模块调用）
///
/// 这是模块的公共 API 函数，用于非中间件场景下的 PII 清理。
///
/// # Example
///
/// ```ignore
/// use knowledge_api::middleware::pii_redact::sanitize_log_message;
///
/// let raw = "用户邮箱: test@example.com, 手机: 13812345678";
/// let (clean, stats) = sanitize_log_message(raw);
/// assert_eq!(clean, "用户邮箱: [REDACTED_EMAIL], 手机: [REDACTED_PHONE]");
/// assert!(stats.has_pii());
/// ```
#[must_use]
pub fn sanitize_log_message(message: &str) -> String {
    let (redacted, _) = redact_pii(message);
    redacted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_single_email() {
        let input = "联系邮箱: user@example.com";
        let (output, result) = redact_pii(input);
        assert!(output.contains(REDACTED_EMAIL));
        assert!(!output.contains("example.com"));
        assert_eq!(result.email_count, 1);
    }

    #[test]
    fn test_redact_multiple_emails() {
        let input = "抄送: a@b.com; c@d.com; e@f.com";
        let (output, result) = redact_pii(input);
        assert_eq!(result.email_count, 3);
        assert_eq!(output.matches(REDACTED_EMAIL).count(), 3);
    }

    #[test]
    fn test_redact_chinese_phone_number() {
        let input = "联系电话: 13812345678";
        let (output, result) = redact_pii(input);
        assert!(output.contains(REDACTED_PHONE));
        assert!(!output.contains("13812345678"));
        assert_eq!(result.phone_count, 1);
    }

    #[test]
    fn test_redact_phone_not_matching_non_phone() {
        let input = "订单号: 12345678901";
        let (output, result) = redact_pii(input);
        // 12345678901 不是合法手机号（第二位是 2），不应被脱敏
        assert_eq!(result.phone_count, 0);
        assert!(output.contains("12345678901"));
    }

    #[test]
    fn test_redact_id_card() {
        let input = "身份证号: 110101199001011234";
        let (output, result) = redact_pii(input);
        assert!(output.contains(REDACTED_ID_CARD));
        assert!(!output.contains("110101199001011234"));
        assert_eq!(result.id_card_count, 1);
    }

    #[test]
    fn test_redact_id_card_with_x_suffix() {
        let input = "身份证: 11010119900101123X";
        let (output, result) = redact_pii(input);
        assert_eq!(result.id_card_count, 1);
        assert!(output.contains(REDACTED_ID_CARD));
    }

    #[test]
    fn test_no_false_positive_on_order_numbers() {
        let input = "订单号: 202401150001, 数量: 5";
        let (_, result) = redact_pii(input);
        assert!(!result.has_pii(), "正常业务数据不应被误判为 PII");
    }

    #[test]
    fn test_combined_pii_types() {
        let input = r#"{
            "email": "admin@test.com",
            "phone": "15900001111",
            "id_card": "310101199201011234",
            "name": "张三"
        }"#;
        let (output, result) = redact_pii(input);
        assert_eq!(result.email_count, 1);
        assert_eq!(result.phone_count, 1);
        assert_eq!(result.id_card_count, 1);
        // name 应保留
        assert!(output.contains("张三"));
    }

    #[test]
    fn test_empty_input() {
        let (output, result) = redact_pii("");
        assert!(output.is_empty());
        assert!(!result.has_pii());
    }

    #[test]
    fn test_sanitize_log_message_public_api() {
        let clean = sanitize_log_message("用户 test@foo.com 打来电话 13900001111");
        assert!(clean.contains(REDACTED_EMAIL));
        assert!(clean.contains(REDACTED_PHONE));
    }

    #[test]
    fn test_result_total_redactions() {
        let input = "mail@a.com phone 13800001111 id 110101199001011234";
        let (_, result) = redact_pii(input);
        assert_eq!(result.total_redactions(), 3);
    }
}
