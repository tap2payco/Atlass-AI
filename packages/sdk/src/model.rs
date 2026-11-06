use async_trait::async_trait;
use tokio::sync::mpsc;

use shared::error::InferenceError;
use platform_contracts::health::HealthState;

// Re-export all model types from `shared::model` so callers import only from `sdk::model`.
pub use shared::model::{
    CapabilityProfile, CircuitState, ContentBlock, ExecutionMetadata, ExecutionPolicy, Message,
    ModelCapability, ModelRequest, ModelResponse, ModelTier, Role, StopReason, ToolCall,
    ToolResult, ToolSpec,
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
/// use sdk::model::{ModelRuntime, ModelRequest, ModelCapability};
///
/// # async fn example(runtime: &dyn ModelRuntime) {
/// let request = ModelRequest::text(ModelCapability::Reasoning, "Why is the sky blue?");
/// let response = runtime.execute_inference(request).await.unwrap();
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
