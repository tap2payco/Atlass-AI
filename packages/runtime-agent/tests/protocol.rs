//! The client protocol: wire format and the `AgentDriver`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use platform_contracts::health::HealthState;
use runtime_agent::protocol::{
    decode_op, encode_line, AgentDriver, ApprovalBroker, Event, Op, ReviewDecision, TurnCause,
};
use runtime_agent::{Agent, InMemorySessionStore, Tool, ToolEffect};
use sdk::model::ModelRuntime;
use serde_json::{json, Value};
use shared::error::InferenceError;
use shared::id::SessionId;
use shared::model::{
    CircuitState, ExecutionMetadata, ModelRequest, ModelResponse, ModelTier, StopReason, ToolCall, ToolSpec,
};
use tokio::sync::{mpsc, Notify};

// ── Test doubles ──────────────────────────────────────────────────────────────

enum Reply {
    Text(&'static str),
    Call(&'static str, &'static str, Value),
    Fail,
}

struct ScriptedModel {
    replies: Mutex<VecDeque<Reply>>,
    requests: Mutex<Vec<ModelRequest>>,
}

impl ScriptedModel {
    fn new(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self { replies: Mutex::new(replies.into()), requests: Mutex::new(Vec::new()) })
    }

    fn requests(&self) -> Vec<ModelRequest> {
        self.requests.lock().unwrap().clone()
    }
}

fn metadata() -> ExecutionMetadata {
    ExecutionMetadata {
        provider_id: "scripted".into(),
        model_id: "scripted".into(),
        tier: ModelTier::Local,
        latency_ms: 0,
        tokens_used: 0,
        cache_hit: false,
        fallback_used: false,
        retry_count: 0,
        circuit_state: CircuitState::Closed,
    }
}

#[async_trait]
impl ModelRuntime for ScriptedModel {
    async fn execute_inference(&self, request: ModelRequest) -> Result<ModelResponse, InferenceError> {
        self.requests.lock().unwrap().push(request.clone());
        let reply = self.replies.lock().unwrap().pop_front().expect("model called more times than scripted");
        let (content, tool_calls, stop_reason) = match reply {
            Reply::Text(t) => (t.to_string(), vec![], StopReason::EndTurn),
            Reply::Call(id, name, arguments) => (
                String::new(),
                vec![ToolCall { id: id.into(), name: name.into(), arguments }],
                StopReason::ToolUse,
            ),
            Reply::Fail => return Err(InferenceError::AllProvidersFailed),
        };
        Ok(ModelResponse {
            content,
            tool_calls,
            stop_reason,
            capability: request.capability,
            metadata: metadata(),
            request_id: request.request_id,
        })
    }

    async fn execute_stream(
        &self,
        _request: ModelRequest,
        _tx: mpsc::Sender<String>,
    ) -> Result<ExecutionMetadata, InferenceError> {
        unimplemented!("the agent does not stream")
    }

    async fn health_check(&self) -> HealthState {
        HealthState::Healthy
    }
}

/// A tool that "writes" (so it needs approval) and records its calls.
#[derive(Default)]
struct WriteTool {
    calls: Mutex<Vec<Value>>,
}

#[async_trait]
impl Tool for WriteTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: "write_file".into(), description: "Write a file".into(), input_schema: json!({}) }
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::WritesWorkspace
    }

    async fn call(&self, arguments: Value) -> Result<String, String> {
        self.calls.lock().unwrap().push(arguments);
        Ok("written".into())
    }
}

/// Read-only; signals when it starts, then never finishes.
struct HangingTool {
    started: Arc<Notify>,
}

#[async_trait]
impl Tool for HangingTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: "hang".into(), description: "Never returns".into(), input_schema: json!({}) }
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::ReadOnly
    }

    async fn call(&self, _arguments: Value) -> Result<String, String> {
        self.started.notify_one();
        std::future::pending().await
    }
}

struct Client {
    ops: mpsc::UnboundedSender<Op>,
    events: mpsc::UnboundedReceiver<Event>,
}

impl Client {
    async fn next(&mut self) -> Event {
        tokio::time::timeout(Duration::from_secs(5), self.events.recv())
            .await
            .expect("timed out waiting for an event")
            .expect("driver stopped")
    }

