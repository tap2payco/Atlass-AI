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
use crate::ops::{
    CompactionOperation, CompactionSettings, InferenceOperation, InferenceSettings, MaxTurnsOperation,
    ToolExecutionOperation,
};
use crate::session::{Session, SessionStore};
use crate::approval::{AutoApprove, ToolApprover};
use crate::tool::{Tool, ToolLimits};

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
            tool_limits: ToolLimits::default(),
            approver: Arc::new(AutoApprove),
            compaction: None,
        }
    }

    pub async fn create_session(&self) -> Result<SessionId, AgentError> {
        self.create_session_with(std::collections::BTreeMap::new()).await
    }

    /// Create a session carrying labels (e.g. the workspace path) that a
    /// store such as `FileSessionStore` keeps with it.
    pub async fn create_session_with(
        &self,
        metadata: std::collections::BTreeMap<String, String>,
    ) -> Result<SessionId, AgentError> {
        let session = Session::with_metadata(metadata);
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
    tool_limits: ToolLimits,
    approver: Arc<dyn ToolApprover>,
    compaction: Option<CompactionSettings>,
}

impl AgentBuilder {
    /// Asked before every tool call that can change something. Default:
    /// `AutoApprove` (allow everything).
    pub fn approver(mut self, approver: Arc<dyn ToolApprover>) -> Self {
        self.approver = approver;
        self
    }

    /// Timeout and output cap applied to every tool call.
    pub fn tool_limits(mut self, limits: ToolLimits) -> Self {
        self.tool_limits = limits;
        self
    }

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

    /// Summarise older history when the conversation nears the context
    /// window. Off unless set.
    pub fn compaction(mut self, settings: CompactionSettings) -> Self {
        self.compaction = Some(settings);
        self
    }

    pub fn build(self) -> Result<Agent, AgentError> {
        if self.max_turns == 0 {
            return Err(AgentError::Config("max_turns must be at least 1".into()));
        }
        if let Some(c) = &self.compaction {
            if c.context_tokens == 0 || !(0.1..=1.0).contains(&c.threshold) {
                return Err(AgentError::Config(
                    "compaction needs context_tokens > 0 and a threshold between 0.1 and 1.0".into(),
                ));
            }
        }
        let mut operations = self.extra_operations;
        operations.push(Arc::new(
            ToolExecutionOperation::new(self.tools, self.tool_limits)?.with_approver(self.approver),
        ));
        operations.push(Arc::new(MaxTurnsOperation::new(self.max_turns)));
        if let Some(settings) = self.compaction {
            operations.push(Arc::new(CompactionOperation::new(self.model.clone(), settings)));
        }
        operations.push(Arc::new(InferenceOperation::new(self.model, self.settings)));
        Ok(Agent { machine: StateMachine::new(operations, self.store.clone())?, store: self.store })
    }
}
