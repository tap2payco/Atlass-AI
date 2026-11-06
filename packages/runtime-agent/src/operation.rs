//! The `Operation` trait: one rule of the agent loop.
//!
//! Each step, the state machine asks its operations in order whether they
//! apply. The first that does produces messages to append and a status that
//! says whether the loop continues. Operations hold no turn state; they read
//! it from the conversation (see `conversation.rs`).

use async_trait::async_trait;
use shared::model::{Message, ToolSpec};

use crate::error::AgentError;
use crate::events::EventSink;
use crate::session::Session;

/// Why a run stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopCause {
    /// The model finished its turn without asking for tools.
    EndTurn,
    /// The configured limit on model responses per turn was reached.
    MaxTurns,
    /// The run was cancelled.
    Cancelled,
    /// No operation applied (e.g. the conversation has no pending request).
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Continue,
    Stop(StopCause),
}

/// What an operation decided to do this step.
#[derive(Debug)]
pub enum OperationResult {
    NotApplicable,
    Applied { messages: Vec<Message>, status: RunStatus },
}

impl OperationResult {
    pub fn continue_with(messages: Vec<Message>) -> Self {
        Self::Applied { messages, status: RunStatus::Continue }
    }

    pub fn stop_with(messages: Vec<Message>, cause: StopCause) -> Self {
        Self::Applied { messages, status: RunStatus::Stop(cause) }
    }
}

/// Everything an operation sees for one step.
pub struct StepContext<'a> {
    pub session: &'a Session,
    /// Tools contributed by all operations, for the next model request.
    pub tools: &'a [ToolSpec],
    /// System-prompt sections contributed by all operations, in order.
    pub prompt_parts: &'a [String],
    pub events: &'a EventSink,
}

#[async_trait]
pub trait Operation: Send + Sync {
    /// Stable name, used in events and logs.
    fn name(&self) -> &'static str;

    /// Tools this operation makes available to the model.
    fn tools(&self) -> Vec<ToolSpec> {
        Vec::new()
    }

    /// Text this operation adds to the system prompt.
    fn prompt_parts(&self, _session: &Session) -> Vec<String> {
        Vec::new()
    }

    async fn run(&self, ctx: &StepContext<'_>) -> Result<OperationResult, AgentError>;
}
