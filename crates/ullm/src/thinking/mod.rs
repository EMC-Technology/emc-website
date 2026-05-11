use serde::{Deserialize, Serialize};

use crate::error::LlmError;

/// 思维链（Thinking）配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// 是否启用思维链
    pub enabled: bool,
    /// 思维链 Token 预算
    pub budget_tokens: Option<usize>,
}

impl ThinkingConfig {
    /// 创建带 Token 预算的思考配置。
    ///
    /// # Errors
    ///
    /// 当 `budget_tokens` 为 0 时返回 `LlmError::InvalidRequest`。
    pub fn with_budget(budget_tokens: usize) -> Result<Self, LlmError> {
        if budget_tokens == 0 {
            return Err(LlmError::InvalidRequest {
                message: "thinking budget_tokens must be positive".into(),
            });
        }
        Ok(Self {
            enabled: true,
            budget_tokens: Some(budget_tokens),
        })
    }

    /// 创建禁用思维链的配置
    #[must_use]
    pub fn disabled() -> Self {
        Self::default()
    }
}

/// 判断模型是否为推理模型（支持思维链）
#[must_use]
pub fn is_reasoning_model(model: &str) -> bool {
    let lower = model.to_lowercase();
    lower.starts_with("o1")
        || lower.starts_with("o3")
        || lower.starts_with("o4")
        || lower.contains("grok-3-mini")
        || lower.contains("qwq")
        || lower.contains("qwen-qwq")
        || lower.contains("qwen3")
        || lower.contains("thinking")
        || lower.contains("deepseek-reasoner")
        || lower.contains("glm-z1")
        || lower.contains("glm-5")
        || lower.contains("kimi-k2")
        || lower.contains("-r1")
        || lower.starts_with("r1-")
}

