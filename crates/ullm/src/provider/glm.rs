use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::error::LlmError;
use crate::provider::openai_compatible::OpenAiCompatibleModel;
use crate::provider::types::{
    LanguageModelRequest, ModelCapabilities, ModelId, ModelName, ProviderConfig, ProviderId,
    ProviderName,
};
use crate::provider::{LanguageModel, LanguageModelProvider};
use crate::stream::StreamFuture;

type HmacSha256 = Hmac<Sha256>;

/// 智谱 GLM 模型实例，封装 JWT 认证逻辑。
pub struct GlmModel {
    inner: crate::provider::openai_compatible::OpenAiCompatibleModel,
}

impl GlmModel {
    fn needs_jwt(api_key: &str) -> bool {
        api_key.contains('.')
    }

    fn generate_jwt_token(api_key: &str) -> Result<String, LlmError> {
        let parts: Vec<&str> = api_key.splitn(2, '.').collect();
        if parts.len() != 2 {
            return Ok(api_key.to_string());
        }
        let id = parts[0];
        let secret = parts[1];
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let now_usize = usize::try_from(now).unwrap_or(0);
        let header = serde_json::json!({"alg": "HS256", "sign_type": "SIGN"});
        let payload =
            serde_json::json!({"api_key": id, "exp": now_usize + 3600, "timestamp": now_usize});
        let header_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::to_string(&header)
                .map_err(|e| LlmError::Other(format!("JWT header 序列化失败: {e}")))?,
        );
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::to_string(&payload)
                .map_err(|e| LlmError::Other(format!("JWT payload 序列化失败: {e}")))?,
        );
        let message = format!("{header_b64}.{payload_b64}");
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| LlmError::Other(format!("HMAC init failed: {e}")))?;
        mac.update(message.as_bytes());
        let sig_b64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
        Ok(format!("{message}.{sig_b64}"))
    }
}

#[async_trait]
impl LanguageModel for GlmModel {
    fn id(&self) -> &ModelId {
        self.inner.id()
    }
    fn name(&self) -> &ModelName {
        self.inner.name()
    }
    fn provider_id(&self) -> &ProviderId {
        self.inner.provider_id()
    }
    fn provider_name(&self) -> &ProviderName {
        self.inner.provider_name()
    }
    fn supports_tools(&self) -> bool {
        self.inner.supports_tools()
    }
    fn supports_streaming_tools(&self) -> bool {
        self.inner.supports_streaming_tools()
    }
    fn supports_images(&self) -> bool {
        self.inner.supports_images()
    }
    fn supports_thinking(&self) -> bool {
        self.inner.supports_thinking()
    }
    fn max_token_count(&self) -> u64 {
        self.inner.max_token_count()
    }
    fn max_output_tokens(&self) -> Option<u64> {
        self.inner.max_output_tokens()
    }
    fn count_tokens(
        &self,
        request: &LanguageModelRequest,
    ) -> crate::token_count::TokenCountFuture<'_> {
        self.inner.count_tokens(request)
    }
    fn stream_completion(&self, request: LanguageModelRequest) -> StreamFuture<'_> {
        let credentials = self.inner.credentials().clone();
        let provider_id = self.inner.provider_id_str().to_string();
        Box::pin(async move {
            let raw_key = credentials.get_api_key(&provider_id).await?;
            let bearer_token = if Self::needs_jwt(&raw_key) {
                Self::generate_jwt_token(&raw_key)?
            } else {
                raw_key
            };
            self.inner
                .stream_completion_with_token(request, &bearer_token)
                .await
        })
    }
}

/// 智谱 GLM 供应商，管理模型列表与认证。
pub struct GlmProvider {
    inner: crate::provider::openai_compatible::OpenAiCompatibleProvider,
}

type ModelSpec = (String, String, u64, Option<u64>, ModelCapabilities);

fn glm4_models() -> Vec<ModelSpec> {
    vec![
        (
            "glm-4-plus".into(),
            "GLM-4 Plus".into(),
            128_000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: true,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 128_000,
            },
        ),
        (
            "glm-4-0520".into(),
            "GLM-4".into(),
            128_000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: true,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 128_000,
            },
        ),
        (
            "glm-4-air".into(),
            "GLM-4 Air".into(),
            128_000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 128_000,
            },
        ),
        (
            "glm-4-airx".into(),
            "GLM-4 AirX".into(),
            8192,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 8192,
            },
        ),
        (
            "glm-4-long".into(),
            "GLM-4 Long".into(),
            1_000_000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 1_000_000,
            },
        ),
        (
            "glm-4-flash".into(),
            "GLM-4 Flash".into(),
            128_000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 128_000,
            },
        ),
        (
            "glm-4-flashx".into(),
            "GLM-4 FlashX".into(),
            128_000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 128_000,
            },
        ),
    ]
}

