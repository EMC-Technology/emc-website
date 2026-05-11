//! 源类型检测器（扩展名 → SourceType 枚举映射）
//!
//! # MECE-01 格式矩阵
//!
//! 本模块实现文件扩展名到 [`SourceType`] 的**互斥且完全穷举（MECE）**映射：
//!
//! | 分类 | 扩展名 | SourceType |
//! |------|--------|------------|
//! | Markdown 族 | .md, .mdx, .markdown | `SourceType::Markdown` |
//! | 代码族 | .rs, .py, .js, .ts, .go, .c, .cpp, .java, .rb, .kt, .swift, .zig, .toml, .yaml, .yml, .json | `SourceType::Code` |
//! | 纯文本 | .txt, .log, .csv | `SourceType::Plain` |
//! | 不支持 | 其他所有扩展名 | 返回 `UnsupportedFormat` (E3002) |

use crate::Result;
use error_core::helpers;
use knowledge_core::model::SourceType;
use std::path::Path;

/// 源类型检测器（扩展名 → `SourceType` 枚举映射）
///
/// 无状态工具结构体，所有方法均为关联函数，无需实例化。
pub struct SourceTypeDetector;

impl SourceTypeDetector {
    /// 根据文件扩展名检测源类型
    ///
    /// 匹配逻辑：
    /// 1. 提取路径的扩展名（小写化）
    /// 2. 按 MECE 矩阵匹配到对应的 `SourceType`
    /// 3. 无匹配时返回 `UnsupportedFormat` 错误
    ///
    /// # 参数
    ///
    /// * `path` - 文件路径
    ///
    /// # Returns
    ///
    /// 成功返回 `SourceType` 枚举，失败返回 E3002 错误
    ///
    /// # Example
    ///
    /// ```
    /// use knowledge_parser::SourceTypeDetector;
    /// use knowledge_core::model::SourceType;
    /// use std::path::Path;
    ///
    /// let st = SourceTypeDetector::detect(Path::new("main.rs"))?;
    /// assert_eq!(st, SourceType::Code);
    /// # Ok::<(), error_core::ErrorObject>(())
    /// ```
    ///
    /// # Errors
    ///
    /// 文件扩展名不在支持列表中或无扩展名时返回 `UnsupportedFormat` 错误
    pub fn detect(path: &Path) -> Result<SourceType> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| helpers::unsupported_format("无扩展名"))?
            .to_lowercase();

        match ext.as_str() {
            // Markdown 族
            "md" | "mdx" | "markdown" => Ok(SourceType::Markdown),

            // 代码族（按语言分类，与 CodePipeline EXTENSION_LANG_MAP 保持一致）
            "rs" | "py" | "pyw" | "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx" | "go" | "c"
            | "h" | "cpp" | "hpp" | "hxx" | "cc" | "cxx" | "java" | "rb" | "rbw" | "kt" | "kts"
            | "swift" | "zig" | "toml" | "yaml" | "yml" | "json" => Ok(SourceType::Code),

            // 纯文本
            "txt" | "log" | "csv" => Ok(SourceType::Plain),

            // 不支持的格式
            _ => Err(helpers::unsupported_format(&format!(
                "不支持的文件格式: .{ext} (支持: md/rs/py/txt等)"
            ))),
        }
    }

    /// 判断是否为支持的格式（不抛异常版本）
    ///
    /// 当需要静默跳过不支持格式的文件时使用此方法，
    /// 避免在循环中频繁创建错误对象。
    ///
    /// # 参数
    ///
    /// * `path` - 文件路径
    ///
    /// # Returns
    ///
    /// * `true` - 扩展名属于支持的格式列表
    /// * `false` - 扩展名为空或不支持
    #[must_use]
    pub fn is_supported(path: &Path) -> bool {
        Self::detect(path).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use error_core::prelude::ErrorSource;
    use std::path::Path;

    // -------------------------------------------------------------------------
    // Markdown 族测试
    // -------------------------------------------------------------------------

    #[test]
    fn test_detect_markdown_files() {
        let cases = vec![
            ("README.md", SourceType::Markdown),
            ("doc.mdx", SourceType::Markdown),
            ("notes.markdown", SourceType::Markdown),
            ("UPPER.MD", SourceType::Markdown), // 大小写不敏感
        ];

        for (filename, expected) in cases {
            let path = Path::new(filename);
            let result = SourceTypeDetector::detect(path)
                .unwrap_or_else(|e| panic!("检测 {filename} 失败: {e}"));
            assert_eq!(result, expected, "{filename} 应被识别为 {expected:?}");
        }
    }

    // -------------------------------------------------------------------------
    // 代码族测试（覆盖全部扩展名）
    // -------------------------------------------------------------------------

    #[test]
    fn test_detect_code_files() {
        let code_extensions: Vec<(&str, SourceType)> = vec![
            ("main.rs", SourceType::Code),
            ("app.py", SourceType::Code),
            ("index.js", SourceType::Code),
            ("app.ts", SourceType::Code),
            ("server.go", SourceType::Code),
            ("main.c", SourceType::Code),
            ("app.cpp", SourceType::Code),
            ("Main.java", SourceType::Code),
            ("script.rb", SourceType::Code),
            ("app.kt", SourceType::Code),
            ("lib.swift", SourceType::Code),
            ("build.zig", SourceType::Code),
            ("Cargo.toml", SourceType::Code),
            ("config.yaml", SourceType::Code),
            ("settings.yml", SourceType::Code),
            ("data.json", SourceType::Code),
        ];

        for (filename, expected) in code_extensions {
            let path = Path::new(filename);
            let result = SourceTypeDetector::detect(path)
                .unwrap_or_else(|e| panic!("检测 {filename} 失败: {e}"));
            assert_eq!(result, expected, "{filename} 应被识别为 Code");
        }
    }

    // -------------------------------------------------------------------------
    // 纯文本测试
    // -------------------------------------------------------------------------

    #[test]
    fn test_detect_plain_text() {
        let plain_cases = vec![
            ("notes.txt", SourceType::Plain),
            ("app.log", SourceType::Plain),
            ("data.csv", SourceType::Plain),
        ];

        for (filename, expected) in plain_cases {
            let path = Path::new(filename);
            let result = SourceTypeDetector::detect(path)
                .unwrap_or_else(|e| panic!("检测 {filename} 失败: {e}"));
            assert_eq!(result, expected, "{filename} 应被识别为 Plain");
        }
    }

    // -------------------------------------------------------------------------
    // 不支持的格式测试
    // -------------------------------------------------------------------------

    #[test]
    fn test_detect_unsupported_format() {
        let unsupported = vec!["document.pdf", "report.docx", "image.png", "archive.zip"];

        for filename in unsupported {
            let path = Path::new(filename);
            let result = SourceTypeDetector::detect(path);

            assert!(result.is_err(), "{filename} 应返回错误");

            match result.unwrap_err() {
                err if err.code().contains("PARSE") && err.source() == ErrorSource::USR => {
                    assert!(
                        err.message().contains("不支持的文件格式"),
                        "{filename} 的错误消息应包含'不支持的文件格式': {}",
                        err.message()
                    );
                    let ext = Path::new(filename).extension().unwrap().to_str().unwrap();
                    assert!(
                        err.message().contains(ext),
                        "{filename} 的错误消息应包含扩展名 {ext}"
                    );
                }
                other => panic!("期望 UnsupportedFormat, 实际: {other}"),
            }
        }
    }

    // -------------------------------------------------------------------------
    // 无扩展名测试
    // -------------------------------------------------------------------------

    #[test]
    fn test_no_extension_error() {
        let path = Path::new("Makefile");
        let result = SourceTypeDetector::detect(path);

        assert!(result.is_err());
        match result.unwrap_err() {
            err if err.code().contains("PARSE") && err.source() == ErrorSource::USR => {
                assert!(
                    err.message().contains("无扩展名"),
                    "错误消息应包含'无扩展名': {}",
                    err.message()
                );
            }
            other => panic!("期望 UnsupportedFormat('无扩展名'), 实际: {other}"),
        }
    }

    // -------------------------------------------------------------------------
    // is_supported 测试
    // -------------------------------------------------------------------------

    #[test]
    fn test_is_supported_true() {
        assert!(SourceTypeDetector::is_supported(Path::new("file.rs")));
        assert!(SourceTypeDetector::is_supported(Path::new("file.md")));
        assert!(SourceTypeDetector::is_supported(Path::new("file.txt")));
    }

    #[test]
    fn test_is_supported_false() {
        assert!(!SourceTypeDetector::is_supported(Path::new("file.pdf")));
        assert!(!SourceTypeDetector::is_supported(Path::new("Makefile")));
        assert!(!SourceTypeDetector::is_supported(Path::new(".hidden")));
    }

    #[test]
    fn test_case_insensitive_extension() {
        let lower = SourceTypeDetector::detect(Path::new("FILE.RS")).unwrap();
        let upper = SourceTypeDetector::detect(Path::new("file.rs")).unwrap();
        assert_eq!(lower, upper, "大小写不应影响检测结果");
    }

    #[test]
    fn test_detect_error_code_is_e3002() {
        let err = SourceTypeDetector::detect(Path::new("file.pdf")).unwrap_err();
        assert!(
            err.code().contains("PARSE"),
            "错误码应包含 PARSE: {}",
            err.code()
        );
    }

    #[test]
    fn test_detect_pyw_extension() {
        let result = SourceTypeDetector::detect(Path::new("script.pyw"));
        assert_eq!(result.unwrap(), SourceType::Code, ".pyw 应被识别为 Code");
    }

    #[test]
    fn test_detect_mjs_extension() {
        let result = SourceTypeDetector::detect(Path::new("app.mjs"));
        assert_eq!(result.unwrap(), SourceType::Code, ".mjs 应被识别为 Code");
    }

    #[test]
    fn test_detect_cjs_extension() {
        let result = SourceTypeDetector::detect(Path::new("app.cjs"));
        assert_eq!(result.unwrap(), SourceType::Code, ".cjs 应被识别为 Code");
    }

    #[test]
    fn test_detect_tsx_extension() {
        let result = SourceTypeDetector::detect(Path::new("component.tsx"));
        assert_eq!(result.unwrap(), SourceType::Code, ".tsx 应被识别为 Code");
    }

    #[test]
    fn test_detect_jsx_extension() {
        let result = SourceTypeDetector::detect(Path::new("component.jsx"));
        assert_eq!(result.unwrap(), SourceType::Code, ".jsx 应被识别为 Code");
    }

    #[test]
    fn test_detect_kts_extension() {
        let result = SourceTypeDetector::detect(Path::new("build.kts"));
        assert_eq!(result.unwrap(), SourceType::Code, ".kts 应被识别为 Code");
    }

    #[test]
    fn test_detect_h_extension() {
        let result = SourceTypeDetector::detect(Path::new("header.h"));
        assert_eq!(result.unwrap(), SourceType::Code, ".h 应被识别为 Code");
    }

    #[test]
    fn test_detect_hpp_extension() {
        let result = SourceTypeDetector::detect(Path::new("header.hpp"));
        assert_eq!(result.unwrap(), SourceType::Code, ".hpp 应被识别为 Code");
    }

    #[test]
    fn test_detect_cc_extension() {
        let result = SourceTypeDetector::detect(Path::new("source.cc"));
        assert_eq!(result.unwrap(), SourceType::Code, ".cc 应被识别为 Code");
    }

    #[test]
    fn test_detect_hidden_file_with_extension() {
        let result = SourceTypeDetector::detect(Path::new(".env.toml"));
        assert_eq!(
            result.unwrap(),
            SourceType::Code,
            ".env.toml 应被识别为 Code"
        );
    }

    #[test]
    fn test_is_supported_with_full_path() {
        assert!(SourceTypeDetector::is_supported(Path::new(
            "/some/deep/path/to/file.rs"
        )));
    }
}
