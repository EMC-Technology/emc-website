//! RAG 流水线使用示例
//!
//! 展示如何使用 RAG (Retrieval-Augmented Generation) 引擎进行
//! 端到端的知识问答，包括混合召回、多阶段重排序和流式输出。
//!
//! 运行方式:
//!   cargo run --example rag_pipeline --package knowledge-api

use futures::StreamExt;
use knowledge_api::rag::{RagEngine, RagQuery, RagResult};
use knowledge_core::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,knowledge_api::rag=debug")
        .init();

    println!("🧠 文本全结构化知识系统 - RAG 流水线示例\n");
    println!("═".repeat(60));

    // =========================================================================
    // 1. 初始化 RAG 引擎
    // =========================================================================
    println!("\n📌 步骤 1: 初始化 RAG 引擎");

    let rag_engine = RagEngine::builder()
        .max_candidates(50)
        .rerank_top_k(10)
        .final_top_k(5)
        .enable_streaming(true)
        .build()?;

    println!("   RAG 配置:");
    println!("   ├─ 最大候选数 (召回): {}", rag_engine.config().max_candidates);
    println!("   ├─ 重排序 Top-K: {}", rag_engine.config().rerank_top_k);
    println!("   └─ 最终返回数: {}", rag_engine.config().final_top_k);

    // =========================================================================
    // 2. 构建用户查询
    // =========================================================================
    println!("\n📌 步骤 2: 构建查询");

    let user_queries = [
        "Rust 中 Async Trait 是什么？如何实现？",
        "SurrealDB 的关系表(RELATION)如何定义图边？",
        "CQRS 模式中 Event Sourcing 的优势是什么？",
    ];

    for (i, query_text) in user_queries.iter().enumerate() {
        println!("\n   ┌─ 查询 #{}: \"{}\"", i + 1, query_text);

        // =========================================================================
        // 3. RAG Pipeline 执行
        // =========================================================================
        println!("   │");
        println!("   │ 📌 步骤 3: RAG Pipeline 执行");

        let query = RagQuery {
            text: query_text.to_string(),
            filters: None,
            stream: true,
        };

        match rag_engine.execute(query).await {
            Ok(mut stream) => {
                print!("   │ 📡 流式输出: ");
                let mut full_answer = String::new();

                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(RagResult::Token(token)) => {
                            print!("{}", token.content);
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                            full_answer.push_str(&token.content);
                        }
                        Ok(RagResult::Metadata(meta)) => {
                            println!("\n   │");
                            println!("   │ 📊 元数据:");
                            println!("   │   ├─ 召回候选数: {}", meta.retrieved_count);
                            println!("   │   ├─ 重排序后: {}", meta.reranked_count);
                            println!("   │   ├─ 最终返回: {}", meta.final_count);
                            println!("   │   ├─ 总延迟: {:?}", meta.total_latency);
                            println!("   │   └─ 置信度: {:.2}%", meta.confidence * 100.0);
                        }
                        Ok(RagResult::Done) => {
                            println!("\n   │ ✅ 回答完成");
                        }
                        Err(e) => {
                            println!("\n   │ ❌ 错误: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                println!("   │ ❌ RAG 执行错误: {}", e);
            }
        }

        println!("   └");
    }

    // =========================================================================
    // RAG Pipeline 架构说明
    // =========================================================================
    println!("\n{}", "═".repeat(60));
    println!("🏗️  RAG 2.0 Pipeline 架构:");
    println!();
    println!("   用户查询");
    println!("      │");
    println!("      ▼");
    println!("   ┌──────────────────────┐");
    println!("   │  Query Understanding  │ ← 意图识别、查询扩展");
    println!("   └──────────┬───────────┘");
    println!("              ▼");
    println!("   ┌─────────────────────────────────────┐");
    println!("   │         Retrieval Stage (召回)       │");
    println!("   │  ┌───────┐  ┌───────┐  ┌─────────┐ │");
    println!("   │  │ BM25  │  │Vector │  │  Graph  │ │");
    println!("   │  │ 关键词│  │语义   │  │ 图遍历  │ │");
    println!("   │  └───┬───┘  └───┬───┘  └────┬────┘ │");
    println!("   │      └──────────┼──────────┘       │");
    println!("   │                 ▼                    │");
    println!("   │         RRF Fusion (Top-50)         │");
    println!("   └─────────────────┬───────────────────┘");
    println!("                     ▼");
    println!("   ┌─────────────────────────────────────┐");
    println!("   │        Reranking Stage (精排)        │");
    println!("   │  Stage 1: Cross-Encoder  (50→20)    │");
    println!("   │  Stage 2: LLM Judge      (20→Top-K) │");
    println!("   │  Stage 3: Context Compression       │");
    println!("   └─────────────────┬───────────────────┘");
    println!("                     ▼");
    println!("   ┌─────────────────┐");
    println!("   │ LLM Generation  │ ← SSE 流式输出");
    println!("   └─────────────────┘");

    println!("\n✅ RAG 示例完成！");
    println!("\n💡 性能目标:");
    println!("   - RAG 准确率: ≥95% (vs 传统 ~70%)");
    println!("   - 端到端延迟: <2s (含 LLM 调用)");
    println!("   - 首 Token 延迟: <500ms");

    Ok(())
}