/// 对推理模型清除不支持的采样参数（`temperature`/`top_p`/`frequency_penalty`/`presence_penalty`）
pub fn strip_reasoning_params(
    temperature: &mut Option<f32>,
    top_p: &mut Option<f32>,
    frequency_penalty: &mut Option<f32>,
    presence_penalty: &mut Option<f32>,
    model: &str,
) {
    if is_reasoning_model(model) {
        *temperature = None;
        *top_p = None;
        *frequency_penalty = None;
        *presence_penalty = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_config_default() {
        let config = ThinkingConfig::default();
        assert!(!config.enabled);
        assert!(config.budget_tokens.is_none());
    }

    #[test]
    fn test_thinking_config_with_budget() {
        let config = ThinkingConfig::with_budget(10000).unwrap();
        assert!(config.enabled);
        assert_eq!(config.budget_tokens, Some(10000));
    }

    #[test]
    fn test_thinking_config_with_budget_zero() {
        let result = ThinkingConfig::with_budget(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_thinking_config_disabled() {
        let config = ThinkingConfig::disabled();
        assert!(!config.enabled);
        assert!(config.budget_tokens.is_none());
    }

    #[test]
    fn test_is_reasoning_model_o1() {
        assert!(is_reasoning_model("o1-preview"));
        assert!(is_reasoning_model("o1-mini"));
        assert!(is_reasoning_model("O1-2024-12-17"));
    }

    #[test]
    fn test_is_reasoning_model_o3() {
        assert!(is_reasoning_model("o3-mini"));
        assert!(is_reasoning_model("o3-2025-01-31"));
    }

    #[test]
    fn test_is_reasoning_model_o4() {
        assert!(is_reasoning_model("o4-mini"));
    }

    #[test]
    fn test_is_reasoning_model_grok() {
        assert!(is_reasoning_model("grok-3-mini-beta"));
    }

    #[test]
    fn test_is_reasoning_model_qwq() {
        assert!(is_reasoning_model("qwq-32b"));
        assert!(is_reasoning_model("qwen-qwq-32b"));
    }

    #[test]
    fn test_is_reasoning_model_thinking() {
        assert!(is_reasoning_model("claude-3.7-thinking"));
    }

    #[test]
    fn test_is_not_reasoning_model() {
        assert!(!is_reasoning_model("gpt-4o"));
        assert!(!is_reasoning_model("claude-3.5-sonnet"));
        assert!(!is_reasoning_model("deepseek-chat"));
    }

    #[test]
    fn test_is_reasoning_model_deepseek_reasoner() {
        assert!(is_reasoning_model("deepseek-reasoner"));
    }

    #[test]
    fn test_is_reasoning_model_glm_z1() {
        assert!(is_reasoning_model("glm-z1-air"));
        assert!(is_reasoning_model("glm-z1-airx"));
        assert!(is_reasoning_model("glm-z1-flash"));
    }

    #[test]
    fn test_is_reasoning_model_glm_51() {
        assert!(is_reasoning_model("glm-5.1"));
    }

    #[test]
    fn test_is_reasoning_model_glm_5() {
        assert!(is_reasoning_model("glm-5"));
        assert!(is_reasoning_model("glm-5-turbo"));
    }

    #[test]
    fn test_is_reasoning_model_qwen3() {
        assert!(is_reasoning_model("qwen3-235b-a22b"));
    }

    #[test]
    fn test_is_reasoning_model_kimi_k2() {
        assert!(is_reasoning_model("kimi-k2-0711"));
    }

    #[test]
    fn test_is_reasoning_model_r1() {
        assert!(is_reasoning_model("deepseek-r1"));
        assert!(is_reasoning_model("r1-preview"));
    }

    #[test]
    fn test_strip_reasoning_params() {
        let mut temp = Some(0.7);
        let mut top_p = Some(0.9);
        let mut freq = Some(0.0);
        let mut pres = Some(0.0);

        strip_reasoning_params(&mut temp, &mut top_p, &mut freq, &mut pres, "o1-preview");

        assert!(temp.is_none());
        assert!(top_p.is_none());
        assert!(freq.is_none());
        assert!(pres.is_none());
    }

    #[test]
    fn test_strip_reasoning_params_non_reasoning() {
        let mut temp = Some(0.7);
        let mut top_p = Some(0.9);
        let mut freq = Some(0.0);
        let mut pres = Some(0.0);

        strip_reasoning_params(&mut temp, &mut top_p, &mut freq, &mut pres, "gpt-4o");

        assert_eq!(temp, Some(0.7));
        assert_eq!(top_p, Some(0.9));
        assert_eq!(freq, Some(0.0));
        assert_eq!(pres, Some(0.0));
    }

    #[test]
    fn test_thinking_config_serialize() {
        let config = ThinkingConfig::with_budget(5000).unwrap();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ThinkingConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.enabled);
        assert_eq!(deserialized.budget_tokens, Some(5000));
    }

    #[test]
    fn test_deductive_with_budget_zero_invariant() {
        let result = ThinkingConfig::with_budget(0);
        assert!(result.is_err(), "with_budget(0) MUST return Err");
        match result {
            Err(LlmError::InvalidRequest { message }) => {
                assert!(
                    message.contains("positive"),
                    "error message must explain the constraint: {message}"
                );
            }
            _ => panic!("expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_deductive_with_budget_positive_implies_enabled() {
        for budget in [1usize, 100, 10000, usize::MAX] {
            let config = ThinkingConfig::with_budget(budget).unwrap();
            assert!(
                config.enabled,
                "with_budget({budget}) MUST set enabled=true"
            );
            assert_eq!(config.budget_tokens, Some(budget));
        }
    }

    #[test]
    fn test_deductive_disabled_implies_not_enabled_and_no_budget() {
        let config = ThinkingConfig::disabled();
        assert!(!config.enabled);
        assert!(config.budget_tokens.is_none());
    }

    #[test]
    fn test_deductive_is_reasoning_model_case_insensitive() {
        assert!(is_reasoning_model("O1-PREVIEW"));
        assert!(is_reasoning_model("O3-MINI"));
        assert!(is_reasoning_model("O4-MINI"));
        assert!(is_reasoning_model("DeepSeek-Reasoner"));
        assert!(is_reasoning_model("GLM-Z1-AIR"));
        assert!(is_reasoning_model("GLM-5"));
        assert!(is_reasoning_model("KIMI-K2"));
    }

    #[test]
    fn test_deductive_strip_reasoning_params_completeness() {
        let mut temp = Some(0.7);
        let mut top_p = Some(0.9);
        let mut freq = Some(0.0);
        let mut pres = Some(0.0);

        strip_reasoning_params(&mut temp, &mut top_p, &mut freq, &mut pres, "o1-preview");

        assert!(
            temp.is_none(),
            "temperature MUST be None for reasoning models"
        );
        assert!(top_p.is_none(), "top_p MUST be None for reasoning models");
        assert!(
            freq.is_none(),
            "frequency_penalty MUST be None for reasoning models"
        );
        assert!(
            pres.is_none(),
            "presence_penalty MUST be None for reasoning models"
        );
    }

    #[test]
    fn test_deductive_strip_reasoning_params_preserves_for_non_reasoning() {
        let models = &["gpt-4o", "claude-3.5-sonnet", "deepseek-chat", "llama-3"];
        for model in models {
            let mut temp = Some(0.7);
            let mut top_p = Some(0.9);
            let mut freq = Some(0.0);
            let mut pres = Some(0.0);

            strip_reasoning_params(&mut temp, &mut top_p, &mut freq, &mut pres, model);

            assert_eq!(temp, Some(0.7), "temp preserved for {model}");
            assert_eq!(top_p, Some(0.9), "top_p preserved for {model}");
            assert_eq!(freq, Some(0.0), "freq preserved for {model}");
            assert_eq!(pres, Some(0.0), "pres preserved for {model}");
        }
    }
}
