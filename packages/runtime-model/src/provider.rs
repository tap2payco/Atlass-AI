/// The hexagonal port for model providers.
///
/// Every concrete adapter (Ollama, Fireworks, future providers) must implement
/// this trait. The routing layer only depends on this trait — it never imports
/// concrete adapter types.
///
/// ## Capability gating
///
/// Before calling `infer`, `stream`, `tool_call`, or `embeddings`, the router
/// checks `supported_capabilities()`. If the desired capability is not
/// advertised, it skips this provider entirely — no runtime panics.
///
/// ## Optional methods
///
/// `tool_call` and `embeddings` return `Err(InferenceError::Unsupported(...))`
/// by default. Providers that support them override these methods.
use async_trait::async_trait;
use tokio::sync::mpsc;
use platform_contracts::health::HealthState;
use shared::error::InferenceError;

use crate::domain::{
    ExecutionMetadata, ModelCapability, ModelRequest, ModelResponse,
    ModelTier, ToolCallResponse, ToolDefinition,
};

#[async_trait]
pub trait ModelProvider: Send + Sync {
    // ── Identity ────────────────────────────────────────────────────────────

    /// Stable, lowercase identifier. Used in logs, events, and `ExecutionMetadata`.
    /// Must not change across restarts (e.g. "ollama", "fireworks").
    fn provider_id(&self) -> &str;

    /// Whether this provider is local or external.
    fn tier(&self) -> ModelTier;

    /// The set of capabilities this provider can serve.
    ///
    /// The router consults this list before dispatching any request.
    /// A provider must never be called for a capability it does not advertise.
    fn supported_capabilities(&self) -> &[ModelCapability];

    // ── Core inference ───────────────────────────────────────────────────────

    /// Blocking (buffered) inference. Returns the full response when complete.
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse, InferenceError>;

    /// Streaming inference. Content tokens are sent over `tx` as they arrive.
    ///
    /// Returns `ExecutionMetadata` when the stream ends (success or error).
    /// The caller is responsible for closing or dropping the receiver.
    async fn stream(
        &self,
        request: &ModelRequest,
        tx: mpsc::Sender<String>,
    ) -> Result<ExecutionMetadata, InferenceError>;

    // ── Optional capabilities ────────────────────────────────────────────────

    /// Generate dense vector embeddings for a batch of texts.
    ///
    /// Defaults to `Err(InferenceError::Unsupported)`. Providers that support
    /// embeddings (e.g. Ollama with bge-m3) override this method.
    async fn embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError> {
        let _ = texts;
        Err(InferenceError::Unsupported(format!(
            "Provider '{}' does not support embeddings",
            self.provider_id()
        )))
    }

    /// Structured tool/function calling.
    ///
    /// Defaults to `Err(InferenceError::Unsupported)`. Only overridden by
    /// providers that advertise `ModelCapability::ToolCalling`.
    async fn tool_call(
        &self,
        request: &ModelRequest,
        tools: &[ToolDefinition],
    ) -> Result<ToolCallResponse, InferenceError> {
        let _ = (request, tools);
        Err(InferenceError::Unsupported(format!(
            "Provider '{}' does not support tool calling",
            self.provider_id()
        )))
    }

    // ── Health ───────────────────────────────────────────────────────────────

    /// Return the current health of this provider.
    ///
    /// The router uses this to seed the circuit breaker's initial state and to
    /// skip providers that report `Offline` before attempting a call.
    async fn health(&self) -> HealthState;
}
