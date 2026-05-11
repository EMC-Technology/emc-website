use std::sync::Arc;
use std::time::Instant;

use futures_util::Stream;
use parking_lot::RwLock;
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, SemaphorePermit};

use crate::error::LlmError;

/// 并发速率限制器，基于信号量控制同时进行的请求数
#[derive(Clone)]
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    info: Arc<RwLock<RateLimitInfo>>,
}

/// 速率限制状态信息
#[derive(Debug, Clone, Default)]
pub struct RateLimitInfo {
    /// 剩余请求数
    pub requests_remaining: Option<u32>,
    /// 剩余 Token 数
    pub tokens_remaining: Option<u64>,
    /// 限制重置时间
    pub reset_at: Option<Instant>,
}

/// 速率限制守卫，在流的生命周期内持有信号量许可
#[pin_project]
pub struct RateLimitGuard<T> {
    #[pin]
    inner: T,
    _permit: OwnedSemaphorePermit,
}

impl<T: Stream> Stream for RateLimitGuard<T> {
    type Item = T::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}

impl RateLimiter {
    /// 创建指定最大并发数的速率限制器
    #[must_use]
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            info: Arc::new(RwLock::new(RateLimitInfo::default())),
        }
    }

    /// 获取一个信号量许可。
    ///
    /// # Errors
    ///
    /// 当速率限制器已关闭时返回 `LlmError`。
    pub async fn acquire(&self) -> Result<SemaphorePermit<'_>, LlmError> {
        self.semaphore
            .acquire()
            .await
            .map_err(|_| LlmError::Other("rate limiter closed".into()))
    }

    /// 获取一个可跨线程移动的信号量许可。
    ///
    /// # Errors
    ///
    /// 当速率限制器已关闭时返回 `LlmError`。
    pub async fn acquire_owned(&self) -> Result<OwnedSemaphorePermit, LlmError> {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| LlmError::Other("rate limiter closed".into()))
    }

    /// 将流包装在速率限制守卫中，确保流的生命周期内持有许可。
    ///
    /// # Errors
    ///
    /// 当获取信号量许可失败（速率限制器已关闭）时返回 `LlmError`。
    pub async fn wrap_stream<T: Stream>(&self, stream: T) -> Result<RateLimitGuard<T>, LlmError> {
        let permit = self.acquire_owned().await?;
        Ok(RateLimitGuard {
            inner: stream,
            _permit: permit,
        })
    }

    /// 从 HTTP 响应头更新速率限制状态
    pub fn update_from_headers(
        &self,
        remaining_requests: Option<u32>,
        remaining_tokens: Option<u64>,
    ) {
        let mut info = self.info.write();
        info.requests_remaining = remaining_requests;
        info.tokens_remaining = remaining_tokens;
    }

    /// 获取当前速率限制状态快照
    #[must_use]
    pub fn info(&self) -> RateLimitInfo {
        self.info.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream::{self, StreamExt};

    #[test]
    fn test_rate_limit_info_default() {
        let info = RateLimitInfo::default();
        assert!(info.requests_remaining.is_none());
        assert!(info.tokens_remaining.is_none());
        assert!(info.reset_at.is_none());
    }

    #[tokio::test]
    async fn test_acquire_permits() {
        let limiter = RateLimiter::new(2);
        let _p1 = limiter.acquire().await.unwrap();
        let _p2 = limiter.acquire().await.unwrap();
    }

    #[tokio::test]
    async fn test_acquire_owned() {
        let limiter = RateLimiter::new(2);
        let _p1 = limiter.acquire_owned().await.unwrap();
        let _p2 = limiter.acquire_owned().await.unwrap();
    }

    #[test]
    fn test_update_from_headers() {
        let limiter = RateLimiter::new(10);
        limiter.update_from_headers(Some(5), Some(1000));
        let info = limiter.info();
        assert_eq!(info.requests_remaining, Some(5));
        assert_eq!(info.tokens_remaining, Some(1000));
    }

    #[tokio::test]
    async fn test_wrap_stream() {
        let limiter = RateLimiter::new(1);
        let stream = stream::iter(vec![Ok::<i32, ()>(1), Ok(2), Ok(3)]);
        let guarded = limiter.wrap_stream(stream).await.unwrap();
        let collected: Vec<_> = guarded.collect::<Vec<_>>().await;
        assert_eq!(collected.len(), 3);
    }

    #[test]
    fn test_rate_limit_info_clone() {
        let info = RateLimitInfo {
            requests_remaining: Some(5),
            tokens_remaining: Some(1000),
            reset_at: None,
        };
        let cloned = info.clone();
        assert_eq!(cloned.requests_remaining, Some(5));
        assert_eq!(cloned.tokens_remaining, Some(1000));
    }

    #[test]
    fn test_update_from_headers_none() {
        let limiter = RateLimiter::new(10);
        limiter.update_from_headers(None, None);
        let info = limiter.info();
        assert!(info.requests_remaining.is_none());
        assert!(info.tokens_remaining.is_none());
    }

    #[test]
    fn test_update_from_headers_partial() {
        let limiter = RateLimiter::new(10);
        limiter.update_from_headers(Some(5), None);
        let info = limiter.info();
        assert_eq!(info.requests_remaining, Some(5));
        assert!(info.tokens_remaining.is_none());
    }

    #[tokio::test]
    async fn test_acquire_and_release() {
        let limiter = RateLimiter::new(1);
        {
            let _permit = limiter.acquire().await.unwrap();
        }
        let _permit2 = limiter.acquire().await.unwrap();
    }
}
