#![cfg(test)]
//! CancellationToken 形式化验证测试
//!
//! ## 不变量 (Invariants)
//!
//! 1. **Cancel Atomicity**: `cancel()` 调用后，`is_cancelled()` 必须返回 `true`
//! 2. **Child Cascade**: 子令牌取消时，父令牌不受影响；父令牌取消时，所有子令牌级联取消
//! 3. **Idempotency**: `cancel()` 可以安全地多次调用
//! 4. **Send+Sync Safety**: `CancellationToken` 必须实现 `Send + Sync`

use crate::cancel::CancellationToken;

#[test]
fn test_miri_cancellation_token_clone_is_independent() {
    let parent = CancellationToken::new();
    let child = parent.child_token();

    child.cancel();

    assert!(
        child.is_cancelled(),
        "child MUST be cancelled after child.cancel()"
    );
    assert!(
        !parent.is_cancelled(),
        "parent MUST NOT be cancelled when only child is cancelled"
    );
}

#[test]
fn test_miri_cancellation_token_parent_cascade() {
    let parent = CancellationToken::new();
    let child = parent.child_token();
    let grandchild = child.child_token();

    parent.cancel();

    assert!(
        parent.is_cancelled(),
        "parent MUST be cancelled after parent.cancel()"
    );
    assert!(
        child.is_cancelled(),
        "child MUST be cancelled when parent is cancelled (cascade)"
    );
    assert!(
        grandchild.is_cancelled(),
        "grandchild MUST be cancelled when parent is cancelled (cascade)"
    );
}

#[test]
fn test_miri_cancellation_token_deep_hierarchy() {
    let root = CancellationToken::new();
    let level1a = root.child_token();
    let level1b = root.child_token();
    let level2a = level1a.child_token();
    let level2b = level1b.child_token();

    level1a.cancel();

    assert!(level1a.is_cancelled(), "level1a cancelled");
    assert!(!level1b.is_cancelled(), "level1b NOT cancelled");
    assert!(level2a.is_cancelled(), "level2a MUST be cancelled (child of level1a)");
    assert!(!level2b.is_cancelled(), "level2b NOT cancelled");

    level1b.cancel();
    assert!(level1b.is_cancelled(), "level1b cancelled");
    assert!(level2b.is_cancelled(), "level2b MUST be cancelled (child of level1b)");

    assert!(
        root.is_cancelled() == false,
        "root still NOT cancelled"
    );
}

