/// Fireworks AI provider adapter.
///
/// Implements `ModelProvider` against the Fireworks AI OpenAI-compatible API:
/// `https://api.fireworks.ai/inference/v1`
///
/// ## Role in Atlas
///
/// Fireworks is the **cold-standby external provider**. It is only called when:
/// 1. All local providers have failed or their circuits are open, AND
/// 2. `ExecutionPolicy` is not `RequireLocal`, AND
/// 3. `privacy_mode` is `false` in `ModelRuntimeConfig`.
///
/// ## Supported capabilities
///
/// Fireworks does not support `Embedding` (kept local-only). All other
/// capabilities are served via the OpenAI `/chat/completions` endpoint.
///
/// ## Authentication
///
/// Reads `ATLAS_FIREWORKS_API_KEY` from config. If the key is absent,
/// this provider is not registered at all.
use async_trait::async_trait;
use platform_contracts::health::HealthState;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::warn;

use shared::error::InferenceError;

use crate::domain::{
    CircuitState, ExecutionMetadata, ModelCapability, ModelRequest, ModelResponse, ModelTier,
};
use crate::profile::ProfileRegistry;
use crate::provider::ModelProvider;

// ── Fireworks API shapes (OpenAI-compatible) ──────────────────────────────────

const FIREWORKS_BASE_URL: &str = "https://api.fireworks.ai/inference/v1";

#[derive(Debug, Serialize)]
struct FwChatRequest {
    model: String,
    messages: Vec<FwMessage>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct FwMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct FwChatResponse {
    choices: Vec<FwChoice>,
    usage: Option<FwUsage>,
}

#[derive(Debug, Deserialize)]
struct FwChoice {
    message: FwChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct FwChoiceMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct FwUsage {
    completion_tokens: u32,
}

// ── FireworksProvider ─────────────────────────────────────────────────────────

pub struct FireworksProvider {
    api_key: String,
    client: reqwest::Client,
    profile_registry: ProfileRegistry,
    capabilities: Vec<ModelCapability>,
}

impl FireworksProvider {
    /// Create the provider.
    ///
    /// Returns `None` if `api_key` is empty — the caller should not register
    /// this provider if there is no key.
    pub fn new(api_key: String, profile_registry: ProfileRegistry) -> Option<Self> {
        if api_key.is_empty() {
            return None;
        }
        Some(Self {
            api_key,
            client: reqwest::Client::new(),
            profile_registry,
            // Fireworks does NOT support Embedding — local-only by design.
            capabilities: vec![
                ModelCapability::Reasoning,
                ModelCapability::CodeGeneration,
                ModelCapability::CodeAnalysis,
                ModelCapability::Summarization,
                ModelCapability::Extraction,
                ModelCapability::Classification,
            ],
        })
    }

    fn resolve_model(&self, request: &ModelRequest) -> Option<String> {
        let profile = self
            .profile_registry
            .resolve(&request.capability, &request.profile)?;
        // Use the first fallback model (Fireworks model path format)
        profile.fallback_models.first().cloned()
    }

    fn build_messages(request: &ModelRequest) -> Vec<FwMessage> {
        let mut messages = Vec::new();
        if let Some(ref sys) = request.system_prompt {
            messages.push(FwMessage {
                role: "system".into(),
                content: sys.clone(),
            });
        }
        messages.push(FwMessage {
            role: "user".into(),
            content: request.prompt.clone(),
        });
        messages
    }
}

#[async_trait]
impl ModelProvider for FireworksProvider {
    fn provider_id(&self) -> &str {
        "fireworks"
    }

    fn tier(&self) -> ModelTier {
        ModelTier::External
    }

    fn supported_capabilities(&self) -> &[ModelCapability] {
        &self.capabilities
    }

    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse, InferenceError> {
        let model = self.resolve_model(request).ok_or_else(|| {
            InferenceError::Unsupported(format!(
                "No Fireworks model configured for capability {:?}",
                request.capability
            ))
        })?;

        let body = FwChatRequest {
            model: model.clone(),
            messages: Self::build_messages(request),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stream: false,
        };

        let resp = self
            .client
            .post(format!("{FIREWORKS_BASE_URL}/chat/completions"))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| InferenceError::ProviderUnavailable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let msg = resp.text().await.unwrap_or_default();
            return Err(InferenceError::ApiError {
                provider: "fireworks".into(),
                message: format!("HTTP {status}: {msg}"),
            });
        }

        let parsed: FwChatResponse = resp.json().await.map_err(|e| InferenceError::ApiError {
            provider: "fireworks".into(),
            message: e.to_string(),
        })?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();

        Ok(ModelResponse {
            content,
            capability: request.capability.clone(),
            metadata: ExecutionMetadata {
                provider_id: "fireworks".into(),
                model_id: model,
                tier: ModelTier::External,
                latency_ms: 0, // filled by Router
                tokens_used: parsed.usage.map(|u| u.completion_tokens).unwrap_or(0),
                cache_hit: false,
                fallback_used: false,
                retry_count: 0,
                circuit_state: CircuitState::Closed,
            },
            request_id: request.request_id.clone(),
        })
    }

    async fn stream(
        &self,
        request: &ModelRequest,
        tx: mpsc::Sender<String>,
    ) -> Result<ExecutionMetadata, InferenceError> {
        let model = self.resolve_model(request).ok_or_else(|| {
            InferenceError::Unsupported(format!(
                "No Fireworks model for capability {:?}",
                request.capability
            ))
        })?;

        let body = FwChatRequest {
            model: model.clone(),
            messages: Self::build_messages(request),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stream: true,
        };

        let mut resp = self
            .client
            .post(format!("{FIREWORKS_BASE_URL}/chat/completions"))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| InferenceError::ProviderUnavailable(e.to_string()))?;

        let mut total_tokens: u32 = 0;

        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| InferenceError::StreamError(e.to_string()))?
        {
            if let Ok(text) = std::str::from_utf8(&chunk) {
                // OpenAI SSE format: "data: {...}\n\n"
                for line in text.lines() {
                    let line = line.trim_start_matches("data: ");
                    if line == "[DONE]" || line.is_empty() {
                        continue;
                    }
                    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                        if let Some(delta) = obj
                            .get("choices")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("delta"))
                            .and_then(|d| d.get("content"))
                            .and_then(|c| c.as_str())
                        {
                            if tx.send(delta.to_string()).await.is_err() {
                                break;
                            }
                        }
                        if let Some(tokens) = obj
                            .get("usage")
                            .and_then(|u| u.get("completion_tokens"))
                            .and_then(|t| t.as_u64())
                        {
                            total_tokens = tokens as u32;
                        }
                    }
                }
            }
        }

        Ok(ExecutionMetadata {
            provider_id: "fireworks".into(),
            model_id: model,
            tier: ModelTier::External,
            latency_ms: 0,
            tokens_used: total_tokens,
            cache_hit: false,
            fallback_used: false,
            retry_count: 0,
            circuit_state: CircuitState::Closed,
        })
    }

    async fn health(&self) -> HealthState {
        // Lightweight probe: list models endpoint
        let resp = self
            .client
            .get(format!("{FIREWORKS_BASE_URL}/models"))
            .bearer_auth(&self.api_key)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => HealthState::Healthy,
            Ok(r) if r.status().as_u16() == 401 => {
                warn!("Fireworks API key rejected (401)");
                HealthState::Offline
            }
            Ok(_) => HealthState::Degraded,
            Err(e) => {
                warn!(error = %e, "Fireworks health check failed");
                HealthState::Offline
            }
        }
    }
}
