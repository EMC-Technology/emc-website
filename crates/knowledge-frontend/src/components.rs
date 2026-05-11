//! 可复用 UI 组件库

#![allow(clippy::future_not_send, clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;
use dioxus_signals::{Owner, Writable};

/// 导航栏组件
#[component]
pub fn Navbar() -> Element {
    rsx! {
        nav {
            class: "bg-gray-800 text-white p-4",
            div {
                class: "container mx-auto flex justify-between items-center",
                div {
                    class: "text-xl font-bold",
                    "文本全结构化知识系统"
                }
                div {
                    class: "flex space-x-4",
                    a {
                        class: "hover:text-gray-300",
                        href: "/",
                        "文档"
                    }
                    a {
                        class: "hover:text-gray-300",
                        href: "/graph",
                        "图谱"
                    }
                    a {
                        class: "hover:text-gray-300",
                        href: "/settings",
                        "设置"
                    }
                }
            }
        }
    }
}

/// 文件上传组件
///
/// 提供隐藏的 `<input type="file">` 和"选择文件"按钮，
/// 选中文件后通过 `FileReader` 异步读取内容，通过回调传递给父组件。
#[component]
pub fn FileUploader(on_file_selected: EventHandler<(String, String)>) -> Element {
    let mut reading = use_signal(|| false);
    let input_id = use_signal(|| {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        format!("file-upload-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
    });

    rsx! {
        div {
            class: "border-2 border-dashed border-gray-300 rounded-lg p-6 text-center",
            h3 {
                class: "text-lg font-medium mb-2",
                "上传文件"
            }
            p {
                class: "text-gray-500 mb-4",
                "支持 Markdown、代码文件和纯文本"
            }

            input {
                id: "{input_id}",
                class: "hidden",
                r#type: "file",
                accept: ".md,.markdown,.txt,.rs,.py,.js,.ts,.go,.java,.c,.cpp,.h,.toml,.json,.yaml,.yml",
                onchange: move |_| {
                        let id = input_id();
                        reading.set(true);
                        wasm_bindgen_futures::spawn_local(async move {
                            let result = read_file_from_input(&id).await;
                            reading.set(false);
                            if let Some((name, content)) = result {
                                on_file_selected.call((name, content));
                            }
                        });
                    }
            }

            if reading() {
                Loading {}
            }

            button {
                class: "bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded",
                onclick: move |_| {
                        if let Some(window) = web_sys::window()
                            && let Some(document) = window.document()
                            && let Some(element) = document.get_element_by_id(&input_id())
                            && let Ok(input) = wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlInputElement>(element)
                        {
                            input.click();
                        }
                    },
                "选择文件"
            }

            p {
                class: "text-sm text-gray-400 mt-2",
                "或在下方面板直接输入文件路径和内容"
            }
        }
    }
}

async fn read_file_from_input(input_id: &str) -> Option<(String, String)> {
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    // SAFETY: Rc<RefCell<>> 在此处安全因为：
    // 1. 本代码运行在 WASM 单线程环境中（由 wasm_bindgen_futures::spawn_local 驱动）
    // 2. WASM 目前不支持多线程（SharedArrayBuffer 除外，本项目未使用）
    // 3. Rc 不实现 Send，但 WASM 单线程环境下 Future 不需要 Send 约束
    //    （文件顶部 #![allow(clippy::future_not_send)] 已确认此约束）
    // 4. RefCell 的运行时借用检查在单线程下不会导致数据竞争
    // 若未来迁移到多线程 WASM 环境（如使用 SharedArrayBuffer），
    // 需将此处替换为 Arc<Mutex<>>。
    type ClosureHolder = Rc<RefCell<Option<wasm_bindgen::closure::Closure<dyn Fn()>>>>;

    let window = web_sys::window()?;
    let document = window.document()?;
    let element = document.get_element_by_id(input_id)?;
    let input: web_sys::HtmlInputElement = element.dyn_into().ok()?;
    let files = input.files()?;
    let file = files.get(0)?;
    let file_name = file.name();

    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let reader = match web_sys::FileReader::new() {
            Ok(r) => r,
            Err(e) => {
                let _ = reject.call1(&wasm_bindgen::JsValue::NULL, &e);
                return;
            }
        };
        let reader_clone = reader.clone();
        let resolve_clone = resolve;
        let reject_clone = reject;

        let onload: ClosureHolder = Rc::new(RefCell::new(None));
        let onerror: ClosureHolder = Rc::new(RefCell::new(None));

        {
            let onload_ref = Rc::clone(&onload);
            let onerror_ref = Rc::clone(&onerror);
            let onload_closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                let result = reader_clone
                    .result()
                    .ok()
                    .and_then(|v| v.as_string())
                    .unwrap_or_default();
                let _ = resolve_clone.call1(
                    &wasm_bindgen::JsValue::NULL,
                    &wasm_bindgen::JsValue::from_str(&result),
                );
                onload_ref.borrow_mut().take();
                onerror_ref.borrow_mut().take();
            })
                as Box<dyn Fn() + 'static>);
            *onload.borrow_mut() = Some(onload_closure);
        }

        {
            let onload_ref = Rc::clone(&onload);
            let onerror_ref = Rc::clone(&onerror);
            let onerror_closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                let _ = reject_clone.call1(
                    &wasm_bindgen::JsValue::NULL,
                    &wasm_bindgen::JsValue::from_str("FileReader error"),
                );
                onload_ref.borrow_mut().take();
                onerror_ref.borrow_mut().take();
            })
                as Box<dyn Fn() + 'static>);
            *onerror.borrow_mut() = Some(onerror_closure);
        }

        reader.set_onload(Some(
            onload
                .borrow()
                .as_ref()
                .expect("onload closure was just set above; this cannot fail")
                .as_ref()
                .unchecked_ref(),
        ));
        reader.set_onerror(Some(
            onerror
                .borrow()
                .as_ref()
                .expect("onerror closure was just set above; this cannot fail")
                .as_ref()
                .unchecked_ref(),
        ));
        let _ = reader.read_as_text(&file);
    });

    let result = JsFuture::from(promise).await.ok()?;
    let content = result.as_string()?;
    Some((file_name, content))
}

/// 文档卡片组件
#[component]
pub fn DocumentCard(doc_id: String, title: String, source_type: String) -> Element {
    rsx! {
        div {
            class: "border border-gray-200 rounded-lg p-4 hover:shadow-md transition-shadow",
            a {
                href: format!("/document/{}", doc_id),
                class: "block",
                h3 {
                    class: "text-lg font-medium mb-2",
                    "{title}"
                }
                p {
                    class: "text-sm text-gray-500",
                    "类型: {source_type}"
                }
            }
        }
    }
}

/// 图谱节点组件
#[component]
pub fn GraphNode(id: String, label: String, node_type: String) -> Element {
    rsx! {
        div {
            class: format!("p-2 rounded-md {}", match node_type.as_str() {
                "Document" => "bg-blue-100",
                "Block" => "bg-green-100",
                "Token" => "bg-yellow-100",
                _ => "bg-gray-100",
            }),
            "{label}"
        }
    }
}

/// 加载状态组件
#[component]
pub fn Loading() -> Element {
    rsx! {
        div {
            class: "flex justify-center items-center",
            div {
                class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500",
            }
        }
    }
}

/// 错误提示组件
#[component]
pub fn ErrorMessage(message: String) -> Element {
    rsx! {
        div {
            class: "bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded",
            "{message}"
        }
    }
}
