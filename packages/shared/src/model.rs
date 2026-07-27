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

// ── Request / Response ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub capability: ModelCapability,
    pub profile: CapabilityProfile,
    pub policy: ExecutionPolicy,
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub request_id: String,
    pub trace_id: String,
    pub stream: bool,
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
    pub content: String,
    pub capability: ModelCapability,
    pub metadata: ExecutionMetadata,
    pub request_id: String,
}
