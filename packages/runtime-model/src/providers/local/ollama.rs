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
    CircuitState, ExecutionMetadata, ModelCapability, ModelRequest, ModelResponse, ModelTier,
    StopReason, ToolCall, ToolSpec,
};
use crate::local_model_registry::LocalModelRegistry;
use crate::profile::ProfileRegistry;
use crate::provider::ModelProvider;
use crate::providers::wire::{self, ChatTurn};

// ── Ollama request/response shapes ────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OllamaTool>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: &'static str,
    content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<OllamaToolCall>,
    /// Ollama identifies a tool result by the tool's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaTool {
    r#type: &'static str,
    function: OllamaFunctionSpec,
}

#[derive(Debug, Serialize)]
struct OllamaFunctionSpec {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaToolCall {
    function: OllamaFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaFunctionCall {
    name: String,
    #[serde(default)]
    arguments: serde_json::Value,
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
    done_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,
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
    /// `ATLAS_OLLAMA_MODEL`: serves every non-embedding capability.
    model_override: Option<String>,
}

/// Capabilities an explicitly chosen chat model is asked to serve.
const OVERRIDE_CAPABILITIES: &[ModelCapability] = &[
    ModelCapability::Reasoning,
    ModelCapability::CodeGeneration,
    ModelCapability::CodeAnalysis,
    ModelCapability::Summarization,
    ModelCapability::Extraction,
    ModelCapability::Classification,
    ModelCapability::ToolCalling,
];

impl OllamaProvider {
    pub fn new(
        base_url: String,
        local_registry: LocalModelRegistry,
        profile_registry: ProfileRegistry,
    ) -> Self {
        Self::with_model_override(base_url, local_registry, profile_registry, None)
    }

    pub fn with_model_override(
        base_url: String,
        local_registry: LocalModelRegistry,
        profile_registry: ProfileRegistry,
        model_override: Option<String>,
    ) -> Self {
        // Compute capability set from discovered models
        let mut caps: Vec<ModelCapability> = local_registry
            .available
            .iter()
            .flat_map(|m| m.capabilities.clone())
            .collect();
        if let Some(model) = &model_override {
            if !local_registry.is_available(model) {
                warn!(model = %model, "ATLAS_OLLAMA_MODEL is not installed in Ollama; requests will fail");
            }
            caps.extend(OVERRIDE_CAPABILITIES.iter().cloned());
        }
        caps.sort_by_key(|c| format!("{c:?}"));
        caps.dedup_by_key(|c| format!("{c:?}"));

        Self {
            base_url,
            client: reqwest::Client::new(),
            local_registry,
            profile_registry,
            capabilities: caps,
            model_override,
        }
    }

    /// Resolve which local model should handle this request.
    ///
    /// Uses the override model when one is configured; otherwise walks the
    /// profile's `preferred_models` then `fallback_models` and returns the
    /// first one that is actually installed locally.
    fn resolve_model(&self, request: &ModelRequest) -> Option<String> {
        if let Some(model) = &self.model_override {
            if request.capability != ModelCapability::Embedding {
                return Some(model.clone());
            }
        }
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
        wire::to_chat_turns(request)
            .into_iter()
            .map(|turn| match turn {
                ChatTurn::System(content) => OllamaMessage::plain("system", content),
                ChatTurn::User(content) => OllamaMessage::plain("user", content),
                ChatTurn::Assistant { text, tool_calls } => OllamaMessage {
                    tool_calls: tool_calls
                        .into_iter()
                        .map(|c| OllamaToolCall {
                            function: OllamaFunctionCall { name: c.name, arguments: c.arguments },
                        })
                        .collect(),
                    ..OllamaMessage::plain("assistant", text)
                },
                ChatTurn::Tool { tool_name, content, .. } => OllamaMessage {
                    tool_name: Some(tool_name),
                    ..OllamaMessage::plain("tool", content)
                },
            })
            .collect()
    }

    fn build_tools(tools: &[ToolSpec]) -> Vec<OllamaTool> {
        tools
            .iter()
            .map(|t| OllamaTool {
                r#type: "function",
                function: OllamaFunctionSpec {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                },
            })
            .collect()
    }