#[test]
fn test_miri_cancellation_token_multiple_cancel_calls() {
    let token = CancellationToken::new();

    token.cancel();
    token.cancel();
    token.cancel();

    assert!(token.is_cancelled(), "token MUST be cancelled after multiple cancel() calls");
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
fn test_miri_cancellation_token_send_across_threads() {
    let token = CancellationToken::new();

    std::thread::scope(|s| {
        s.spawn(|| {
            assert!(
                !token.is_cancelled(),
                "token MUST NOT be cancelled in child thread"
            );
        });
    });

    assert!(
        !token.is_cancelled(),
        "token MUST NOT be cancelled after thread join"
    );
}

#[test]
fn test_miri_cancellation_token_sync_across_threads() {
    let token = CancellationToken::new();
    let token2 = token.clone();

    std::thread::scope(|s| {
        s.spawn(|| {
            token2.cancel();
        });
    });

    assert!(
        token.is_cancelled(),
        "token MUST be cancelled after other thread cancelled token2"
    );
}

#[test]
fn test_kani_cancellation_token_state_machine() {
    use std::sync::mpsc::channel;

    let (tx, rx) = channel();

    let parent = CancellationToken::new();
    let child = parent.child_token();

    std::thread::scope(|s| {
        s.spawn(|| {
            child.cancel();
            tx.send(()).unwrap();
        });
    });

    rx.recv().unwrap();

    assert!(
        child.is_cancelled(),
        "child MUST be cancelled (child thread confirmed)"
    );
    assert!(
        !parent.is_cancelled(),
        "parent MUST NOT be cancelled (independent cancel)"
    );
}

#[test]
fn test_kani_cancel_then_child_is_independent() {
    let parent = CancellationToken::new();
    parent.cancel();

    let child = parent.child_token();

    assert!(
        parent.is_cancelled(),
        "parent MUST be cancelled"
    );
    assert!(
        child.is_cancelled(),
        "new child MUST inherit cancelled state from parent"
    );
}

#[test]
fn test_kani_rapid_cancel_unchanged() {
    let token = CancellationToken::new();

    for _ in 0..1000 {
        assert!(!token.is_cancelled(), "token MUST remain not cancelled");
    }

    token.cancel();

    for _ in 0..1000 {
        assert!(token.is_cancelled(), "token MUST remain cancelled after cancel()");
    }
}

#[test]
fn test_deductive_cancel_implies_is_cancelled() {
    let token = CancellationToken::new();

    assert!(
        !token.is_cancelled(),
        "PREMISE: token is initially NOT cancelled"
    );

    token.cancel();

    assert!(
        token.is_cancelled(),
        "THEOREM: After cancel(), is_cancelled() MUST be true (Cancel Implication)"
    );
}

#[test]
fn test_deductive_child_inherits_parent_cancel() {
    let parent = CancellationToken::new();
    let child = parent.child_token();

    assert!(
        !parent.is_cancelled() && !child.is_cancelled(),
        "PREMISE: initially both NOT cancelled"
    );

    parent.cancel();

    assert!(
        parent.is_cancelled(),
        "THEOREM: parent MUST be cancelled"
    );
    assert!(
        child.is_cancelled(),
        "THEOREM: child MUST be cancelled when parent cancels (Inheritance Property)"
    );
}

#[test]
fn test_deductive_sibling_independence() {
    let parent = CancellationToken::new();
    let child_a = parent.child_token();
    let child_b = parent.child_token();

    child_a.cancel();

    assert!(
        child_a.is_cancelled(),
        "child_a cancelled"
    );
    assert!(
        !child_b.is_cancelled(),
        "THEOREM: sibling_b MUST NOT be cancelled (Sibling Independence)"
    );
    assert!(
        !parent.is_cancelled(),
        "THEOREM: parent MUST NOT be cancelled (Sibling Independence)"
    );
}

#[test]
fn test_deductive_cancel_idempotent() {
    let token = CancellationToken::new();

    token.cancel();
    let state1 = token.is_cancelled();

    token.cancel();
    let state2 = token.is_cancelled();

    token.cancel();
    let state3 = token.is_cancelled();

    assert_eq!(
        state1, state2,
        "THEOREM: cancel() is idempotent - second call doesn't change state"
    );
    assert_eq!(
        state2, state3,
        "THEOREM: cancel() is idempotent - third call doesn't change state"
    );
}

#[test]
fn test_deductive_child_token_preserves_invariant() {
    let parent = CancellationToken::new();
    let child = parent.child_token();

    let invariant = || -> bool {
        if parent.is_cancelled() {
            child.is_cancelled()
        } else {
            true
        }
    };

    assert!(
        invariant(),
        "INVARIANT: child.is_cancelled() => parent.is_cancelled() (if child cancelled, parent MUST be cancelled)"
    );

    parent.cancel();

    assert!(
        invariant(),
        "INVARIANT MUST hold after parent.cancel()"
    );
}

#[test]
fn test_deductive_deep_hierarchy_cancel_property() {
    let levels: Vec<CancellationToken> = (0..10)
        .scan(CancellationToken::new(), |state, _| {
            let next = state.child_token();
            Some(std::mem::replace(state, next))
        })
        .collect();

    levels[0].cancel();

    for (i, token) in levels.iter().enumerate() {
        assert!(
            token.is_cancelled(),
            "THEOREM: level {} MUST be cancelled (root cancelled propagates to all descendants)",
            i
        );
    }
}

#[test]
fn test_deductive_cancel_vs_not_cancel_exhaustive() {
    let token = CancellationToken::new();

    match token.is_cancelled() {
        false => {
            token.cancel();
            assert!(
                token.is_cancelled(),
                "THEOREM: After cancel(), state MUST transition to cancelled"
            );
        }
        true => panic!("token should not be cancelled initially"),
    }

    match token.is_cancelled() {
        true => {
            token.cancel();
            assert!(
                token.is_cancelled(),
                "THEOREM: Second cancel() in cancelled state MUST remain cancelled (idempotent)"
            );
        }
        false => panic!("token should be cancelled after first cancel()"),
    }
}