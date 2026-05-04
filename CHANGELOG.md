# 更新日志

所有重要的项目更新都将记录在此文件中。更新日志遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 格式。

## [0.1.0] - 2026-04-26

### 首次开源发布

这是文本全结构化知识系统的首个开源版本，作为基线产品供非商业场景下的技术研究与应用落地。

### 新增组件

#### error-core - 统一错误处理核心库
- 四维分类指纹（ErrorSource、ErrorSeverity、ErrorImpact、ErrorRecoverability）
- 错误码全局注册表（ErrorCode Registry）
- 类型状态 Builder 模式
- 上下文帧和因果链双链传播
- 恢复状态机 + 断路器 + 确定性退避
- 完整的 Kani 形式化验证测试

#### knowledge-core - 核心业务逻辑库
- 三路混合检索（BM25 + Vector + Graph）
- 加权 RRF 融合算法
- Leiden 社区检测
- CQRS + Event Sourcing 架构
- Cedar 风格 ABAC 引擎
- PII 扫描（11 种敏感数据模式）
- AES-256-GCM 加密
- 审计链完整追踪

#### knowledge-parser - 多格式解析引擎
- DAG 编排解析管道
- Markdown/Code 双分支解析
- Tree-sitter AST 解析
- 多语言支持（Rust、Python、JavaScript、Go 等）
- 社区检测集成

#### knowledge-api - HTTP/WebSocket API 服务器
- GEMMA4-E4B 纯 Rust 推理（42 层 Transformer）
- 2560 维语义向量
- MCP Server + ReAct Agent
- RAG 引擎完整实现
- WebSocket 实时通信
- JWT 认证
- Prometheus 指标导出
- OpenTelemetry 分布式追踪

#### knowledge-frontend - Dioxus WASM 前端
- 知识库 CRUD 操作
- 图可视化
- WebSocket 实时通信

### 技术特性

- **全栈 Rust 自研**：从内核到前端完全使用 Rust 实现
- **WASM 编译**：前端编译为 WebAssembly，支持跨平台运行
- **no_std 支持**：error-core 支持嵌入式开发
- **零成本抽象**：使用 trait 和泛型，无运行时开销
- **形式化验证**：使用 Kani 进行形式化验证测试

### 文档更新

- 完整的设计文档（spec/ 目录）
- 详细的架构说明
- 错误处理方法论
- 贡献指南

### 授权协议

本项目软件代码采用 MIT OR Apache-2.0 双许可，知识内容采用 CC-BY-SA 4.0 许可。详细信息请参阅 [LICENSE](LICENSE) 和 [LICENSE-COMMERCIAL.md](LICENSE-COMMERCIAL.md) 文件。

---

## 版本历史

- [0.1.0] - 2026-04-26
  - 首次开源发布