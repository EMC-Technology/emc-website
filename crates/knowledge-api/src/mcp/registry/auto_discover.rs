//! 自动发现机制
//!
//! 提供 proc macro 和运行时自动注册能力，简化工具开发流程。

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, AttributeArgs, NestedMeta};

/// MCP 工具自动注册宏
///
/// 使用此宏标注的函数将自动：
/// 1. 生成符合 MCP 规范的 ToolDefinition
/// 2. 实现 ToolHandler trait
/// 3. 生成注册代码
///
/// # Attributes
///
/// - `name`: 工具名称（必填）
/// - `description`: 工具描述（可选，从 doc comment 提取）
/// - `category`: 工具类别（默认: Custom）
/// - `tags`: 标签列表（可选）
///
/// # Examples
///
/// ```ignore
/// use mcp_registry::mcp_tool;
///
/// #[mcp_tool(
///     name = "search_knowledge",
///     category = "Search",
///     description = "搜索知识库"
/// )]
/// async fn search_knowledge(
///     query: String,
///     limit: Option<u32>,
/// ) -> Result<SearchResults> {
///     // 实现逻辑
/// }
/// ```
#[proc_macro_attribute]
pub fn mcp_tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_args = parse_macro_input!(attr as AttributeArgs);
    let input_fn = parse_macro_input!(item as ItemFn);

    let tool_name = extract_attr_string(&attr_args, "name")
        .expect("name 属性为必填项");
    let description = extract_attr_string(&attr_args, "description");
    let category = extract_attr_string(&attr_args, "category")
        .unwrap_or_else(|| "Custom".to_string());
    let tags = extract_attr_list(&attr_args, "tags");

    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_inputs = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;

    let struct_name =
        syn::Ident::new(&format!("{}Handler", fn_name), fn_name.span());

    let expanded = quote! {
        #[derive(Clone)]
        #fn_vis struct #struct_name;

        impl #struct_name {
            pub fn new() -> Self {
                Self
            }
        }

        #[async_trait::async_trait]
        impl mcp_registry::ToolHandler for #struct_name {
            async fn handle(
                &self,
                args: serde_json::Value,
                ctx: mcp_registry::CallContext,
            ) -> std::result::Result<mcp_registry::ToolCallResult, error_core::ErrorObject> {
                // 参数反序列化
                let params: #fn_name Params = serde_json::from_value(args)
                    .map_err(|e| error_core::ErrorObject::bad_request(&format!(
                        "参数解析失败: {}", e
                    )))?;

                // 调用原始函数
                let result = #fn_name(#fn_inputs_to_params).await
                    .map_err(|e| error_core::ErrorObject::internal_error(&e.to_string()))?;

                // 包装结果
                Ok(mcp_registry::ToolCallResult {
                    content: vec![mcp_registry::ContentBlock::Text {
                        text: serde_json::to_string(&result).unwrap_or_default()
                    }],
                    is_error: false,
                    metadata: mcp_registry::CallMetadata {
                        duration_ms: 0,
                        tokens_used: 0,
                        cached: false,
                    },
                })
            }
        }

        /// 自动生成的参数结构体
        #[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
        pub struct #fn_name Params {
            #(#fn_inputs)*
        }

        /// 原始函数保留供直接调用
        #input_fn

        /// 注册辅助函数
        pub fn register_#fn_name(
            registry: &mcp_registry::ToolRegistry,
        ) -> std::result::Result<uuid::Uuid, error_core::ErrorObject> {
            let definition = mcp_registry::ToolDefinition {
                name: #tool_name.to_string(),
                description: #description,
                input_schema: schemars::schema_for!(#fn_name Params),
                capabilities: mcp_registry::ToolCapabilities::default(),
                rate_limits: mcp_registry::RateLimitConfig::default(),
                auth_requirements: mcp_registry::AuthRequirements::default(),
                metadata: mcp_registry::ToolMetadata {
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    author: env!("CARGO_PKG_AUTHORS").to_string(),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                    tags: vec![#(#tags),*],
                    category: mcp_registry::ToolCategory::#category,
                },
            };

            let handler = std::sync::Arc::new(#struct_name::new());
            registry.register_tool(definition, handler).await
        }
    };

    TokenStream::from(expanded)
}

fn extract_attr_string(args: &AttributeArgs, key: &str) -> Option<String> {
    for arg in args {
        if let NestedMeta::Meta(syn::Meta::NameValue(nv)) = arg {
            if nv.path.is_ident(key) {
                if let syn::Lit::Str(lit_str) = &nv.lit {
                    return Some(lit_str.value());
                }
            }
        }
    }
    None
}

fn extract_attr_list(args: &AttributeArgs, key: &str) -> Vec<String> {
    let mut result = Vec::new();
    for arg in args {
        if let NestedMeta::Meta(syn::Meta::List(list)) = arg {
            if list.path.is_ident(key) {
                for nested in &list.nested {
                    if let NestedMeta::Meta(syn::Meta::Path(path)) = nested {
                        if let Some(ident) = path.get_ident() {
                            result.push(ident.to_string());
                        }
                    } else if let NestedMeta::Lit(syn::Lit::Str(lit)) = nested {
                        result.push(lit.value());
                    }
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_extract_attr_string() {
        use super::*;

        let input: AttributeArgs = syn::parse_quote! {
            name = "test_tool",
            description = "A test tool"
        };

        assert_eq!(
            extract_attr_string(&input, "name"),
            Some("test_tool".to_string())
        );
        assert_eq!(
            extract_attr_string(&input, "description"),
            Some("A test tool".to_string())
        );
        assert_eq!(extract_attr_string(&input, "nonexistent"), None);
    }

    #[test]
    fn test_extract_attr_list() {
        use super::*;

        let input: AttributeArgs = syn::parse_quote! {
            name = "test",
            tags = ["search", "fast", "knowledge"]
        };

        let tags = extract_attr_list(&input, "tags");
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&"search".to_string()));
        assert!(tags.contains(&"fast".to_string()));
        assert!(tags.contains(&"knowledge".to_string()));
    }
}
