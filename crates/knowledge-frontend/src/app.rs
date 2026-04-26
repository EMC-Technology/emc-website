//! Dioxus 根组件
//!
//! 四个核心视图：
//! - `DocumentListView`: 文档列表与上传
//! - `BlockDetailView`: 文档详情/Block/Token

#![allow(clippy::derive_partial_eq_without_eq)]
//! - `GraphVisualizationView`: 图谱可视化
//! - `SettingsView`: 系统设置

use dioxus::prelude::*;
use dioxus_signals::{Readable, Signal, Writable};
use crate::hooks::{ApiClient, RealtimeConnection, RealtimeSync};
use crate::components::{ErrorMessage, FileUploader, Loading, DocumentCard};
use knowledge_core::model::{Block, Document, Token, TokenType};

fn id_str(id: Option<&knowledge_core::model::RecordIdType>) -> String {
    id.map(ToString::to_string).unwrap_or_default()
}

/// 文档列表视图
#[component]
pub fn DocumentListView() -> Element {
    let mut documents = use_signal(Vec::<Document>::new);
    let mut loading = use_signal(|| false);
    let mut error_msg = use_signal(|| None::<String>);
    let mut show_upload = use_signal(|| false);
    let mut upload_path = use_signal(String::new);
    let mut upload_content = use_signal(String::new);
    let api: Signal<ApiClient> = use_context();

    use_future(move || {
        let api = api;
        async move {
            loading.set(true);
            match api().get_documents_typed().await {
                Ok(docs) => {
                    documents.set(docs);
                    error_msg.set(None);
                }
                Err(e) => {
                    error_msg.set(Some(e.to_string()));
                }
            }
            loading.set(false);
        }
    });

    rsx! {
        div {
            class: "container mx-auto p-4",
            h1 {
                class: "text-2xl font-bold mb-4",
                "文档列表"
            }

            if loading() {
                Loading {}
            }

            if let Some(err) = error_msg() {
                ErrorMessage { message: err }
            }

            div {
                class: "mb-4 flex gap-2",
                button {
                    class: "bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded",
                    onclick: move |_| show_upload.set(!show_upload()),
                    if show_upload() { "取消上传" } else { "上传文件" }
                }
            }

            if show_upload() {
                FileUploader {
                    on_file_selected: move |(name, content)| {
                        upload_path.set(name);
                        upload_content.set(content);
                    },
                }
                div {
                    class: "mt-4 p-4 bg-gray-50 rounded-lg",
                    input {
                        class: "w-full mb-2 p-2 border rounded",
                        r#type: "text",
                        placeholder: "文件路径（如 /docs/readme.md）",
                        value: "{upload_path}",
                        oninput: move |e| upload_path.set(e.value()),
                    }
                    textarea {
                        class: "w-full mb-2 p-2 border rounded h-32",
                        placeholder: "文件内容",
                        value: "{upload_content}",
                        oninput: move |e| upload_content.set(e.value()),
                    }
                    button {
                        class: "bg-green-500 hover:bg-green-700 text-white font-bold py-2 px-4 rounded",
                        disabled: upload_path().trim().is_empty() || upload_content().trim().is_empty(),
                        onclick: move |_| {
                            let path = upload_path();
                            let content = upload_content();
                            let api = api;
                            async move {
                                match api().upload_file(&path, None, &content).await {
                                    Ok(_) => {
                                        show_upload.set(false);
                                        upload_path.set(String::new());
                                        upload_content.set(String::new());
                                        match api().get_documents_typed().await {
                                            Ok(docs) => documents.set(docs),
                                            Err(e) => error_msg.set(Some(e.to_string())),
                                        }
                                    }
                                    Err(e) => error_msg.set(Some(e.to_string())),
                                }
                            }
                        },
                        "提交"
                    }
                }
            }

            div {
                class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mt-4",
                for doc in documents() {
                    {
                        let doc_id = id_str(doc.id.as_ref());
                        let title = doc.title.clone();
                        let source_type = format!("{:?}", doc.source_type);
                        rsx! {
                            DocumentCard {
                                doc_id: doc_id,
                                title: title,
                                source_type: source_type,
                            }
                        }
                    }
                }
            }

            if documents().is_empty() && !loading() {
                div {
                    class: "text-center text-gray-500 mt-8",
                    "暂无文档，点击「上传文件」开始"
                }
            }
        }
    }
}

