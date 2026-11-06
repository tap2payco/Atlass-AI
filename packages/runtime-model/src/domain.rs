/// Domain types for the Model Runtime.
///
/// The model vocabulary (requests, responses, conversation messages, tool
/// specs and calls) lives in `shared::model` so both `sdk` and `runtime-model`
/// can import it without circular dependencies. See ADR-017.
///
/// This module re-exports it so the rest of this crate imports from
/// `crate::domain` (a single stable location).
pub use shared::model::{
    CapabilityProfile, CircuitState, ContentBlock, ExecutionMetadata, ExecutionPolicy, Message,
    ModelCapability, ModelRequest, ModelResponse, ModelTier, Role, StopReason, ToolCall,
    ToolResult, ToolSpec,
};
