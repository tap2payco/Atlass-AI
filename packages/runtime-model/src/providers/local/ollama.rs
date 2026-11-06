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
///
/// ## Tools
///
/// Tools go in `/api/chat`'s `tools` field when the model supports them. For
/// a model that does not, the request is emulated (`providers::tool_emulation`):
/// tools are described in the system prompt and JSON calls are parsed from the
/// reply. `ATLAS_OLLAMA_TOOL_MODE` picks `native`, `emulated` or `auto`; in
/// `auto` a model Ollama rejects with "does not support tools" is retried once
/// in emulation and remembered, so later requests go straight to emulation.
use std::collections::HashSet;
use std::sync::Mutex;

use async_trait::async_trait;
use platform_contracts::health::HealthState;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{info, warn};

use shared::error::InferenceError;

use crate::config::OllamaToolMode;
use crate::domain::{
    CircuitState, ExecutionMetadata, ModelCapability, ModelRequest, ModelResponse, ModelTier,
    StopReason, ToolCall, ToolSpec,
};
use crate::local_model_registry::LocalModelRegistry;
use crate::profile::ProfileRegistry;
use crate::provider::ModelProvider;
use crate::providers::tool_emulation;
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
    /// Context window. Ollama's default is small (2–4k tokens) and it drops
    /// the oldest part of a longer prompt without telling the caller.
    #[serde(skip_serializing_if = "Option::is_none")]
    num_ctx: Option<u32>,
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
    /// `ATLAS_OLLAMA_NUM_CTX`: context window sent with every chat request.
    num_ctx: Option<u32>,
    /// `ATLAS_OLLAMA_TOOL_MODE`.
    tool_mode: OllamaToolMode,
    /// Models found (in `auto` mode) to have no native tool support.
    emulated_models: Mutex<HashSet<String>>,
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
            num_ctx: None,
            tool_mode: OllamaToolMode::default(),
            emulated_models: Mutex::new(HashSet::new()),
        }
    }

    /// Ask Ollama for a context window of `num_ctx` tokens on every chat request.
    pub fn with_num_ctx(mut self, num_ctx: Option<u32>) -> Self {
        self.num_ctx = num_ctx;
        self
    }

    /// Choose native tools, emulated tools, or native with fallback.
    pub fn with_tool_mode(mut self, tool_mode: OllamaToolMode) -> Self {
        self.tool_mode = tool_mode;
        self
    }

    /// Whether a tool request for `model` should be emulated from the start.
    fn emulates(&self, model: &str) -> bool {
        match self.tool_mode {
            OllamaToolMode::Native => false,
            OllamaToolMode::Emulated => true,
            OllamaToolMode::Auto => self
                .emulated_models
                .lock()
                .map(|set| set.contains(model))
                .unwrap_or(false),
        }
    }

    fn options(&self, request: &ModelRequest) -> OllamaOptions {
        OllamaOptions {
            num_predict: request.max_tokens,
            temperature: request.temperature,
            num_ctx: self.num_ctx,
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
        Self::turns_to_messages(wire::to_chat_turns(request))
    }

    fn turns_to_messages(turns: Vec<ChatTurn>) -> Vec<OllamaMessage> {
        turns
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

    /// One `/api/chat` call, native or emulated.
    async fn chat(
        &self,
        request: &ModelRequest,
        model: &str,
        emulate: bool,
    ) -> Result<ModelResponse, InferenceError> {
        let (messages, tools) = if emulate {
            (Self::turns_to_messages(tool_emulation::to_emulated_turns(request)), Vec::new())
        } else {
            (Self::build_messages(request), Self::build_tools(&request.tools))
        };
        let body = OllamaChatRequest {
            model: model.to_string(),
            messages,
            tools,
            stream: false,
            options: self.options(request),
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

        let (content, tool_calls) = if emulate {
            let known: HashSet<&str> = request.tools.iter().map(|t| t.name.as_str()).collect();
            tool_emulation::parse_reply(&parsed.message.content, &known)
        } else {
            (parsed.message.content, Self::parse_tool_calls(parsed.message.tool_calls))
        };
        let stop_reason = Self::stop_reason(parsed.done_reason.as_deref(), !tool_calls.is_empty());

        Ok(ModelResponse {
            content,
            tool_calls,
            stop_reason,
            capability: request.capability.clone(),
            metadata: ExecutionMetadata {
                provider_id: "ollama".into(),
                model_id: model.to_string(),
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

/// Whether a failed native tool request should be retried in emulation:
/// only in `auto` mode, and only for Ollama's "does not support tools" error.
fn falls_back_to_emulation(mode: OllamaToolMode, emulated: bool, error: &InferenceError) -> bool {
    mode == OllamaToolMode::Auto
        && !emulated
        && matches!(error, InferenceError::ApiError { message, .. }
            if tool_emulation::is_tools_unsupported_error(message))
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

    /// Always true: a model without native tool support is served by
    /// emulation (see the module docs).
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

        let emulate = !request.tools.is_empty() && self.emulates(&model);
        match self.chat(request, &model, emulate).await {
            Err(e) if !request.tools.is_empty() && falls_back_to_emulation(self.tool_mode, emulate, &e) => {
                info!(model = %model, "Ollama model has no native tool support; emulating tool calls");
                if let Ok(mut set) = self.emulated_models.lock() {
                    set.insert(model.clone());
                }
                self.chat(request, &model, true).await
            }
            other => other,
        }
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
            options: self.options(request),
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


    #[test]
    fn fallback_only_on_unsupported_tools_in_auto_mode() {
        let unsupported = InferenceError::ApiError {
            provider: "ollama".into(),
            message: "HTTP 400 Bad Request: {\"error\":\"gemma2:2b does not support tools\"}".into(),
        };
        let other = InferenceError::ApiError { provider: "ollama".into(), message: "HTTP 500: boom".into() };
        assert!(falls_back_to_emulation(OllamaToolMode::Auto, false, &unsupported));
        assert!(!falls_back_to_emulation(OllamaToolMode::Auto, true, &unsupported));
        assert!(!falls_back_to_emulation(OllamaToolMode::Native, false, &unsupported));
        assert!(!falls_back_to_emulation(OllamaToolMode::Auto, false, &other));
        assert!(!falls_back_to_emulation(
            OllamaToolMode::Auto,
            false,
            &InferenceError::ProviderUnavailable("does not support tools".into())
        ));
    }

    #[test]
    fn tool_mode_decides_emulation_up_front() {
        let provider = |mode| test_provider("http://unused".into()).with_tool_mode(mode);
        assert!(!provider(OllamaToolMode::Native).emulates("m"));
        assert!(provider(OllamaToolMode::Emulated).emulates("m"));
        let auto = provider(OllamaToolMode::Auto);
        assert!(!auto.emulates("m"));
        auto.emulated_models.lock().unwrap().insert("m".into());
        assert!(auto.emulates("m"));
        assert!(!auto.emulates("other"));
    }

    fn test_provider(base_url: String) -> OllamaProvider {
        OllamaProvider::with_model_override(
            base_url,
            LocalModelRegistry { available: vec![] },
            ProfileRegistry::with_defaults(),
            Some("tiny".into()),
        )
    }

    type Seen = std::sync::Arc<Mutex<Vec<serde_json::Value>>>;

    /// A one-request-per-connection HTTP server standing in for Ollama: it
    /// rejects bodies that carry `tools` the way Ollama does for a model
    /// without tool support, and answers others with an emulated call.
    async fn fake_ollama() -> (String, Seen) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen: Seen = Default::default();
        let log = seen.clone();
        tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let body = loop {
                    let n = sock.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        break None;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some(head_end) = text.find("\r\n\r\n") {
                        let len = text[..head_end]
                            .lines()
                            .find_map(|l| {
                                let (k, v) = l.split_once(':')?;
                                if k.eq_ignore_ascii_case("content-length") {
                                    v.trim().parse::<usize>().ok()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0);
                        if buf.len() >= head_end + 4 + len {
                            break Some(buf[head_end + 4..head_end + 4 + len].to_vec());
                        }
                    }
                };
                let Some(body) = body else { continue };
                let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
                let (status, reply) = if json.get("tools").is_some() {
                    (
                        "400 Bad Request",
                        json!({ "error": "registry.ollama.ai/library/tiny:latest does not support tools" }),
                    )
                } else {
                    (
                        "200 OK",
                        json!({
                            "message": {
                                "role": "assistant",
                                "content": "Reading it.\n```json\n{\"name\": \"read_file\", \"arguments\": {\"path\": \"b.rs\"}}\n```"
                            },
                            "done_reason": "stop",
                            "eval_count": 3
                        }),
                    )
                };
                log.lock().unwrap().push(json);
                let payload = reply.to_string();
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = sock.write_all(response.as_bytes()).await;
                let _ = sock.shutdown().await;
            }
        });
        (url, seen)
    }

    #[tokio::test]
    async fn auto_mode_retries_unsupported_model_in_emulation_and_remembers_it() {
        let (url, seen) = fake_ollama().await;
        let provider = test_provider(url);
        let req = tool_loop_request();

        let resp = provider.infer(&req).await.unwrap();
        assert_eq!(resp.content, "Reading it.");
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "read_file");
        assert_eq!(resp.tool_calls[0].arguments, json!({ "path": "b.rs" }));
        {
            let seen = seen.lock().unwrap();
            assert_eq!(seen.len(), 2, "native attempt, then emulated retry");
            assert!(seen[0].get("tools").is_some());
            let retry = &seen[1];
            assert!(retry.get("tools").is_none());
            let system = retry["messages"][0]["content"].as_str().unwrap();
            assert!(system.starts_with("sys\n\n# Tools") && system.contains("### read_file"));
            // Earlier calls and results are replayed as text, not native fields.
            assert!(retry["messages"][2].get("tool_calls").is_none());
            assert!(retry["messages"][2]["content"].as_str().unwrap().contains("```json"));
            assert_eq!(retry["messages"][3]["role"], "user");
        }

        // The model is remembered: the next request goes straight to emulation.
        provider.infer(&req).await.unwrap();
        assert_eq!(seen.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn native_mode_reports_the_error_without_retrying() {
        let (url, seen) = fake_ollama().await;
        let provider = test_provider(url).with_tool_mode(OllamaToolMode::Native);
        let err = provider.infer(&tool_loop_request()).await.unwrap_err();
        assert!(matches!(&err, InferenceError::ApiError { message, .. } if message.contains("does not support tools")));
        assert_eq!(seen.lock().unwrap().len(), 1);
    }
}
