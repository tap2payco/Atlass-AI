//! The client protocol (roadmap step 5, after Codex's submission/event
//! queues): clients drive an agent by sending `Op`s and render the `Event`s
//! it sends back. Both are tagged JSON (`{"type": "turn_input", ...}`), so
//! the same protocol runs over JSON lines on stdio, a WebSocket or an HTTP
//! stream.
//!
//! ```text
//! client ── Op ──▶ AgentDriver ── submit/resume ──▶ Agent
//!        ◀─ Event ─            ◀── AgentEvent ────
//!                  ApprovalBroker ◀── ToolApprover::approve
//! ```
//!
//! - `AgentDriver::run` owns one agent and one current session. It runs at
//!   most one turn at a time and keeps reading ops while a turn runs, so
//!   `Interrupt` and `ApprovalResponse` arrive mid-turn.
//! - `ApprovalBroker` is a `ToolApprover` that asks the client: it sends
//!   `ApprovalRequest` and waits for the matching `ApprovalResponse`. No
//!   answer within the timeout, an interrupt, or a closed client all deny.
//! - Every turn ends with exactly one `TurnComplete`, also when it fails
//!   (an `Error` event comes first, and the cause is `failed`).
//!
//! The types live here rather than in `runtime-bus` because they carry agent
//! concepts (stop causes, tool effects, sessions); `runtime-bus` stays a
//! generic envelope crate, and no new dependency edge is needed.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shared::id::SessionId;
use shared::model::{Role, ToolCall};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::agent::{Agent, TurnOutcome};
use crate::approval::{ApprovalDecision, ToolApprover};
use crate::error::AgentError;
use crate::events::{AgentEvent, EventSink};
use crate::operation::StopCause;
use crate::tool::{truncate_output, ToolEffect};

/// How long an `ApprovalRequest` waits for an answer before it is denied.
pub const DEFAULT_APPROVAL_TIMEOUT: Duration = Duration::from_secs(300);

/// Longest tool output copied into a `ToolCallEnd` event, in characters.
pub const EVENT_OUTPUT_CHARS: usize = 4000;

// ── Wire types ───────────────────────────────────────────────────────────────

/// A request from the client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Op {
    /// Send a user message and run a turn. Without `session` the current
    /// session is used (one is created at start-up).
    TurnInput {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<SessionId>,
        text: String,
    },
    /// Cancel the running turn. Open tool calls get an error result and
    /// pending approvals are denied. Ignored when no turn is running.
    Interrupt,
    /// Answer an `ApprovalRequest`.
    ApprovalResponse {
        call_id: String,
        decision: ReviewDecision,
        /// Shown to the model when the call is denied.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// Start a fresh conversation and make it current.
    NewSession,
    /// Make a stored conversation current.
    ResumeSession { session: SessionId },
}

/// The client's answer to an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Allow,
    Deny,
}

/// Something the agent reports to the client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// The current session changed (start-up, `NewSession`, `ResumeSession`).
    SessionConfigured {
        session: SessionId,
        /// Messages already stored in the session.
        message_count: usize,
        resumed: bool,
    },
    /// Part of an assistant message, for streaming clients. The current
    /// agent loop does not stream, so it sends whole `AgentMessage`s; this
    /// variant is reserved so clients can handle both.
    MessageDelta { delta: String },
    /// Text the assistant wrote: commentary next to tool calls, or the reply.
    AgentMessage { text: String },
    /// A tool call started (after approval, if one was needed).
    ToolCallBegin { call_id: String, tool: String, arguments: serde_json::Value },
    /// A started tool call finished. `output` may be shortened.
    ToolCallEnd { call_id: String, output: String, is_error: bool },
    /// The call was denied and did not run.
    ToolDenied { call_id: String, tool: String },
    /// The agent wants to run a call that changes something. Answer with
    /// `ApprovalResponse` carrying the same `call_id`.
    ApprovalRequest {
        call_id: String,
        tool: String,
        /// One line describing the call, e.g. `run_command cargo test`.
        summary: String,
        effect: String,
        arguments: serde_json::Value,
    },
    /// Older history was summarised to stay within the context window.
    Compacted { covers: usize },
    /// The turn ended. Sent exactly once per `TurnInput` that started a turn.
    TurnComplete {
        session: SessionId,
        cause: TurnCause,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reply: Option<String>,
    },
    /// Something went wrong: a bad op, a failed model call, a missing session.
    Error { message: String },
}

/// Why a turn ended: `StopCause`, plus `Failed` for turns that errored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnCause {
    EndTurn,
    MaxTurns,
    Cancelled,
    Idle,
    Failed,
}

