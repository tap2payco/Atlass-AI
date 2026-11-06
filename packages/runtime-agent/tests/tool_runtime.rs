//! Tool runtime behaviour: parallel read-only batches, exclusive mutating
//! calls, call-order results, timeouts and output caps.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use platform_contracts::health::HealthState;
use runtime_agent::{Agent, EventSink, InMemorySessionStore, StopCause, Tool, ToolEffect, ToolLimits};
use sdk::model::ModelRuntime;
use serde_json::{json, Value};
use shared::error::InferenceError;
use shared::model::{
    CircuitState, ExecutionMetadata, Message, ModelRequest, ModelResponse, ModelTier, StopReason,
    ToolCall, ToolResult, ToolSpec,
};
use tokio::sync::{mpsc, Barrier};
use tokio_util::sync::CancellationToken;

/// First reply: the given tool calls. Second reply: "done".
struct CallsThenDone {
    calls: Vec<(&'static str, &'static str)>,
    turns: Mutex<usize>,
}

impl CallsThenDone {
    fn new(calls: Vec<(&'static str, &'static str)>) -> Arc<Self> {
        Arc::new(Self { calls, turns: Mutex::new(0) })
    }
}

#[async_trait]
impl ModelRuntime for CallsThenDone {
    async fn execute_inference(&self, request: ModelRequest) -> Result<ModelResponse, InferenceError> {
        let first = {
            let mut turns = self.turns.lock().unwrap();
            *turns += 1;
            *turns == 1
        };
        let tool_calls: Vec<ToolCall> = if first {
            self.calls
                .iter()
                .map(|(id, name)| ToolCall { id: id.to_string(), name: name.to_string(), arguments: json!({}) })
                .collect()
        } else {
            Vec::new()
        };
        Ok(ModelResponse {
            content: if first { String::new() } else { "done".into() },
            stop_reason: if first { StopReason::ToolUse } else { StopReason::EndTurn },
            tool_calls,
            capability: request.capability,
            metadata: ExecutionMetadata {
                provider_id: "test".into(),
                model_id: "test".into(),
                tier: ModelTier::Local,
                latency_ms: 0,
                tokens_used: 0,
                cache_hit: false,
                fallback_used: false,
                retry_count: 0,
                circuit_state: CircuitState::Closed,
            },
            request_id: request.request_id,
        })
    }

    async fn execute_stream(
        &self,
        _request: ModelRequest,
        _tx: mpsc::Sender<String>,
    ) -> Result<ExecutionMetadata, InferenceError> {
        unimplemented!()
    }

    async fn health_check(&self) -> HealthState {
        HealthState::Healthy
    }
}

type Log = Arc<Mutex<Vec<String>>>;

/// Logs "start:<name>" and "end:<name>", optionally waiting on a barrier or sleeping in between.
struct Probe {
    name: &'static str,
    effect: ToolEffect,
    log: Log,
    barrier: Option<Arc<Barrier>>,
    sleep: Duration,
    output: String,
}

impl Probe {
    fn new(name: &'static str, effect: ToolEffect, log: &Log) -> Self {
        Self { name, effect, log: log.clone(), barrier: None, sleep: Duration::ZERO, output: format!("{name} ok") }
    }
}

#[async_trait]
impl Tool for Probe {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: self.name.into(), description: "probe".into(), input_schema: json!({}) }
    }

    fn effect(&self) -> ToolEffect {
        self.effect
    }

    async fn call(&self, _arguments: Value) -> Result<String, String> {
        self.log.lock().unwrap().push(format!("start:{}", self.name));
        if let Some(barrier) = &self.barrier {
            barrier.wait().await;
        }
        tokio::time::sleep(self.sleep).await;
        self.log.lock().unwrap().push(format!("end:{}", self.name));
        Ok(self.output.clone())
    }
}

async fn run(model: Arc<CallsThenDone>, tools: Vec<Probe>, limits: ToolLimits) -> Vec<ToolResult> {
    let mut builder = Agent::builder(model, Arc::new(InMemorySessionStore::new())).tool_limits(limits);
    for tool in tools {
        builder = builder.tool(Arc::new(tool));
    }
    let agent = builder.build().unwrap();
    let session = agent.create_session().await.unwrap();
    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        agent.submit(session, "go", &CancellationToken::new(), &EventSink::none()),
    )
    .await
    .expect("the agent hung: read-only tools did not run in parallel")
    .unwrap();
    assert_eq!(outcome.cause, StopCause::EndTurn);
    agent
        .session(session)
        .await
        .unwrap()
        .messages
        .iter()
        .flat_map(Message::tool_result_blocks)
        .cloned()
        .collect()
}

#[tokio::test]
async fn read_only_calls_run_in_parallel() {
    let log = Log::default();
    let barrier = Arc::new(Barrier::new(2));
    let mut a = Probe::new("read_a", ToolEffect::ReadOnly, &log);
    let mut b = Probe::new("read_b", ToolEffect::ReadOnly, &log);
    a.barrier = Some(barrier.clone());
    b.barrier = Some(barrier);
    // Each waits for the other inside its call, so this only finishes if both run at once.
    let results = run(CallsThenDone::new(vec![("1", "read_a"), ("2", "read_b")]), vec![a, b], ToolLimits::default()).await;
    let ids: Vec<_> = results.iter().map(|r| r.call_id.as_str()).collect();
    assert_eq!(ids, ["1", "2"]);
    assert!(results.iter().all(|r| !r.is_error));
}

