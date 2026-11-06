use async_trait::async_trait;

use crate::conversation::{assistant_messages_in_turn, awaiting_model};
use crate::error::AgentError;
use crate::operation::{Operation, OperationResult, StepContext, StopCause};

/// Stops the run before another model call once the current turn already has
/// `max` model responses. A new user message starts a fresh count.
pub struct MaxTurnsOperation {
    max: usize,
}

impl MaxTurnsOperation {
    pub fn new(max: usize) -> Self {
        Self { max }
    }
}

#[async_trait]
impl Operation for MaxTurnsOperation {
    fn name(&self) -> &'static str {
        "max_turns"
    }

    async fn run(&self, ctx: &StepContext<'_>) -> Result<OperationResult, AgentError> {
        let messages = &ctx.session.messages;
        if awaiting_model(messages) && assistant_messages_in_turn(messages) >= self.max {
            return Ok(OperationResult::stop_with(Vec::new(), StopCause::MaxTurns));
        }
        Ok(OperationResult::NotApplicable)
    }
}
