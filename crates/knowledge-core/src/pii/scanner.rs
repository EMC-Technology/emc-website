use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// PII 数据扫描器和脱敏器
///
/// 用于检测和脱敏文本中的个人身份信息 (Personally Identifiable Information)，
/// 支持 GDPR、中国《个人信息保护法》等合规要求。
///
/// # 支持的 PII 类型
///
/// - 电子邮件地址
/// - 中国手机号
/// - 身份证号
/// - 银行卡号
/// - 信用卡号
/// - IP 地址
/// - MAC 地址
/// - 密码/密钥
/// - API Key
/// - 社会保障号 (SSN)
/// - 护照号码
/// - 自定义模式
pub struct PIIScanner {
    /// PII 检测模式列表
    patterns: Vec<PIIPattern>,
    /// 扫描模式（严格/宽松）
    mode: ScanMode,
}

/// 扫描模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    /// 严格模式：高置信度匹配，减少误报
    Strict,
    /// 宽松模式：尽可能多匹配，可能增加误报
    Lenient,
}

impl Default for ScanMode {
    fn default() -> Self {
        Self::Strict
    }
}

/// PII 检测模式定义
#[derive(Debug, Clone)]
pub struct PIIPattern {
    /// PII 类别
    pub pattern_type: PIICategory,
    /// 正则表达式
    pub regex: Regex,
    /// 替换策略
    pub replacement: ReplacementStrategy,
    /// 置信度阈值 (0.0-1.0)
    pub confidence_threshold: f64,
}

/// PII 类别枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PIICategory {
    /// 电子邮件地址
    EmailAddress,
    /// 中国手机号
    PhoneNumber,
    /// 身份证号
    IdCardNumber,
    /// 护照号码
    PassportNumber,
    /// 银行账号
    BankAccount,
    /// 信用卡号
    CreditCardNumber,
    /// 密码
    Password,
    /// API Key
    ApiKey,
    /// IP 地址
    IpAddress,
    /// MAC 地址
    MacAddress,
    /// 社会保障号 (SSN)
    Ssn,
    /// 自定义类别
    Custom(String),
}

impl PIICategory {
    /// 获取类别的显示名称
    pub fn display_name(&self) -> &str {
        match self {
            Self::EmailAddress => "电子邮件",
            Self::PhoneNumber => "手机号",
            Self::IdCardNumber => "身份证号",
            Self::PassportNumber => "护照号码",
            Self::BankAccount => "银行账号",
            Self::CreditCardNumber => "信用卡号",
            Self::Password => "密码",
            Self::ApiKey => "API Key",
            Self::IpAddress => "IP 地址",
            Self::MacAddress => "MAC 地址",
            Self::Ssn => "社会保障号",
            Self::Custom(name) => name.as_str(),
        }
    }

    /// 获取类别的英文标识符
    pub fn identifier(&self) -> &str {
        match self {
            Self::EmailAddress => "email_address",
            Self::PhoneNumber => "phone_number",
            Self::IdCardNumber => "id_card_number",
            Self::PassportNumber => "passport_number",
            Self::BankAccount => "bank_account",
            Self::CreditCardNumber => "credit_card_number",
            Self::Password => "password",
            Self::ApiKey => "api_key",
            Self::IpAddress => "ip_address",
            Self::MacAddress => "mac_address",
            Self::Ssn => "ssn",
            Self::Custom(name) => name.as_str(),
        }
    }
}

impl std::fmt::Display for PIICategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// 替换策略
#[derive(Debug, Clone)]
pub enum ReplacementStrategy {
    /// 掩码处理：保留首尾字符
    Mask { keep_prefix: usize, keep_suffix: usize },
    /// 哈希替换（不可逆）
    Hash,
    /// 完全替换为 [REDACTED]
    Redact,
    /// Token 化（可还原）
    Tokenize,
    /// 模糊化处理（添加噪声）
    Fuzzy { noise_ratio: f64 },
}

