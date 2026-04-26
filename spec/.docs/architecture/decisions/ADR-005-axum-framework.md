# ADR-005: 使用 Axum 作为 Web 框架

## 状态
✅ **Accepted** (2026-04-13)

## Context (背景)

knowledge-api 层需要一个高性能、类型安全的 Web 框架来提供：
- RESTful API 端点（CRUD 操作）
- WebSocket 实时双向通信（实时推送）
- 中间件链（认证、限流、日志、PII 脱敏）
- 文件上传/下载（文档摄入）
- SSE (Server-Sent Events) 流式输出（RAG 回答流式返回）

Rust 生态中有多个成熟的 Web 框架可选，各有不同的设计哲学和适用场景。

## Decision (决定)

选择 **Axum 0.7** 作为 Web 框架，由 Tokio 团队维护，基于 hyper 构建。

### 核心选型理由

#### 1. 类型安全的路由提取器 (Extractors)

```rust
// 路径参数自动提取和类型校验
async fn get_document(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<DocumentDto>> {
    let doc = state.document_repo.get_by_id(&id).await?;
    Ok(Json(doc.into()))
}
```

- 编译期保证路由参数的类型正确性
- 无需运行时的字符串解析和类型转换
- IDE 自动补全和重构支持完善

#### 2. 原生 WebSocket 支持

```rust
// WebSocket 升级处理
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}
```

- Axum 内置 WebSocket 协议升级，无需第三方 crate
- 全双工通信用于实时数据推送（搜索结果、Agent 进度等）

#### 3. Layered 中间件架构

```rust
let app = Router::new()
    .route("/api/v1/documents", post(create_document).get(list_documents))
    .route("/api/v1/documents/:id", get(get_document).put(update_document).delete(delete_document))
    .route("/ws", get(ws_handler))
    .layer(AuthLayer::new())
    .layer(RateLimitLayer::new())
    .layer(PiiSanitizeLayer::new())
    .layer(TraceLayer::new_for_http())
    .with_state(app_state);
```

- Tower 中间件生态丰富（cors、compression、timeout 等）
- 中间件顺序精确可控

#### 4. 与 Tokio 生态无缝集成

Axum 由 Tokio 团队开发维护，与项目使用的 Tokio 异步运行时完美配合：
- 共享 runtime 和 executor
- 统一的 tracing 日志体系
- 兼容 tokio::sync 原语

## Consequences (影响)

### 正面影响

- 🟢 **性能卓越**：基于 hyper 的非阻塞 I/O，基准测试表现优异
- 🟢 **类型安全**：路由提取器在编译期捕获大部分错误
- 🟢 **生态成熟**：Tower 中间件生态丰富（cors、limit、trace、compression）
- 🟢 **WebSocket 原生**：无需额外依赖即可支持实时双向通信
- 🟢 **社区活跃**：Tokio 团队维护，更新频繁，文档完善

### 负面影响

- 🔴 **宏系统侵入性**：`#[handler]` 等过程宏可能影响编译时间和错误信息
- 🔴 **静态路由限制**：动态路由（如 `/files/*path`）的支持不如某些框架灵活
- 🔴 **GraphQL 需额外集成**：Axum 本身不内置 GraphQL，需搭配 async-graphql

### 缓解措施

- 使用 `axum-extra` 补充缺失功能（如 cookie、cached）
- GraphQL 需求通过独立的 `/graphql` 端点 + async-graphql 集成

## Alternatives (替代方案)

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| **Actix-web v4** | 性能极高、Actor 模型 | Actor 模型学习曲线陡峭、extractor 类型安全性较弱 | ❌ 过度工程化 |
| **Rocket** | 宏系统强大、类似 Express | 默认同步模型、与 Tokio 集成不够深入 | ❌ 异步体验不佳 |
| **warp** | Filter 组合模式优雅 | 错误类型处理复杂、性能略低于 Axum | ❌ 大型项目可维护性不足 |
| **Poem** | 性能好、API 简洁 | 社区较小、中间件生态有限 | ❌ 生态不够成熟 |
| **Axum 0.7** ✅ | 类型安全、Tokio 原生、WS 原生 | 相对年轻 | ✅ **选定方案** |

## References

- [Axum 官方文档](https://docs.rs/axum/latest/axum/)
- [Tokio 项目主页](https://tokio.rs/)
- [Tower 中间件文档](https://docs.rs/tower/latest/tower/)
