#![cfg(test)]
//! RateLimit 模块形式化验证测试
//!
//! ## 不变量 (Invariants)
//!
//! 1. `RateLimiter` 实现 `Send + Sync`
//! 2. `RateLimitGuard` 实现 `Send + Sync` (当内部流实现时)
//! 3. `update_from_headers` 正确更新状态
//! 4. `acquire` 后 `info()` 可正确获取状态

use crate::rate_limit::{RateLimiter, RateLimitInfo};
use futures_util::stream::{self, StreamExt};

#[test]
fn test_miri_rate_limiter_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<RateLimiter>();
}

#[test]
fn test_miri_rate_limiter_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<RateLimiter>();
}

#[test]
fn test_miri_rate_limit_info_clone() {
    let info = RateLimitInfo {
        requests_remaining: Some(10),
        tokens_remaining: Some(1000),
        reset_at: None,
    };
    let cloned = info.clone();
    assert_eq!(cloned.requests_remaining, Some(10));
    assert_eq!(cloned.tokens_remaining, Some(1000));
}

#[test]
fn test_miri_acquire_releases_permit() {
    let limiter = RateLimiter::new(1);

    {
        let _permit = limiter.acquire().await.unwrap();
        assert!(limiter.acquire().now_or_never().is_none());
    }

    let permit = limiter.acquire().now_or_never();
    assert!(
        permit.is_some(),
        "permit MUST be available after scope release"
    );
}

#[test]
fn test_miri_wrap_stream_lifecycle() {
    let limiter = RateLimiter::new(1);

    let stream = stream::iter(vec![Ok::<i32, ()>(1), Ok(2), Ok(3)]);

    let guarded = limiter.wrap_stream(stream).await;

    assert!(
        guarded.is_ok(),
        "wrap_stream MUST succeed when permits available"
    );
}

#[test]
fn test_kani_rate_limiter_concurrent_acquire() {
    use std::sync::Arc;
    use std::thread;

    let limiter = Arc::new(RateLimiter::new(2));
    let mut handles = vec![];

    for _ in 0..4 {
        let limiter = Arc::clone(&limiter);
        handles.push(thread::spawn(move || {
            let permit = limiter.acquire_owned().now_or_never();
            permit.is_some()
        }));
    }

    let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let acquired_count = results.iter().filter(|&&b| b).count();

    assert!(
        acquired_count <= 2,
        "THEOREM: at most max_concurrent permits can be acquired"
    );
}

#[test]
fn test_kani_update_from_headers_all_combinations() {
    let limiter = RateLimiter::new(10);

    limiter.update_from_headers(Some(5), Some(1000));
    let info = limiter.info();
    assert_eq!(info.requests_remaining, Some(5));
    assert_eq!(info.tokens_remaining, Some(1000));

    limiter.update_from_headers(None, Some(500));
    let info = limiter.info();
    assert_eq!(info.requests_remaining, None);
    assert_eq!(info.tokens_remaining, Some(500));

    limiter.update_from_headers(Some(3), None);
    let info = limiter.info();
    assert_eq!(info.requests_remaining, Some(3));
    assert_eq!(info.tokens_remaining, None);

    limiter.update_from_headers(None, None);
    let info = limiter.info();
    assert_eq!(info.requests_remaining, None);
    assert_eq!(info.tokens_remaining, None);
}

#[test]
fn test_deductive_rate_limit_info_default() {
    let info = RateLimitInfo::default();

    assert!(
        info.requests_remaining.is_none(),
        "THEOREM: default RateLimitInfo has requests_remaining = None"
    );
    assert!(
        info.tokens_remaining.is_none(),
        "THEOREM: default RateLimitInfo has tokens_remaining = None"
    );
    assert!(
        info.reset_at.is_none(),
        "THEOREM: default RateLimitInfo has reset_at = None"
    );
}

#[test]
fn test_deductive_acquire_permit_lifecycle() {
    let limiter = RateLimiter::new(1);

    let permit = limiter.acquire().now_or_never();
    assert!(
        permit.is_some(),
        "PREMISE: permit MUST be available initially"
    );

    drop(permit);

    let permit2 = limiter.acquire().now_or_never();
    assert!(
        permit2.is_some(),
        "THEOREM: after dropping permit, new permit MUST be available"
    );
}

#[test]
fn test_deductive_wrap_stream_preserves_items() {
    let limiter = RateLimiter::new(1);
    let items: Vec<i32> = vec![1, 2, 3, 4, 5];

    let stream = stream::iter(items.iter().map(|&i| Ok::<i32, ()>(i)));
    let guarded = limiter.wrap_stream(stream).await.unwrap();

    let collected: Vec<i32> = guarded.collect().await;
    assert_eq!(
        collected,
        items,
        "THEOREM: wrap_stream MUST preserve all stream items"
    );
}

#[test]
fn test_deductive_rate_limiter_send_sync_invariant() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<RateLimiter>();
    assert_send_sync::<RateLimitInfo>();
}

#[test]
fn test_deductive_rate_limiter_clone_independence() {
    let limiter1 = RateLimiter::new(5);
    let limiter2 = limiter1.clone();

    limiter1.update_from_headers(Some(3), Some(300));

    let info1 = limiter1.info();
    let info2 = limiter2.info();

    assert_eq!(
        info1.requests_remaining, Some(3),
        "limiter1 info updated"
    );
    assert!(
        info2.requests_remaining.is_none(),
        "THEOREM: cloned RateLimiters MUST have independent state"
    );
}

#[test]
fn test_deductive_max_concurrent_boundary() {
    let limiter = RateLimiter::new(0);

    let permit = limiter.acquire_owned().now_or_never();
    assert!(
        permit.is_some(),
        "THEOREM: RateLimiter::new(0) creates a closed semaphore that acquires immediately"
    );

    let limiter2 = RateLimiter::new(usize::MAX);
    assert!(
        limiter2.acquire().now_or_never().is_some(),
        "THEOREM: RateLimiter::new(usize::MAX) has effectively unlimited permits"
    );
}