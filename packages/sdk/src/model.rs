use async_trait::async_trait;
use tokio::sync::mpsc;

use shared::error::InferenceError;
use platform_contracts::health::HealthState;

// Re-export all model types from `shared::model` so callers import only from `sdk::model`.
pub use shared::model::{
    CapabilityProfile, CircuitState, ExecutionMetadata, ExecutionPolicy,
    ModelCapability, ModelRequest, ModelResponse, ModelTier,
};

/// The public SDK contract for the Model Runtime.
///
/// Every cognitive runtime (reasoning, code-gen, summarization) depends
/// on this trait — never on a concrete provider. The implementation lives
/// in `packages/runtime-model`.
///
/// Callers ask for a **capability** + **profile**. The runtime decides
/// which provider answers.
///
/// # Example
///
/// ```rust,no_run
/// # async fn example(runtime: &dyn ModelRuntime) {
/// use sdk::model::{ModelRequest, ModelCapability, CapabilityProfile, ExecutionPolicy};
///
/// let response = runtime.execute_inference(ModelRequest {
///     capability:    ModelCapability::Reasoning,
///     profile:       CapabilityProfile::Balanced,
///     policy:        ExecutionPolicy::PreferLocal,
///     prompt:        "Why is the sky blue?".into(),
///     system_prompt: None,
///     max_tokens:    512,
///     temperature:   0.7,
///     request_id:    "req-001".into(),
///     trace_id:      "trace-001".into(),
///     stream:        false,
/// }).await.unwrap();
///
/// println!("Provider: {} ({})", response.metadata.provider_id, response.metadata.model_id);
/// # }
/// ```
#[async_trait]
pub trait ModelRuntime: Send + Sync {
    /// Execute a single inference request and return the full response.
    ///
    /// Internally performs: cache lookup → primary provider → fallback chain.
    /// `ModelResponse.metadata.cache_hit = true` when the response was served
    /// from cache without calling any provider.
    async fn execute_inference(
        &self,
        request: ModelRequest,
    ) -> Result<ModelResponse, InferenceError>;

    /// Execute a streaming inference request.
    ///
    /// Content tokens are sent over `tx` as they arrive from the provider.
    /// Returns `ExecutionMetadata` when the stream is fully complete.
    async fn execute_stream(
        &self,
        request: ModelRequest,
        tx: mpsc::Sender<String>,
    ) -> Result<ExecutionMetadata, InferenceError>;

    /// Report the aggregate health of the runtime.
    async fn health_check(&self) -> HealthState;
}