fn glm4v_models() -> Vec<ModelSpec> {
    vec![
        (
            "glm-4v".into(),
            "GLM-4V".into(),
            128_000,
            Some(1024),
            ModelCapabilities {
                tools: false,
                streaming_tools: false,
                images: true,
                thinking: false,
                parallel_tool_calls: false,
                max_tokens: 128_000,
            },
        ),
        (
            "glm-4v-plus".into(),
            "GLM-4V Plus".into(),
            8192,
            Some(4096),
            ModelCapabilities {
                tools: false,
                streaming_tools: false,
                images: true,
                thinking: false,
                parallel_tool_calls: false,
                max_tokens: 8192,
            },
        ),
    ]
}

fn glmz1_models() -> Vec<ModelSpec> {
    vec![
        (
            "glm-z1-air".into(),
            "GLM-Z1 Air".into(),
            128_000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: true,
                parallel_tool_calls: true,
                max_tokens: 128_000,
            },
        ),
        (
            "glm-z1-airx".into(),
            "GLM-Z1 AirX".into(),
            8192,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: true,
                parallel_tool_calls: true,
                max_tokens: 8192,
            },
        ),
        (
            "glm-z1-flash".into(),
            "GLM-Z1 Flash".into(),
            128_000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: true,
                parallel_tool_calls: true,
                max_tokens: 128_000,
            },
        ),
    ]
}

fn glm5_models() -> Vec<ModelSpec> {
    vec![
        (
            "glm-4.7-flash".into(),
            "GLM-4.7 Flash".into(),
            200_000,
            Some(4096),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: false,
                thinking: false,
                parallel_tool_calls: true,
                max_tokens: 200_000,
            },
        ),
        (
            "glm-5".into(),
            "GLM-5".into(),
            200_000,
            Some(16384),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: true,
                thinking: true,
                parallel_tool_calls: true,
                max_tokens: 200_000,
            },
        ),
        (
            "glm-5.1".into(),
            "GLM-5.1".into(),
            200_000,
            Some(16384),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: true,
                thinking: true,
                parallel_tool_calls: true,
                max_tokens: 200_000,
            },
        ),
        (
            "glm-5-turbo".into(),
            "GLM-5 Turbo".into(),
            200_000,
            Some(16384),
            ModelCapabilities {
                tools: true,
                streaming_tools: true,
                images: true,
                thinking: true,
                parallel_tool_calls: true,
                max_tokens: 200_000,
            },
        ),
    ]
}

fn glm_models() -> Vec<ModelSpec> {
    let mut models = Vec::with_capacity(16);
    models.extend(glm4_models());
    models.extend(glm4v_models());
    models.extend(glmz1_models());
    models.extend(glm5_models());
    models
}

impl Default for GlmProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GlmProvider {
    /// 创建智谱 GLM 供应商实例。
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: crate::provider::openai_compatible::OpenAiCompatibleProvider::new(
                "glm",
                "GLM（智谱AI）",
                "https://open.bigmodel.cn/api/paas/v4",
                Some("GLM_API_KEY".to_string()),
                glm_models(),
            ),
        }
    }
}

#[async_trait]
impl LanguageModelProvider for GlmProvider {
    fn id(&self) -> &ProviderId {
        self.inner.id()
    }
    fn name(&self) -> &ProviderName {
        self.inner.name()
    }

    fn provided_models(&self) -> Vec<Arc<dyn LanguageModel>> {
        self.inner
            .provided_models_raw()
            .into_iter()
            .map(|model| {
                let inner: OpenAiCompatibleModel =
                    Arc::try_unwrap(model).unwrap_or_else(|arc| (*arc).clone());
                Arc::new(GlmModel { inner }) as Arc<dyn LanguageModel>
            })
            .collect()
    }

