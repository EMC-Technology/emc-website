//! Grammar 缓存池（按需懒加载 grammar 包，缓存已加载实例）
//!
//! # 设计目标
//!
//! - 首次解析某语言时动态加载 grammar（~2-5MB/语言）
//! - 后续复用已加载实例（避免重复内存分配）
//! - 线程安全（OnceLock 保证初始化原子性）
//!
//! # Feature Flag 策略
//!
//! 每个 tree-sitter 语言 grammar 通过独立的 feature flag 控制，
//! 用户可以仅启用需要的语言以减小编译体积：
//!
//! ```toml
//! [dependencies.knowledge-parser]
//! features = ["lang-rust", "lang-python"]  # 仅启用 Rust 和 Python
//! ```
//!
//! # 内存占用参考
//!
//! | Language   | Grammar Size (approx) |
//! |------------|----------------------|
//! | rust       | ~2 MB                |
//! | python     | ~1.5 MB              |
//! | javascript | ~2 MB                |
//! | typescript | ~3 MB                |

use std::sync::OnceLock;

use crate::Result;
use error_core::helpers;
use error_core::prelude::ErrorSource;

macro_rules! define_lang_getter {
    ($func_name:ident, $feature:literal, $crate_path:path) => {
        #[cfg(feature = $feature)]
        fn $func_name() -> &'static tree_sitter::Language {
            static CACHE: OnceLock<tree_sitter::Language> = OnceLock::new();
            CACHE.get_or_init(|| $crate_path.into())
        }
    };
}

macro_rules! define_lang_getter_legacy {
    ($func_name:ident, $feature:literal, $crate_name:ident) => {
        #[cfg(feature = $feature)]
        fn $func_name() -> &'static tree_sitter::Language {
            static CACHE: OnceLock<tree_sitter::Language> = OnceLock::new();
            CACHE.get_or_init(|| {
                let old_lang = $crate_name::language();
                const _: () = assert!(
                    std::mem::size_of::<$crate_name::Language>()
                        == std::mem::size_of::<tree_sitter::Language>(),
                    concat!(stringify!($crate_name), "::Language 与 tree_sitter::Language 大小不一致")
                );
                const _: () = assert!(
                    std::mem::align_of::<$crate_name::Language>()
                        == std::mem::align_of::<tree_sitter::Language>(),
                    concat!(stringify!($crate_name), "::Language 与 tree_sitter::Language 对齐不一致")
                );
                // SAFETY: 旧版 `language()` 返回的 `Language` 类型与 `tree_sitter::Language`
                // 内存布局完全相同（均为 C FFI 指针的 newtype wrapper），
                // 上述编译期断言已验证 size_of 和 align_of 一致。
                // transmute_copy 仅复制字节而不改变语义，因此转换是安全的。
                // 前置条件：old_lang 是由对应 tree-sitter crate 返回的有效 Language 实例。
                // 不变量：两种类型的内存布局在语义上等价（均为封装单个 C 指针的 newtype wrapper）。
                // 字段布局等价性论证：size_of/align_of 一致是必要条件而非充分条件，
                // 但此处依赖 tree-sitter 生态的领域知识：旧版 crate 的 Language 类型
                // 和新版 tree_sitter::Language 均为对 C 结构体 `TSLanguage*` 的
                // 单字段 newtype wrapper，其字段顺序和类型在语义上保证一致。
                // 编译期断言已保证两者大小完全相等，因此 transmute_copy 不存在截断风险。
                unsafe {
                    std::mem::transmute_copy::<_, tree_sitter::Language>(&old_lang)
                }
            })
        }
    };
}

/// Grammar 缓存池（按需懒加载 grammar 包，缓存已加载实例）
///
/// 使用 `OnceLock` 实现线程安全的懒加载单例模式，
/// 保证每种语言的 grammar 只被初始化一次。
///
/// # 线程安全保证
///
/// `OnceLock::get_or_init()` 提供原子性的初始化语义，
/// 即使多个线程同时首次请求同一语言，grammar 也只会加载一次。
pub struct GrammarCachePool;

