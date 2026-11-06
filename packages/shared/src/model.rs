/// Shared model inference types.
///
/// These types are the vocabulary used by:
/// - `sdk::model::ModelRuntime` (the trait)
/// - `runtime-model` (the implementation)
/// - Any cognitive runtime that issues inference requests
///
/// Placing types here avoids a circular dependency between `sdk` and
/// `runtime-model` (both depend on `shared`; neither depends on the other).
use serde::{Deserialize, Serialize};

// ── Capability ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ModelCapability {
    Reasoning,
    CodeGeneration,
    CodeAnalysis,
    Summarization,
    Extraction,
    Embedding,
    Classification,
    /// Not all providers support this — guarded by `ModelProvider::supported_capabilities`.
    ToolCalling,
}

impl std::fmt::Display for ModelCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Reasoning => "reasoning",
            Self::CodeGeneration => "code-generation",
            Self::CodeAnalysis => "code-analysis",
            Self::Summarization => "summarization",
            Self::Extraction => "extraction",
            Self::Embedding => "embedding",
            Self::Classification => "classification",
            Self::ToolCalling => "tool-calling",
        };
        write!(f, "{s}")
    }
}

// ── Quality Profile ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CapabilityProfile {
    Fast,
    #[default]
    Balanced,
    HighQuality,
    OfflineOnly,
    LowMemory,
}

// ── Execution Policy ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ExecutionPolicy {
    #[default]
    PreferLocal,
    RequireLocal,
    AllowFallback,
    LowLatency,
    LowCost,
    HighReliability,
}

// ── Provider Tier ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelTier {
    Local,
    External,
}

// ── Circuit State ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

// ── Conversation ──────────────────────────────────────────────────────────────
//
// See ADR-017. The system prompt is carried separately on `ModelRequest`, so a
// conversation only contains user and assistant turns. Tool results are sent
// back to the model inside a `User` message, as `ContentBlock::ToolResult`.

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// A tool invocation requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    /// Unique within a conversation. Providers that don't supply ids get an
    /// Atlas-generated one, so a `ToolResult` can always be matched to its call.
    pub id: String,
    pub name: String,
    /// Arguments as a JSON object, already parsed.
    pub arguments: serde_json::Value,
}

/// The outcome of executing a `ToolCall`, sent back to the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    /// The `ToolCall::id` this result answers.
    pub call_id: String,
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ToolCall(ToolCall),
    ToolResult(ToolResult),
    /// A summary that replaces the first `covers` messages of the stored
    /// conversation when it is sent to a model (context compaction). Stored
    /// histories are append-only, so the summarised messages are kept on
    /// disk; only the model's view of them changes.
    Summary { text: String, covers: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: Role::User, content: vec![ContentBlock::Text { text: text.into() }] }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: vec![ContentBlock::Text { text: text.into() }] }
    }

    /// A user turn carrying the results of one or more tool calls.
    pub fn tool_results(results: impl IntoIterator<Item = ToolResult>) -> Self {
        Self {
            role: Role::User,
            content: results.into_iter().map(ContentBlock::ToolResult).collect(),
        }
    }

    /// All text blocks joined with newlines.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn tool_calls(&self) -> impl Iterator<Item = &ToolCall> {
        self.content.iter().filter_map(|b| match b {
            ContentBlock::ToolCall(c) => Some(c),
            _ => None,
        })
    }

    pub fn tool_result_blocks(&self) -> impl Iterator<Item = &ToolResult> {
        self.content.iter().filter_map(|b| match b {
            ContentBlock::ToolResult(r) => Some(r),
            _ => None,
        })
    }

    /// A user-role message holding a compaction summary.
    pub fn summary(text: impl Into<String>, covers: usize) -> Self {
        Self { role: Role::User, content: vec![ContentBlock::Summary { text: text.into(), covers }] }
    }

    /// The summary block's text and coverage, if this is a summary message.
    pub fn as_summary(&self) -> Option<(&str, usize)> {
        self.content.iter().find_map(|b| match b {
            ContentBlock::Summary { text, covers } => Some((text.as_str(), *covers)),
            _ => None,
        })
    }
}

/// A tool the model may call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for the arguments object.
    pub input_schema: serde_json::Value,
}

/// Why the model stopped generating.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The model finished its turn.
    #[default]
    EndTurn,
    /// The model wants tool results before continuing.
    ToolUse,
    /// `max_tokens` was reached.
    MaxTokens,
    /// Anything else the provider reported.
    Other,
}

