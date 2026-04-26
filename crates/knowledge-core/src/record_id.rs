//! 类型安全的 RecordID 包装器
//!
//! 本模块提供 `SafeRecordId` 类型，对 SurrealDB 的 `RecordId` 进行封装，
//! 添加格式验证逻辑，防止非法 ID 导致运行时错误。
//!
//! # 设计目标
//!
//! - **类型安全**：编译时确保 ID 格式合法
//! - **验证逻辑**：解析时检查 `table_name:id_value` 格式
//! - **零成本抽象**：内部直接包装 `RecordId`，无运行时开销

use serde::{Deserialize, Serialize};
use surrealdb::opt::RecordId;

use crate::Result;
use crate::error::helpers;

/// 类型安全的 RecordID 包装器
///
/// 封装 SurrealDB 的原生 `RecordId`，在构造时进行格式验证。
/// 所有公共 API 均返回 `Result`，避免使用 `unwrap()` 处理外部输入。
///
/// # ID 格式规范
///
/// 合法格式为 `table_name:id_value`：
/// - `table_name`：表名（小写字母、数字、下划线）
/// - `id_value`：ID 值（支持字符串、数字、UUID 等 SurrealDB 合法类型）
///
/// # 示例
///
/// ```rust
/// use knowledge_core::record_id::SafeRecordId;
///
/// // 解析合法 ID
/// let id = SafeRecordId::parse("document:abc123")?;
/// println!("表名: {}", id.table_name());
///
/// // 错误处理
/// let result = SafeRecordId::parse("invalid_format");
/// assert!(result.is_err());
/// # Ok::<(), error_core::ErrorObject>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SafeRecordId(RecordId);

impl SafeRecordId {
    /// 从字符串解析 RecordID（格式：`table_name:id_value`）
    ///
    /// # 参数
    ///
    /// * `input` - 符合 SurrealDB RecordID 格式的字符串
    ///
    /// # 返回值
    ///
    /// 成功时返回 `SafeRecordId`，失败时返回 `error_core::helpers::invalid_record_id`
    ///
    /// # 验证规则
    ///
    /// 1. 必须包含冒号分隔符 (`:`)
    /// 2. 表名部分不能为空
    /// 3. ID 值部分不能为空
    /// 4. 必须能被 SurrealDB 的 `RecordId::from` 正确解析
    pub fn parse(input: &str) -> Result<Self> {
        let parts: Vec<&str> = input.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(helpers::validation_error(
                &format!("ID 格式错误：缺少冒号分隔符，输入 '{input}'"),
                "parse_record_id",
            ));
        }

        let table_name = parts[0];
        let id_value = parts[1];

        if table_name.is_empty() {
            return Err(helpers::validation_error(
                "ID 格式错误：表名不能为空",
                "parse_record_id",
            ));
        }

        if id_value.is_empty() {
            return Err(helpers::validation_error(
                "ID 格式错误：ID 值不能为空",
                "parse_record_id",
            ));
        }

        let record_id = RecordId::from((table_name.to_string(), id_value.to_string()));

        Ok(Self(record_id))
    }

    /// 获取内部 RecordId 的不可变引用
    ///
    /// 当需要与 SurrealDB SDK 直接交互时使用此方法获取底层类型。
    #[inline]
    pub const fn inner(&self) -> &RecordId {
        &self.0
    }

    /// 获取内部 RecordId 的所有权
    ///
    /// 用于需要转移所有权的场景（如传递给 SurrealDB API）。
    #[inline]
    pub fn into_inner(self) -> RecordId {
        self.0
    }

    /// 获取表名部分
    ///
    /// 返回 ID 中冒号前的表名字符串引用。
    pub fn table_name(&self) -> &str {
        &self.0.tb
    }

    /// 获取 ID 值部分的字符串表示
    ///
    /// 将 SurrealDB 的 ID 值转换为可读字符串，
    /// 去除数字 ID 的 `⟨⟩` 括号。
    pub fn id_value(&self) -> String {
        let raw = self.0.id.to_string();
        raw.trim_start_matches('⟨').trim_end_matches('⟩').to_string()
    }
}

impl From<SafeRecordId> for RecordId {
    fn from(safe_id: SafeRecordId) -> Self {
        safe_id.0
    }
}

impl std::fmt::Display for SafeRecordId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_string_id() {
        let id = SafeRecordId::parse("document:abc123").expect("解析失败");
        assert_eq!(id.table_name(), "document");
        assert_eq!(id.id_value(), "abc123");
    }

    #[test]
    fn test_parse_valid_numeric_id() {
        let id = SafeRecordId::parse("token:42").expect("解析失败");
        assert_eq!(id.table_name(), "token");
        assert_eq!(id.id_value(), "42");
    }

    #[test]
    fn test_parse_valid_uuid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let id = SafeRecordId::parse(&format!("block:{uuid_str}")).expect("解析失败");
        assert_eq!(id.table_name(), "block");
        assert!(id.id_value().contains(uuid_str));
    }

    #[test]
    fn test_parse_missing_colon() {
        let result = SafeRecordId::parse("invalid_format");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("缺少冒号分隔符"));
    }

    #[test]
    fn test_parse_empty_table_name() {
        let result = SafeRecordId::parse(":abc123");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("表名不能为空"));
    }

    #[test]
    fn test_parse_empty_id_value() {
        let result = SafeRecordId::parse("document:");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("ID 值不能为空"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let original = SafeRecordId::parse("reference:link001").expect("解析失败");
        let json = serde_json::to_string(&original).expect("序列化失败");
        let deserialized: SafeRecordId =
            serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(original, deserialized);
        assert_eq!(original.table_name(), deserialized.table_name());
    }

    #[test]
    fn test_display_format() {
        let id = SafeRecordId::parse("doc:test123").expect("解析失败");
        let display_str = format!("{id}");
        assert_eq!(display_str, "doc:test123");
    }

    #[test]
    fn test_into_record_id() {
        let safe_id = SafeRecordId::parse("block:x").expect("解析失败");
        let record_id: RecordId = safe_id.into();
        assert_eq!(record_id.tb, "block");
    }

    #[test]
    fn test_inner_access() {
        let safe_id = SafeRecordId::parse("token:y").expect("解析失败");
        let inner = safe_id.inner();
        assert_eq!(inner.tb.clone(), "token");
    }

    #[test]
    fn test_hash_and_equality() {
        use std::collections::HashSet;

        let id1 = SafeRecordId::parse("doc:a").expect("解析失败");
        let id2 = SafeRecordId::parse("doc:a").expect("解析失败");
        let id3 = SafeRecordId::parse("doc:b").expect("解析失败");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);

        #[allow(clippy::mutable_key_type)]
        let mut set: HashSet<SafeRecordId> = HashSet::new();
        set.insert(id1);
        set.insert(id2);
        set.insert(id3);

        assert_eq!(set.len(), 2);
    }
}
