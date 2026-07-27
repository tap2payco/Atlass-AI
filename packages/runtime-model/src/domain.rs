/// Domain types for the Model Runtime.
///
/// Core vocabulary types (`ModelCapability`, `ModelRequest`, `ModelResponse`,
/// `ExecutionMetadata`, `ExecutionPolicy`, `CapabilityProfile`, `ModelTier`,
/// `CircuitState`) live in `shared::model` so both `sdk` and `runtime-model`
/// can import them without circular dependencies.
///
/// This module re-exports those types and adds the few runtime-internal types
/// (`ToolDefinition`, `ToolCallResponse`) that are implementation details
/// not needed by callers.

// Re-export the shared vocabulary so the rest of this crate imports from
// `crate::domain` (a single stable location).
pub use shared::model::{
    CapabilityProfile, CircuitState, ExecutionMetadata, ExecutionPolicy,
    ModelCapability, ModelRequest, ModelResponse, ModelTier,
};

use serde::{Deserialize, Serialize};

// ── Tool calling types (runtime-internal) ─────────────────────────────────────

/// Describes a tool exposed to the model for structured function calling.
/// Used only by providers that advertise `ModelCapability::ToolCalling`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema describing the input parameters.
    pub parameters_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResponse {
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub metadata: ExecutionMetadata,
    pub request_id: String,
}
