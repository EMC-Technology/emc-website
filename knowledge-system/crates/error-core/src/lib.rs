//! 统一错误处理核心库 — 四维分类指纹、错误码注册表、恢复状态机

pub mod classification;
pub mod conversions;
pub mod error_capture;
pub mod error_code;
pub mod error_object;
pub mod logging;
pub mod propagation;
pub mod recovery;
pub mod user_prompt;
pub mod utils;

pub mod helpers;
pub mod prelude;

/// 错误处理核心库版本
pub const VERSION: &str = "0.1.0";

/// 错误处理核心库名称
pub const NAME: &str = "error-core";
