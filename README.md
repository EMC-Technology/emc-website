<div align="center">

![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)
![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)
![CI](https://github.com/EMC-Technology/emc-website/actions/workflows/ci.yml/badge.svg)
![deps](https://img.shields.io/badge/Status-WASM%20Ready-green.svg)

# 文本全结构化知识系统 — 纯 Rust 全结构化知识库引擎

</div>

## 项目宪章

- **使命**：让全球生态感知纯 Rust 知识库解决方案的可行性，公开验证错误驱动开发和流程驱动开发两大技术哲学
- **愿景**：以 Rust 统一完整基础设施闭环，UPCM 驱动包含操作系统在内的所有相关软件
- **设计哲学**：0 随机性，0 黑盒推断

## 授权协议

本项目采用**分层双许可策略**：

| 内容类型 | 许可证 | 适用范围 |
|---------|--------|---------|
| 软件代码 | **MIT OR Apache-2.0** | `crates/`、`examples/`、`tests/`、`scripts/`、`docker/` |
| 知识内容 | **CC-BY-SA 4.0** | `spec/`、`docs/` |
| 模型权重 | Google Gemma License | 按需下载，不随代码分发 |

软件代码双许可与 Rust 语言自身及 crates.io 生态一致，是企业采用的黄金标准。

**详细授权说明**：请参阅 [LICENSE-COMMERCIAL.md](LICENSE-COMMERCIAL.md) 文件。

## 快速上手

### 前置条件

| 工具 | 版本 | 安装 |
|------|------|------|
| Rust | ≥ 1.85 | [rustup.rs](https://rustup.rs/) |
| SurrealDB | ≥ 1.3 | `curl -sSf https://install.surrealdb.com \| sh` |
| just | 最新 | `cargo install just` |

### 构建与运行

```bash
# 克隆仓库
git clone https://github.com/EMC-Technology/emc-website.git
cd emc-website

# 构建
just build

# 启动 SurrealDB（新终端）
surreal start --bind 0.0.0.0:8000 --user root --pass root memory

# 运行 API 服务器
just dev-run

# 运行测试
just test

# 完整 lint 检查
just lint
```

### Docker 一键启动

```bash
just docker-build
just docker-up
# API: http://localhost:3000
# 健康检查: http://localhost:3000/health
```

> 更多详情请参阅 [快速开始指南](spec/.docs/getting-started.md)

## 核心特性

- 🔍 **三路混合检索** — BM25 + Vector + Graph，加权 RRF 融合
- 🧠 **GEMMA4-E4B 纯 Rust 推理** — 42 层 Transformer，Candle 框架，2560 维语义向量
- 🏗️ **四层节点模型** — Document → Block → Token → Community，Token 级全局偏移量
- 🔐 **企业级安全** — Cedar 风格 ABAC + PII 扫描(11 种模式) + AES-256-GCM + 审计链
- 📊 **CQRS + Event Sourcing** — 天然审计追踪，事件流即合规证据
- 🔄 **error-core 统一错误处理** — 四维分类指纹 + 错误码全局注册表 + 恢复状态机 + 断路器
- 📋 **UPCM 流程驱动先行实践** — DAG 编排引擎 + YAML 工作流定义 + 统一状态机
- 🤖 **MCP Server + ReAct Agent** — 7 Tool + 2 Prompt + Resources，推理-行动循环

## 产品矩阵

本项目是公司纯 Rust 产品矩阵的核心知识引擎层：

| 项目 | 定位 | 许可证 |
|------|------|--------|
| **ullm** | LLM API 网关 (14 提供商/61+ 模型) | 开源 (MIT OR Apache-2.0) |
| **知识库系统**（本项目） | 全结构化知识引擎 | 开源 (MIT OR Apache-2.0 + CC-BY-SA 4.0) |
| **knowledge-infra** | DevOps 基础设施层 | 闭源 |
| **error-core** | 统一错误处理核心库 | 开源 (MIT OR Apache-2.0) |
| **UPCM** | 通用流程控制引擎 | 闭源（规划中） |

## 商业能力获取

如需使用分布式数据库、完整3D浏览器操作系统、AI IDE、大模型推理实现等商业闭源产品，请联系武汉质能科技有限公司（EMC）获取授权：

- 联系邮箱：EMCTechnology@163.COM
- 官方网址：https://emc-technology.github.io/emc-website

## 免责声明

开放内容按"现状"提供，武汉质能科技有限公司（EMC）不承担任何明示/默示担保责任。

## 版权归属

© 2026 武汉质能科技有限公司（EMC）保留所有权利。

## 其他资源

- [更新日志](CHANGELOG.md) - 版本历史和更新记录
- [贡献指南](CONTRIBUTING.md) - 如何参与本项目贡献
- [详细授权说明](LICENSE-COMMERCIAL.md) - 各组件授权详情