#[tokio::test]
async fn mutating_calls_run_alone_and_in_call_order() {
    let log = Log::default();
    let mut r1 = Probe::new("read_1", ToolEffect::ReadOnly, &log);
    r1.sleep = Duration::from_millis(30);
    let w = Probe::new("write", ToolEffect::WritesWorkspace, &log);
    let r2 = Probe::new("read_2", ToolEffect::ReadOnly, &log);
    let c = Probe::new("cmd", ToolEffect::RunsProcesses, &log);
    let results = run(
        CallsThenDone::new(vec![("a", "read_1"), ("b", "write"), ("c", "read_2"), ("d", "cmd")]),
        vec![r1, w, r2, c],
        ToolLimits::default(),
    )
    .await;

    assert_eq!(
        *log.lock().unwrap(),
        [
            "start:read_1", "end:read_1",
            "start:write", "end:write",
            "start:read_2", "end:read_2",
            "start:cmd", "end:cmd",
        ]
    );
    let ids: Vec<_> = results.iter().map(|r| r.call_id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c", "d"]);
}

#[tokio::test]
async fn slow_tools_time_out_and_long_output_is_capped() {
    let log = Log::default();
    let mut slow = Probe::new("slow", ToolEffect::ReadOnly, &log);
    slow.sleep = Duration::from_secs(30);
    let mut chatty = Probe::new("chatty", ToolEffect::ReadOnly, &log);
    chatty.output = "x".repeat(1000);
    let limits = ToolLimits { timeout: Duration::from_millis(100), max_output_chars: 100 };

    let results = run(CallsThenDone::new(vec![("s", "slow"), ("c", "chatty")]), vec![slow, chatty], limits).await;

    assert!(results[0].is_error);
    assert!(results[0].content.contains("timed out"), "{}", results[0].content);
    assert!(!results[1].is_error);
    assert!(results[1].content.contains("[900 characters omitted]"), "{}", results[1].content);
    assert!(results[1].content.chars().count() < 200);
}

#[test]
fn undeclared_effect_defaults_to_the_most_powerful() {
    struct Bare;
    #[async_trait]
    impl Tool for Bare {
        fn spec(&self) -> ToolSpec {
            ToolSpec { name: "bare".into(), description: String::new(), input_schema: json!({}) }
        }
        async fn call(&self, _arguments: Value) -> Result<String, String> {
            Ok(String::new())
        }
    }
    assert_eq!(Bare.effect(), ToolEffect::RunsProcesses);
}

// ── Approvals ─────────────────────────────────────────────────────────────────

/// Records what it was asked about; denies writes, allows everything else.
#[derive(Default)]
struct DenyWrites {
    asked: Mutex<Vec<(String, ToolEffect)>>,
}

#[async_trait]
impl runtime_agent::ToolApprover for DenyWrites {
    async fn approve(&self, call: &ToolCall, effect: ToolEffect) -> runtime_agent::ApprovalDecision {
        self.asked.lock().unwrap().push((call.name.clone(), effect));
        if effect == ToolEffect::WritesWorkspace {
            runtime_agent::ApprovalDecision::Deny { reason: "edit the test file instead".into() }
        } else {
            runtime_agent::ApprovalDecision::Allow
        }
    }
}

#[tokio::test]
async fn approver_is_asked_only_for_mutating_calls_and_denials_skip_the_tool() {
    let log = Log::default();
    let approver = Arc::new(DenyWrites::default());
    let tools = vec![
        Probe::new("read", ToolEffect::ReadOnly, &log),
        Probe::new("write", ToolEffect::WritesWorkspace, &log),
        Probe::new("cmd", ToolEffect::RunsProcesses, &log),
    ];
    let mut builder = Agent::builder(
        CallsThenDone::new(vec![("r", "read"), ("w", "write"), ("c", "cmd")]),
        Arc::new(InMemorySessionStore::new()),
    )
    .approver(approver.clone());
    for tool in tools {
        builder = builder.tool(Arc::new(tool));
    }
    let agent = builder.build().unwrap();
    let session = agent.create_session().await.unwrap();
    let (events, mut rx) = EventSink::channel();
    agent.submit(session, "go", &CancellationToken::new(), &events).await.unwrap();
    drop(events);

    assert_eq!(
        *approver.asked.lock().unwrap(),
        [("write".to_string(), ToolEffect::WritesWorkspace), ("cmd".to_string(), ToolEffect::RunsProcesses)]
    );
    // The denied tool never ran; the others did.
    assert_eq!(*log.lock().unwrap(), ["start:read", "end:read", "start:cmd", "end:cmd"]);

    let results: Vec<ToolResult> = agent
        .session(session)
        .await
        .unwrap()
        .messages
        .iter()
        .flat_map(Message::tool_result_blocks)
        .cloned()
        .collect();
    assert_eq!(results.len(), 3);
    assert!(!results[0].is_error && !results[2].is_error);
    assert!(results[1].is_error);
    assert_eq!(results[1].content, "The user declined to run this tool call: edit the test file instead");

    let mut denied = Vec::new();
    while let Some(event) = rx.recv().await {
        if let runtime_agent::AgentEvent::ToolDenied { name, .. } = event {
            denied.push(name);
        }
    }
    assert_eq!(denied, ["write"]);
}