impl From<StopCause> for TurnCause {
    fn from(cause: StopCause) -> Self {
        match cause {
            StopCause::EndTurn => Self::EndTurn,
            StopCause::MaxTurns => Self::MaxTurns,
            StopCause::Cancelled => Self::Cancelled,
            StopCause::Idle => Self::Idle,
        }
    }
}

/// Encode a value as one JSON line (with the trailing newline).
pub fn encode_line<T: Serialize>(value: &T) -> String {
    let mut line = serde_json::to_string(value).expect("protocol types always serialise");
    line.push('\n');
    line
}

/// Decode one JSON line into an `Op`. Surrounding whitespace is ignored.
pub fn decode_op(line: &str) -> Result<Op, String> {
    serde_json::from_str(line.trim()).map_err(|e| format!("invalid op: {e}"))
}

/// Snake-case name of a tool effect, as used on the wire.
pub fn effect_name(effect: ToolEffect) -> &'static str {
    match effect {
        ToolEffect::ReadOnly => "read_only",
        ToolEffect::WritesWorkspace => "writes_workspace",
        ToolEffect::RunsProcesses => "runs_processes",
    }
}

/// One line describing a call: the tool name and its main argument.
pub fn summarize_call(call: &ToolCall) -> String {
    let arg = ["command", "path", "query"]
        .iter()
        .find_map(|key| call.arguments.get(*key).and_then(|v| v.as_str()));
    match arg {
        Some(value) => {
            let short: String = value.chars().take(120).collect();
            let ellipsis = if value.chars().count() > 120 { "…" } else { "" };
            format!("{} {short}{ellipsis}", call.name)
        }
        None => call.name.clone(),
    }
}

// ── Approvals ────────────────────────────────────────────────────────────────

struct BrokerState {
    events: Option<mpsc::UnboundedSender<Event>>,
    pending: HashMap<String, oneshot::Sender<ApprovalDecision>>,
}

/// A `ToolApprover` that asks the protocol client. Install
/// `broker.approver()` on the `AgentBuilder`, then pass the same broker to
/// `AgentDriver::with_approvals`.
#[derive(Clone)]
pub struct ApprovalBroker {
    state: Arc<Mutex<BrokerState>>,
    timeout: Duration,
}

impl ApprovalBroker {
    pub fn new(timeout: Duration) -> Self {
        Self { state: Arc::new(Mutex::new(BrokerState { events: None, pending: HashMap::new() })), timeout }
    }

    pub fn approver(&self) -> Arc<dyn ToolApprover> {
        Arc::new(self.clone())
    }

    fn attach(&self, events: mpsc::UnboundedSender<Event>) {
        self.state.lock().unwrap().events = Some(events);
    }

    /// Stop asking: deny everything pending, and later calls immediately.
    fn detach(&self, reason: &str) {
        let mut state = self.state.lock().unwrap();
        state.events = None;
        Self::deny_pending(&mut state, reason);
    }

    fn deny_all(&self, reason: &str) {
        Self::deny_pending(&mut self.state.lock().unwrap(), reason);
    }

    fn deny_pending(state: &mut BrokerState, reason: &str) {
        for (_, tx) in state.pending.drain() {
            let _ = tx.send(ApprovalDecision::Deny { reason: reason.to_string() });
        }
    }

    /// Deliver the client's answer. Returns false when nothing waits for `call_id`.
    fn resolve(&self, call_id: &str, decision: ApprovalDecision) -> bool {
        match self.state.lock().unwrap().pending.remove(call_id) {
            Some(tx) => tx.send(decision).is_ok(),
            None => false,
        }
    }
}

#[async_trait]
impl ToolApprover for ApprovalBroker {
    async fn approve(&self, call: &ToolCall, effect: ToolEffect) -> ApprovalDecision {
        let rx = {
            let mut state = self.state.lock().unwrap();
            let Some(events) = state.events.clone() else {
                return ApprovalDecision::Deny { reason: "no client is connected to approve it".into() };
            };
            let (tx, rx) = oneshot::channel();
            state.pending.insert(call.id.clone(), tx);
            let request = Event::ApprovalRequest {
                call_id: call.id.clone(),
                tool: call.name.clone(),
                summary: summarize_call(call),
                effect: effect_name(effect).to_string(),
                arguments: call.arguments.clone(),
            };
            if events.send(request).is_err() {
                state.pending.remove(&call.id);
                return ApprovalDecision::Deny { reason: "no client is connected to approve it".into() };
            }
            rx
        };
        // Removes the entry if this future is dropped (e.g. the turn was cancelled).
        let _cleanup = PendingGuard { state: &self.state, call_id: &call.id };
        match tokio::time::timeout(self.timeout, rx).await {
            Ok(Ok(decision)) => decision,
            Ok(Err(_)) => ApprovalDecision::Deny { reason: "the approval request was abandoned".into() },
            Err(_) => ApprovalDecision::Deny {
                reason: format!("no answer within {} seconds", self.timeout.as_secs()),
            },
        }
    }
}

