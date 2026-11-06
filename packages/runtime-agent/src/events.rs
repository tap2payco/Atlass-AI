//! Events a running agent emits, for UIs and logs.

use shared::model::Message;
use tokio::sync::mpsc;

use crate::operation::StopCause;

#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    /// An operation applied and its messages were stored.
    StepApplied { operation: &'static str },
    /// A message was appended to the conversation.
    MessageAppended(Message),
    /// A tool started running.
    ToolStarted { call_id: String, name: String },
    /// The approver denied a tool call; it was not run.
    ToolDenied { call_id: String, name: String },
    /// The run ended.
    Finished(StopCause),
}

/// Optional event channel. Sending never blocks or fails the run: a closed or
/// absent receiver just drops events.
#[derive(Clone, Default)]
pub struct EventSink {
    tx: Option<mpsc::UnboundedSender<AgentEvent>>,
}

impl EventSink {
    pub fn none() -> Self {
        Self { tx: None }
    }

    pub fn channel() -> (Self, mpsc::UnboundedReceiver<AgentEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx: Some(tx) }, rx)
    }

    pub fn emit(&self, event: AgentEvent) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(event);
        }
    }
}
