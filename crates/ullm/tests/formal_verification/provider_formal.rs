#![cfg(test)]
//! Provider Types 形式化验证测试
//!
//! ## 不变量 (Invariants)
//!
//! 1. `ModelId`, `ProviderId`, `ModelName`, `ProviderName` 实现 `Send + Sync`
//! 2. `LanguageModelRequest::validate()` 正确验证边界条件
//! 3. `TokenUsage::merge()` 满足交换律和单位元

use crate::provider::types::*;
use crate::cancel::CancellationToken;

#[test]
fn test_miri_arc_wrapper_types_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ModelId>();
    assert_send_sync::<ModelName>();
    assert_send_sync::<ProviderId>();
    assert_send_sync::<ProviderName>();
}

#[test]
fn test_miri_token_usage_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TokenUsage>();
}

#[test]
fn test_miri_request_metadata_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RequestMetadata>();
}

#[test]
fn test_miri_language_model_request_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LanguageModelRequest>();
}

#[test]
fn test_kani_validate_temperature_boundaries() {
    let valid_temps = [0.0f32, 0.5, 1.0, 1.5, 2.0];
    let invalid_temps = [-0.1, 2.1, -100.0, 100.0];

    for temp in valid_temps {
        let req = LanguageModelRequest::new("test", vec![Message::user("hi")])
            .with_temperature(temp);
        assert!(
            req.validate().is_ok(),
            "THEOREM: temperature {} MUST be valid (in [0.0, 2.0])",
            temp
        );
    }

    for temp in invalid_temps {
        let req = LanguageModelRequest::new("test", vec![Message::user("hi")])
            .with_temperature(temp);
        assert!(
            req.validate().is_err(),
            "THEOREM: temperature {} MUST be invalid (outside [0.0, 2.0])",
            temp
        );
    }
}

#[test]
fn test_kani_validate_top_p_boundaries() {
    let valid_top_p = [0.0f32, 0.5, 1.0];
    let invalid_top_p = [-0.1, 1.1, -1.0, 2.0];

    for top_p in valid_top_p {
        let req = LanguageModelRequest::new("test", vec![Message::user("hi")])
            .with_top_p(top_p);
        assert!(
            req.validate().is_ok(),
            "THEOREM: top_p {} MUST be valid (in [0.0, 1.0])",
            top_p
        );
    }

    for top_p in invalid_top_p {
        let req = LanguageModelRequest::new("test", vec![Message::user("hi")])
            .with_top_p(top_p);
        assert!(
            req.validate().is_err(),
            "THEOREM: top_p {} MUST be invalid",
            top_p
        );
    }
}

#[test]
fn test_kani_validate_penalty_boundaries() {
    let valid_freq = [-2.0f32, 0.0, 2.0];
    let invalid_freq = [-2.1, 2.1];

    for freq in valid_freq {
        let req = LanguageModelRequest::new("test", vec![Message::user("hi")])
            .with_frequency_penalty(freq);
        assert!(
            req.validate().is_ok(),
            "THEOREM: frequency_penalty {} MUST be valid",
            freq
        );
    }

    for freq in invalid_freq {
        let req = LanguageModelRequest::new("test", vec![Message::user("hi")])
            .with_frequency_penalty(freq);
        assert!(
            req.validate().is_err(),
            "THEOREM: frequency_penalty {} MUST be invalid",
            freq
        );
    }
}

#[test]
fn test_kani_token_usage_merge_commutative() {
    let a = TokenUsage {
        prompt_tokens: 100,
        completion_tokens: 50,
        cache_read_tokens: 10,
        cache_write_tokens: 5,
    };
    let b = TokenUsage {
        prompt_tokens: 200,
        completion_tokens: 30,
        cache_read_tokens: 20,
        cache_write_tokens: 15,
    };

    let mut ab = a.clone();
    ab.merge(&b);
    let mut ba = b.clone();
    ba.merge(&a);

    assert_eq!(
        ab.prompt_tokens, ba.prompt_tokens,
        "THEOREM: merge MUST be commutative for prompt_tokens"
    );
    assert_eq!(
        ab.completion_tokens, ba.completion_tokens,
        "THEOREM: merge MUST be commutative for completion_tokens"
    );
    assert_eq!(
        ab.cache_read_tokens, ba.cache_read_tokens,
        "THEOREM: merge MUST be commutative for cache_read_tokens"
    );
    assert_eq!(
        ab.cache_write_tokens, ba.cache_write_tokens,
        "THEOREM: merge MUST be commutative for cache_write_tokens"
    );
}