    fn is_authenticated(&self) -> bool {
        self.inner.is_authenticated()
    }
    async fn authenticate(&self) -> Result<(), LlmError> {
        self.inner.authenticate().await
    }
    async fn reset_credentials(&self) -> Result<(), LlmError> {
        self.inner.reset_credentials().await
    }
    fn configuration(&self) -> &ProviderConfig {
        self.inner.configuration()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn test_glm_needs_jwt_dot_format() {
        assert!(GlmModel::needs_jwt("abc.def"));
    }

    #[test]
    fn test_glm_no_jwt_new_format() {
        assert!(!GlmModel::needs_jwt("abcdef123456"));
    }

    #[test]
    fn test_glm_generate_jwt_no_dot() {
        let result = GlmModel::generate_jwt_token("directkey").unwrap();
        assert_eq!(result, "directkey");
    }

    #[test]
    fn test_glm_generate_jwt_valid() {
        let result = GlmModel::generate_jwt_token("id.secret").unwrap();
        assert!(result.contains('.'));
        let parts: Vec<&str> = result.split('.').collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn test_glm_generate_jwt_token_valid_key_returns_token() {
        let result = GlmModel::generate_jwt_token("myid.mysecret").unwrap();
        assert!(!result.is_empty());
        assert_ne!(result, "myid.mysecret");
        let parts: Vec<&str> = result.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "JWT should have 3 parts: header.payload.signature"
        );
        let header_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(parts[0])
            .unwrap();
        let header: serde_json::Value = serde_json::from_slice(&header_bytes).unwrap();
        assert_eq!(header["alg"], "HS256");
        assert_eq!(header["sign_type"], "SIGN");
        let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(parts[1])
            .unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&payload_bytes).unwrap();
        assert_eq!(payload["api_key"], "myid");
        assert!(payload["exp"].is_number());
        assert!(payload["timestamp"].is_number());
    }

    #[test]
    fn test_glm_provider_models_count() {
        let provider = GlmProvider::new();
        assert_eq!(provider.provided_models().len(), 16);
    }

    #[test]
    fn test_glm_provider_id() {
        let provider = GlmProvider::new();
        assert_eq!(provider.id().as_ref(), "glm");
    }

    #[test]
    fn test_glm_provider_name() {
        let provider = GlmProvider::new();
        assert_eq!(provider.name().as_ref(), "GLM（智谱AI）");
    }

    #[test]
    fn test_glm_provider_configuration() {
        let provider = GlmProvider::new();
        let config = provider.configuration();
        assert_eq!(config.id.as_ref(), "glm");
        assert_eq!(config.api_url, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(config.api_key_env, Some("GLM_API_KEY".to_string()));
    }

    #[test]
    fn test_glm_provider_default() {
        let provider = GlmProvider::default();
        assert_eq!(provider.id().as_ref(), "glm");
    }

    #[test]
    fn test_glm_model_capabilities() {
        let provider = GlmProvider::new();
        let models = provider.provided_models();
        let glm_4_plus = models
            .iter()
            .find(|m| m.id().as_str() == "glm-4-plus")
            .unwrap();
        assert!(glm_4_plus.supports_tools());
        assert!(glm_4_plus.supports_images());
        assert!(!glm_4_plus.supports_thinking());
        assert_eq!(glm_4_plus.max_token_count(), 128_000);
        assert_eq!(glm_4_plus.max_output_tokens(), Some(4096));
    }

    #[test]
    fn test_glm_thinking_models() {
        let provider = GlmProvider::new();
        let models = provider.provided_models();
        let glm_z1 = models
            .iter()
            .find(|m| m.id().as_str() == "glm-z1-air")
            .unwrap();
        assert!(glm_z1.supports_thinking());
        let glm_5 = models.iter().find(|m| m.id().as_str() == "glm-5").unwrap();
        assert!(glm_5.supports_thinking());
        assert!(glm_5.supports_images());
    }

    #[test]
    fn test_glm_vision_models_no_tools() {
        let provider = GlmProvider::new();
        let models = provider.provided_models();
        let glm_4v = models.iter().find(|m| m.id().as_str() == "glm-4v").unwrap();
        assert!(!glm_4v.supports_tools());
        assert!(glm_4v.supports_images());
    }

    #[test]
    fn test_glm_generate_jwt_empty_secret() {
        let result = GlmModel::generate_jwt_token(".");
        assert!(result.is_ok());
    }

    #[test]
    fn test_glm_needs_jwt_empty_string() {
        assert!(!GlmModel::needs_jwt(""));
    }
}
