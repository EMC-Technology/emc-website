# 快速开始指南 (Getting Started)

本文档帮助你从零开始搭建文本全结构化知识系统的开发环境并运行项目。

> **项目宪章**：使命 — 让全球生态感知纯 Rust 知识库解决方案的可行性，公开验证错误驱动开发和流程驱动开发两大技术哲学；设计哲学 — 0 随机性，0 黑盒推断
>
> 本项目是公司纯 Rust 产品矩阵的核心知识引擎层，与 ullm（开源 LLM 网关）和 knowledge-infra（闭源 DevOps 基础设施层）协同。底层由 error-core（统一错误处理）和 UPCM（通用流程控制，规划中）贯穿。

## 目录

- [前置条件](#前置条件)
- [克隆与构建](#克隆与构建)
- [本地开发服务器启动](#本地开发服务器启动)
- [运行测试](#运行测试)
- [项目结构概览](#项目结构概览)
- [常见问题排查](#常见问题排查)

---

## 前置条件

### 必需工具

| 工具 | 版本要求 | 安装方式 | 验证命令 |
|------|----------|----------|----------|
| **Rust toolchain** | ≥ 1.85 (stable) | [rustup.rs](https://rustup.rs/) | `rustc --version` |
| **Cargo** | 随 Rust 安装 | — | `cargo --version` |
| **SurrealDB** | ≥ 1.3 | Docker / 二进制 | `surreal version` |
| **Git** | ≥ 2.30 | [git-scm.com](https://git-scm.com/) | `git --version` |

### 可选工具（推荐安装）

| 工具 | 用途 | 安装命令 |
|------|------|----------|
| `cargo-watch` | 文件变更自动重编译 | `cargo install cargo-watch` |
| `cargo-nextest` | 增强测试运行器 | `cargo install cargo-nextest` |
| `cargo-llvm-cov` | 代码覆盖率工具 | `cargo install cargo-llvm-cov` |
| `cargo-audit` | 依赖安全审计 | `cargo install cargo-audit` |
| `cargo-deny` | 依赖许可审查 | `cargo install cargo-deny` |
| **Just** | 命令运行器（替代 Make） | `cargo install just` |

### 平台特定说明

#### Windows

```powershell
# 使用 PowerShell 安装 Rust
winget install Rustlang.Rust.MSVC

# 安装 Visual Studio C++ Build Tools（编译某些原生依赖需要）
winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# 验证
rustc --version  # 应显示 >= 1.85.0
```

#### macOS

```bash
# 使用 Homebrew 安装
brew install rust

# macOS 可能需要的额外依赖
brew install openssl@3

# 验证
rustc --version
```

#### Linux (Ubuntu/Debian)

```bash
# 安装构建依赖
sudo apt update
sudo apt install build-essential pkg-config libssl-dev

# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# 验证
rustc --version
```

---

## 克隆与构建

### 方式一：使用 Git 克隆

```bash
git clone <repository-url>
cd 文本全结构化知识系统.md
```

### 方式二：一键环境搭建（推荐）

```bash
# 使用 Justfile 自动安装所有开发工具和依赖
just dev-setup
```

这会自动执行：
1. 安装 `cargo-watch`, `cargo-nextest`, `cargo-llvm-cov`, `cargo-audit`, `cargo-deny`
2. 输出环境就绪确认

### 编译项目

```bash
# 完整 workspace 编译（开发模式）
cargo build --workspace

# 仅编译指定 crate（更快）
cargo build --package knowledge-core
cargo build --package knowledge-api

# 发布模式编译（带优化）
cargo build --workspace --release
```

> ⏱️ 首次完整编译可能需要 **5-15 分钟**（取决于机器性能），后续增量编译通常 < 10 秒。

### 编译 Feature 选择

项目大量使用 feature flags 按需编译。常用组合：

```bash
# 最小化构建（仅核心模型，无数据库）
cargo build --package knowledge-core --no-default-features

# 完整功能构建（含数据库 + 事件驱动 + 向量搜索）
cargo build --package knowledge-core --features "db,event-driven,qdrant"

# 解析器：仅支持 Rust + Python
cargo build --package knowledge-parser --features "lang-rust,lang-python"

# 解析器：支持全部语言
cargo build --package knowledge-parser --features "lang-full,parallel"

# API 服务：含事件驱动
cargo build --package knowledge-api --features "event-driven"
```

---

## 本地开发服务器启动

### 1. 启动 SurrealDB 数据库

#### 方式 A：Docker Compose（推荐）

```bash
# 启动数据库服务
docker compose up -d surrealdb

# 验证服务状态
docker compose ps

# 查看日志
docker compose logs -f surrealdb
```

默认连接信息：
- **地址**: `ws://localhost:8000/rpc`
- **用户名**: `root`
- **密码**: `root`
- **命名空间**: `knowledge`
- **数据库**: `knowledge`

#### 方式 B：直接运行 SurrealDB

```bash
# 下载 SurrealDB 二进制文件
# https://surrealdb.com/install

# 启动内存模式（适合开发测试）
surreal start memory --user root --pass root --bind 0.0.0.0:8000

# 启动持久化模式（RocksDB 存储）
surreal start file:data/db/surreal.db --user root --pass root
```

### 2. 初始化数据库 Schema

```bash
# 方式一：通过 SurrealDB CLI
surreal sql --file crates/knowledge-core/schema.surql \
    --endpoint http://localhost:8000 \
    --ns knowledge --db knowledge \
    --username root --password root

# 方式二：程序启动时自动初始化（SchemaManager 会处理）
# 首次运行 knowledge-api 时会自动创建表和索引
```

### 3. 启动 API 服务

#### 开发模式（热重载）

```bash
# 使用 cargo-watch 监听文件变化并自动重启
cargo watch -x "run --bin knowledge-api"
```

或使用 Justfile：

```bash
just dev-run
```

#### 直接运行

```bash
# 默认配置启动
cargo run --bin knowledge-api

# 指定配置文件
cargo run --bin knowledge-api -- --config config/dev.yaml

# 指定日志级别
RUST_LOG=debug cargo run --bin knowledge-api
```

启动成功后你会看到：

```
   _                    _ _     _
  (_)                  (_) |   |
   _  _____ ____      ___| |__ |
  | |/ _ \ \ /\ \ /\ / / | '_ \| | '__/
  | |  __/\ V  V V V /| | |_) | | |
  |_|\___| \_/\_/\_/ |_|_.__/|_|_|

✨ Knowledge System API v0.1.0
📍 Listening on http://0.0.0.0:8080
🔗 Database: ws://localhost:8000/rpc (knowledge/knowledge)
📊 Metrics: http://localhost:8080/metrics
```

### 4. 验证服务健康

```bash
# 健康检查端点
curl http://localhost:8080/health

# API 版本信息
curl http://localhost:8080/api/v1/version

# Prometheus 指标
curl http://localhost:8080/metrics
```

---

## 运行测试

### 全量测试

```bash
# 使用 nextest（推荐，更快的并行测试）
cargo nextest run --workspace

# 使用标准 cargo test
cargo test --workspace

# 带覆盖率报告
cargo llvm-cov --workspace --lcov
```

### 指定包/测试

```bash
# 仅运行 knowledge-core 的测试
cargo test --package knowledge-core

# 运行特定测试函数
cargo test --package knowledge-core -- test_document_new_valid_hash

# 运行集成测试
cargo test --test integration

# 运行文档测试（验证示例代码）
cargo test --doc --workspace
```

### 基准测试

```bash
# 运行所有基准测试
cargo bench --workspace

# 运行特定基准
cargo bench --package knowledge-core --bench crypto_bench

# 与基线对比
cargo bench --package knowledge-api --bench api_bench --baseline main
```

### 代码质量检查

```bash
# 格式检查
cargo fmt --check --all

# Lint 检查（严格模式）
cargo clippy --workspace --all-targets -- -W clippy::all -W clippy::pedantic -D warnings

# 安全审计
cargo audit

# 依赖许可检查
cargo deny check

# 一键全部质量检查
just lint
```

---

## 项目结构概览

```
文本全结构化知识系统.md/
│
├── Cargo.toml                  # Workspace 根配置
├── clippy.toml                 # Clippy lint 规则
├── rustfmt.toml                # 代码格式化配置
├── deny.toml                   # 依赖安全策略
│
├── crates/                     # 核心 Crate
│   ├── error-core/            # 🔴 企业级错误处理
│   │   ├── src/               #    源码 (capture, classification,
│   │   │                      #    propagation, recovery...)
│   │   ├── tests/             #    测试 (单元/集成/Fuzz/Kani)
│   │   └── benches/           #    基准测试
│   │
│   ├── knowledge-core/        # 🟣 核心领域逻辑
│   │   ├── src/               #    cqrs/, cache/, search/,
│   │   │                      #    vector_store/, reranker/, event/
│   │   ├── schema.surql       #    SurrealDB Schema 定义
│   │   └── tests/             #    集成测试
│   │
│   ├── knowledge-api/         # 🟠 HTTP/WebSocket API
│   │   ├── src/               #    router, handler, mcp/, agent/,
│   │   │                      #    rag/, observability/
│   │   ├── examples/          #    示例代码
│   │   └── tests/             #    集成测试
│   │
│   ├── knowledge-parser/      # 🟢 多格式解析器
│   │   ├── src/               #    markdown_parser, tree_sitter_parser,
│   │   │                      #    chunker, pipeline...
│   │   └── tests/             #    解析器测试
│   │
│   └── knowledge-frontend/    # 🔵 Dioxus WASM 前端
│       └── src/               #    app.rs, components/, hooks/
│
├── tests/                     # 跨 crate 集成测试
├── scripts/                   # 辅助脚本
├── docker/                    # Docker 配置
│   ├── config.docker.yaml
│   └── entrypoint.sh
│
├── .docs/                     # 📚 项目文档
│   ├── architecture/          #    架构设计文档
│   │   ├── system-overview.md
│   │   ├── data-model.md
│   │   └── decisions/         #    ADR (10 个决策记录)
│   ├── security/              #    安全相关文档
│   └── getting-started.md     #    ← 本文档
│
├── examples/                  # 📖 用户级示例代码
├── .github/workflows/         # CI/CD 流水线
├── docker-compose.yml         # Docker 编排
├── Dockerfile                 # 容器镜像定义
├── CONTRIBUTING.md            # 贡献指南
└── justfile                   # 开发命令快捷方式
```

---

## 常见问题排查

### 编译问题

<details>
<summary><b>❌ 编译报错 "cannot find crate `surrealdb`"</b></summary>

确保启用了正确的 feature flags：

```bash
# knowledge-core 需要 db feature 才能使用 SurrealDB
cargo build --package knowledge-core --features "db"
```

</details>

<details>
<summary><b>❌ tree-sitter grammar 编译失败</b></summary>

tree-sitter 需要 C/C++ 编译器和 CMake：

```bash
# Windows (Visual Studio Build Tools)
# 确保安装了 MSVC Build Tools

# Ubuntu/Debian
sudo apt install build-essential cmake

# macOS
xcode-select --install
```

如果不需要某种语言支持，可以禁用对应的 feature：

```bash
cargo build --package knowledge-parser --no-default-features --features "lang-rust"
```

</details>

<details>
<summary><b>❌ WASM 编译失败 (knowledge-frontend)</b></summary>

需要安装 wasm32 target：

```bash
rustup target add wasm32-unknown-unknown
```

</details>

### 运行时问题

<details>
<summary><b>❌ 无法连接到 SurrealDB</b></summary>

1. 确认 SurrealDB 正在运行：`docker compose ps`
2. 确认端口未被占用：`netstat -an | findstr 8000` (Windows)
3. 检查防火墙设置
4. 尝试使用内存模式：`surreal start memory`

</details>

<details>
<summary><b>❌ 端口 8080 已被占用</b></summary>

修改配置文件中的端口，或终止占用进程：

```bash
# Windows
netstat -ano | findstr :8080
taskkill /PID <pid> /F

# Linux/macOS
lsof -i :8080
kill -9 <pid>
```

</details>

<details>
<summary><b>⚠️ Clippy 有大量 warnings</b></summary>

运行自动修复脚本：

```bash
# 尝试自动修复
cargo clippy --fix --allow-dirty --allow-staged --workspace

# 如果仍有警告，查看 scripts/fix-clippy.sh 了解详情
```

</details>

### 性能问题

<details>
<summary><b>🐌 首次编译非常慢</b></summary>

这是正常的。首次完整 workspace 编译可能需要 5-15 分钟。后续增量编译会很快。

加速技巧：
1. 使用 `sccache` 缓存编译产物
2. 使用 `mold` 替代默认链接器（Linux）
3. 增加并行编译 jobs：`cargo build -j <cpu核心数>`
4. 只编译你正在工作的 crate

</details>

---

## 下一步

- 📖 阅读 [架构总览](architecture/system-overview.md) 了解系统设计
- 📝 参考 [贡献指南](../CONTRIBUTING.md) 了解开发规范
- 🔧 查看 [ADR 决策记录](architecture/decisions/) 理解关键技术选择
- 🧪 运行 `examples/` 下的示例代码快速上手 API
- 🏗️ 尝试修改代码并用 `cargo watch` 观察热重载效果

祝开发愉快！ 🦀