// ── Request / Response ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub capability: ModelCapability,
    pub profile: CapabilityProfile,
    pub policy: ExecutionPolicy,
    /// Conversation so far, oldest first. Must not be empty.
    pub messages: Vec<Message>,
    pub system_prompt: Option<String>,
    /// Tools offered to the model. Non-empty tools restrict routing to
    /// providers whose `supports_tool_use()` is true.
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub request_id: String,
    pub trace_id: String,
    pub stream: bool,
}

impl ModelRequest {
    /// A single-turn request with default profile, policy and sampling settings.
    pub fn text(capability: ModelCapability, prompt: impl Into<String>) -> Self {
        Self::conversation(capability, vec![Message::user(prompt)])
    }

    /// A request over an existing conversation with default settings.
    pub fn conversation(capability: ModelCapability, messages: Vec<Message>) -> Self {
        Self {
            capability,
            profile: CapabilityProfile::default(),
            policy: ExecutionPolicy::default(),
            messages,
            system_prompt: None,
            tools: Vec::new(),
            max_tokens: 1024,
            temperature: 0.7,
            request_id: uuid::Uuid::new_v4().to_string(),
            trace_id: uuid::Uuid::new_v4().to_string(),
            stream: false,
        }
    }

    pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(system_prompt.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = tools;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    pub provider_id: String,
    pub model_id: String,
    pub tier: ModelTier,
    pub latency_ms: u64,
    pub tokens_used: u32,
    pub cache_hit: bool,
    pub fallback_used: bool,
    pub retry_count: u32,
    pub circuit_state: CircuitState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    /// Assistant text, with multiple text blocks joined by newlines. May be
    /// empty when the model only returned tool calls.
    pub content: String,
    /// Tool calls requested by the model, in order.
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub stop_reason: StopReason,
    pub capability: ModelCapability,
    pub metadata: ExecutionMetadata,
    pub request_id: String,
}

impl ModelResponse {
    /// The assistant turn to append to the conversation history.
    pub fn to_message(&self) -> Message {
        let mut content = Vec::new();
        if !self.content.is_empty() {
            content.push(ContentBlock::Text { text: self.content.clone() });
        }
        content.extend(self.tool_calls.iter().cloned().map(ContentBlock::ToolCall));
        Message { role: Role::Assistant, content }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn content_blocks_round_trip_with_type_tags() {
        let msg = Message {
            role: Role::Assistant,
            content: vec![
                ContentBlock::Text { text: "Reading the file.".into() },
                ContentBlock::ToolCall(ToolCall {
                    id: "call_1".into(),
                    name: "read_file".into(),
                    arguments: json!({ "path": "src/lib.rs" }),
                }),
            ],
        };
        let value = serde_json::to_value(&msg).unwrap();
        assert_eq!(value["role"], "assistant");
        assert_eq!(value["content"][0]["type"], "text");
        assert_eq!(value["content"][1]["type"], "tool_call");
        assert_eq!(value["content"][1]["name"], "read_file");
        let back: Message = serde_json::from_value(value).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn text_request_has_one_user_message_and_no_tools() {
        let req = ModelRequest::text(ModelCapability::Reasoning, "hi").with_system_prompt("sys");
        assert_eq!(req.messages, vec![Message::user("hi")]);
        assert_eq!(req.system_prompt.as_deref(), Some("sys"));
        assert!(req.tools.is_empty());
    }

    #[test]
    fn response_to_message_keeps_text_then_tool_calls() {
        let call = ToolCall { id: "c1".into(), name: "ls".into(), arguments: json!({}) };
        let resp = ModelResponse {
            content: String::new(),
            tool_calls: vec![call.clone()],
            stop_reason: StopReason::ToolUse,
            capability: ModelCapability::ToolCalling,
            metadata: ExecutionMetadata {
                provider_id: "p".into(),
                model_id: "m".into(),
                tier: ModelTier::Local,
                latency_ms: 0,
                tokens_used: 0,
                cache_hit: false,
                fallback_used: false,
                retry_count: 0,
                circuit_state: CircuitState::Closed,
            },
            request_id: "r".into(),
        };
        let msg = resp.to_message();
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content, vec![ContentBlock::ToolCall(call)]);
        assert_eq!(msg.text(), "");
    }

    #[test]
    fn tool_results_message_is_a_user_turn() {
        let msg = Message::tool_results([ToolResult {
            call_id: "c1".into(),
            content: "ok".into(),
            is_error: false,
        }]);
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.tool_result_blocks().count(), 1);
    }
}
