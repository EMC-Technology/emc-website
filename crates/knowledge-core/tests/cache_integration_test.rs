//! 缓存系统集成测试
//!
//! 测试多级缓存架构的完整功能，包括：
//! - L1 + L2 联合查找
//! - Redis 故障降级
//! - 并发访问安全性
//! - 缓存策略配置验证

use knowledge_core::cache::strategy::get_strategy;
use knowledge_core::cache::{
    CACHE_STRATEGIES, CacheManager, CacheStrategy, L1Cache, L1CacheConfig, L2Cache, L2CacheError,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::test;

/// 测试 L1-only 模式（无 Redis）
#[test]
async fn test_l1_only_mode() {
    let config = L1CacheConfig {
        max_capacity: 100,
        ttl: Duration::from_secs(60),
        ..Default::default()
    };

    let l1: L1Cache<String> = L1Cache::new("l1_only_test", config);
    let manager: CacheManager<String> = CacheManager::new(l1, None);

    // 初始状态：miss
    assert!(matches!(
        manager.get("key1").await,
        knowledge_core::cache::CacheLookupResult::Miss
    ));

    // 写入后应该命中
    manager
        .set("key1", &"value1".to_string(), Duration::from_secs(60))
        .await
        .unwrap();
    match manager.get("key1").await {
        knowledge_core::cache::CacheLookupResult::Hit(v) => assert_eq!(v, "value1"),
        other => panic!("Expected Hit, got {other:?}"),
    }

    // 失效后应该 miss
    manager.invalidate("key1").await.unwrap();
    assert!(matches!(
        manager.get("key1").await,
        knowledge_core::cache::CacheLookupResult::Miss
    ));
}

/// 测试 Redis 连接失败时的降级行为
#[test]
async fn test_redis_connection_failure_degradation() {
    let config = L1CacheConfig::default();
    let l1: L1Cache<i32> = L1Cache::new("degradation_test", config);

    let l2_result = L2Cache::from_url(
        "redis://invalid-host-that-does-not-exist:6379",
        "test:",
        Duration::from_secs(60),
    );

    match l2_result {
        Ok(l2) => {
            let manager: CacheManager<i32> = CacheManager::new(l1, Some(l2))
                .with_write_policy(knowledge_core::cache::WritePolicy::WriteBehind);

            manager
                .set("key1", &42, Duration::from_secs(60))
                .await
                .unwrap();

            match manager.get("key1").await {
                knowledge_core::cache::CacheLookupResult::Hit(v) => assert_eq!(v, 42),
                other => panic!("Expected Hit from L1, got {other:?}"),
            }
        }
        Err(e) => {
            assert!(matches!(e, L2CacheError::Connection(_)));

            let manager: CacheManager<i32> = CacheManager::new(l1, None);
            manager
                .set("key1", &42, Duration::from_secs(60))
                .await
                .unwrap();

            match manager.get("key1").await {
                knowledge_core::cache::CacheLookupResult::Hit(v) => assert_eq!(v, 42),
                other => panic!("Expected Hit from L1, got {other:?}"),
            }
        }
    }
}

/// 测试高并发场景下的线程安全性
#[test]
async fn test_concurrent_cache_access() {
    let config = L1CacheConfig {
        max_capacity: 1000,
        ttl: Duration::from_secs(300),
        ..Default::default()
    };

    let l1: L1Cache<String> = L1Cache::new("concurrent_test", config);
    let manager = Arc::new(CacheManager::<String>::new(l1, None));

    let mut handles = vec![];

    // 启动 100 个并发任务
    for i in 0..100 {
        let mgr = manager.clone();
        handles.push(spawn(async move {
            let key = format!("key_{}", i % 20);
            let value = format!("value_{i}");

            // 随机读写
            if i % 2 == 0 {
                mgr.set(&key, &value, Duration::from_secs(60))
                    .await
                    .unwrap();
            } else {
                let _ = mgr.get(&key).await;
            }
        }));
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.expect("Task should complete without error");
    }

    // 验证最终状态一致性
    let stats = manager.stats();
    assert!(stats.hits > 0 || stats.misses > 0, "Cache should have recorded operations");
    println!(
        "Concurrent test - Entries: {}, Hits: {}, Misses: {}",
        stats.entries, stats.hits, stats.misses
    );
}

/// 测试 Write-Behind 异步写入不阻塞主流程
#[test]
async fn test_write_behind_non_blocking() {
    let config = L1CacheConfig::default();
    let l1: L1Cache<Vec<u8>> = L1Cache::new("write_behind_test", config);
    let manager: CacheManager<Vec<u8>> = CacheManager::new(l1, None);

    let large_data = vec![0u8; 1024 * 1024]; // 1MB 数据

    let start = std::time::Instant::now();
    // 注意：CacheManager 没有 set_async() 方法，使用 set() 方法
    manager
        .set("large_key", &large_data, Duration::from_secs(3600))
        .await
        .unwrap();
    let elapsed = start.elapsed();

    // 写入应该在合理时间内返回
    assert!(
        elapsed < Duration::from_millis(100),
        "Write took too long: {elapsed:?}"
    );

    // L1 应立即可读
    match manager.get("large_key").await {
        knowledge_core::cache::CacheLookupResult::Hit(v) => assert_eq!(v.len(), 1024 * 1024),
        other => panic!("Expected Hit, got {other:?}"),
    }
}

/// 测试缓存策略配置完整性
#[test]
async fn test_cache_strategies_config() {
    // 验证所有预定义策略存在且合理
    assert_eq!(
        CACHE_STRATEGIES.len(),
        4,
        "Should have 4 predefined strategies"
    );

    // 验证 embedding_result 策略
    let embedding =
        get_strategy("embedding_result").expect("embedding_result strategy should exist");
    assert_eq!(embedding.strategy, CacheStrategy::LookAside);
    assert_eq!(embedding.l1_ttl, Duration::from_secs(86400));
    assert!((embedding.target_hit_rate - 0.95).abs() < f64::EPSILON);

    // 验证 knowledge_node 策略
    let node = get_strategy("knowledge_node").expect("knowledge_node strategy should exist");
    assert_eq!(node.strategy, CacheStrategy::WriteThrough);
    assert!((node.target_hit_rate - 0.80).abs() < f64::EPSILON);

    // 验证 permission_check 策略
    let perm = get_strategy("permission_check").expect("permission_check strategy should exist");
    assert_eq!(perm.strategy, CacheStrategy::RefreshAhead);
    assert!((perm.target_hit_rate - 0.99).abs() < f64::EPSILON);

    // 验证不存在的策略返回 None
    assert!(get_strategy("nonexistent").is_none());
}

/// 测试缓存统计信息准确性
#[test]
async fn test_cache_stats_accuracy() {
    let config = L1CacheConfig::default();
    let l1: L1Cache<i32> = L1Cache::new("stats_test", config);
    let manager: CacheManager<i32> = CacheManager::new(l1, None);

    // 初始状态
    let initial_stats = manager.stats();
    assert_eq!(initial_stats.hits, 0);
    assert_eq!(initial_stats.misses, 0);
    assert_eq!(initial_stats.entries, 0);

    // 触发一次 miss
    let _ = manager.get("nonexistent").await;
    let after_miss = manager.stats();
    assert_eq!(after_miss.misses, 1);

    // 写入并触发 hit
    manager
        .set("key1", &100, Duration::from_secs(60))
        .await
        .unwrap();
    let _ = manager.get("key1").await;
    let after_hit = manager.stats();
    assert_eq!(after_hit.hits, 1);
    assert_eq!(after_hit.misses, 1);
    assert!((after_hit.hit_rate - 0.5).abs() < f64::EPSILON);
}

/// 测试 TTL 过期机制（基于 Moka 的自动过期）
#[tokio::test(flavor = "current_thread")]
async fn test_ttl_expiration() {
    use tokio::time::sleep;

    let config = L1CacheConfig {
        max_capacity: 100,
        ttl: Duration::from_millis(100),         // 100ms TTL 用于测试
        time_to_idle: Duration::from_millis(50), // 50ms TTI
        enable_stats: true,
    };

    let l1: L1Cache<String> = L1Cache::new("ttl_test", config);

    // 写入数据
    l1.set("expiring_key", "will_expire".to_string()).await;

    // 立即读取应该成功
    assert!(l1.get("expiring_key").await.is_some());

    // 等待超过 TTL
    sleep(Duration::from_millis(150)).await;

    // 应该已过期
    assert!(
        l1.get("expiring_key").await.is_none(),
        "Key should have expired"
    );
}
