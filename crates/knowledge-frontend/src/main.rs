#[cfg(target_arch = "wasm32")]
use knowledge_frontend::run;

#[cfg(target_arch = "wasm32")]
fn main() {
    run();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("此 crate 仅支持 wasm32 目标，请使用 --target wasm32-unknown-unknown 编译");
    std::process::exit(1);
}
