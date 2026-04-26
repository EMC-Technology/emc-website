# 贡献指南 (Contributing Guide)

感谢你对文本全结构化知识系统的关注！我们欢迎各种形式的贡献，包括但不限于：代码提交、文档改进、Bug 报告、功能建议。

> **项目宪章**：使命 — 让全球生态感知纯 Rust 知识库解决方案的可行性，公开验证错误驱动开发和流程驱动开发两大技术哲学；设计哲学 — 0 随机性，0 黑盒推断
>
> 本项目是公司纯 Rust 产品矩阵的核心知识引擎层，与 ullm（开源 LLM 网关）和 knowledge-infra（闭源 DevOps 基础设施层）协同。底层由 error-core（统一错误处理）和 UPCM（通用流程控制，规划中）贯穿。

## 目录

- [许可证](#许可证)
- [开发者原创证书 (DCO)](#开发者原创证书-dco)
- [开发环境搭建](#开发环境搭建)
- [代码风格规范](#代码风格规范)
- [提交信息规范](#提交信息规范-conventional-commits)
- [PR 流程](#pr-流程)
- [测试要求](#测试要求)
- [文档要求](#文档要求)

---

## 许可证

本项目采用**分层双许可策略**：

- **软件代码**（`src/`、`tests/`、`benches/`、`crates/` 目录）：采用 **MIT OR Apache-2.0** 双许可，与 Rust 社区惯例一致。详见 `LICENSE-MIT` 和 `LICENSE-APACHE` 文件。
- **知识内容**（`spec/`、`docs/` 目录）：采用 **CC-BY-SA 4.0** 许可证，确保知识贡献在相同条件下共享。详见 `spec/LICENSE` 文件。

提交贡献即表示你同意上述许可条款适用于你的贡献内容。

---

## 开发者原创证书 (DCO)

本项目采用 [Developer Certificate of Origin (DCO)](https://developercertificate.org/) 策略。每个 Git commit 必须包含 `Signed-off-by:` 标记，确认你拥有提交代码的合法权利。

### DCO 声明

```
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.


Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

### 如何签署

在提交 commit 时添加 `-s` 参数：

```bash
git commit -s -m "feat(core): add hybrid search with RRF fusion"
```

这将自动在 commit 信息末尾添加：

```
Signed-off-by: Your Name <your.email@example.com>
```

确保 `user.name` 和 `user.email` 已正确配置：

```bash
git config --global user.name "Your Name"
git config --global user.email "your.email@example.com"
```

**注意**：缺少 `Signed-off-by:` 标记的 PR 将无法合并。

---

## 开发环境搭建

### 前置条件

| 工具 | 最低版本 | 推荐版本 | 用途 |
|------|----------|----------|------|
| **Rust** | 1.85+ | stable (最新) | 编译语言 |
| **SurrealDB** | 1.3+ | 1.5+ | 主数据库 |
| **Node.js** | 18+ | 20 LTS | Dioxus 前端构建工具链 |
| **Git** | 2.30+ | 最新版 | 版本控制 |
| **Docker** | 24+ | 最新版 | 容器化开发环境（可选） |

### 快速开始

```bash
# 1. 克隆仓库
git clone <repository-url>
cd 文本全结构化知识系统.md

# 2. 安装 Rust 开发工具链
rustup default stable
rustup component add rustfmt clippy rust-analyzer llvm-tools-preview

# 3. 安装开发辅助工具
cargo install cargo-watch        # 热重载开发
cargo install cargo-nextest      # 增强测试运行器
cargo install cargo-llvm-cov     # 代码覆盖率
cargo install cargo-audit        # 安全审计
cargo install cargo-deny         # 依赖审查

# 4. 启动 SurrealDB (使用 Docker)
docker compose up -d surrealdb

# 或使用 Makefile 一键搭建
make dev-setup

# 5. 验证环境
cargo check --workspace
cargo test --workspace --no-run
```

> 💡 详细的环境搭建步骤请参考 [`.docs/getting-started.md`](.docs/getting-started.md)。

---

## 代码风格规范

### Rust 代码规范

本项目严格遵循 **Rust 官方 API 指南** 和以下规则：

#### 命名约定

| 类型 | 规范 | 示例 |
|------|------|------|
| 类型/枚举/特征 | **PascalCase** (大驼峰) | `DocumentAggregate`, `HttpStatusCode` |
| 函数/方法/变量/模块 | **snake_case** (小蛇形) | `get_user_name`, `network::tcp_listener` |
| 常量/静态变量 | **SCREAMING_SNAKE_CASE** | `MAX_CONNECTIONS`, `DEFAULT_TTL` |
| 泛型参数 | 单大写字母 | `<T>`, `<K, V>` |
| 构造函数 | `new()` 或 `new_{variant}()` | `new()`, `new_with_config()` |

#### 格式化要求

项目配置了 [`rustfmt.toml`](rustfmt.toml)，所有代码必须通过格式化检查：

```bash
cargo fmt --check --all    # 检查格式
cargo fmt --all            # 自动修复格式
```

关键配置项：
- **Edition**: 2024
- **最大宽度**: 100 字符
- **Tab 大小**: 4 空格
- **导入分组**: Std → External → Crate（`group_imports = "StdExternalCrate"`）
- **导入粒度**: Crate 级别（`imports_granularity = "Crate"`）

#### Lint 规则

项目使用 [`clippy.toml`](clippy.toml) 配置，必须通过 Clippy 检查：

```bash
cargo clippy --workspace --all-targets -- -W clippy::all -W clippy::pedantic -D warnings
```

#### 错误处理规范

1. **可恢复错误**：必须返回 `Result<T, E>`，使用 `?` 传播
2. **不可恢复错误（外部输入）**：禁止使用 `unwrap()` / `expect()`，使用 `.context("描述")?`
3. **内部不变量违约**：允许使用 `expect("不变量说明")` 进行断言式 panic
4. **错误类型**：
   - 库代码 (`error-core`, `knowledge-core`)：使用 `error_core::helpers` 构造错误 + `error_core::ErrorObject`
   - 应用代码 (`knowledge-api`)：使用 `error_core::Result<T>` 别名

```rust
// ✅ 正确: 外部输入使用 ? 传播
pub async fn get_document(&self, id: &str) -> Result<DocumentDto> {
    if id.is_empty() {
        return Err(helpers::validation_error("文档 ID 不能为空"));
    }
    // ...
}

// ✅ 正确: 内部不变量使用 expect
let version = self.version.expect("聚合根版本号必须在构造时初始化");

// ❌ 错误: 对外部输入使用 unwrap
let doc = db.get(user_input).unwrap();  // 禁止!
```

#### 注释与文档

- **公开 API**：必须有 `///` 文档注释，包含功能简述、`# Example`、`# Errors`、`# Panics`（如适用）
- **复杂逻辑**：添加 `//` 行注释解释"为什么这么做"
- **禁止**：无意义的注释（如重复代码含义的注释）

```rust
/// 创建新的文档聚合根并执行初始化命令。
///
/// # Examples
///
/// ```ignore
/// let aggregate = DocumentAggregate::new("doc_001");
/// let result = mediator.send(CreateDocumentCommand { ... }).await?;
/// ```
///
/// # Errors
///
/// 返回 [`AggregateError::ValidationError`] 当输入验证失败，
/// 返回 [`AggregateError::ConcurrencyConflict`] 当版本冲突时。
///
/// # Panics
///
/// 当 `id` 为空字符串时 panic（内部不变量）。
pub struct DocumentAggregate { /* ... */ }
```

---

## 提交信息规范 (Conventional Commits)

本项目采用 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

### 格式

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Type 列表

| Type | 说明 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `docs` | 文档变更 |
| `style` | 代码格式（不影响功能） |
| `refactor` | 重构（非新功能、非 Bug 修复） |
| `perf` | 性能优化 |
| `test` | 测试相关 |
| `chore` | 构建/工具链变更 |
| `ci` | CI/CD 配置变更 |
| `security` | 安全修复 |

### Scope 列表

| Scope | 对应模块 |
|-------|----------|
| `core` | knowledge-core |
| `api` | knowledge-api |
| `parser` | knowledge-parser |
| `frontend` | knowledge-frontend |
| `error` | error-core |
| `docs` | 文档 |
| `ci` | CI/CD |

### 示例

```
feat(core): add hybrid search with RRF fusion

Implement reciprocal rank fusion for combining BM25 and vector
search results. This improves search accuracy from ~72% to ~89%
for natural language queries.

Closes #123
```

```
fix(api): resolve WebSocket connection leak on client disconnect

The WS handler was not properly cleaning up connections when clients
disconnected without sending a close frame. Added a timeout-based
cleanup task.

Fixes #456
```

---

## PR 流程

### 1. Fork & 分支

```bash
# 从 main 创建特性分支
git checkout -b feat/core/add-hybrid-search
```

分支命名规范：`<type>/<scope>/<short-description>`

### 2. 开发 & 提交

```bash
# 开发过程中保持 commit 整洁
git add .
git commit -m "feat(core): implement BM25 index builder"

# 推送到你的 fork
git push origin feat/core/add-hybrid-search
```

### 3. 创建 PR

在 GitHub 上创建 Pull Request，PR 描述模板应包含：

```markdown
## 变更概述
<!-- 简要描述这个 PR 做了什么 -->

## 变更类型
- [ ] Bug 修复
- [ ] 新功能
- [ ] 重构
- [ ] 文档更新
- [ ] 其他: ___

## 测试计划
<!-- 描述如何验证这个 PR -->

## 关联 Issue
Closes #___

## 检查清单
- [ ] 代码通过 `cargo fmt --check --all`
- [ ] 代码通过 `cargo clippy --workspace -- -D warnings`
- [ ] 所有测试通过 `cargo nextest run --workspace`
- [ ] 新增功能有对应的单元测试
- [ ] 公开 API 有完整的文档注释
- [ ] ADR 已更新（如有架构变更）
- [ ] 所有 commit 包含 `Signed-off-by:` 标记（DCO）
```

### 4. 质量门禁

每个 PR 必须通过以下自动检查：

| 检查项 | 命令 | 要求 |
|--------|------|------|
| **Format** | `cargo fmt --check --all` | ✅ 通过 |
| **Clippy** | `cargo clippy --workspace -- -D warnings` | ✅ 零 warning |
| **Test** | `cargo nextest run --workspace` | ✅ 全部通过 |
| **Security** | `cargo audit` + `cargo deny check` | ✅ 无已知漏洞 |
| **Docs** | 公开 API 文档完整性 | ✅ 无 missing_docs |

### 5. Code Review

- 至少需要 **1 位 maintainer** 的 Approval
- Reviewer 应关注：正确性 > 性能 > 可读性
- 对于并发和 unsafe 代码，安全性置于首位

---

## 测试要求

### 测试层级

| 层级 | 位置 | 运行方式 | 说明 |
|------|------|----------|------|
| **单元测试** | 同文件 `#[cfg(test)]` | `cargo test --package <pkg>` | 核心逻辑覆盖 |
| **集成测试** | `tests/` 目录 | `cargo test --test integration` | 跨模块交互 |
| **文档测试** | `///` 示例代码 | `cargo test --doc` | API 文档验证 |
| **基准测试** | `benches/` 目录 | `cargo bench --package <pkg>` | 性能回归检测 |
| **模糊测试** | `tests/*_fuzz_test.rs` | `cargo fuzz run ...` | 边缘情况覆盖 |
| **Kani 验证** | `tests/*_kani_test.rs` | `kani ...` | 形式化验证 (error-core) |

### 测试命名规范

```
test_<function>_<scenario>_<expected_result>
```

示例：
- `test_document_new_valid_hash_returns_ok`
- `test_block_with_embedding_wrong_dim_returns_error`
- `test_reference_bidirectional_check_two_way_returns_true`

### 覆盖率目标

| 模块 | 目标覆盖率 |
|------|-----------|
| error-core | ≥ 90% |
| knowledge-core | ≥ 85% |
| knowledge-api | ≥ 80% |
| knowledge-parser | ≥ 80% |

覆盖率报告通过 `cargo llvm-cov` 生成。

---

## 文档要求

### 必需文档

- 新的公开类型/函数必须有 `///` 文档注释
- 架构变更需要对应 [ADR](.docs/architecture/decisions/) 记录
- 复杂算法需要内联注释说明设计意图

### 文档目录结构

```
.docs/
├── architecture/
│   ├── system-overview.md     # 架构总览
│   ├── data-model.md          # 数据模型
│   └── decisions/             # ADR 决策记录
├── security/
│   └── unsafe-audit.md        # Unsafe 审计
└── getting-started.md         # 快速开始
```

---

## 行为准则

本项目遵循 **Rust 行为准则**：

- 🤝 **包容开放**：欢迎不同背景和经验水平的贡献者
- 🎯 **专注技术**：讨论围绕代码和设计，而非个人
- 🔄 **建设性反馈**：批评应具体、可操作、尊重他人
- 📚 **持续学习**：鼓励提问和知识分享

---

## 需要帮助？

- 📖 查看 [架构文档](.docs/architecture/system-overview.md) 了解系统设计
- 🔍 搜索 [现有 Issues](../../issues) 看是否已有类似讨论
- 💬 在 [Discussions](../../discussions) 中提出问题
- 🐛 发现 Bug？请提交 [Issue](../../issues/new?template=bug_report.md)

感谢你的贡献！🚀