impl GrammarCachePool {
    define_lang_getter!(rust, "lang-rust", tree_sitter_rust::LANGUAGE);
    define_lang_getter!(python, "lang-python", tree_sitter_python::LANGUAGE);
    define_lang_getter!(javascript, "lang-javascript", tree_sitter_javascript::LANGUAGE);
    define_lang_getter!(typescript, "lang-typescript", tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
    define_lang_getter!(go, "lang-go", tree_sitter_go::LANGUAGE);
    define_lang_getter!(c, "lang-c", tree_sitter_c::LANGUAGE);
    define_lang_getter!(cpp, "lang-cpp", tree_sitter_cpp::LANGUAGE);
    define_lang_getter!(java, "lang-java", tree_sitter_java::LANGUAGE);
    define_lang_getter!(ruby, "lang-ruby", tree_sitter_ruby::LANGUAGE);
    define_lang_getter_legacy!(kotlin, "lang-kotlin", tree_sitter_kotlin);
    define_lang_getter!(swift, "lang-swift", tree_sitter_swift::LANGUAGE);
    define_lang_getter!(zig, "lang-zig", tree_sitter_zig::LANGUAGE);
    define_lang_getter_legacy!(toml, "lang-toml", tree_sitter_toml);
    define_lang_getter!(yaml, "lang-yaml", tree_sitter_yaml::LANGUAGE);
    define_lang_getter!(json, "lang-json", tree_sitter_json::LANGUAGE);
}

impl GrammarCachePool {
    /// 获取指定语言的 tree-sitter Language 对象
    ///
    /// 首次调用时会初始化对应语言的 grammar（可能耗时几毫秒），
    /// 后续调用直接返回缓存的静态引用，零开销。
    ///
    /// # 参数
    ///
    /// * `language` - 编程语言标识符（小写，如 "rust"、"python"）
    ///
    /// # Errors
    ///
    /// * 不支持的格式（对应 `ErrorObject` code `ERR-USR-PARSE-002`） - 语言不被支持或对应的 `feature flag` 未启用
    pub fn get_language(language: &str) -> Result<&'static tree_sitter::Language> {
        match language {
            #[cfg(feature = "lang-rust")]
            "rust" => Ok(Self::rust()),
            #[cfg(feature = "lang-python")]
            "python" => Ok(Self::python()),
            #[cfg(feature = "lang-javascript")]
            "javascript" => Ok(Self::javascript()),
            #[cfg(feature = "lang-typescript")]
            "typescript" => Ok(Self::typescript()),
            #[cfg(feature = "lang-go")]
            "go" => Ok(Self::go()),
            #[cfg(feature = "lang-c")]
            "c" => Ok(Self::c()),
            #[cfg(feature = "lang-cpp")]
            "cpp" => Ok(Self::cpp()),
            #[cfg(feature = "lang-java")]
            "java" => Ok(Self::java()),
            #[cfg(feature = "lang-ruby")]
            "ruby" => Ok(Self::ruby()),
            #[cfg(feature = "lang-kotlin")]
            "kotlin" => Ok(Self::kotlin()),
            #[cfg(feature = "lang-swift")]
            "swift" => Ok(Self::swift()),
            #[cfg(feature = "lang-zig")]
            "zig" => Ok(Self::zig()),
            #[cfg(feature = "lang-toml")]
            "toml" => Ok(Self::toml()),
            #[cfg(feature = "lang-yaml")]
            "yaml" => Ok(Self::yaml()),
            #[cfg(feature = "lang-json")]
            "json" => Ok(Self::json()),
            _ => Err(helpers::unsupported_format(&format!(
                "不支持的编程语言: {language}（请确认是否启用了对应的 lang-* feature flag）"
            ))),
        }
    }

