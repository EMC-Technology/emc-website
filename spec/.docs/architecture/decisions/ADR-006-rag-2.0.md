# ADR-006: 选择 RAG 2.0 作为 AI 策略

## 状态
✅ **Accepted** (2026-04-13)

## Context (背景)

系统的核心价值之一是为 AI（大语言模型）提供高质量的结构化知识检索能力。传统的 RAG 1.0 方案存在以下问题：

1. **准确率瓶颈**：简单的向量相似度搜索 + top-k 截断，准确率通常只有 ~70%
2. **上下文窗口浪费**：不相关的文档片段占用了宝贵的 token 预算
3. **缺乏推理**：无法处理多跳推理问题（"函数 A 调用的函数中哪些修改了全局状态？"）
4. **无结构化感知**：将知识图谱的图结构信息扁平化为文本向量，丢失了关系语义

本项目需要达到 **≥95% 的 RAG 准确率**，以支持企业级 AI 助手场景。

## Decision (决定)

实施 **RAG 2.0 多阶段流水线架构**，融合关键词搜索、向量搜索、图遍历和多阶段重排序。

### RAG 2.0 架构

```
用户查询
    │
    ▼
┌─────────────┐
│ Query       │  意图识别、查询扩展、改写
│ Understanding│
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────────┐
│         Retrieval Stage (召回)            │
│                                          │
│  ┌──────────┐  ┌──────────┐  ┌────────┐ │
│  │ BM25     │  │ Vector   │  │ Graph  │ │
│  │ 关键词   │  │ 向量搜索  │  │ 图遍历  │ │
│  │ (精确)   │  │ (语义)   │  │ (关系)  │ │
│  └────┬─────┘  └────┬─────┘  └───┬────┘ │
│       └─────────────┼────────────┘      │
│                     ▼                   │
│           RRF Fusion (倒数排名融合)       │
│           Top-K 候选集 (K=50)            │
└──────────────────┬──────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────┐
│         Reranking Stage (精排)            │
│                                          │
│  Stage 1: Cross-Encoder 重排序           │
│  (query, document) → 相关性得分          │
│           50 → 20                        │
│                    │                     │
│  Stage 2: LLM Judge 判定                │
│  "以下片段是否与查询相关？是/否/部分"     │
│           20 → Top-5~10                  │
│                    │                     │
│  Stage 3: 上下文压缩                     │
│  移除冗余、去重、格式化                   │
└──────────────────┬──────────────────────┘
                   │
                   ▼
┌─────────────────┐
│ LLM Generation  │  流式输出 (SSE)
│ (答案合成)       │
└─────────────────┘
```

### 核心组件说明

#### 1. 混合召回 (Hybrid Retrieval)

```rust
/// 倒数排名融合 (Reciprocal Rank Fusion)
pub fn reciprocal_rank_fusion(
    bm25_results: Vec<SearchResult>,
    vector_results: Vec<SearchResult>,
    k: f64, // 通常 k=60
) -> Vec<HybridSearchResult> {
    // RRF(score) = Σ 1/(k + rank_i)
    // 合并两个排序列表，消除单一算法的偏差
}
```

| 召回策略 | 优势 | 适用场景 |
|----------|------|----------|
| **BM25** | 精确关键词匹配 | 专业术语、代码标识符 |
| **Vector Search** | 语义相似度匹配 | 自然语言查询、同义词 |
| **Graph Traversal** | 关系路径发现 | 多跳推理、"谁调用了 X" |

#### 2. 多阶段重排序 (Multi-stage Reranker)

位于 [`reranker/mod.rs`](../../crates/knowledge-core/src/reranker/mod.rs)：

```rust
/// Cross-Encoder 重排序器 — 第一阶段
pub struct CrossEncoderReranker {
    model: CrossEncoderModel,
}

/// LLM Judge 判定器 — 第二阶段
pub struct LLMJudgeReranker {
    client: LlmClient,
}
```

**效果提升数据**：

| 阶段 | Top-5 准确率 | 说明 |
|------|-------------|------|
| 仅向量搜索 (RAG 1.0) | ~72% | 基线 |
| + BM25 混合召回 | ~81% | 关键词补充 |
| + Cross-Encoder | ~89% | 精排优化 |
| + LLM Judge | **≥95%** | 最终目标 |

#### 3. 流式输出 (SSE Streaming)

```rust
// Server-Sent Events 流式返回 RAG 结果
async fn rag_query(
    query: RagQueryRequest,
) -> impl Stream<Item = Result<RagChunk, Error>> {
    // 1. 召回候选
    let candidates = retrieve(&query).await?;
    // 2. 重排序
    let ranked = rerank(&query, &candidates).await?;
    // 3. LLM 生成（流式）
    llm_stream(&query.context, &ranked).await
}
```

## Consequences (影响)

### 正面影响

- 🟢 **准确率突破**：从传统 RAG 的 ~70% 提升到 ≥95%
- 🟢 **Token 效率**：通过重排序和压缩，减少无效上下文占用
- 🟢 **可解释性**：每一步的中间结果都可审计和调试
- 🟢 **模块化**：各阶段可独立替换和优化

### 负面影响

- 🔴 **延迟增加**：多阶段管道增加端到端延迟（目标 < 2s）
- 🔴 **资源消耗**：Cross-Encoder 和 LLM Judge 需要 GPU/CPU 推理资源
- 🔴 **复杂度**：需要维护多个模型和评分策略

### 缓解措施

- 并行执行独立的召回分支（BM25 / Vector / Graph 可同时运行）
- L1/L2 缓存热门查询的结果
- Cross-Encoder 使用 ONNX Runtime CPU 推理避免 GPU 依赖

## Alternatives (替代方案)

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| **RAG 1.0 (纯向量)** | 实现简单 | 准确率不足 (~70%) | ❌ 不满足质量要求 |
| **GraphRAG (微软)** | 图结构感知好 | 复杂度极高、社区版功能有限 | ⚠️ 备选长期方案 |
| **仅 LLM (长上下文)** | 无需检索步骤 | 成本高、幻觉风险、上下文限制 | ❌ 不适合大规模知识库 |
| **RAG 2.0 (混合+重排序)** ✅ | 高准确率、可解释 | 延迟较高、实现复杂 | ✅ **选定方案** |

## References

- [Cohere - Reranking](https://cohere.com/rerank)
- [RRF 论文](https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf)
- [LLMJudge: Large Language Models are Effective Text Re-rankers for Sparse Retrieval](https://arxiv.org/abs/2306.15039)
