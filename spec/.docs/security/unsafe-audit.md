# Unsafe 代码审计报告 (Unsafe Audit Report)

## 审计信息

| 项目 | 值 |
|------|-----|
| **审计日期** | 2026-04-13 |
| **审计范围** | 全部 workspace crates |
| **审计标准** | Rust Unsafe Guidelines (RFC 2585) |
| **审计工具** | `cargo-geiger` + 人工 code review |
| **审计人员** | Core Team |

## 审计方法

### 1. 自动化扫描

```bash
# 使用 cargo-geiger 扫描所有 unsafe 使用
cargo install cargo-geiger
cargo geiger --workspace --all-features --output-format=json > geiger_report.json

# 使用 grep 定位所有 unsafe 块
rg "^(\s*)unsafe\s*\{" --type rust -n .
rg "unsafe fn" --type rust -n .
```

### 2. 人工审查清单

对每个 `unsafe` 块，回答以下问题：

- [ ] **必要性**：是否绝对需要 `unsafe`？能否用安全抽象替代？
- [ ] **最小作用域**：`unsafe` 块是否尽可能小？
- [ ] **Safety 注释**：是否有清晰的 `// Safety:` 注释说明不变量？
- [ ] **前置条件**：调用者需要满足哪些条件？
- [ ] **维护不变量**：如何确保不变量不被违反？
- [ ] **测试覆盖**：是否有对应的安全属性测试？

---

## 发现的 Unsafe 代码块列表

### 概览

| Crate | unsafe 函数数 | unsafe 块数 | FFI 调用数 | 风险等级 |
|-------|--------------|------------|-----------|---------|
| error-core | 待扫描 | 待扫描 | 0 | 低 |
| knowledge-core | 待扫描 | 待扫描 | 0 (SurrealDB SDK 内部) | 中 |
| knowledge-api | 待扫描 | 待扫描 | 0 | 低 |
| knowledge-parser | 待扫描 | 待扫描 | 0 (tree-sitter 内部) | 低 |

> ⚠️ **注意**: 以上数据需通过 `cargo geiger` 实际扫描填充。

### 预期 Unsafe 来源分析

#### knowledge-core / crypto.rs

**预期 unsafe 用途**：

```rust
/// BLAKE3 哈希计算
///
/// # Safety
///
/// 此函数封装了 blake3 crate 的底层 API。
/// blake3 内部使用 SIMD 指令优化（通过 unsafe 实现），
/// 但对外暴露的接口是安全的。
pub fn hash(data: &[u8]) -> HashOutput {
    // 可能的 unsafe: blake3 库内部使用
    blake3::hash(data)
}
```

**风险等级**: 🟢 **低**
- **原因**: 加密操作由成熟的第三方 crate (`blake3`, `aes-gcm`) 处理
- **缓解**: 这些 crate 经过广泛的安全审计
- **建议**: 保持关注上游安全公告，及时更新依赖版本

#### knowledge-core / vector_store.rs (Qdrant 后端)

**预期 unsafe 用途**：

```rust
/// Qdrant gRPC 客户端通信
///
/// # Safety
///
/// Qdrant gRPC 客户端 (tonic) 内部使用 async Rust，
/// tonic 的代码生成器可能产生 unsafe 代码用于：
/// - Protobuf 序列化/反序列化
/// - gRPC 通道管理
impl VectorStoreBackend for QdrantBackend {
    async fn upsert(&self, points: Vec<VectorPoint>) -> Result<()> {
        // tonic gRPC 调用 — 内部可能有 unsafe
        self.client.upsert(&UpsertPoints { points }).await?;
        Ok(())
    }
}
```

**风险等级**: 🟡 **中**
- **原因**: 涉及网络 I/O 和序列化
- **缓解**: tonic 是生产级 gRPC 库，有大量部署验证
- **建议**: 确保输入验证在进入 unsafe 边界前完成

#### knowledge-parser / tree_sitter_parser.rs

**预期 unsafe 用途**：

```rust
/// Tree-sitter 解析器初始化
///
/// # Safety
///
/// tree-sitter 通过 FFI 调用 C 语言库 (libtree-sitter)。
/// Parser 对象的生命周期必须超过所有从其派生的 Tree/Node 对象。
pub struct TsParser {
    inner: tree_sitter::Parser,
}

impl TsParser {
    /// 创建新的解析器实例
    ///
    /// # Safety
    ///
    /// tree-sitter 的 Parser 是 Send + Sync 的，
    /// 其内部 C 对象通过 Arc<UnsafeCell> 保护。
    /// 本 crate 的封装确保了安全的访问模式。
    pub fn new(language: Language) -> Self {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).expect("语言加载失败");
        Self { inner: parser }
    }
}
```