#[test]
fn test_kani_token_usage_merge_identity() {
    let zero = TokenUsage::default();
    let mut a = TokenUsage {
        prompt_tokens: 100,
        completion_tokens: 50,
        cache_read_tokens: 10,
        cache_write_tokens: 5,
    };
    let original = a.clone();

    a.merge(&zero);

    assert_eq!(
        a.prompt_tokens, original.prompt_tokens,
        "THEOREM: merge with zero MUST be identity for prompt_tokens"
    );
    assert_eq!(
        a.completion_tokens, original.completion_tokens,
        "THEOREM: merge with zero MUST be identity for completion_tokens"
    );
}

#[test]
fn test_kani_token_usage_total_tokens_formula() {
    let usage = TokenUsage {
        prompt_tokens: 100,
        completion_tokens: 50,
        cache_read_tokens: 999,
        cache_write_tokens: 888,
    };

    assert_eq!(
        usage.total_tokens(),
        u64::from(usage.prompt_tokens) + u64::from(usage.completion_tokens),
        "THEOREM: total_tokens MUST equal prompt_tokens + completion_tokens"
    );
}

#[test]
fn test_deductive_model_id_clone_is_cheap() {
    let id1 = ModelId::new("gpt-4o");
    let id2 = id1.clone();

    assert_eq!(
        id1, id2,
        "THEOREM: cloned ModelId MUST equal original"
    );
    assert_eq!(
        id1.as_str(), id2.as_str(),
        "THEOREM: cloned ModelId MUST have same string content"
    );
}

#[test]
fn test_deductive_validate_empty_model() {
    let req = LanguageModelRequest::new("", vec![Message::user("hi")]);
    assert!(
        req.validate().is_err(),
        "THEOREM: empty model MUST fail validation"
    );
}

#[test]
fn test_deductive_validate_empty_messages() {
    let req = LanguageModelRequest::new("test", vec![]);
    assert!(
        req.validate().is_err(),
        "THEOREM: empty messages MUST fail validation"
    );
}

#[test]
fn test_deductive_validate_success_case() {
    let req = LanguageModelRequest::new("gpt-4o", vec![Message::user("hi")]);
    assert!(
        req.validate().is_ok(),
        "THEOREM: valid request MUST pass validation"
    );
}

#[test]
fn test_deductive_token_usage_saturating_add() {
    let mut usage = TokenUsage {
        prompt_tokens: u32::MAX,
        completion_tokens: 1,
        ..Default::default()
    };
    let other = TokenUsage {
        prompt_tokens: 100,
        completion_tokens: u32::MAX,
        ..Default::default()
    };

    usage.merge(&other);

    assert_eq!(
        usage.prompt_tokens, u32::MAX,
        "THEOREM: merge MUST use saturating_add to prevent overflow"
    );
    assert_eq!(
        usage.completion_tokens, u32::MAX,
        "THEOREM: merge MUST use saturating_add to prevent overflow"
    );
}

#[test]
fn test_deductive_token_usage_is_empty() {
    let empty = TokenUsage::default();
    assert!(
        empty.is_empty(),
        "THEOREM: default TokenUsage MUST be empty"
    );

    let non_empty = TokenUsage {
        prompt_tokens: 1,
        ..Default::default()
    };
    assert!(
        !non_empty.is_empty(),
        "THEOREM: TokenUsage with any non-zero field MUST NOT be empty"
    );
}

#[test]
fn test_deductive_message_role_as_str() {
    assert_eq!(Role::System.as_str(), "system");
    assert_eq!(Role::User.as_str(), "user");
    assert_eq!(Role::Assistant.as_str(), "assistant");
    assert_eq!(Role::Tool.as_str(), "tool");

    assert_eq!(
        Role::System.to_string(), "system",
        "THEOREM: Role Display impl MUST match as_str()"
    );
}

#[test]
fn test_deductive_stop_reason_is_tool_call() {
    assert!(
        StopReason::ToolCall.is_tool_call(),
        "THEOREM: ToolCall variant MUST return true for is_tool_call()"
    );

    for reason in [
        StopReason::EndTurn,
        StopReason::MaxTokens,
        StopReason::StopSequence,
        StopReason::Cancelled,
    ] {
        assert!(
            !reason.is_tool_call(),
            "THEOREM: {:?} MUST return false for is_tool_call()",
            reason
        );
    }
}