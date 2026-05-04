# 武汉质能科技有限公司（EMC）授权协议说明

## 授权协议统一说明

为避免授权协议混淆，本文件详细说明仓库中各部分内容的授权情况。

### 1. 分层双许可策略

本项目采用分层双许可策略，与项目宪章（spec/开源目的说明.md）保持一致：

| 内容类型 | 许可证 | 适用范围 |
|---------|--------|---------|
| 软件代码 | MIT OR Apache-2.0 | `crates/`、`examples/`、`tests/`、`benches/`、`scripts/`、`docker/` |
| 知识内容 | CC-BY-SA 4.0 | `spec/`、`docs/` |
| 模型权重 | Google Gemma License | 按需下载，不随代码分发 |

### 2. 软件代码许可（MIT OR Apache-2.0）

- **MIT**：简洁宽松，最大化采用率
- **Apache-2.0**：明确专利授权 + 报复条款，保护贡献者和使用者
- **双许可**：与 Rust 语言自身、crates.io 生态一致，是企业采用的黄金标准
- 详见 LICENSE-MIT 和 LICENSE-APACHE

### 3. 知识内容许可（CC-BY-SA 4.0）

- **BY（署名）**：使用时需标注原作者
- **SA（相同方式共享）**：衍生知识内容必须以相同许可发布，防止知识封闭化
- 详见：https://creativecommons.org/licenses/by-sa/4.0/

### 4. 各目录授权明细

| 目录/文件 | 授权类型 | 说明 |
|----------|---------|------|
| `crates/` | MIT OR Apache-2.0 | 所有 Rust 源代码组件 |
| `knowledge-system/` | MIT OR Apache-2.0 | 知识系统配置和核心组件 |
| `spec/` | CC-BY-SA 4.0 | 设计文档、规范和方法论 |
| `docs/` | CC-BY-SA 4.0 | 文档和使用说明 |
| `examples/` | MIT OR Apache-2.0 | 示例代码和使用案例 |
| `scripts/` | MIT OR Apache-2.0 | 构建和部署脚本 |
| `tests/` | MIT OR Apache-2.0 | 测试代码和测试数据 |
| `docker/` | MIT OR Apache-2.0 | Docker 相关配置 |

### 5. 商标与品牌资源

- **商标**："EMC"、"武汉质能科技"等商标归武汉质能科技有限公司所有
- **Logo**：仓库中的 Logo 和品牌标识保留所有权利
- **品牌资源**：未经授权，不得使用本公司商标和品牌资源进行商业活动

### 6. 商业闭源内容

以下内容为专有知识产权，未经武汉质能科技有限公司（EMC）书面授权，禁止任何形式的使用、复制、分发：

- 自研分布式内存堆图对象数据库（LightField）
- knowledge-infra DevOps 基础设施层
- UPCM 通用流程控制引擎
- 完整3D浏览器操作系统
- AI IDE

如需商业授权，请联系：
- 联系邮箱：EMCTechnology@163.COM
- 官方网址：https://emc-technology.github.io/emc-website

### 7. 免责声明

开放内容按"现状"提供，武汉质能科技有限公司不承担任何明示/默示担保责任，包括但不限于适销性、特定用途适用性、无侵权等；因使用本项目内容产生的直接/间接损失，我方不承担赔偿责任（法律强制要求除外）。

### 8. 版权归属

© 2026 武汉质能科技有限公司（EMC）保留所有权利。
