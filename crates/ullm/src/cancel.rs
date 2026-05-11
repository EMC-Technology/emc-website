use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

/// 协作式请求取消令牌，支持无限层级父子级联取消
#[derive(Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    waker: Arc<Notify>,
    parent: Option<Arc<ParentState>>,
}

struct ParentState {
    cancelled: Arc<AtomicBool>,
    waker: Arc<Notify>,
    parent: Option<Arc<ParentState>>,
}

impl ParentState {
    fn is_cancelled(&self) -> bool {
        if self.cancelled.load(Ordering::SeqCst) {
            return true;
        }
        if let Some(ref parent) = self.parent {
            return parent.is_cancelled();
        }
        false
    }
}

impl CancellationToken {
    /// 创建未被取消的令牌
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            waker: Arc::new(Notify::new()),
            parent: None,
        }
    }

    /// 标记令牌为已取消，并唤醒所有等待者
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.waker.notify_waiters();
    }

    /// 检查令牌是否已被取消（递归检查整个父链）
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        if self.cancelled.load(Ordering::SeqCst) {
            return true;
        }
        if let Some(ref parent) = self.parent {
            return parent.is_cancelled();
        }
        false
    }

    /// 异步等待令牌被取消
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        tokio::select! {
            () = self.waker.notified() => (),
            () = async {
                if let Some(ref parent) = self.parent {
                    parent.waker.notified().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => (),
        }
    }

    /// 创建子令牌：父取消时子也取消，子取消不影响父
    #[must_use]
    pub fn child_token(&self) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            waker: Arc::new(Notify::new()),
            parent: Some(Arc::new(ParentState {
                cancelled: Arc::clone(&self.cancelled),
                waker: Arc::clone(&self.waker),
                parent: self.parent.clone(),
            })),
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CancellationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationToken")
            .field("is_cancelled", &self.is_cancelled())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancellation_token_new() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_cancellation_token_cancel() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_cancellation_token_child() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        assert!(!child.is_cancelled());
        parent.cancel();
        assert!(parent.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[test]
    fn test_cancellation_token_child_independent_cancel() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        child.cancel();
        assert!(child.is_cancelled());
        assert!(!parent.is_cancelled());
    }

    #[test]
    fn test_cancellation_token_default() {
        let token = CancellationToken::default();
        assert!(!token.is_cancelled());
    }

    #[tokio::test]
    async fn test_cancellation_token_cancelled_future() {
        let token = CancellationToken::new();
        token.cancel();
        token.cancelled().await;
    }

    #[test]
    fn test_cancellation_token_debug() {
        let token = CancellationToken::new();
        let debug_str = format!("{token:?}");
        assert!(debug_str.contains("is_cancelled"));
    }

    #[test]
    fn test_miri_cancellation_token_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<CancellationToken>();
    }

    #[test]
    fn test_miri_cancellation_token_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CancellationToken>();
    }

    #[test]
    fn test_miri_cancellation_token_send_across_thread() {
        let token = CancellationToken::new();
        std::thread::scope(|s| {
            s.spawn(move || {
                assert!(!token.is_cancelled());
            });
        });
    }

    #[test]
    fn test_deductive_cancelled_token_always_reports_cancelled() {
        let token = CancellationToken::new();
        token.cancel();
        for _ in 0..100 {
            assert!(token.is_cancelled());
        }
    }

    #[test]
    fn test_deductive_child_cascade_cancel_invariant() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        parent.cancel();
        assert!(child.is_cancelled());
    }

    #[test]
    fn test_deductive_grandchild_cascade_cancel_invariant() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        let grandchild = child.child_token();
        parent.cancel();
        assert!(child.is_cancelled());
        assert!(grandchild.is_cancelled());
    }

    #[test]
    fn test_deductive_great_grandchild_cascade_cancel_invariant() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        let grandchild = child.child_token();
        let great_grandchild = grandchild.child_token();
        parent.cancel();
        assert!(child.is_cancelled());
        assert!(grandchild.is_cancelled());
        assert!(great_grandchild.is_cancelled());
    }

    #[test]
    fn test_deductive_child_cancel_does_not_affect_parent() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        child.cancel();
        assert!(child.is_cancelled());
        assert!(!parent.is_cancelled());
    }

    #[test]
    fn test_deductive_sibling_independence_invariant() {
        let parent = CancellationToken::new();
        let child_a = parent.child_token();
        let child_b = parent.child_token();
        child_a.cancel();
        assert!(child_a.is_cancelled());
        assert!(!child_b.is_cancelled());
        assert!(!parent.is_cancelled());
    }

    #[test]
    fn test_deductive_multiple_cancel_idempotent() {
        let token = CancellationToken::new();
        token.cancel();
        token.cancel();
        token.cancel();
        assert!(token.is_cancelled());
    }
}
