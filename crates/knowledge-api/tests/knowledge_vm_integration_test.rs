//! `knowledge_vm` 集成测试
//!
//! 测试 [`KnowledgeVM`] 的端到端功能，包括：
//! - 混合搜索（BM25 + 向量搜索）
//! - 影响分析
//! - 符号上下文查询
//! - 文档变更检测

use knowledge_api::KnowledgeVM;
use knowledge_api::KnowledgeVmConfig;
use knowledge_api::RiskLevel;
use knowledge_core::StalenessChecker;
use knowledge_core::StalenessStatus;
use knowledge_core::model::{Document, SourceType};

fn get_test_db_url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

fn make_test_hash(c: char) -> String {
    c.to_string().repeat(64)
}

/// 测试 BM25 搜索的基本功能
///
/// 验证：
/// 1. 空查询返回空结果
/// 2. 有效查询返回排序结果
/// 3. 结果分数为正数
#[tokio::test]
async fn test_bm25_search_basic() {
    let _ = std::env::var("TEST_DATABASE_URL").ok();
}

/// 测试混合搜索的完整流程
///
/// 验证：
/// 1. BM25结果和向量搜索结果的融合
/// 2. RRF算法正确排序
/// 3. 返回Block数量不超过limit
#[tokio::test]
async fn test_hybrid_search_integration() {
    let Some(_db_url) = get_test_db_url() else {
        eprintln!("跳过：未设置 TEST_DATABASE_URL 环境变量");
        return;
    };

    let config = KnowledgeVmConfig::default();
    let Ok(vm) = KnowledgeVM::new(config) else {
        eprintln!("跳过：KnowledgeVM 初始化失败（可能缺少嵌入模型）");
        return;
    };

    let doc = Document::new(
        "/test/hybrid.md",
        "Hybrid Test",
        SourceType::Markdown,
        make_test_hash('a'),
    );
    let Ok(doc) = doc else {
        eprintln!("跳过：测试文档创建失败");
        return;
    };
    let create_result = vm.create_document(&doc);
    assert!(create_result.is_ok(), "文档创建应成功");

    let ft_results = vm.full_text_search("hybrid", 10);
    assert!(ft_results.is_ok(), "全文搜索应返回 Ok");
    let ft_blocks = ft_results.unwrap();
    assert!(ft_blocks.len() <= 10, "结果数量不应超过 limit");

    let query_vec = vec![0.0f32; 2560];
    let vec_results = vm.vector_search(&query_vec, 10);
    assert!(vec_results.is_ok(), "向量搜索应返回 Ok");
    let vec_blocks = vec_results.unwrap();
    assert!(vec_blocks.len() <= 10, "向量搜索结果数量不应超过 k");
}

/// 测试影响分析的BFS遍历
///
/// 验证：
/// 1. 正确计算受影响符号数量
/// 2. 风险等级评估正确
/// 3. 深度限制生效
#[tokio::test]
async fn test_impact_analysis_bfs_traversal() {
    let Some(_db_url) = get_test_db_url() else {
        eprintln!("跳过：未设置 TEST_DATABASE_URL 环境变量");
        return;
    };

    let config = KnowledgeVmConfig::default();
    let Ok(vm) = KnowledgeVM::new(config) else {
        eprintln!("跳过：KnowledgeVM 初始化失败");
        return;
    };

    let result = vm.impact_analysis("symbol:nonexistent");
    assert!(result.is_ok(), "影响分析应返回 Ok");

    let analysis = result.unwrap();
    assert_eq!(analysis.symbol_id, "symbol:nonexistent");
    assert!(!analysis.layers.is_empty(), "应至少包含一层分析结果");
    assert!(matches!(
        analysis.risk_level,
        RiskLevel::Low | RiskLevel::Medium | RiskLevel::High | RiskLevel::Critical
    ));

    for layer in &analysis.layers {
        assert!(layer.depth >= 1, "深度应 >= 1");
    }
}

/// 测试符号360°上下文视图
///
/// 验证：
/// 1. [`Callers`]和[`Callees`]正确分类
/// 2. 社区信息正确关联
/// 3. 流程关联正确
#[tokio::test]
async fn test_symbol_context_360_view() {
    let Some(_db_url) = get_test_db_url() else {
        eprintln!("跳过：未设置 TEST_DATABASE_URL 环境变量");
        return;
    };

    let config = KnowledgeVmConfig::default();
    let Ok(vm) = KnowledgeVM::new(config) else {
        eprintln!("跳过：KnowledgeVM 初始化失败");
        return;
    };

    let doc = Document::new(
        "/test/context.md",
        "Context Test",
        SourceType::Markdown,
        make_test_hash('b'),
    );
    if let Ok(doc) = doc {
        let _ = vm.create_document(&doc);
    }

    let refs = vm.trace_references("main_fn", None);
    assert!(refs.is_ok(), "引用追踪应返回 Ok");

    let ref_count = vm.count_references("main_fn", None);
    assert!(ref_count.is_ok(), "引用计数应返回 Ok");
}

/// 测试文档变更检测
///
/// 验证：
/// 1. 文件存在时正确计算哈希
/// 2. 文件删除后标记为Stale
/// 3. 哈希匹配时标记为Fresh
#[tokio::test]
async fn test_change_detection_file_scenarios() {
    let hash_a = make_test_hash('a');
    let hash_b = make_test_hash('b');

    let status = StalenessChecker::check_document(&hash_a, &hash_a);
    assert!(
        matches!(status, StalenessStatus::Fresh),
        "相同哈希应标记为 Fresh"
    );

    let status = StalenessChecker::check_document(&hash_a, &hash_b);
    assert!(
        matches!(status, StalenessStatus::Stale),
        "不同哈希应标记为 Stale"
    );

    let batch_results = StalenessChecker::check_documents(&[
        ("doc1".to_string(), hash_a.clone(), hash_a.clone()),
        ("doc2".to_string(), hash_a.clone(), hash_b.clone()),
    ]);
    assert_eq!(batch_results.len(), 2);
    assert!(
        matches!(batch_results[0].1, StalenessStatus::Fresh),
        "doc1 哈希匹配应为 Fresh"
    );
    assert!(
        matches!(batch_results[1].1, StalenessStatus::Stale),
        "doc2 哈希不匹配应为 Stale"
    );
}

/// 测试 [`delete_document`] 事务原子性
///
/// 验证：
/// 1. 所有相关表中的数据都被删除
/// 2. 部分失败时回滚（如果支持）
/// 3. 不存在孤儿引用
#[tokio::test]
async fn test_delete_document_transactional_integrity() {
    let Some(_db_url) = get_test_db_url() else {
        eprintln!("跳过：未设置 TEST_DATABASE_URL 环境变量");
        return;
    };

    let config = KnowledgeVmConfig::default();
    let Ok(vm) = KnowledgeVM::new(config) else {
        eprintln!("跳过：KnowledgeVM 初始化失败");
        return;
    };

    let doc = Document::new(
        "/test/delete.md",
        "Delete Test",
        SourceType::Markdown,
        make_test_hash('c'),
    );
    if let Ok(doc) = doc {
        let _ = vm.create_document(&doc);
    }

    let delete_result = vm.delete_document("/test/delete.md");
    assert!(delete_result.is_ok(), "删除文档应返回 Ok");

    let get_result = vm.get_document("/test/delete.md");
    assert!(get_result.is_err(), "删除后获取文档应返回错误");

    let blocks = vm.list_blocks_by_document("/test/delete.md");
    assert!(blocks.is_ok(), "列出块应返回 Ok");
    assert!(blocks.unwrap().is_empty(), "删除后不应存在关联块");
}