    /// Events up to and including the next `TurnComplete`.
    async fn until_complete(&mut self) -> Vec<Event> {
        let mut seen = Vec::new();
        loop {
            let event = self.next().await;
            let done = matches!(event, Event::TurnComplete { .. });
            seen.push(event);
            if done {
                return seen;
            }
        }
    }

    async fn configured(&mut self) -> SessionId {
        match self.next().await {
            Event::SessionConfigured { session, .. } => session,
            other => panic!("expected SessionConfigured, got {other:?}"),
        }
    }

    fn send(&self, op: Op) {
        self.ops.send(op).unwrap();
    }

    fn say(&self, text: &str) {
        self.send(Op::TurnInput { session: None, text: text.into() });
    }
}

fn start(model: Arc<ScriptedModel>, tools: Vec<Arc<dyn Tool>>, timeout: Duration) -> Client {
    let broker = ApprovalBroker::new(timeout);
    let mut builder = Agent::builder(model, Arc::new(InMemorySessionStore::new())).approver(broker.approver());
    for tool in tools {
        builder = builder.tool(tool);
    }
    let (ops, events) = AgentDriver::new(builder.build().unwrap()).with_approvals(broker).spawn();
    Client { ops, events }
}

fn cause_of(events: &[Event]) -> TurnCause {
    match events.last() {
        Some(Event::TurnComplete { cause, .. }) => *cause,
        other => panic!("expected TurnComplete, got {other:?}"),
    }
}

// ── Wire format ───────────────────────────────────────────────────────────────

fn all_ops() -> Vec<Op> {
    let session = SessionId::new();
    vec![
        Op::TurnInput { session: Some(session), text: "hi".into() },
        Op::TurnInput { session: None, text: "hi".into() },
        Op::Interrupt,
        Op::ApprovalResponse { call_id: "c1".into(), decision: ReviewDecision::Allow, reason: None },
        Op::ApprovalResponse { call_id: "c1".into(), decision: ReviewDecision::Deny, reason: Some("no".into()) },
        Op::NewSession,
        Op::ResumeSession { session },
    ]
}

fn all_events() -> Vec<Event> {
    let session = SessionId::new();
    vec![
        Event::SessionConfigured { session, message_count: 3, resumed: true },
        Event::MessageDelta { delta: "he".into() },
        Event::AgentMessage { text: "hello".into() },
        Event::ToolCallBegin { call_id: "c1".into(), tool: "read_file".into(), arguments: json!({ "path": "a" }) },
        Event::ToolCallEnd { call_id: "c1".into(), output: "text".into(), is_error: false },
        Event::ToolDenied { call_id: "c2".into(), tool: "write_file".into() },
        Event::ApprovalRequest {
            call_id: "c2".into(),
            tool: "write_file".into(),
            summary: "write_file a".into(),
            effect: "writes_workspace".into(),
            arguments: json!({ "path": "a" }),
        },
        Event::Compacted { covers: 12 },
        Event::TurnComplete { session, cause: TurnCause::EndTurn, reply: Some("done".into()) },
        Event::TurnComplete { session, cause: TurnCause::Failed, reply: None },
        Event::Error { message: "boom".into() },
    ]
}

#[test]
fn every_op_round_trips_as_one_json_line() {
    for op in all_ops() {
        let line = encode_line(&op);
        assert!(line.ends_with('\n') && !line.trim_end().contains('\n'), "{line}");
        assert_eq!(decode_op(&line).unwrap(), op);
    }
}

#[test]
fn every_event_round_trips() {
    for event in all_events() {
        let line = encode_line(&event);
        assert!(!line.trim_end().contains('\n'), "{line}");
        let back: Event = serde_json::from_str(&line).unwrap();
        assert_eq!(back, event);
    }
}

