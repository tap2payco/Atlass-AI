use shared::error::InferenceError;
use shared::id::SessionId;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("session not found: {0}")]
    SessionNotFound(SessionId),

    #[error("session store error: {0}")]
    Store(String),

    /// The model call failed. The conversation is unchanged, so the turn can
    /// be retried by running the session again.
    #[error("model inference failed: {0}")]
    Inference(#[from] InferenceError),

    #[error("invalid agent configuration: {0}")]
    Config(String),
}
