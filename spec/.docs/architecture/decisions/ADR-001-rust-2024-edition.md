# ADR-001: 采用 Rust 2024 Edition

## 状态
✅ **Accepted** (2026-04-13)

## Context (背景)

本项目是一个企业级知识图谱系统，对性能、安全性和可维护性有极高要求。在选择 Rust Edition 时，需要平衡以下因素：

1. **语言特性需求**：项目大量使用泛型、trait 对象、async/await、模式匹配等高级特性
2. **生态兼容性**：依赖库（Axum 0.7, SurrealDB 1.3, Tokio 1.35 等）的最低 Rust 版本要求
3. **长期维护性**：Edition 选择影响未来数年的代码演进路径
4. **WASM 前端支持**：knowledge-frontend 需要编译为 `wasm32-unknown-unknown` 目标

Rust 2024 Edition 是截至 2026 年最新的稳定 Edition，带来了多项重要改进。

## Decision (决定)

采用 **Rust 2024 Edition** 作为项目的标准 Edition，MSRV (Minimum Supported Rust Version) 设定为 **1.85**。

### 关键配置

```toml
# Cargo.toml (workspace)
edition = "2024"
rust-version = "1.85"

# .cargo/config.toml 中可能需要的配置
[build]
rust-edition = "2024"
```

### Rust 2024 Edition 为本项目带来的核心收益

| 特性 | 收益 | 应用场景 |
|------|------|----------|
| **Genric 增强** | 更灵活的泛型约束语法 | `VectorStoreBackend<T>` trait 定义 |
| **Async 改进** | 更好的 async trait 支持 | MCP Server / Agent 异步工具 |
| **模式匹配增强** | 更强大的解构能力 | CQRS Command 分发 |
| **安全性提升** | 更严格的 unsafe 审计规则 | Crypto 模块 |
| **WASM 优化** | 更好的 WASM 编译目标支持 | Dioxus 前端 |

## Consequences (影响)

### 正面影响

- 🟢 获得最新语言特性，代码更简洁、表达力更强
- 🟢 与主流 crate 生态系统保持同步（多数新版本 crate 已要求 2024 edition）
- 🟢 更好的编译器诊断信息和错误提示
- 🟢 未来-proof：为后续 Rust 版本升级预留空间

### 负面影响

- 🔴 要求开发者安装 Rust 1.85+，提高了入门门槛
- 🔴 部分 CI 环境（如旧版 Docker 镜像）需要更新 toolchain
- 🔴 某些老旧 crate 可能不兼容 2024 edition，需要寻找替代方案或 fork

### 缓解措施

- CI 流水线使用 `dtolnay/rust-toolchain@stable` 自动安装最新 stable 工具链
- 文档中明确标注 MSRV 要求
- `deny.toml` 配置依赖版本锁定策略

## Alternatives (替代方案)

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| **Rust 2021 Edition** | 兼容性最广，几乎所有 crate 都支持 | 错过 2024 的语言改进，部分新 crate 不再支持 2021 | ❌ 否决 - 技术债务风险高 |
| **Rust Nightly** | 可使用实验性特性 | 编译不稳定、无法发布到 crates.io、CI 复杂度激增 | ❌ 否决 - 生产系统不可接受 |
| **条件化 Edition** | 不同 crate 使用不同 edition | workspace 统一性破坏、依赖管理复杂 | ❌ 否决 - 维护成本过高 |

## References

- [Rust Edition Guide](https://doc.rust-lang.org/edition-guide/)
- [Rust 2024 Release Notes](https://blog.rust-lang.org/2024/00/00/rust-1.85.0.html)
- [MSRV 最佳实践](https://rust-lang.github.io/rfcs/2495-msrv-rationale.html)