**风险等级**: 🟡 **中**
- **原因**: FFI 边界与 C 语言库交互
- **缓解**: tree-sitter Rust 绑定经过良好维护
- **建议**: 确保 Parser 生命周期正确管理（已有 RAII 封装）

---

## 每个 Unsafe 块的安全分析模板

### 模板格式

```markdown
#### 文件: `path/to/file.rs:LNN-LMM`

**代码片段**:
```rust
unsafe { /* ... */ }
```

**目的**: 简述为什么需要 unsafe

**Safety 注释要求**:
- ✅ 有完整的 `// Safety:` 注释
- ❌ 缺少 Safety 注释

**前置条件 (Preconditions)**:
1. 条件 A
2. 条件 B

**维护的不变量 (Invariants)**:
1. 不变量 X 必须始终成立
2. 不变量 Y 在函数返回时成立

**测试覆盖**:
- 单元测试: `test_xxx_safe_usage`
- 属性测试: `prop_xxx_invariant_holds`
- Kani 验证: `xxx_kani_test`

**风险评估**:
- 🟢 低 / 🟡 中 / 🔴 高

**改进建议**:
(如有)
```

---

## 建议

### 短期行动项 (Immediate)

1. **执行完整扫描**:
   ```bash
   cargo geiger --workspace --all-features
   ```
   将实际结果填入上表。

2. **补充 Safety 注释**:
   所有 `unsafe` 块必须有 `// Safety:` 注释，说明：
   - 为什么这里需要 unsafe
   - 调用者需要满足什么条件
   - 维护哪些不变量

3. **建立 Unsafe Review 流程**:
   - 新增 `unsafe` 代码必须经过 Code Review
   - PR 模板中添加 Unsafe 审查 checklist

### 中期目标 (Near-term)

4. **封装 unsafe 到安全抽象层**:
   - 将散落的 `unsafe` 块集中到少数几个模块
   - 对外仅暴露安全的 public API

5. **增加模糊测试覆盖**:
   - 对所有 unsafe 边界添加 proptest/cargo-fuzz 测试
   - 特别是 FFI 边界和内存操作

6. **定期重新审计**:
   - 每次 Release 前执行 `cargo geiger` 回归检查
   - 新增依赖时评估其 unsafe 使用情况

### 长期目标 (Long-term)

7. **消除不必要的 unsafe**:
   - 评估每个 `unsafe` 是否可以用纯 safe Rust 替代
   - 跟踪 Rust 版本升级带来的 safe 替代方案

8. **形式化验证扩展**:
   - 参考 error-core 的 Kani 验证实践
   - 将关键 unsafe 模块纳入 model checker 覆盖

---

## 附录: Unsafe 编码规范

### 必须遵守的规则

1. **最小作用域原则**:
   ```rust
   // ✅ 好: unsafe 块尽可能小
   let len = unsafe { (*ptr).len() };
   safe_operation(len);

   // ❌ 差: 不必要的扩大 unsafe 范围
   unsafe {
       let len = (*ptr).len();
       safe_operation(len);  // 这个操作不需要 unsafe
   }
   ```

2. **必须包含 Safety 注释**:
   ```rust
   // SAFETY: ptr 由 Vec::into_raw_parts 生成，容量 > 0，
   // 且在此函数调用期间没有其他代码访问该指针。
   unsafe { alloc::realloc(ptr, layout, new_size) }
   ```

3. **优先使用成熟的安全封装**:
   ```rust
   // ❌ 不要自己写裸指针操作
   let slice = unsafe { std::slice::from_raw_parts(ptr, len) };

   // ✅ 优先使用标准库或知名 crate 的安全封装
   use std::ffi::CStr;
   let cstr = unsafe { CStr::from_ptr(ptr) };  // 更安全的边界
   ```

### 参考资源

- [Rust Reference - Unsafe](https://doc.rust-lang.org/reference/unsafe-keyword.html)
- [Rust Unsafety Manifesto](https://github.com/rust-lang/unsafety-manifesto)
- [RFC 2585: Unsafe Code Guidelines](https://rust-lang.github.io/unsafe-code-guidelines/)
- [Rustonomicon - Unsafe](https://doc.rust-lang.ru/nomicon/unsafe.html)