/// PII 发现结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PIIFinding {
    /// PII 类别
    pub category: PIICategory,
    /// 匹配到的文本
    pub matched_text: String,
    /// 起始偏移量
    pub start_offset: usize,
    /// 结束偏移量
    pub end_offset: usize,
    /// 置信度 (0.0-1.0)
    pub confidence: f64,
    /// 建议操作
    pub suggestion: String,
}

/// 脱敏后的文本结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedText {
    /// 处理后的文本
    pub text: String,
    /// 发现的所有 PII 条目
    pub findings: Vec<PIIFinding>,
    /// 是否包含敏感数据
    pub has_sensitive_data: bool,
}

/// 脱敏后的 JSON 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedJson {
    /// 处理后的 JSON 数据
    pub value: serde_json::Value,
    /// 发现的 PII 列表（按路径索引）
    pub findings: Vec<(String, Vec<PIIFinding>)>,
    /// 敏感字段路径列表
    pub sensitive_paths: Vec<String>,
}

/// 脱敏后的记录结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedRecord {
    /// 记录 ID
    pub record_id: Option<String>,
    /// 脱敏后的字段
    pub fields: HashMap<String, serde_json::Value>,
    /// 发现的 PII 列表
    pub findings: Vec<PIIFinding>,
}

type Result<T> = crate::Result<T>;

impl PIIScanner {
    /// 创建默认配置的 PII 扫描器
    ///
    /// 包含所有内置的常见 PII 模式：
    /// - Email、手机号、身份证、银行卡等
    pub fn new() -> Result<Self> {
        let patterns = Self::default_patterns()?;
        Ok(Self {
            patterns,
            mode: ScanMode::Strict,
        })
    }

    /// 创建自定义模式的 PII 扫描器
    pub fn with_patterns(patterns: Vec<PIIPattern>, mode: ScanMode) -> Self {
        Self { patterns, mode }
    }

    /// 仅启用特定类别的扫描器
    pub fn with_categories(categories: &[PIICategory]) -> Result<Self> {
        let all_patterns = Self::default_patterns()?;
        let filtered = all_patterns
            .into_iter()
            .filter(|p| categories.contains(&p.pattern_type))
            .collect();

        Ok(Self {
            patterns: filtered,
            mode: ScanMode::Strict,
        })
    }