/// 文档详情视图
#[component]
pub fn BlockDetailView(id: String) -> Element {
    let mut document = use_signal(|| None::<Document>);
    let mut blocks = use_signal(Vec::<Block>::new);
    let mut tokens = use_signal(Vec::<Token>::new);
    let mut loading = use_signal(|| false);
    let mut error_msg = use_signal(|| None::<String>);
    let display_id = id.clone();
    let api: Signal<ApiClient> = use_context();

    use_future(move || {
        let doc_id = id.clone();
        let api = api;
        async move {
            loading.set(true);
            match api().get_document_typed(&doc_id).await {
                Ok(doc) => document.set(Some(doc)),
                Err(e) => error_msg.set(Some(e.to_string())),
            }
            match api().get_document_blocks_typed(&doc_id).await {
                Ok(blks) => blocks.set(blks),
                Err(e) => error_msg.set(Some(e.to_string())),
            }
            loading.set(false);
        }
    });

    let doc_path = document().as_ref().map(|d| d.path.clone()).unwrap_or_default();
    let doc_source = document().as_ref().map(|d| format!("{:?}", d.source_type)).unwrap_or_default();
    let doc_hash = document().as_ref().map(|d| d.hash.clone()).unwrap_or_default();

    rsx! {
        div {
            class: "container mx-auto p-4",
            h1 {
                class: "text-2xl font-bold mb-4",
                "文档详情: {display_id}"
            }

            if loading() {
                Loading {}
            }

            if let Some(err) = error_msg() {
                ErrorMessage { message: err }
            }

            div {
                class: "grid grid-cols-1 lg:grid-cols-3 gap-4",
                div {
                    class: "lg:col-span-1",
                    h2 {
                        class: "text-lg font-semibold mb-2",
                        "文档信息"
                    }
                    if document().is_some() {
                        div {
                            class: "bg-white shadow rounded p-4",
                            p {
                                class: "text-sm text-gray-600",
                                "路径: {doc_path}"
                            }
                            p {
                                class: "text-sm text-gray-600",
                                "类型: {doc_source}"
                            }
                            p {
                                class: "text-sm text-gray-600",
                                "哈希: {doc_hash}"
                            }
                        }
                    }
                }

                div {
                    class: "lg:col-span-1",
                    h2 {
                        class: "text-lg font-semibold mb-2",
                        "Block 列表"
                    }
                    div {
                        class: "space-y-2",
                        for block in blocks() {
                            {
                                let block_id = id_str(block.id.as_ref());
                                let block_type = format!("{:?}", block.block_type);
                                let start = block.start_line;
                                let end = block.end_line;
                                let bid = block_id;
                                let label = format!("{block_type} (L{start}-{end})");
                                rsx! {
                                    div {
                                        class: "bg-white shadow rounded p-3 cursor-pointer hover:bg-blue-50",
                                        onclick: move |_| {
                                            let bid_clone = bid.clone();
                                            let api = api;
                                            async move {
                                                match api().get_block_tokens_typed(&bid_clone).await {
                                                    Ok(toks) => tokens.set(toks),
                                                    Err(e) => error_msg.set(Some(e.to_string())),
                                                }
                                            }
                                        },
                                        p {
                                            class: "font-medium",
                                            "{label}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div {
                    class: "lg:col-span-1",
                    h2 {
                        class: "text-lg font-semibold mb-2",
                        "Token 列表"
                    }
                    if !tokens().is_empty() {
                        div {
                            class: "space-y-1 max-h-96 overflow-y-auto",
                            for token in tokens() {
                                {
                                    let content = token.content.clone();
                                    let css_class = match token.token_type {
                                        TokenType::Word => "inline-block px-2 py-1 m-1 text-sm rounded bg-blue-100 text-blue-800",
                                        TokenType::Keyword => "inline-block px-2 py-1 m-1 text-sm rounded bg-purple-100 text-purple-800",
                                        TokenType::Identifier => "inline-block px-2 py-1 m-1 text-sm rounded bg-green-100 text-green-800",
                                        TokenType::Punct => "inline-block px-2 py-1 m-1 text-sm rounded bg-gray-100 text-gray-800",
                                        TokenType::Symbol => "inline-block px-2 py-1 m-1 text-sm rounded bg-yellow-100 text-yellow-800",
                                    };
                                    rsx! {
                                        span {
                                            class: "{css_class}",
                                            "{content}"
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        p {
                            class: "text-gray-500",
                            "点击 Block 查看 Token"
                        }
                    }
                }
            }
        }
    }
}

/// 图谱可视化视图
#[component]
pub fn GraphVisualizationView() -> Element {
    let mut nodes = use_signal(Vec::<serde_json::Value>::new);
    let mut edges = use_signal(Vec::<serde_json::Value>::new);
    let mut loading = use_signal(|| false);
    let mut error_msg = use_signal(|| None::<String>);
    let mut search_query = use_signal(String::new);
    let api: Signal<ApiClient> = use_context();

    use_future(move || {
        let api = api;
        async move {
            loading.set(true);
            match api().get_documents_typed().await {
                Ok(docs) => {
                    let node_list: Vec<serde_json::Value> = docs
                        .iter()
                        .map(|doc| {
                            serde_json::json!({
                                "id": id_str(doc.id.as_ref()),
                                "label": doc.title,
                                "type": "Document",
                            })
                        })
                        .collect();
                    nodes.set(node_list);
                    edges.set(vec![]);
                    error_msg.set(None);
                }
                Err(e) => error_msg.set(Some(e.to_string())),
            }
            loading.set(false);
        }
    });

    let node_count = nodes().len();
    let edge_count = edges().len();

    rsx! {
        div {
            class: "container mx-auto p-4",
            h1 {
                class: "text-2xl font-bold mb-4",
                "图谱可视化"
            }

            if let Some(err) = error_msg() {
                ErrorMessage { message: err }
            }

            div {
                class: "mb-4 flex gap-2",
                input {
                    class: "flex-1 p-2 border rounded",
                    r#type: "text",
                    placeholder: "搜索文档...",
                    value: "{search_query}",
                    oninput: move |e| search_query.set(e.value()),
                }
                button {
                    class: "bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded",
                    onclick: move |_| {
                        let query = search_query();
                        let api = api;
                        async move {
                            loading.set(true);
                            match api().full_text_search_typed(&query, Some(20)).await {
                                Ok(results) => {
                                    let node_list: Vec<serde_json::Value> = results
                                        .iter()
                                        .map(|block| {
                                            serde_json::json!({
                                                "id": id_str(block.id.as_ref()),
                                                "label": format!("{:?} L{}", block.block_type, block.start_line),
                                                "type": "Block",
                                            })
                                        })
                                        .collect();
                                    nodes.set(node_list);
                                    edges.set(vec![]);
                                    error_msg.set(None);
                                }
                                Err(e) => error_msg.set(Some(e.to_string())),
                            }
                            loading.set(false);
                        }
                    },
                    "搜索"
                }
            }

            div {
                class: "h-[600px] border border-gray-300 rounded bg-gray-50 relative overflow-auto",
                if loading() {
                    div {
                        class: "absolute inset-0 flex items-center justify-center bg-white bg-opacity-75",
                        Loading {}
                    }
                }

                svg {
                    class: "w-full h-full",
                    view_box: "0 0 800 600",
                    {
                        let current_nodes = nodes();
                        let current_edges = edges();
                        let n_count = current_nodes.len();
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
                        let cols = (n_count as f64).sqrt().ceil() as usize;
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
                        let spacing_x = 700.0 / (cols.max(1) as f64).max(1.0);
                        let rows = n_count.div_ceil(cols.max(1));
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
                        let spacing_y = if rows > 1 { 500.0 / (rows as f64) } else { 100.0 };

                        let mut svg_elements = Vec::new();
                        let mut node_positions: std::collections::HashMap<String, (f64, f64)> = std::collections::HashMap::new();

                        for (i, node) in current_nodes.iter().enumerate() {
                            let col = i % cols.max(1);
                            let row = i / cols.max(1);
                            #[allow(clippy::cast_precision_loss)]
                            let cx = (col as f64).mul_add(spacing_x, 50.0) + spacing_x / 2.0;
                            #[allow(clippy::cast_precision_loss)]
                            let cy = (row as f64).mul_add(spacing_y, 50.0) + spacing_y / 2.0;

                            let node_id = node.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            node_positions.insert(node_id.clone(), (cx, cy));

                            let label = node.get("label").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                            let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let fill = match node_type.as_str() {
                                "Document" => "#3B82F6".to_string(),
                                "Block" => "#10B981".to_string(),
                                "Token" => "#F59E0B".to_string(),
                                _ => "#6B7280".to_string(),
                            };
                            let text_y = cy + 35.0;

                            svg_elements.push(rsx! {
                                g {
                                    key: "{i}",
                                    circle {
                                        cx: "{cx}",
                                        cy: "{cy}",
                                        r: "20",
                                        fill: "{fill}",
                                        stroke: "#1F2937",
                                        stroke_width: "1.5",
                                    }
                                    text {
                                        x: "{cx}",
                                        y: "{text_y}",
                                        text_anchor: "middle",
                                        font_size: "10",
                                        fill: "#374151",
                                        "{label}"
                                    }
                                }
                            });
                        }

                        for (i, edge) in current_edges.iter().enumerate() {
                            let from_id = edge.get("from_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let to_id = edge.get("to_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            if let (Some((x1, y1)), Some((x2, y2))) = (node_positions.get(&from_id), node_positions.get(&to_id)) {
                                svg_elements.push(rsx! {
                                    line {
                                        key: "edge-{i}",
                                        x1: "{x1}",
                                        y1: "{y1}",
                                        x2: "{x2}",
                                        y2: "{y2}",
                                        stroke: "#9CA3AF",
                                        stroke_width: "1",
                                    }
                                });
                            }
                        }

                        svg_elements.into_iter()
                    }
                }

                if nodes().is_empty() && !loading() {
                    div {
                        class: "absolute inset-0 flex items-center justify-center text-gray-500",
                        "暂无图谱数据"
                    }
                }
            }

            div {
                class: "mt-2 text-sm text-gray-500",
                "节点: {node_count} | 边: {edge_count}"
            }
        }
    }
}

/// 设置视图
#[component]
pub fn SettingsView() -> Element {
    let mut api_url = use_signal(|| "http://127.0.0.1:3000".to_string());
    let mut health_status = use_signal(|| "未检测".to_string());
    let mut ws_status = use_signal(|| "未连接".to_string());
    let mut ws_connection = use_signal(|| None::<RealtimeConnection>);
    let mut api: Signal<ApiClient> = use_context();

    rsx! {
        div {
            class: "container mx-auto p-4",
            h1 {
                class: "text-2xl font-bold mb-4",
                "设置"
            }

            div {
                class: "space-y-6",
                div {
                    class: "bg-white shadow rounded p-4",
                    h2 {
                        class: "text-lg font-semibold mb-3",
                        "连接配置"
                    }
                    div {
                        class: "mb-3",
                        label {
                            class: "block text-sm font-medium text-gray-700 mb-1",
                            "API 地址"
                        }
                        input {
                            class: "w-full p-2 border rounded",
                            r#type: "text",
                            value: "{api_url}",
                            oninput: move |e| api_url.set(e.value()),
                        }
                    }
                    button {
                        class: "bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded",
                        onclick: move |_| {
                            let url = api_url();
                            async move {
                                api.set(ApiClient::with_base_url(&url));
                                match api().health_check().await {
                                    Ok(_) => health_status.set("连接正常".to_string()),
                                    Err(_) => health_status.set("连接失败，请检查地址".to_string()),
                                }
                            }
                        },
                        "测试连接"
                    }
                    p {
                        class: "mt-2 text-sm",
                        "状态: {health_status}"
                    }
                }

                div {
                    class: "bg-white shadow rounded p-4",
                    h2 {
                        class: "text-lg font-semibold mb-3",
                        "WebSocket 状态"
                    }
                    p {
                        class: "text-sm text-gray-600",
                        "WebSocket: {ws_status}"
                    }
                    div {
                        class: "mt-2 flex gap-2",
                        button {
                            class: "bg-green-500 hover:bg-green-700 text-white font-bold py-2 px-4 rounded",
                            onclick: move |_| {
                                let sync = RealtimeSync::new();
                                match sync.connect() {
                                    Some(conn) => {
                                        ws_connection.set(Some(conn));
                                        ws_status.set("已连接".to_string());
                                    }
                                    None => ws_status.set("连接失败".to_string()),
                                }
                            },
                            "连接 WebSocket"
                        }
                        button {
                            class: "bg-red-500 hover:bg-red-700 text-white font-bold py-2 px-4 rounded",
                            disabled: ws_connection.read().is_none(),
                            onclick: move |_| {
                                if let Some(mut conn) = ws_connection.take() {
                                    conn.close();
                                    ws_status.set("已断开".to_string());
                                }
                            },
                            "断开连接"
                        }
                    }
                }

                div {
                    class: "bg-white shadow rounded p-4",
                    h2 {
                        class: "text-lg font-semibold mb-3",
                        "关于"
                    }
                    p {
                        class: "text-sm text-gray-600",
                        "文本全结构化知识系统 {crate::FRONTEND_VERSION}"
                    }
                    p {
                        class: "text-sm text-gray-600",
                        "基于 Dioxus + SurrealDB 构建"
                    }
                }
            }
        }
    }
}
