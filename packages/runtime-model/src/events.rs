/// Telemetry events emitted by the Model Runtime.
///
/// Follows the same pattern as `runtime-knowledge/src/events.rs`:
/// each event type is a named struct, wrapped in a `ModelEvent` enum.
///
/// Events are emitted via `observability::EventRecorder` (injected into
/// the runtime at construction time). All event variants are `Serialize`
/// so they can be forwarded to any backend (NATS, stdout, OpenTelemetry).
use serde::{Deserialize, Serialize};

use crate::domain::ModelCapability;

// ── Event variants ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceStarted {
    pub request_id: String,
    pub trace_id: String,
    pub capability: ModelCapability,
    pub provider: String,
    pub model: String,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceCompleted {
    pub request_id: String,
    pub capability: ModelCapability,
    pub provider: String,
    pub model: String,
    pub latency_ms: u64,
    pub tokens_used: u32,
    pub cache_hit: bool,
    pub fallback_used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceFailed {
    pub request_id: String,
    pub capability: ModelCapability,
    pub provider: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderFallback {
    pub request_id: String,
    pub from_provider: String,
    pub to_provider: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitOpened {
    pub provider: String,
    pub failure_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitRecovered {
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHit {
    pub request_id: String,
    pub capability: ModelCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingCompleted {
    pub request_id: String,
    pub provider: String,
    pub input_count: usize,
    pub latency_ms: u64,
}

// ── Top-level enum ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelEvent {
    InferenceStarted(InferenceStarted),
    InferenceCompleted(InferenceCompleted),
    InferenceFailed(InferenceFailed),
    ProviderFallback(ProviderFallback),
    CircuitOpened(CircuitOpened),
    CircuitRecovered(CircuitRecovered),
    CacheHit(CacheHit),
    EmbeddingCompleted(EmbeddingCompleted),
}

impl ModelEvent {
    /// Stable string name for this event — used as the `name` argument to
    /// `observability::EventRecorder::record_event`.
    pub fn name(&self) -> &'static str {
        match self {
            Self::InferenceStarted(_) => "model.inference.started",
            Self::InferenceCompleted(_) => "model.inference.completed",
            Self::InferenceFailed(_) => "model.inference.failed",
            Self::ProviderFallback(_) => "model.provider.fallback",
            Self::CircuitOpened(_) => "model.circuit.opened",
            Self::CircuitRecovered(_) => "model.circuit.recovered",
            Self::CacheHit(_) => "model.cache.hit",
            Self::EmbeddingCompleted(_) => "model.embedding.completed",
        }
    }

    /// Convert to the `HashMap<String, String>` format expected by
    /// `observability::EventRecorder::record_event`.
    pub fn attributes(&self) -> std::collections::HashMap<String, String> {
        // Serialize to JSON value and flatten top-level fields into the map.
        let json = serde_json::to_value(self).unwrap_or_default();
        let mut map = std::collections::HashMap::new();
        if let serde_json::Value::Object(obj) = json {
            // Skip the outer enum wrapper key
            for (_, inner) in obj {
                if let serde_json::Value::Object(fields) = inner {
                    for (k, v) in fields {
                        map.insert(k, v.to_string().trim_matches('"').to_string());
                    }
                }
            }
        }
        map
    }
}