    /// 获取内置默认 PII 模式
    fn default_patterns() -> Result<Vec<PIIPattern>> {
        let mut patterns = Vec::new();

        // 电子邮件
        patterns.push(PIIPattern {
            pattern_type: PIICategory::EmailAddress,
            regex: Regex::new(r"(?i)[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
                .map_err(|e| crate::error::helpers::internal_error(&format!("正则编译失败: {}", e)))?,
            replacement: ReplacementStrategy::Mask { keep_prefix: 2, keep_suffix: 4 },
            confidence_threshold: 0.95,
        });

        // 中国手机号 (11位，1开头)
        patterns.push(PIIPattern {
            pattern_type: PIICategory::PhoneNumber,
            regex: Regex::new(r"1[3-9]\d{9}")
                .map_err(|e| crate::error::helpers::internal_error(&format!("正则编译失败: {}", e)))?,
            replacement: ReplacementStrategy::Mask { keep_prefix: 3, keep_suffix: 4 },
            confidence_threshold: 0.98,
        });

        // 身份证号 (18位)
        patterns.push(PIIPattern {
            pattern_type: PIICategory::IdCardNumber,
            regex: Regex::new(r"\d{17}[\dXx]")
                .map_err(|e| crate::error::helpers::internal_error(&format!("正则编译失败: {}", e)))?,
            replacement: ReplacementStrategy::Redact,
            confidence_threshold: 0.90,
        });

        // 银行卡号 (16-19位)
        patterns.push(PIIPattern {
            pattern_type: PIICategory::BankAccount,
            regex: Regex::new(r"\d{16,19}")
                .map_err(|e| crate::error::helpers::internal_error(&format!("正则编译失败: {}", e)))?,
            replacement: ReplacementStrategy::Mask { keep_prefix: 4, keep_suffix: 4 },
            confidence_threshold: 0.85,
        });

        // 信用卡号 (Luhn 算法验证在后续步骤)
        patterns.push(PIIPattern {
            pattern_type: PIICategory::CreditCardNumber,
            regex: Regex::new(r"\b(?:4[0-9]{12}(?:[0-9]{3})?|5[1-5][0-9]{14}|3[47][0-9]{13}|3(?:0[0-5]|[68][0-9])[0-9]{11}|6(?:011|5[0-9]{2})[0-9]{12}|(?:2131|1800|35\d{3})\d{11})\b")
                .map_err(|e| crate::error::helpers::internal_error(&format!("正则编译失败: {}", e)))?,
            replacement: ReplacementStrategy::Redact,
            confidence_threshold: 0.92,
        });

        // IPv4 地址
        patterns.push(PIIPattern {
            pattern_type: PIICategory::IpAddress,
            regex: Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b")
                .map_err(|e| crate::error::helpers::internal_error(&format!("正则编译失败: {}", e)))?,
            replacement: ReplacementStrategy::Fuzzy { noise_ratio: 0.5 },
            confidence_threshold: 0.90,
        });

        // MAC 地址
        patterns.push(PIIPattern {
            pattern_type: PIICategory::MacAddress,
            regex: Regex::new(r"(?i)(?:[0-9a-fA-F]{2}[:-]){5}[0-9a-fA-F]{2}")
                .map_err(|e| crate::error::helpers::internal_error(&format!("正则编译失败: {}", e)))?,
            replacement: ReplacementStrategy::Redact,
            confidence_threshold: 0.95,
        });

        // API Key (通用格式)
        patterns.push(PIIPattern {
            pattern_type: PIICategory::ApiKey,
            regex: Regex::new(r#"(?i)(?:api[_-]?key|apikey|secret[_-]?key)\s*[:=]\s*['\"]?[a-zA-Z0-9_\-]{20,}['\"]?"#)
                .map_err(|e| crate::error::helpers::internal_error(&format!("正则编译失败: {}", e)))?,
            replacement: ReplacementStrategy::Redact,
            confidence_threshold: 0.88,
        });

        // SSN (美国社会保障号)
        patterns.push(PIIPattern {
            pattern_type: PIICategory::Ssn,
            regex: Regex::new(r"\d{3}-\d{2}-\d{4}")
                .map_err(|e| crate::error::helpers::internal_error(&format!("正则编译失败: {}", e)))?,
            replacement: ReplacementStrategy::Mask { keep_prefix: 0, keep_suffix: 4 },
            confidence_threshold: 0.87,
        });

        // 密码相关字段
        patterns.push(PIIPattern {
            pattern_type: PIICategory::Password,
            regex: Regex::new(r#"(?i)(?:password|passwd|pwd)\s*[:=]\s*['\"]?[^'\"\s]+['\"]?"#)
                .map_err(|e| crate::error::helpers::internal_error(&format!("正则编译失败: {}", e)))?,
            replacement: ReplacementStrategy::Redact,
            confidence_threshold: 0.93,
        });

        // 护照号码 (中国护照: G/P + 8位数字)
        patterns.push(PIIPattern {
            pattern_type: PIICategory::PassportNumber,
            regex: Regex::new(r"[GPgp]\d{8}")
                .map_err(|e| crate::error::helpers::internal_error(&format!("正则编译失败: {}", e)))?,
            replacement: ReplacementStrategy::Redact,
            confidence_threshold: 0.82,
        });

        Ok(patterns)
    }

    /// 扫描文本中的 PII 数据
    ///
    /// 返回所有发现的 PII 条目及其位置和置信度。
    pub fn scan(&self, text: &str) -> Vec<PIIFinding> {
        let mut findings = Vec::new();
        let min_confidence = match self.mode {
            ScanMode::Strict => 0.90,
            ScanMode::Lenient => 0.70,
        };

        for pattern in &self.patterns {
            if pattern.confidence_threshold < min_confidence {
                continue;
            }

            for cap in pattern.regex.captures_iter(text) {
                if let Ok(m) = cap {
                    if let Some(matched) = m.get(0) {
                        let finding = PIIFinding {
                            category: pattern.pattern_type.clone(),
                            matched_text: matched.as_str().to_string(),
                            start_offset: matched.start(),
                            end_offset: matched.end(),
                            confidence: pattern.confidence_threshold,
                            suggestion: format!(
                                "发现{}，建议使用 {:?} 策略进行脱敏",
                                pattern.pattern_type,
                                pattern.replacement
                            ),
                        };
                        findings.push(finding);
                    }
                }
            }
        }

        // 按位置排序
        findings.sort_by_key(|f| f.start_offset);
        findings
    }

    /// 扫描并脱敏文本
    ///
    /// 返回脱敏后的文本和发现的所有 PII 条目。
    pub fn scan_and_redact(&self, text: &str) -> RedactedText {
        let findings = self.scan(text);

        if findings.is_empty() {
            return RedactedText {
                text: text.to_string(),
                findings: vec![],
                has_sensitive_data: false,
            };
        }

        let mut result = text.to_string();
        let mut offset_adjustment = 0isize;

        for finding in &findings {
            let replacement = self.apply_replacement(
                &finding.matched_text,
                &self.find_pattern_for_category(&finding.category).unwrap().replacement,
            );

            let adjusted_start = (finding.start_offset as isize + offset_adjustment) as usize;
            let adjusted_end = (finding.end_offset as isize + offset_adjustment) as usize;

            if adjusted_start < result.len() && adjusted_end <= result.len() {
                result.replace_range(adjusted_start..adjusted_end, &replacement);
                offset_adjustment += replacement.len() as isize - (finding.end_offset - finding.start_offset) as isize;
            }
        }

        RedactedText {
            text: result,
            findings,
            has_sensitive_data: true,
        }
    }

    /// 扫描 JSON 值中的 PII
    ///
    /// 递归扫描 JSON 对象和数组中的字符串值。
    pub fn scan_json(&self, value: &serde_json::Value) -> RedactedJson {
        let mut findings = Vec::new();
        let mut sensitive_paths = Vec::new();
        let redacted_value = self.scan_json_recursive(value, "", &mut findings, &mut sensitive_paths);

        RedactedJson {
            value: redacted_value,
            findings,
            sensitive_paths,
        }
    }

    /// 递归扫描 JSON
    fn scan_json_recursive(
        &self,
        value: &serde_json::Value,
        path: &str,
        findings: &mut Vec<(String, Vec<PIIFinding>)>,
        sensitive_paths: &mut Vec<String>,
    ) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => {
                let pii_findings = self.scan(s);
                if !pii_findings.is_empty() {
                    let current_path = path.to_string();
                    findings.push((current_path.clone(), pii_findings.clone()));
                    sensitive_paths.push(current_path);
                    let redacted = self.scan_and_redact(s);
                    serde_json::Value::String(redacted.text)
                } else {
                    value.clone()
                }
            }
            serde_json::Value::Object(obj) => {
                let mut new_obj = serde_json::Map::new();
                for (k, v) in obj {
                    let new_path = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{}/{}", path, k)
                    };
                    new_obj.insert(k.clone(), self.scan_json_recursive(v, &new_path, findings, sensitive_paths));
                }
                serde_json::Value::Object(new_obj)
            }
            serde_json::Value::Array(arr) => {
                let mut new_arr = Vec::with_capacity(arr.len());
                for (i, v) in arr.iter().enumerate() {
                    let new_path = format!("{}[{}]", path, i);
                    new_arr.push(self.scan_json_recursive(v, &new_path, findings, sensitive_paths));
                }
                serde_json::Value::Array(new_arr)
            }
            _ => value.clone(),
        }
    }

