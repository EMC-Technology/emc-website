# 更新日志

所有重要的项目更新都将记录在此文件中。更新日志遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 格式。

## [0.2.0] - 2026-05-12

### 新增组件

#### ullm - 通用 LLM 访问生命周期管理模块
- 统一的多 LLM Provider 接口（OpenAI、Anthropic、Gemini、DeepSeek、GLM、Qwen、Ollama、Groq、Mistral、Kimi、CodeGeeX、OpenRouter、xAI 等）
- 流式响应处理（SSE 解析、Anthropic/OpenAI 映射器、工具调用状态机）
- 凭证管理（多门户认证：Copilot、Gemini CLI、MiniMax、Qwen）
- 速率限制与令牌桶算法
- 指数退避重试策略
- 工具调用（Function Calling）完整支持
- 思维链（Chain-of-Thought）解析
- Token 计数（tiktoken-rs 集成）
- 形式化验证测试（Kani）

### 新增功能

#### knowledge-core
- CQRS 投影（Projection）支持
- 数学工具模块（math.rs）
- 模型 ID 类型系统（ids.rs）
- 形式化验证框架（proofs/）
- SurrealDB Schema V3
- HNSWLib 向量存储适配器
- Reranker 模型加载器（candle_reranker_model.rs）
- 中间件追踪（tracing_middleware.rs）

#### knowledge-api
- MCP 会话管理器（session_manager.rs）
- 上下文生成集成测试
- 插件系统增强（远程 Agent、质量门控）

#### knowledge-parser
- Markdown 解析管道（markdown_parser.rs、markdown_pipeline.rs）
- 语义分块器（semantic_chunker.rs）
- 源类型检测器（source_type_detector.rs）
- 作用域栈（scope_stack.rs）
- 符号解析器（symbol_resolver.rs）
- 文本分割器（text_splitter.rs）
- Tree-sitter 解析器（tree_sitter_parser.rs）
- 摘要引擎（summarizer/）
- 属性测试（parser_property_test.rs）

#### knowledge-evaluator
- 答案相关性指标（answer_relevancy.rs）
- 答案相似度指标（answer_similarity.rs）
- 上下文召回指标（context_recall.rs）

### 依赖更新
- vaultrs 0.7 → 0.8（切换到 rustls 后端）
- reqwest 0.12 → 0.13
- rcgen 0.13 → 0.14
- criterion 0.5 → 0.8

### 基础设施改进
- Release 工作流优化：publish 容错 + GitHub Release 独立于 crates.io
- Dockerfile 修复：Rust 基础镜像版本对齐（1.91）、添加 ullm crate、修正 schema 文件路径
- dependabot 增加 github-actions 和 docker 生态系统配置
- .gitignore 完善覆盖率数据和历史记录文件覆盖
- 部署脚本：Podman Pod 部署、SurrealDB 构建脚本
- ADR-011：Podman Pod 部署架构决策

### 文档更新
- 架构概览文档更新
- 快速入门文档更新
- 设计文档更新
- 删除过时的插件化集成架构文档

---

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

- [0.2.0] - 2026-05-12
  - 新增 ullm 模块，增强知识处理管道，依赖更新，基础设施改进
- [0.1.0] - 2026-04-26
  - 首次开源发布