    /// Ollama returns no call ids, so Atlas assigns them.
    fn parse_tool_calls(calls: Vec<OllamaToolCall>) -> Vec<ToolCall> {
        calls
            .into_iter()
            .map(|c| ToolCall {
                id: wire::generated_call_id(),
                name: c.function.name,
                arguments: wire::parse_arguments(c.function.arguments),
            })
            .collect()
    }

    fn stop_reason(done_reason: Option<&str>, has_tool_calls: bool) -> StopReason {
        if has_tool_calls {
            return StopReason::ToolUse;
        }
        match done_reason {
            None | Some("stop") => StopReason::EndTurn,
            Some("length") => StopReason::MaxTokens,
            Some(_) => StopReason::Other,
        }
    }
}

impl OllamaMessage {
    fn plain(role: &'static str, content: String) -> Self {
        Self { role, content, tool_calls: Vec::new(), tool_name: None }
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

    /// `/api/chat` accepts tools. A model without tool support returns an HTTP
    /// error, which the router treats as a provider failure and falls back.
    fn supports_tool_use(&self) -> bool {
        true
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
            tools: Self::build_tools(&request.tools),
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

        let tool_calls = Self::parse_tool_calls(parsed.message.tool_calls);
        let stop_reason = Self::stop_reason(parsed.done_reason.as_deref(), !tool_calls.is_empty());

        Ok(ModelResponse {
            content: parsed.message.content,
            tool_calls,
            stop_reason,
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

        // Streaming is text-only (ADR-017); tools are not offered here.
        let body = OllamaChatRequest {
            model: model.clone(),
            messages: Self::build_messages(request),
            tools: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ContentBlock, Message, Role, ToolResult};
    use serde_json::json;

    fn tool_loop_request() -> ModelRequest {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolCall(ToolCall {
                id: "c1".into(),
                name: "read_file".into(),
                arguments: json!({ "path": "a.rs" }),
            })],
        };
        ModelRequest::conversation(
            ModelCapability::CodeGeneration,
            vec![
                Message::user("Open a.rs"),
                assistant,
                Message::tool_results([ToolResult {
                    call_id: "c1".into(),
                    content: "fn main() {}".into(),
                    is_error: false,
                }]),
            ],
        )
        .with_system_prompt("sys")
        .with_tools(vec![ToolSpec {
            name: "read_file".into(),
            description: "Read a file".into(),
            input_schema: json!({ "type": "object" }),
        }])
    }

    #[test]
    fn request_json_matches_ollama_chat_shape() {
        let req = tool_loop_request();
        let messages = serde_json::to_value(OllamaProvider::build_messages(&req)).unwrap();
        assert_eq!(
            messages,
            json!([
                { "role": "system", "content": "sys" },
                { "role": "user", "content": "Open a.rs" },
                { "role": "assistant", "content": "",
                  "tool_calls": [{ "function": { "name": "read_file", "arguments": { "path": "a.rs" } } }] },
                { "role": "tool", "content": "fn main() {}", "tool_name": "read_file" }
            ])
        );
        let tools = serde_json::to_value(OllamaProvider::build_tools(&req.tools)).unwrap();
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["parameters"], json!({ "type": "object" }));
    }

    #[test]
    fn response_tool_calls_get_ids_and_tool_use_stop_reason() {
        let parsed: OllamaChatResponse = serde_json::from_value(json!({
            "message": { "role": "assistant", "content": "",
                         "tool_calls": [{ "function": { "name": "ls", "arguments": { "dir": "." } } }] },
            "done_reason": "stop",
            "eval_count": 7
        }))
        .unwrap();
        let calls = OllamaProvider::parse_tool_calls(parsed.message.tool_calls);
        assert_eq!(calls.len(), 1);
        assert!(calls[0].id.starts_with("call_"));
        assert_eq!(calls[0].arguments, json!({ "dir": "." }));
        assert_eq!(OllamaProvider::stop_reason(Some("stop"), true), StopReason::ToolUse);
        assert_eq!(OllamaProvider::stop_reason(Some("length"), false), StopReason::MaxTokens);
    }
}
