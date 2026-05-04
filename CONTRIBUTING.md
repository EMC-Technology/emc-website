# 贡献指南

感谢您对本项目的兴趣！我们欢迎任何形式的贡献，包括但不限于代码优化、功能扩展、文档完善、问题反馈等。

## 项目授权说明

本项目采用**分层双许可策略**：

| 内容类型 | 许可证 | 适用范围 |
|---------|--------|---------|
| 软件代码 | **MIT OR Apache-2.0** | `crates/`、`examples/`、`tests/`、`scripts/`、`docker/` |
| 知识内容 | **CC-BY-SA 4.0** | `spec/`、`docs/` |

贡献本项目即表示您同意：

- 您的软件代码贡献将采用 **MIT OR Apache-2.0** 双许可授权
- 您的知识内容贡献将采用 **CC-BY-SA 4.0** 协议授权
- 您保留对您贡献部分的原始著作权
- 您同意武汉质能科技有限公司（EMC）作为项目维护者使用您的贡献

如需将本项目或衍生作品用于商业目的，请联系 `EMCTechnology@163.COM` 获取授权。

## 快速开始

### 环境要求

- Rust 1.85 或更高版本
- SurrealDB（用于本地测试）
- just（任务运行器，`cargo install just`）

### 本地开发

```bash
# 克隆仓库
git clone https://github.com/EMC-Technology/emc-website.git
cd emc-website

# 安装依赖
cargo build --workspace

# 运行测试
cargo test --workspace

# 运行格式检查
cargo fmt --all
cargo clippy --workspace --all-features -- -D warnings
```

## 代码规范

### 提交信息格式

我们使用语义化提交格式：

```
<type>(<scope>): <subject>

[可选正文]

[可选页脚]
```

**类型（type）**：
- `feat`: 新功能
- `fix`: 错误修复
- `docs`: 文档更新
- `style`: 代码格式（不影响功能）
- `refactor`: 重构
- `perf`: 性能优化
- `test`: 测试相关
- `chore`: 构建/工具相关

**示例**：
```
feat(error-core): 添加四维分类指纹支持

实现了 ErrorClassification 结构，支持：
- ErrorSource 错误来源分类
- ErrorSeverity 错误严重级别
- ErrorImpact 错误影响范围
- ErrorRecoverability 错误可恢复性

Closes #123
```

### 代码风格

- 使用 `cargo fmt` 自动格式化
- 使用 `cargo clippy` 进行静态分析
- 遵循 Rust API 指导原则
- 所有公开 API 必须有文档注释

### 文档要求

- 所有公开函数、结构体、枚举必须有 `///` 文档注释
- 复杂算法需要添加 `//` 注释解释逻辑
- 示例代码必须可编译并通过测试

## 分支管理

- `master`: 主分支，仅通过 PR 合并
- 功能开发请创建新分支：`feature/<功能名称>`
- 错误修复请创建新分支：`fix/<问题描述>`

## Pull Request 流程

1. Fork 本仓库
2. 创建功能分支：`git checkout -b feature/your-feature`
3. 进行开发和测试
4. 提交更改：`git commit -m 'feat: add some feature'`
5. 推送到远程：`git push origin feature/your-feature`
6. 打开 Pull Request，等待代码审查

## 测试要求

- 所有新功能必须包含单元测试
- 确保 `cargo test --workspace` 通过
- 确保 `cargo clippy --workspace --all-features -- -D warnings` 无警告

## 问题反馈

请通过 GitHub Issues 反馈问题，包含以下信息：

- 问题描述
- 复现步骤
- 预期行为
- 实际行为
- 环境信息（Rust 版本、操作系统等）

## 许可证

本项目软件代码采用 MIT OR Apache-2.0 双许可，知识内容采用 CC-BY-SA 4.0 许可。详细信息请参阅 [LICENSE](LICENSE) 和 [LICENSE-COMMERCIAL.md](LICENSE-COMMERCIAL.md) 文件。

## 联系方式

- 邮箱：EMCTechnology@163.COM
- 官方网站：https://emc-technology.github.io/emc-website

感谢您的贡献！