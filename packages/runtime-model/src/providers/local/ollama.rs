/// Ollama provider adapter.
///
/// Implements `ModelProvider` against the Ollama REST API
/// (`http://localhost:11434` by default).
///
/// ## Supported capabilities
///
/// The actual set is determined at startup by `LocalModelRegistry::discover()`.
/// The adapter exposes all capabilities for which at least one matching model
/// is installed. Embedding is handled via `/api/embeddings`.
///
/// ## API shape
///
/// - Inference: `POST /api/chat`   (OpenAI-compatible messages format)
/// - Streaming: `POST /api/chat`   (with `stream: true`)
/// - Embeddings: `POST /api/embeddings`
use async_trait::async_trait;
use platform_contracts::health::HealthState;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::warn;

use shared::error::InferenceError;

use crate::domain::{
    ExecutionMetadata, ModelCapability, ModelRequest, ModelResponse,
    ModelTier, CircuitState, ToolCallResponse, ToolDefinition,
};
use crate::local_model_registry::LocalModelRegistry;
use crate::profile::ProfileRegistry;
use crate::provider::ModelProvider;

// ── Ollama request/response shapes ────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    num_predict: u32,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

// ── OllamaProvider ────────────────────────────────────────────────────────────

pub struct OllamaProvider {
    base_url: String,
    client: reqwest::Client,
    local_registry: LocalModelRegistry,
    profile_registry: ProfileRegistry,
    /// Pre-computed list of capabilities for which at least one model exists locally.
    capabilities: Vec<ModelCapability>,
}

impl OllamaProvider {
    pub fn new(
        base_url: String,
        local_registry: LocalModelRegistry,
        profile_registry: ProfileRegistry,
    ) -> Self {
        // Compute capability set from discovered models
        let mut caps: Vec<ModelCapability> = local_registry
            .available
            .iter()
            .flat_map(|m| m.capabilities.clone())
            .collect();
        caps.sort_by_key(|c| format!("{c:?}"));
        caps.dedup_by_key(|c| format!("{c:?}"));

        Self {
            base_url,
            client: reqwest::Client::new(),
            local_registry,
            profile_registry,
            capabilities: caps,
        }
    }

    /// Resolve which local model should handle this request.
    ///
    /// Walks the `preferred_models` list in the profile; returns the first
    /// one that is actually installed locally.
    fn resolve_model(&self, request: &ModelRequest) -> Option<String> {
        let profile = self
            .profile_registry
            .resolve(&request.capability, &request.profile)?;

        // Try preferred models first, then fallback list
        for model in profile.preferred_models.iter().chain(profile.fallback_models.iter()) {
            if self.local_registry.is_available(model) {
                return Some(model.clone());
            }
        }
        None
    }

    fn build_messages(request: &ModelRequest) -> Vec<OllamaMessage> {
        let mut messages = Vec::new();
        if let Some(ref sys) = request.system_prompt {
            messages.push(OllamaMessage {
                role: "system".into(),
                content: sys.clone(),
            });
        }
        messages.push(OllamaMessage {
            role: "user".into(),
            content: request.prompt.clone(),
        });
        messages
    }
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    fn provider_id(&self) -> &str {
        "ollama"
    }

    fn tier(&self) -> ModelTier {
        ModelTier::Local
    }

    fn supported_capabilities(&self) -> &[ModelCapability] {
        &self.capabilities
    }

    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse, InferenceError> {
        let model = self.resolve_model(request).ok_or_else(|| {
            InferenceError::ProviderUnavailable(format!(
                "No local model available for capability {:?}",
                request.capability
            ))
        })?;

        let body = OllamaChatRequest {
            model: model.clone(),
            messages: Self::build_messages(request),
            stream: false,
            options: OllamaOptions {
                num_predict: request.max_tokens,
                temperature: request.temperature,
            },
        };

        let url = format!("{}/api/chat", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| InferenceError::ProviderUnavailable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(InferenceError::ApiError {
                provider: "ollama".into(),
                message: format!("HTTP {status}: {text}"),
            });
        }

        let parsed: OllamaChatResponse = resp
            .json()
            .await
            .map_err(|e| InferenceError::ApiError {
                provider: "ollama".into(),
                message: e.to_string(),
            })?;

        Ok(ModelResponse {
            content: parsed.message.content,
            capability: request.capability.clone(),
            metadata: ExecutionMetadata {
                provider_id: "ollama".into(),
                model_id: model,
                tier: ModelTier::Local,
                latency_ms: 0, // filled by Router
                tokens_used: parsed.eval_count.unwrap_or(0),
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
            InferenceError::ProviderUnavailable(format!(
                "No local model for capability {:?}",
                request.capability
            ))
        })?;

        let body = OllamaChatRequest {
            model: model.clone(),
            messages: Self::build_messages(request),
            stream: true,
            options: OllamaOptions {
                num_predict: request.max_tokens,
                temperature: request.temperature,
            },
        };

        let url = format!("{}/api/chat", self.base_url);
        let mut resp = self
            .client
            .post(&url)
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
                // Ollama streams newline-delimited JSON objects
                for line in text.lines() {
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                        if let Some(content) = obj
                            .get("message")
                            .and_then(|m| m.get("content"))
                            .and_then(|c| c.as_str())
                        {
                            if tx.send(content.to_string()).await.is_err() {
                                // Receiver dropped — treat as normal completion
                                break;
                            }
                        }
                        if let Some(count) = obj.get("eval_count").and_then(|v| v.as_u64()) {
                            total_tokens = count as u32;
                        }
                    }
                }
            }
        }

        Ok(ExecutionMetadata {
            provider_id: "ollama".into(),
            model_id: model,
            tier: ModelTier::Local,
            latency_ms: 0,
            tokens_used: total_tokens,
            cache_hit: false,
            fallback_used: false,
            retry_count: 0,
            circuit_state: CircuitState::Closed,
        })
    }

    async fn embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError> {
        // Use bge-m3 if available; otherwise use any embedding-capable model
        let model = self
            .local_registry
            .models_for(&ModelCapability::Embedding)
            .first()
            .map(|m| m.name.clone())
            .ok_or_else(|| {
                InferenceError::ProviderUnavailable(
                    "No embedding model available locally".into(),
                )
            })?;

        let body = OllamaEmbedRequest {
            model,
            input: texts.to_vec(),
        };

        let url = format!("{}/api/embed", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| InferenceError::ProviderUnavailable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let msg = resp.text().await.unwrap_or_default();
            return Err(InferenceError::ApiError {
                provider: "ollama".into(),
                message: format!("HTTP {status}: {msg}"),
            });
        }

        let parsed: OllamaEmbedResponse = resp
            .json()
            .await
            .map_err(|e| InferenceError::ApiError {
                provider: "ollama".into(),
                message: e.to_string(),
            })?;

        Ok(parsed.embeddings)
    }

    async fn health(&self) -> HealthState {
        let url = format!("{}/api/tags", self.base_url);
        match self.client.get(&url).send().await {
            Ok(r) if r.status().is_success() => HealthState::Healthy,
            Ok(_) => HealthState::Degraded,
            Err(e) => {
                warn!(error = %e, "Ollama health check failed");
                HealthState::Offline
            }
        }
    }
}