    /// 预加载常用语言的 grammar（启动时调用以避免冷启动延迟）
    ///
    /// 在应用启动阶段预先初始化常用语言的 grammar，
    /// 将冷启动时的语法解析延迟从用户请求路径移至启动阶段。
    ///
    /// # Example
    ///
    /// ```ignore
    /// // 在 main() 或 async fn startup() 中调用
    /// GrammarCachePool::preload_common_languages().await?;
    /// ```
    /// # Errors
    ///
    /// 语言不支持（feature flag 未启用）或 grammar 初始化失败时返回错误
    pub fn preload_common_languages() -> Result<()> {
        let languages = ["rust", "python", "javascript", "typescript"];

        for lang in languages {
            match Self::get_language(lang) {
                Ok(_) => {
                    tracing::debug!("预加载 {} grammar 成功", lang);
                }
                Err(e) if e.code().contains("PARSE") && e.source() == ErrorSource::USR => {
                    tracing::debug!(
                        "{} grammar 未安装（feature flag 未启用），跳过预加载",
                        lang
                    );
                }
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    /// 获取所有支持的语言列表（基于当前编译的 feature flags）
    ///
    /// 返回当前二进制文件中实际可用的语言标识符列表，
    /// 用于前端展示、文档生成等场景。
    #[must_use]
    pub fn supported_languages() -> Vec<&'static str> {
        let langs = vec![
            #[cfg(feature = "lang-rust")]
            "rust",
            #[cfg(feature = "lang-python")]
            "python",
            #[cfg(feature = "lang-javascript")]
            "javascript",
            #[cfg(feature = "lang-typescript")]
            "typescript",
            #[cfg(feature = "lang-go")]
            "go",
            #[cfg(feature = "lang-c")]
            "c",
            #[cfg(feature = "lang-cpp")]
            "cpp",
            #[cfg(feature = "lang-java")]
            "java",
            #[cfg(feature = "lang-ruby")]
            "ruby",
            #[cfg(feature = "lang-kotlin")]
            "kotlin",
            #[cfg(feature = "lang-swift")]
            "swift",
            #[cfg(feature = "lang-zig")]
            "zig",
            #[cfg(feature = "lang-toml")]
            "toml",
            #[cfg(feature = "lang-yaml")]
            "yaml",
            #[cfg(feature = "lang-json")]
            "json",
        ];

        langs
    }
}

#[cfg(test)]
mod tests {
    use error_core::prelude::ErrorSource;
    use super::*;

    /// 测试 Rust grammar 加载（仅在 lang-rust feature 启用时有效）
    #[test]
    fn test_get_rust_language() {
        match GrammarCachePool::get_language("rust") {
            Ok(_) => {}
            Err(e) if e.code().contains("PARSE") && e.source() == ErrorSource::USR => {
                eprintln!("Rust grammar 未安装（未启用 lang-rust feature），跳过测试: {}", e.message());
            }
            Err(e) => panic!("意外的错误类型: {e}"),
        }
    }

    /// 测试 Python grammar 加载（仅在 lang-python feature 启用时有效）
    #[test]
    fn test_get_python_language() {
        match GrammarCachePool::get_language("python") {
            Ok(_) => {}
            Err(e) if e.code().contains("PARSE") && e.source() == ErrorSource::USR => {
                eprintln!("Python grammar 未安装（未启用 lang-python feature），跳过测试: {}", e.message());
            }
            Err(e) => panic!("意外的错误类型: {e}"),
        }
    }

    /// 测试不支持的语言返回正确的错误
    #[test]
    fn test_unsupported_language_returns_error() {
        let result = GrammarCachePool::get_language("brainfuck");

        assert!(result.is_err());
        match result.unwrap_err() {
            err if err.code().contains("PARSE") && err.source() == ErrorSource::USR => {
                assert!(
                    err.message().contains("brainfuck"),
                    "错误消息应包含语言名称: {}",
                    err.message()
                );
                assert!(
                    err.message().contains("feature flag"),
                    "错误消息应提示 feature flag 配置: {}",
                    err.message()
                );
            }
            other => panic!("期望 UnsupportedFormat 错误，实际: {other}"),
        }
    }

    /// 测试 `OnceLock` 保证懒加载的单例语义
    ///
    /// 多次获取同一语言应返回相同的指针地址，
    /// 证明 `OnceLock` 只执行了一次初始化。
    #[test]
    fn test_lazy_loading_once() {
        let result1 = GrammarCachePool::get_language("rust");
        let result2 = GrammarCachePool::get_language("rust");

        match (result1, result2) {
            (Ok(lang1), Ok(lang2)) => {
                assert_eq!(
                    std::ptr::addr_of!(*lang1),
                    std::ptr::addr_of!(*lang2),
                    "多次获取同一语言应返回相同的实例"
                );
            }
            (Err(e1), Err(e2)) if e1.code().contains("PARSE") && e1.source() == ErrorSource::USR && e2.code().contains("PARSE") && e2.source() == ErrorSource::USR => {
                eprintln!("Rust grammar 未安装（可选依赖），跳过测试");
            }
            (Err(e), _) | (_, Err(e)) => panic!("意外的错误: {e}"),
        }
    }

    /// 测试 `supported_languages` 返回非空列表（当至少有一个 feature 启用时）
    #[test]
    fn test_supported_languages_non_empty_when_features_enabled() {
        let langs = GrammarCachePool::supported_languages();

        if cfg!(any(
            feature = "lang-rust",
            feature = "lang-python",
            feature = "lang-javascript",
            feature = "lang-typescript"
        )) {
            assert!(
                !langs.is_empty(),
                "当启用任何语言 feature 时，supported_languages 应返回非空列表"
            );
        } else {
            assert!(
                langs.is_empty(),
                "当没有启用任何语言 feature 时，supported_languages 应返回空列表"
            );
        }
    }

    /// 测试预加载功能不会 panic（即使部分 grammar 未安装）
    #[tokio::test]
    async fn test_preload_does_not_panic_on_missing_grammars() {
        let result = GrammarCachePool::preload_common_languages();
        assert!(
            result.is_ok(),
            "预加载不应失败（缺失的 grammar 应被静默跳过）: {:?}",
            result.err()
        );
    }
}
