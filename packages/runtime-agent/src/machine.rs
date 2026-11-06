//! The agent state machine (pattern from Goose's `goose-agent` crate; see
//! `docs/appendix/research/goose-codex-reference-study.md` §3).
//!
//! ```text
//! loop:
//!   cancelled?            → finalize (answer open tool calls) → Stop(Cancelled)
//!   session = store.load(id)
//!   for op in operations:  first op that applies → append its messages
//!       Continue          → next iteration (session reloaded from the store)
//!       Stop(cause)       → return cause
//!   nothing applied       → Stop(Idle)
//! ```

use std::collections::HashSet;
use std::sync::Arc;

use shared::id::SessionId;
use shared::model::{Message, ToolResult, ToolSpec};
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::conversation::open_tool_calls;
use crate::error::AgentError;
use crate::events::{AgentEvent, EventSink};
use crate::operation::{Operation, OperationResult, RunStatus, StepContext, StopCause};
use crate::session::SessionStore;

pub const INTERRUPTED_TOOL_MESSAGE: &str = "Tool call was interrupted before completing.";

pub struct StateMachine {
    operations: Vec<Arc<dyn Operation>>,
    tools: Vec<ToolSpec>,
    store: Arc<dyn SessionStore>,
}

impl StateMachine {
    /// Fails if two operations contribute a tool with the same name.
    pub fn new(
        operations: Vec<Arc<dyn Operation>>,
        store: Arc<dyn SessionStore>,
    ) -> Result<Self, AgentError> {
        let mut names = HashSet::new();
        let mut tools = Vec::new();
        for op in &operations {
            for tool in op.tools() {
                if !names.insert(tool.name.clone()) {
                    return Err(AgentError::Config(format!(
                        "tool '{}' is registered by more than one operation",
                        tool.name
                    )));
                }
                tools.push(tool);
            }
        }
        Ok(Self { operations, tools, store })
    }

    pub fn tools(&self) -> &[ToolSpec] {
        &self.tools
    }

    /// Run the session until an operation stops, nothing applies, or `cancel`
    /// fires. Every applied step is persisted before the next one starts.
    pub async fn run(
        &self,
        session_id: SessionId,
        cancel: &CancellationToken,
        events: &EventSink,
    ) -> Result<StopCause, AgentError> {
        let cause = self.run_inner(session_id, cancel, events).await?;
        events.emit(AgentEvent::Finished(cause));
        Ok(cause)
    }

    async fn run_inner(
        &self,
        session_id: SessionId,
        cancel: &CancellationToken,
        events: &EventSink,
    ) -> Result<StopCause, AgentError> {
        'steps: loop {
            if cancel.is_cancelled() {
                return self.finalize_cancelled(session_id, events).await;
            }

            let session = self.store.load(session_id).await?;
            for op in &self.operations {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        return self.finalize_cancelled(session_id, events).await;
                    }
                    result = op.prepare(&session) => result?,
                }
            }
            let prompt_parts: Vec<String> =
                self.operations.iter().flat_map(|op| op.prompt_parts(&session)).collect();
            let ctx = StepContext {
                session: &session,
                tools: &self.tools,
                prompt_parts: &prompt_parts,
                events,
            };

            for op in &self.operations {
                let result = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        return self.finalize_cancelled(session_id, events).await;
                    }
                    result = op.run(&ctx) => result?,
                };

                let OperationResult::Applied { messages, status } = result else {
                    continue;
                };
                debug!(operation = op.name(), messages = messages.len(), ?status, "step applied");

                if messages.is_empty() && status == RunStatus::Continue {
                    return Err(AgentError::Config(format!(
                        "operation '{}' asked to continue without changing the conversation",
                        op.name()
                    )));
                }
                self.append(session_id, messages, events).await?;
                events.emit(AgentEvent::StepApplied { operation: op.name() });

                match status {
                    RunStatus::Continue => continue 'steps,
                    RunStatus::Stop(cause) => return Ok(cause),
                }
            }

            return Ok(StopCause::Idle);
        }
    }

    /// Answer every open tool call with an error result, so the stored
    /// conversation stays valid for the next model request.
    async fn finalize_cancelled(
        &self,
        session_id: SessionId,
        events: &EventSink,
    ) -> Result<StopCause, AgentError> {
        let session = self.store.load(session_id).await?;
        let results: Vec<ToolResult> = open_tool_calls(&session.messages)
            .into_iter()
            .map(|call| ToolResult {
                call_id: call.id.clone(),
                content: INTERRUPTED_TOOL_MESSAGE.into(),
                is_error: true,
            })
            .collect();
        if !results.is_empty() {
            self.append(session_id, vec![Message::tool_results(results)], events).await?;
        }
        Ok(StopCause::Cancelled)
    }

    async fn append(
        &self,
        session_id: SessionId,
        messages: Vec<Message>,
        events: &EventSink,
    ) -> Result<(), AgentError> {
        if messages.is_empty() {
            return Ok(());
        }
        self.store.append(session_id, messages.clone()).await?;
        for message in messages {
            events.emit(AgentEvent::MessageAppended(message));
        }
        Ok(())
    }
}
