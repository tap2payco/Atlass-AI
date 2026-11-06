//! `Agent`: the entry point that wires the default operations together.

use std::sync::Arc;

use sdk::model::ModelRuntime;
use shared::id::SessionId;
use shared::model::{Message, Role};
use tokio_util::sync::CancellationToken;

use crate::error::AgentError;
use crate::events::EventSink;
use crate::machine::StateMachine;
use crate::operation::{Operation, StopCause};
use crate::ops::{InferenceOperation, InferenceSettings, MaxTurnsOperation, ToolExecutionOperation};
use crate::session::{Session, SessionStore};
use crate::tool::Tool;

pub const DEFAULT_MAX_TURNS: usize = 25;

/// The result of running a turn.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnOutcome {
    pub cause: StopCause,
    /// Text of the last assistant message, if the conversation ends with one.
    pub reply: Option<String>,
}

pub struct Agent {
    machine: StateMachine,
    store: Arc<dyn SessionStore>,
}

impl Agent {
    pub fn builder(model: Arc<dyn ModelRuntime>, store: Arc<dyn SessionStore>) -> AgentBuilder {
        AgentBuilder {
            model,
            store,
            tools: Vec::new(),
            extra_operations: Vec::new(),
            settings: InferenceSettings::default(),
            max_turns: DEFAULT_MAX_TURNS,
        }
    }

    pub async fn create_session(&self) -> Result<SessionId, AgentError> {
        let session = Session::new();
        let id = session.id;
        self.store.create(session).await?;
        Ok(id)
    }

    /// Append a user message and run until the agent stops.
    pub async fn submit(
        &self,
        session_id: SessionId,
        text: impl Into<String>,
        cancel: &CancellationToken,
        events: &EventSink,
    ) -> Result<TurnOutcome, AgentError> {
        self.store.append(session_id, vec![Message::user(text)]).await?;
        self.resume(session_id, cancel, events).await
    }

    /// Continue a session from its stored state, e.g. after a crash, a
    /// cancelled run, a model error, or reaching the turn limit.
    pub async fn resume(
        &self,
        session_id: SessionId,
        cancel: &CancellationToken,
        events: &EventSink,
    ) -> Result<TurnOutcome, AgentError> {
        let cause = self.machine.run(session_id, cancel, events).await?;
        let session = self.store.load(session_id).await?;
        let reply = session
            .messages
            .last()
            .filter(|m| m.role == Role::Assistant)
            .map(Message::text);
        Ok(TurnOutcome { cause, reply })
    }

    pub async fn session(&self, session_id: SessionId) -> Result<Session, AgentError> {
        self.store.load(session_id).await
    }
}

pub struct AgentBuilder {
    model: Arc<dyn ModelRuntime>,
    store: Arc<dyn SessionStore>,
    tools: Vec<Arc<dyn Tool>>,
    extra_operations: Vec<Arc<dyn Operation>>,
    settings: InferenceSettings,
    max_turns: usize,
}

impl AgentBuilder {
    pub fn tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add an operation that runs before the built-in ones (e.g. approvals or
    /// hooks), in the order added.
    pub fn operation(mut self, operation: Arc<dyn Operation>) -> Self {
        self.extra_operations.push(operation);
        self
    }

    pub fn settings(mut self, settings: InferenceSettings) -> Self {
        self.settings = settings;
        self
    }

    /// Maximum model responses per user turn. Must be at least 1.
    pub fn max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    pub fn build(self) -> Result<Agent, AgentError> {
        if self.max_turns == 0 {
            return Err(AgentError::Config("max_turns must be at least 1".into()));
        }
        let mut operations = self.extra_operations;
        operations.push(Arc::new(ToolExecutionOperation::new(self.tools)?));
        operations.push(Arc::new(MaxTurnsOperation::new(self.max_turns)));
        operations.push(Arc::new(InferenceOperation::new(self.model, self.settings)));
        Ok(Agent { machine: StateMachine::new(operations, self.store.clone())?, store: self.store })
    }
}