#[test]
fn wire_format_uses_a_snake_case_type_tag() {
    let json = serde_json::to_value(Op::ApprovalResponse {
        call_id: "c1".into(),
        decision: ReviewDecision::Deny,
        reason: Some("use edit_file".into()),
    })
    .unwrap();
    assert_eq!(json, json!({ "type": "approval_response", "call_id": "c1", "decision": "deny", "reason": "use edit_file" }));

    assert_eq!(decode_op(r#"{"type":"turn_input","text":"hi"}"#).unwrap(), Op::TurnInput { session: None, text: "hi".into() });
    assert_eq!(decode_op(" {\"type\":\"interrupt\"}\r\n").unwrap(), Op::Interrupt);
    assert!(decode_op(r#"{"type":"launch_missiles"}"#).is_err());
    assert!(decode_op("not json").is_err());

    let session = SessionId::new();
    let json = serde_json::to_value(Event::TurnComplete { session, cause: TurnCause::MaxTurns, reply: None }).unwrap();
    assert_eq!(json, json!({ "type": "turn_complete", "session": session.0.to_string(), "cause": "max_turns" }));
}

// ── Driver ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn approved_call_runs_and_streams_its_events() {
    let model = ScriptedModel::new(vec![
        Reply::Call("c1", "write_file", json!({ "path": "notes.md" })),
        Reply::Text("Wrote notes.md."),
    ]);
    let tool = Arc::new(WriteTool::default());
    let mut client = start(model, vec![tool.clone()], Duration::from_secs(5));
    let session = client.configured().await;

    client.say("write the notes");
    match client.next().await {
        Event::ApprovalRequest { call_id, tool, summary, effect, .. } => {
            assert_eq!((call_id.as_str(), tool.as_str()), ("c1", "write_file"));
            assert_eq!(summary, "write_file notes.md");
            assert_eq!(effect, "writes_workspace");
        }
        other => panic!("expected ApprovalRequest, got {other:?}"),
    }
    client.send(Op::ApprovalResponse { call_id: "c1".into(), decision: ReviewDecision::Allow, reason: None });

    let events = client.until_complete().await;
    assert_eq!(
        events,
        vec![
            Event::ToolCallBegin { call_id: "c1".into(), tool: "write_file".into(), arguments: json!({ "path": "notes.md" }) },
            Event::ToolCallEnd { call_id: "c1".into(), output: "written".into(), is_error: false },
            Event::AgentMessage { text: "Wrote notes.md.".into() },
            Event::TurnComplete { session, cause: TurnCause::EndTurn, reply: Some("Wrote notes.md.".into()) },
        ]
    );
    assert_eq!(tool.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn denied_call_does_not_run_and_the_model_hears_why() {
    let model = ScriptedModel::new(vec![
        Reply::Call("c1", "write_file", json!({ "path": "notes.md" })),
        Reply::Text("OK, I will not write it."),
    ]);
    let tool = Arc::new(WriteTool::default());
    let mut client = start(model.clone(), vec![tool.clone()], Duration::from_secs(5));
    client.configured().await;

    client.say("write the notes");
    assert!(matches!(client.next().await, Event::ApprovalRequest { .. }));
    client.send(Op::ApprovalResponse {
        call_id: "c1".into(),
        decision: ReviewDecision::Deny,
        reason: Some("show me a diff first".into()),
    });

    let events = client.until_complete().await;
    assert_eq!(events[0], Event::ToolDenied { call_id: "c1".into(), tool: "write_file".into() });
    assert!(!events.iter().any(|e| matches!(e, Event::ToolCallBegin { .. } | Event::ToolCallEnd { .. })));
    assert_eq!(cause_of(&events), TurnCause::EndTurn);
    assert!(tool.calls.lock().unwrap().is_empty());

    let result = model.requests()[1].messages.last().unwrap().tool_result_blocks().next().unwrap().clone();
    assert!(result.is_error && result.content.contains("show me a diff first"), "{}", result.content);
}

#[tokio::test]
async fn unanswered_approval_times_out_as_a_denial() {
    let model = ScriptedModel::new(vec![Reply::Call("c1", "write_file", json!({})), Reply::Text("Skipped.")]);
    let tool = Arc::new(WriteTool::default());
    let mut client = start(model.clone(), vec![tool.clone()], Duration::from_millis(50));
    client.configured().await;

    client.say("write");
    assert!(matches!(client.next().await, Event::ApprovalRequest { .. }));
    let events = client.until_complete().await;
    assert!(matches!(events[0], Event::ToolDenied { .. }));
    assert!(tool.calls.lock().unwrap().is_empty());
    let result = model.requests()[1].messages.last().unwrap().tool_result_blocks().next().unwrap().clone();
    assert!(result.content.contains("no answer within"), "{}", result.content);

    // A late answer has nothing to resolve.
    client.send(Op::ApprovalResponse { call_id: "c1".into(), decision: ReviewDecision::Allow, reason: None });
    assert!(matches!(client.next().await, Event::Error { .. }));
}

#[tokio::test]
async fn interrupt_cancels_a_running_tool() {
    let model = ScriptedModel::new(vec![Reply::Call("c1", "hang", json!({}))]);
    let started = Arc::new(Notify::new());
    let mut client = start(model, vec![Arc::new(HangingTool { started: started.clone() })], Duration::from_secs(5));
    client.configured().await;

    client.say("run it");
    started.notified().await;
    client.send(Op::Interrupt);

    let events = client.until_complete().await;
    assert!(matches!(events[0], Event::ToolCallBegin { .. }));
    assert!(events.iter().any(|e| matches!(e, Event::ToolCallEnd { is_error: true, .. })));
    assert_eq!(cause_of(&events), TurnCause::Cancelled);
}

#[tokio::test]
async fn interrupt_while_waiting_for_approval_ends_the_turn() {
    let model = ScriptedModel::new(vec![Reply::Call("c1", "write_file", json!({}))]);
    let tool = Arc::new(WriteTool::default());
    let mut client = start(model, vec![tool.clone()], Duration::from_secs(30));
    client.configured().await;

    client.say("write");
    assert!(matches!(client.next().await, Event::ApprovalRequest { .. }));
    client.send(Op::Interrupt);
    let events = client.until_complete().await;
    assert_eq!(cause_of(&events), TurnCause::Cancelled);
    assert!(tool.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_second_turn_input_during_a_turn_is_rejected() {
    let model = ScriptedModel::new(vec![Reply::Call("c1", "write_file", json!({})), Reply::Text("done")]);
    let mut client = start(model, vec![Arc::new(WriteTool::default())], Duration::from_secs(5));
    client.configured().await;

    client.say("one");
    assert!(matches!(client.next().await, Event::ApprovalRequest { .. }));
    client.say("two");
    assert!(matches!(client.next().await, Event::Error { .. }));
    client.send(Op::ApprovalResponse { call_id: "c1".into(), decision: ReviewDecision::Allow, reason: None });
    assert_eq!(cause_of(&client.until_complete().await), TurnCause::EndTurn);
}

#[tokio::test]
async fn model_failure_reports_an_error_then_completes_the_turn() {
    let model = ScriptedModel::new(vec![Reply::Fail]);
    let mut client = start(model, vec![], Duration::from_secs(5));
    let session = client.configured().await;

    client.say("hello");
    let events = client.until_complete().await;
    assert!(matches!(events[0], Event::Error { .. }));
    assert_eq!(events[1], Event::TurnComplete { session, cause: TurnCause::Failed, reply: None });
}

#[tokio::test]
async fn sessions_can_be_created_and_resumed() {
    let model = ScriptedModel::new(vec![Reply::Text("first"), Reply::Text("second")]);
    let mut client = start(model.clone(), vec![], Duration::from_secs(5));
    let first = client.configured().await;

    client.say("hello");
    assert_eq!(cause_of(&client.until_complete().await), TurnCause::EndTurn);

    client.send(Op::NewSession);
    let second = client.configured().await;
    assert_ne!(first, second);

    client.send(Op::ResumeSession { session: first });
    match client.next().await {
        Event::SessionConfigured { session, message_count, resumed } => {
            assert_eq!((session, message_count, resumed), (first, 2, true));
        }
        other => panic!("expected SessionConfigured, got {other:?}"),
    }
    client.say("again");
    let events = client.until_complete().await;
    assert!(matches!(events.last(), Some(Event::TurnComplete { session, .. }) if *session == first));
    // The resumed session's history reached the model.
    assert_eq!(model.requests()[1].messages.len(), 3);

    client.send(Op::ResumeSession { session: SessionId::new() });
    assert!(matches!(client.next().await, Event::Error { .. }));
}

#[tokio::test]
async fn closing_the_op_channel_finishes_the_running_turn() {
    let model = ScriptedModel::new(vec![Reply::Text("bye")]);
    let mut client = start(model, vec![], Duration::from_secs(5));
    client.configured().await;
    client.say("hello");
    drop(client.ops);
    let mut last = None;
    while let Some(event) = client.events.recv().await {
        last = Some(event);
    }
    assert!(matches!(last, Some(Event::TurnComplete { cause: TurnCause::EndTurn, .. })));
}
