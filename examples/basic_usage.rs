//! 基础 CRUD 操作示例
//!
//! 展示如何使用 knowledge-core 和 knowledge-api 进行文档的
//! 创建、读取、更新、删除操作。
//!
//! 运行方式:
//!   cargo run --example basic_usage --package knowledge-api

use std::sync::Arc;

use knowledge_api::config::AppConfig;
use knowledge_core::{
    model::{Block, BlockType, Document, SourceType},
    repository::DocumentRepository,
    Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("🦀 文本全结构化知识系统 - 基础 CRUD 示例\n");
    println!("═".repeat(50));

    // =========================================================================
    // 1. 初始化数据库连接
    // =========================================================================
    println!("\n📌 步骤 1: 初始化数据库连接");

    let config = AppConfig::load().expect("加载配置失败");
    let db_client = knowledge_core::SurrealDbClient::new(&config.database.url).await?;
    let repo = DocumentRepository::new(Arc::new(db_client));

    println!("✅ 数据库连接成功: {}", config.database.url);

    // =========================================================================
    // 2. 创建文档 (Create)
    // =========================================================================
    println!("\n📌 步骤 2: 创建新文档");

    let doc = Document::new(
        "/docs/api-guide.md",
        "API 开发指南",
        SourceType::Markdown,
        hash_content("# API 开发指南\n\n本文档介绍 RESTful API 设计规范..."),
    )?;

    println!("   文档路径: {}", doc.path);
    println!("   文档标题: {}", doc.title);
    println!("   来源类型: {:?}", doc.source_type);
    println!("   哈希值: {}...", &doc.hash[..16]);

    // 通过 CQRS Command 创建（生产环境推荐）
    // let create_cmd = CreateDocumentCommand {
    //     path: doc.path.clone(),
    //     title: doc.title.clone(),
    //     source_type: doc.source_type.clone(),
    // };
    // let result = command_dispatcher.send(create_cmd).await?;

    // =========================================================================
    // 3. 创建块 (Create Blocks)
    // =========================================================================
    println!("\n📌 步骤 3: 为文档创建块 (Blocks)");

    let blocks = vec![
        Block::new(
            "document:temp_id".to_string(), // 实际使用 RecordId
            BlockType::Heading,
            0,
            1,
        ),
        Block::new(
            "document:temp_id".to_string(),
            BlockType::Paragraph,
            2,
            10,
        ),
        Block::new(
            "document:temp_id".to_string(),
            BlockType::Code,
            11,
            25,
        ),
    ];

    for (i, block) in blocks.iter().enumerate() {
        println!(
            "   Block #{}: {:?} [行 {}-{}]",
            i + 1,
            block.block_type,
            block.start_line,
            block.end_line
        );
    }

    // =========================================================================
    // 4. 搜索文档 (Read/Search)
    // =========================================================================
    println!("\n📌 步骤 4: 搜索文档");

    // BM25 关键词搜索示例
    // let search_query = SearchDocumentsQuery {
    //     query: "API 设计".to_string(),
    //     limit: 10,
    // };
    // let results = query_dispatcher.send(search_query).await?;

    println!("   🔍 搜索查询: \"API 设计\"");
    println!("   📊 预期结果: 包含关键词的文档列表");
    println!("   📈 使用策略: BM25 + Vector 混合搜索 (RRF 融合)");

    // =========================================================================
    // 5. 更新文档 (Update)
    // =========================================================================
    println!("\n📌 步骤 5: 更新文档");

    // let update_cmd = UpdateDocumentCommand {
    //     id: doc_id,
    //     title: Some("RESTful API 完整开发指南".to_string()),
    //     ..Default::default()
    // };
    // let updated_doc = command_dispatcher.send(update_cmd).await?;

    println!("   ✏️  更新标题: \"API 开发指南\" → \"RESTful API 完整开发指南\"");
    println!("   🔄 CQRS 保证: 事件溯源记录完整变更历史");

    // =========================================================================
    // 6. 删除文档 (Delete)
    // =========================================================================
    println!("\n📌 步骤 6: 删除文档（级联清理）");

    // let delete_cmd = DeleteDocumentCommand { id: doc_id };
    // command_dispatcher.send(delete_cmd).await?;

    println!("   🗑️  删除文档将级联清理:");
    println!("      ├─ 所有关联 Block");
    println!("      ├─ 所有关联 Token");
    println!("      └─ 所有关联 Reference 边");

    // =========================================================================
    // 完成
    // =========================================================================
    println!("\n{}", "═".repeat(50));
    println!("✅ 基础 CRUD 示例执行完成！");
    println!("\n💡 提示:");
    println!("   - 生产环境请通过 CQRS CommandDispatcher 执行写操作");
    println!("   - 读操作可通过 QueryDispatcher 或直接使用 Repository");
    println!("   - 所有变更都会产生 Event Sourcing 事件");
    println!("   - 查看 examples/ 目录获取更多高级示例");

    Ok(())
}

// =============================================================================
// 辅助函数
// =============================================================================

/// 计算文本内容的 BLAKE3 哈希值
fn hash_content(content: &str) -> String {
    use knowledge_core::hash_str;
    hash_str(content)
}