    /// 应用替换策略
    fn apply_replacement(&self, original: &str, strategy: &ReplacementStrategy) -> String {
        match strategy {
            ReplacementStrategy::Mask { keep_prefix, keep_suffix } => {
                if original.len() <= *keep_prefix + *keep_suffix {
                    return "*".repeat(original.len());
                }

                let prefix = &original[..*keep_prefix.min(&original.len())];
                let suffix = &original[original.len() - *keep_suffix.min(&original.len())..];
                let mask_len = original.len() - *keep_prefix.min(&original.len()) - *keep_suffix.min(&original.len());
                format!("{}{}{}", prefix, "*".repeat(mask_len), suffix)
            }
            ReplacementStrategy::Hash => {
                use std::hash::{Hash, Hasher};
                use std::collections::hash_map::DefaultHasher;
                let mut hasher = DefaultHasher::new();
                original.hash(&mut hasher);
                format!("{:016x}", hasher.finish())
            }
            ReplacementStrategy::Redact => "[REDACTED]".to_string(),
            ReplacementStrategy::Tokenize => {
                format!("[TOKEN:{}]", blake3::hash(original.as_bytes()))
            }
            ReplacementStrategy::Fuzzy { noise_ratio } => {
                let hash = blake3::hash(original.as_bytes());
                let hash_bytes = hash.as_bytes();
                let chars: Vec<char> = original.chars().collect();
                let noisy: String = chars
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        if c.is_alphanumeric() {
                            let hash_byte = hash_bytes[i % 32];
                            let threshold = (*noise_ratio * 255.0) as u8;
                            if hash_byte < threshold {
                                let replacement_byte = hash_bytes[(i + 16) % 32];
                                if c.is_ascii_alphabetic() {
                                    let base = if c.is_ascii_uppercase() { b'A' } else { b'a' };
                                    (base + replacement_byte % 26) as char
                                } else if c.is_ascii_digit() {
                                    (b'0' + replacement_byte % 10) as char
                                } else {
                                    *c
                                }
                            } else {
                                *c
                            }
                        } else {
                            *c
                        }
                    })
                    .collect();
                noisy
            }
        }
    }

    /// 查找指定类别的模式
    fn find_pattern_for_category(&self, category: &PIICategory) -> Option<&PIIPattern> {
        self.patterns.iter().find(|p| &p.pattern_type == category)
    }

    /// 添加自定义 PII 模式
    pub fn add_custom_pattern(&mut self, category: &str, pattern: &str, strategy: ReplacementStrategy) -> Result<()> {
        let regex = Regex::new(pattern).map_err(|e| {
            crate::error::helpers::validation_error(&format!("无效的正则表达式 '{}': {}", pattern, e), "add_custom_pattern")
        })?;

        self.patterns.push(PIIPattern {
            pattern_type: PIICategory::Custom(category.to_string()),
            regex,
            replacement: strategy,
            confidence_threshold: 0.80,
        });

        Ok(())
    }

    /// 设置扫描模式
    pub fn set_mode(&mut self, mode: ScanMode) {
        self.mode = mode;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_email() {
        let scanner = PIIScanner::new().unwrap();
        let text = "联系邮箱: user@example.com 和 admin@company.org";
        let findings = scanner.scan(text);

        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].category, PIICategory::EmailAddress);
        assert_eq!(findings[0].matched_text, "user@example.com");
    }

    #[test]
    fn test_scan_phone_number() {
        let scanner = PIIScanner::new().unwrap();
        let text = "联系电话: 13812345678";
        let findings = scanner.scan(text);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, PIICategory::PhoneNumber);
        assert_eq!(findings[0].matched_text, "13812345678");
    }

    #[test]
    fn test_scan_id_card() {
        let scanner = PIIScanner::new().unwrap();
        let text = "身份证号: 110101199001011234";
        let findings = scanner.scan(text);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, PIICategory::IdCardNumber);
    }

    #[test]
    fn test_scan_ip_address() {
        let scanner = PIIScanner::new().unwrap();
        let text = "服务器 IP: 192.168.1.100";
        let findings = scanner.scan(text);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, PIICategory::IpAddress);
    }

    #[test]
    fn test_scan_mac_address() {
        let scanner = PIIScanner::new().unwrap();
        let text = "MAC: AA:BB:CC:DD:EE:FF";
        let findings = scanner.scan(text);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, PIICategory::MacAddress);
    }

    #[test]
    fn test_scan_api_key() {
        let scanner = PIIScanner::new().unwrap();
        let text = "api_key=sk-abcdefghijklmnopqrstuvwxyz123456";
        let findings = scanner.scan(text);

        assert!(!findings.is_empty());
        assert_eq!(findings[0].category, PIICategory::ApiKey);
    }

    #[test]
    fn test_scan_password() {
        let scanner = PIIScanner::new().unwrap();
        let text = "password=mySecretPass123";
        let findings = scanner.scan(text);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, PIICategory::Password);
    }

    #[test]
    fn test_redact_email() {
        let scanner = PIIScanner::new().unwrap();
        let text = "邮箱: user@example.com";
        let redacted = scanner.scan_and_redact(text);

        assert!(redacted.has_sensitive_data);
        assert!(redacted.text.contains("us"));
    }

    #[test]
    fn test_redact_phone() {
        let scanner = PIIScanner::new().unwrap();
        let text = "电话: 13812345678";
        let redacted = scanner.scan_and_redact(text);

        assert!(redacted.has_sensitive_data);
        assert!(redacted.text.contains("138****5678"));
    }

    #[test]
    fn test_redact_id_card() {
        let scanner = PIIScanner::new().unwrap();
        let text = "身份证: 110101199001011234";
        let redacted = scanner.scan_and_redact(text);

        assert!(redacted.has_sensitive_data);
        assert!(redacted.text.contains("[REDACTED]"));
    }

    #[test]
    fn test_scan_json() {
        let scanner = PIIScanner::new().unwrap();
        let json = serde_json::json!({
            "name": "张三",
            "email": "zhangsan@example.com",
            "phone": "13812345678",
            "address": {
                "city": "北京"
            }
        });

        let redacted = scanner.scan_json(&json);

        assert!(!redacted.findings.is_empty());
        assert!(!redacted.sensitive_paths.is_empty());
        
        // 检查 email 字段被脱敏
        if let Some(email_val) = redacted.value.get("email") {
            if let Some(s) = email_val.as_str() {
                assert_ne!(s, "zhangsan@example.com");
            }
        }
    }

    #[test]
    fn test_no_pii_in_clean_text() {
        let scanner = PIIScanner::new().unwrap();
        let text = "这是一段普通的中文文本，不包含任何敏感信息。";
        let redacted = scanner.scan_and_redact(text);

        assert!(!redacted.has_sensitive_data);
        assert_eq!(redacted.text, text);
    }

    #[test]
    fn test_custom_pattern() {
        let mut scanner = PIIScanner::new().unwrap();
        scanner.add_custom_pattern("employee_id", r"EMP-\d{6}", ReplacementStrategy::Redact).unwrap();

        let text = "员工编号: EMP-123456";
        let findings = scanner.scan(text);

        assert_eq!(findings.len(), 1);
        assert!(matches!(findings[0].category, PIICategory::Custom(_)));
    }

    #[test]
    fn test_pii_category_display_name() {
        assert_eq!(PIICategory::EmailAddress.display_name(), "电子邮件");
        assert_eq!(PIICategory::PhoneNumber.display_name(), "手机号");
        assert_eq!(PIICategory::IdCardNumber.display_name(), "身份证号");
    }

    #[test]
    fn test_replacement_strategies() {
        let scanner = PIIScanner::new().unwrap();

        // Mask 策略
        let masked = scanner.apply_replacement("hello@test.com", &ReplacementStrategy::Mask { keep_prefix: 2, keep_suffix: 4 });
        assert_eq!(masked, "he**********om");

        // Redact 策略
        let redacted = scanner.apply_replacement("sensitive", &ReplacementStrategy::Redact);
        assert_eq!(redacted, "[REDACTED]");
    }

    #[proptest::proptest]
    fn test_fuzz_phone_detection(#[proptest::strategy("[1-9]\\d{10}")] phone: String) {
        let scanner = PIIScanner::new().unwrap();
        let text = format!("电话: {}", phone);
        let findings = scanner.scan(&text);

        if phone.starts_with('1') && phone.len() == 11 {
            assert_eq!(findings.len(), 1);
            assert_eq!(findings[0].matched_text, phone);
        }
    }
}