struct PendingGuard<'a> {
    state: &'a Mutex<BrokerState>,
    call_id: &'a str,
}

impl Drop for PendingGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.pending.remove(self.call_id);
        }
    }
}

// ── Driver ───────────────────────────────────────────────────────────────────

type TurnFuture = Pin<Box<dyn Future<Output = (SessionId, Result<TurnOutcome, AgentError>)> + Send>>;

/// Runs an `Agent` for one protocol client.
pub struct AgentDriver {
    agent: Arc<Agent>,
    approvals: Option<ApprovalBroker>,
    metadata: BTreeMap<String, String>,
    initial_session: Option<SessionId>,
}

impl AgentDriver {
    pub fn new(agent: Agent) -> Self {
        Self::shared(Arc::new(agent))
    }

    pub fn shared(agent: Arc<Agent>) -> Self {
        Self { agent, approvals: None, metadata: BTreeMap::new(), initial_session: None }
    }

    /// Route approval requests to the client. `broker.approver()` must be
    /// the agent's approver for this to have any effect.
    pub fn with_approvals(mut self, broker: ApprovalBroker) -> Self {
        self.approvals = Some(broker);
        self
    }

    /// Labels stored with every session this driver creates.
    pub fn with_session_metadata(mut self, metadata: BTreeMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Start on a stored session instead of creating a new one.
    pub fn with_session(mut self, session: SessionId) -> Self {
        self.initial_session = Some(session);
        self
    }

    /// Spawn `run` on the tokio runtime and return the client's ends.
    pub fn spawn(self) -> (mpsc::UnboundedSender<Op>, mpsc::UnboundedReceiver<Event>) {
        let (op_tx, op_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        tokio::spawn(self.run(op_rx, event_tx));
        (op_tx, event_rx)
    }

    /// Serve ops until the op channel closes. A turn that is running then
    /// finishes (approvals it asks for are denied, as no one can answer) and
    /// its events are sent before this returns.
    pub async fn run(self, mut ops: mpsc::UnboundedReceiver<Op>, events: mpsc::UnboundedSender<Event>) {
        // Approval requests go through their own channel so they can be
        // ordered after the agent events emitted before them (see `biased`).
        let (approval_tx, mut approval_rx) = mpsc::unbounded_channel();
        if let Some(broker) = &self.approvals {
            broker.attach(approval_tx);
        }
        let (sink, mut agent_events) = EventSink::channel();
        let mut mapper = EventMapper::default();

        let mut current = match self.initial_session {
            Some(id) => self.configure(id, true, &events).await,
            None => self.create(&events).await,
        };

        let mut turn: Option<(TurnFuture, CancellationToken)> = None;
        let mut ops_open = true;

        loop {
            if !ops_open && turn.is_none() {
                break;
            }
            tokio::select! {
                biased;
                Some(event) = agent_events.recv() => {
                    for out in mapper.map(event) {
                        let _ = events.send(out);
                    }
                }
                Some(request) = approval_rx.recv() => {
                    let _ = events.send(request);
                }
                op = ops.recv(), if ops_open => match op {
                    None => {
                        ops_open = false;
                        if let Some(broker) = &self.approvals {
                            broker.detach("the client disconnected");
                        }
                    }
                    Some(Op::TurnInput { session, text }) => {
                        if turn.is_some() {
                            let _ = events.send(Event::Error {
                                message: "a turn is already running; send interrupt first".into(),
                            });
                            continue;
                        }
                        if let Some(id) = session {
                            if Some(id) != current {
                                current = self.configure(id, true, &events).await;
                            }
                        }
                        let Some(session) = current else {
                            let _ = events.send(Event::Error { message: "no session is configured".into() });
                            continue;
                        };
                        let cancel = CancellationToken::new();
                        let fut: TurnFuture = {
                            let (agent, cancel, sink) = (self.agent.clone(), cancel.clone(), sink.clone());
                            Box::pin(async move { (session, agent.submit(session, text, &cancel, &sink).await) })
                        };
                        turn = Some((fut, cancel));
                    }
                    Some(Op::Interrupt) => {
                        if let Some((_, cancel)) = &turn {
                            cancel.cancel();
                            if let Some(broker) = &self.approvals {
                                broker.deny_all("the user interrupted the turn");
                            }
                        }
                    }
                    Some(Op::ApprovalResponse { call_id, decision, reason }) => {
                        let decision = match decision {
                            ReviewDecision::Allow => ApprovalDecision::Allow,
                            ReviewDecision::Deny => ApprovalDecision::Deny { reason: reason.unwrap_or_default() },
                        };
                        let delivered = self.approvals.as_ref().is_some_and(|b| b.resolve(&call_id, decision));
                        if !delivered {
                            let _ = events.send(Event::Error {
                                message: format!("no approval is pending for call '{call_id}'"),
                            });
                        }
                    }
                    Some(Op::NewSession) => {
                        if turn.is_some() {
                            let _ = events.send(Event::Error { message: "cannot switch sessions during a turn".into() });
                        } else {
                            current = self.create(&events).await.or(current);
                        }
                    }
                    Some(Op::ResumeSession { session }) => {
                        if turn.is_some() {
                            let _ = events.send(Event::Error { message: "cannot switch sessions during a turn".into() });
                        } else {
                            current = self.configure(session, true, &events).await.or(current);
                        }
                    }
                },
                (session, result) = async { turn.as_mut().expect("guarded").0.as_mut().await }, if turn.is_some() => {
                    turn = None;
                    // Everything the turn emitted is already queued: the sink is synchronous.
                    while let Ok(event) = agent_events.try_recv() {
                        for out in mapper.map(event) {
                            let _ = events.send(out);
                        }
                    }
                    mapper.reset();
                    let complete = match result {
                        Ok(outcome) => Event::TurnComplete {
                            session,
                            cause: outcome.cause.into(),
                            reply: outcome.reply.filter(|r| !r.trim().is_empty()),
                        },
                        Err(e) => {
                            let _ = events.send(Event::Error { message: e.to_string() });
                            Event::TurnComplete { session, cause: TurnCause::Failed, reply: None }
                        }
                    };
                    let _ = events.send(complete);
                }
            }
        }
    }

    async fn create(&self, events: &mpsc::UnboundedSender<Event>) -> Option<SessionId> {
        match self.agent.create_session_with(self.metadata.clone()).await {
            Ok(id) => {
                let _ = events.send(Event::SessionConfigured { session: id, message_count: 0, resumed: false });
                Some(id)
            }
            Err(e) => {
                let _ = events.send(Event::Error { message: e.to_string() });
                None
            }
        }
    }

    async fn configure(&self, id: SessionId, resumed: bool, events: &mpsc::UnboundedSender<Event>) -> Option<SessionId> {
        match self.agent.session(id).await {
            Ok(session) => {
                let _ = events.send(Event::SessionConfigured {
                    session: id,
                    message_count: session.messages.len(),
                    resumed,
                });
                Some(id)
            }
            Err(e) => {
                let _ = events.send(Event::Error { message: e.to_string() });
                None
            }
        }
    }
}

/// Turns `AgentEvent`s into protocol events. Remembers the arguments of
/// requested calls (for `ToolCallBegin`) and which calls started (only those
/// get a `ToolCallEnd`).
#[derive(Default)]
struct EventMapper {
    arguments: HashMap<String, serde_json::Value>,
    started: HashSet<String>,
}

impl EventMapper {
    fn reset(&mut self) {
        self.arguments.clear();
        self.started.clear();
    }

    fn map(&mut self, event: AgentEvent) -> Vec<Event> {
        match event {
            AgentEvent::MessageAppended(message) => {
                if let Some((_, covers)) = message.as_summary() {
                    return vec![Event::Compacted { covers }];
                }
                let mut out = Vec::new();
                if message.role == Role::Assistant {
                    let text = message.text();
                    if !text.trim().is_empty() {
                        out.push(Event::AgentMessage { text });
                    }
                    for call in message.tool_calls() {
                        self.arguments.insert(call.id.clone(), call.arguments.clone());
                    }
                } else {
                    for result in message.tool_result_blocks() {
                        if self.started.remove(&result.call_id) {
                            out.push(Event::ToolCallEnd {
                                call_id: result.call_id.clone(),
                                output: truncate_output(&result.content, EVENT_OUTPUT_CHARS),
                                is_error: result.is_error,
                            });
                        }
                    }
                }
                out
            }
            AgentEvent::ToolStarted { call_id, name } => {
                self.started.insert(call_id.clone());
                let arguments = self.arguments.get(&call_id).cloned().unwrap_or(serde_json::Value::Null);
                vec![Event::ToolCallBegin { call_id, tool: name, arguments }]
            }
            AgentEvent::ToolDenied { call_id, name } => vec![Event::ToolDenied { call_id, tool: name }],
            // Step bookkeeping and `Finished` (the driver sends `TurnComplete`
            // from the outcome); new event kinds are ignored until mapped.
            #[allow(unreachable_patterns)]
            _ => Vec::new(),
        }
    }
}
