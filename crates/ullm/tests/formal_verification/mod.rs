#![cfg(test)]
//! 形式化验证测试模块 (Formal Verification Tests)
//!
//! 本模块提供三类形式化验证测试：
//!
//! ## 1. MIRI 测试 (MIRI Tests)
//! 使用 `cargo +miri test` 运行，检测未定义行为 (UB) 和内存安全问题。
//! 测试名称以 `test_miri_` 为前缀。
//!
//! ## 2. Kani 测试 (Kani-style Bounded Model Checking)
//! 使用 `cargo +kani verify` 运行（需要 Kani 工具链），验证有界状态空间的所有可达状态。
//! 测试名称以 `test_kani_` 为前缀。
//!
//! ## 3. 演绎证明测试 (Deductive Proof Tests)
//! 使用 `cargo test` 运行，通过不变量和前置条件/后置条件证明程序正确性。
//! 测试名称以 `test_deductive_` 为前缀。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

mod cancel_formal;
mod error_formal;
mod middleware_formal;
mod rate_limit_formal;
mod retry_formal;
mod stream_formal;
mod registry_formal;
mod provider_formal;
mod tool_formal;
mod response_formal;

pub use cancel_formal::*;
pub use error_formal::*;
pub use middleware_formal::*;
pub use rate_limit_formal::*;
pub use retry_formal::*;
pub use stream_formal::*;
pub use registry_formal::*;
pub use provider_formal::*;
pub use tool_formal::*;
pub use response_formal::*;