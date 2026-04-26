#![allow(clippy::result_large_err)]
//! 前端 UI 层（Dioxus → WASM）
//!
//! 本 crate 基于 Dioxus 框架编译为 WebAssembly：
//! - 声明式组件体系，与 React/Vue 概念对齐
//! - 通过 knowledge-core 共享后端数据模型，保证类型安全
//! - WebSocket 实时通信桥接

use dioxus::prelude::*;
use dioxus_signals::Signal;

pub mod app;
pub mod components;
pub mod hooks;

use app::{BlockDetailView, DocumentListView, GraphVisualizationView, SettingsView};
use components::Navbar;
use hooks::ApiClient;

/// 前端版本标识
pub const FRONTEND_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 应用路由定义（Dioxus 0.5 Routable 派生宏）
#[derive(Clone, Debug, PartialEq, Eq, Routable)]
pub enum Route {
    #[route("/")]
    DocumentListView {},

    #[route("/document/:id")]
    BlockDetailView { id: String },

    #[route("/graph")]
    GraphVisualizationView {},

    #[route("/settings")]
    SettingsView {},
}

/// 前端应用入口（仅在 WASM 目标上可用）
#[cfg(target_arch = "wasm32")]
pub fn run() {
    let cfg = dioxus::web::Config::new().rootname("main");
    dioxus::web::launch::launch_cfg(App, cfg);
}

/// 应用根组件
#[component]
pub fn App() -> Element {
    use_context_provider(|| Signal::new(ApiClient::new()));

    rsx! {
        Navbar {}
        Router::<Route> { }
    }
}
