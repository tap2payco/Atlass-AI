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
    StopReason, ToolCall, ToolSpec,
};
use crate::profile::ProfileRegistry;
use crate::provider::ModelProvider;
use crate::providers::wire::{self, ChatTurn};

// ── Fireworks API shapes (OpenAI-compatible) ──────────────────────────────────

const FIREWORKS_BASE_URL: &str = "https://api.fireworks.ai/inference/v1";

#[derive(Debug, Serialize)]
struct FwChatRequest {
    model: String,
    messages: Vec<FwMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<FwTool>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct FwMessage {
    role: &'static str,
    /// OpenAI allows `null` content on an assistant turn that only calls tools.
    content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<FwToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct FwTool {
    r#type: &'static str,
    function: FwFunctionSpec,
}

#[derive(Debug, Serialize)]
struct FwFunctionSpec {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct FwToolCall {
    #[serde(default)]
    id: String,
    #[serde(default = "function_type")]
    r#type: String,
    function: FwFunctionCall,
}

fn function_type() -> String {
    "function".into()
}

#[derive(Debug, Serialize, Deserialize)]
struct FwFunctionCall {
    name: String,
    /// A JSON-encoded string in the OpenAI format.
    #[serde(default)]
    arguments: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct FwChatResponse {
    choices: Vec<FwChoice>,
    usage: Option<FwUsage>,
}

#[derive(Debug, Deserialize)]
struct FwChoice {
    message: FwChoiceMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FwChoiceMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<FwToolCall>,
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
        wire::to_chat_turns(request)
            .into_iter()
            .map(|turn| match turn {
                ChatTurn::System(content) => FwMessage::plain("system", Some(content)),
                ChatTurn::User(content) => FwMessage::plain("user", Some(content)),
                ChatTurn::Assistant { text, tool_calls } => FwMessage {
                    tool_calls: tool_calls
                        .into_iter()
                        .map(|c| FwToolCall {
                            id: c.id,
                            r#type: function_type(),
                            function: FwFunctionCall {
                                name: c.name,
                                arguments: serde_json::Value::String(c.arguments.to_string()),
                            },
                        })
                        .collect(),
                    ..FwMessage::plain("assistant", (!text.is_empty()).then_some(text))
                },
                ChatTurn::Tool { call_id, content, .. } => FwMessage {
                    tool_call_id: Some(call_id),
                    ..FwMessage::plain("tool", Some(content))
                },
            })
            .collect()
    }

    fn build_tools(tools: &[ToolSpec]) -> Vec<FwTool> {
        tools
            .iter()
            .map(|t| FwTool {
                r#type: "function",
                function: FwFunctionSpec {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                },
            })
            .collect()
    }

    fn parse_tool_calls(calls: Vec<FwToolCall>) -> Vec<ToolCall> {
        calls
            .into_iter()
            .map(|c| ToolCall {
                id: if c.id.is_empty() { wire::generated_call_id() } else { c.id },
                name: c.function.name,
                arguments: wire::parse_arguments(c.function.arguments),
            })
            .collect()
    }

    fn stop_reason(finish_reason: Option<&str>, has_tool_calls: bool) -> StopReason {
        if has_tool_calls {
            return StopReason::ToolUse;
        }
        match finish_reason {
            None | Some("stop") => StopReason::EndTurn,
            Some("length") => StopReason::MaxTokens,
            Some("tool_calls") => StopReason::ToolUse,
            Some(_) => StopReason::Other,
        }
    }
}

impl FwMessage {
    fn plain(role: &'static str, content: Option<String>) -> Self {
        Self { role, content, tool_calls: Vec::new(), tool_call_id: None }
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

    fn supports_tool_use(&self) -> bool {
        true
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
            tools: Self::build_tools(&request.tools),
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

        let tokens_used = parsed.usage.map(|u| u.completion_tokens).unwrap_or(0);
        let (content, tool_calls, finish_reason) = match parsed.choices.into_iter().next() {
            Some(choice) => (
                choice.message.content.unwrap_or_default(),
                Self::parse_tool_calls(choice.message.tool_calls),
                choice.finish_reason,
            ),
            None => (String::new(), Vec::new(), None),
        };
        let stop_reason = Self::stop_reason(finish_reason.as_deref(), !tool_calls.is_empty());

        Ok(ModelResponse {
            content,
            tool_calls,
            stop_reason,
            capability: request.capability.clone(),
            metadata: ExecutionMetadata {
                provider_id: "fireworks".into(),
                model_id: model,
                tier: ModelTier::External,
                latency_ms: 0, // filled by Router
                tokens_used,
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

        // Streaming is text-only (ADR-017); tools are not offered here.
        let body = FwChatRequest {
            model: model.clone(),
            messages: Self::build_messages(request),
            tools: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ContentBlock, Message, Role, ToolResult};
    use serde_json::json;

    #[test]
    fn request_json_matches_openai_tool_shape() {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolCall(ToolCall {
                id: "call_9".into(),
                name: "read_file".into(),
                arguments: json!({ "path": "a.rs" }),
            })],
        };
        let req = ModelRequest::conversation(
            ModelCapability::CodeGeneration,
            vec![
                Message::user("Open a.rs"),
                assistant,
                Message::tool_results([ToolResult {
                    call_id: "call_9".into(),
                    content: "fn main() {}".into(),
                    is_error: false,
                }]),
            ],
        );
        let messages = serde_json::to_value(FireworksProvider::build_messages(&req)).unwrap();
        assert_eq!(
            messages,
            json!([
                { "role": "user", "content": "Open a.rs" },
                { "role": "assistant", "content": null,
                  "tool_calls": [{ "id": "call_9", "type": "function",
                                   "function": { "name": "read_file", "arguments": "{\"path\":\"a.rs\"}" } }] },
                { "role": "tool", "content": "fn main() {}", "tool_call_id": "call_9" }
            ])
        );
    }

    #[test]
    fn response_parses_string_arguments_and_keeps_ids() {
        let parsed: FwChatResponse = serde_json::from_value(json!({
            "choices": [{
                "message": { "role": "assistant", "content": null,
                             "tool_calls": [{ "id": "call_1", "type": "function",
                                              "function": { "name": "ls", "arguments": "{\"dir\":\".\"}" } }] },
                "finish_reason": "tool_calls"
            }],
            "usage": { "completion_tokens": 5 }
        }))
        .unwrap();
        let choice = parsed.choices.into_iter().next().unwrap();
        assert!(choice.message.content.is_none());
        let calls = FireworksProvider::parse_tool_calls(choice.message.tool_calls);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].arguments, json!({ "dir": "." }));
        assert_eq!(FireworksProvider::stop_reason(Some("tool_calls"), false), StopReason::ToolUse);
    }
}
