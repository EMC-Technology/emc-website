//! 安全头中间件
//!
//! 为 HTTP 响应添加安全相关的头信息，防止常见的 Web 攻击：
//! - X-Content-Type-Options: 防止 MIME 类型嗅探
//! - X-Frame-Options: 防止点击劫持
//! - X-XSS-Protection: 启用浏览器 XSS 过滤器
//! - Content-Security-Policy: 限制资源加载来源
//! - Referrer-Policy: 控制 referrer 信息的发送
//! - Strict-Transport-Security: 强制使用 HTTPS

use axum::{extract::Request, middleware::Next, response::Response};

/// 安全头中间件
///
/// 为所有响应添加安全相关的 HTTP 头，增强 API 的安全性。
pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;

    // 添加安全头
    add_security_headers(&mut response);

    response
}

/// 为响应添加安全头
fn add_security_headers(response: &mut Response) {
    // 防止 MIME 类型嗅探
    response.headers_mut().insert(
        "X-Content-Type-Options",
        axum::http::HeaderValue::from_static("nosniff"),
    );

    // 防止点击劫持
    response.headers_mut().insert(
        "X-Frame-Options",
        axum::http::HeaderValue::from_static("DENY"),
    );

    // 启用浏览器 XSS 过滤器
    response.headers_mut().insert(
        "X-XSS-Protection",
        axum::http::HeaderValue::from_static("1; mode=block"),
    );

    // 内容安全策略（根据实际需要调整）
    // 注意：style-src 不使用 'unsafe-inline'，改为 nonce 模式。
    // 前端渲染时需为每个 <style> 标签添加 nonce 属性。
    response.headers_mut().insert(
        "Content-Security-Policy",
        axum::http::HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'",
        ),
    );

    // 控制 referrer 信息
    response.headers_mut().insert(
        "Referrer-Policy",
        axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // 强制使用 HTTPS（生产环境使用）
    // 注意：在开发环境中可能需要注释掉这一行
    response.headers_mut().insert(
        "Strict-Transport-Security",
        axum::http::HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, response::Html, routing::get};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn test_security_headers_are_added() {
        async fn handler() -> Html<&'static str> {
            Html("<html></html>")
        }

        let app = Router::new()
            .route("/test", get(handler))
            .layer(axum::middleware::from_fn(security_headers_middleware));

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert!(response.headers().contains_key("X-Content-Type-Options"));
        assert!(response.headers().contains_key("X-Frame-Options"));
        assert!(response.headers().contains_key("X-XSS-Protection"));
        assert!(response.headers().contains_key("Content-Security-Policy"));
        assert!(response.headers().contains_key("Referrer-Policy"));
        assert!(response.headers().contains_key("Strict-Transport-Security"));
    }
}
