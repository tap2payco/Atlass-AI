use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use platform_contracts::health::HealthState;
use runtime_agent::conversation::open_tool_calls;
use runtime_agent::machine::INTERRUPTED_TOOL_MESSAGE;
use runtime_agent::{
    Agent, AgentError, AgentEvent, EventSink, InMemorySessionStore, StopCause, Tool,
};
use sdk::model::ModelRuntime;
use serde_json::{json, Value};
use shared::error::InferenceError;
use shared::model::{
    CircuitState, ExecutionMetadata, ModelRequest, ModelResponse, ModelTier, Role, StopReason,
    ToolCall, ToolSpec,
};
use tokio::sync::{mpsc, Notify};
use tokio_util::sync::CancellationToken;

// ── Test doubles ──────────────────────────────────────────────────────────────

enum Reply {
    Text(&'static str),
    Call(&'static str, &'static str, Value),
    Fail,
}

/// Returns scripted replies in order and records every request it receives.
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

/// Echoes its `text` argument and records the arguments it was called with.
#[derive(Default)]
struct EchoTool {
    calls: Mutex<Vec<Value>>,
}

#[async_trait]
impl Tool for EchoTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo".into(),
            description: "Echo text back".into(),
            input_schema: json!({ "type": "object", "properties": { "text": { "type": "string" } } }),
        }
    }

    async fn call(&self, arguments: Value) -> Result<String, String> {
        self.calls.lock().unwrap().push(arguments.clone());
        arguments["text"].as_str().map(str::to_string).ok_or_else(|| "missing 'text'".to_string())
    }
}

/// Signals when it starts, then never finishes.
struct HangingTool {
    started: Arc<Notify>,
}

#[async_trait]
impl Tool for HangingTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: "hang".into(), description: "Never returns".into(), input_schema: json!({}) }
    }

    async fn call(&self, _arguments: Value) -> Result<String, String> {
        self.started.notify_one();
        std::future::pending().await
    }
}

fn agent_with(model: Arc<ScriptedModel>, tools: Vec<Arc<dyn Tool>>) -> Agent {
    let mut builder = Agent::builder(model, Arc::new(InMemorySessionStore::new()));
    for tool in tools {
        builder = builder.tool(tool);
    }
    builder.build().unwrap()
}

fn no_cancel() -> CancellationToken {
    CancellationToken::new()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn runs_a_tool_and_feeds_the_result_back_to_the_model() {
    let model = ScriptedModel::new(vec![
        Reply::Call("c1", "echo", json!({ "text": "hello" })),
        Reply::Text("The tool said hello."),
    ]);
    let echo = Arc::new(EchoTool::default());
    let agent = agent_with(model.clone(), vec![echo.clone()]);
    let session = agent.create_session().await.unwrap();

    let outcome = agent.submit(session, "Say hello via the tool", &no_cancel(), &EventSink::none()).await.unwrap();

    assert_eq!(outcome.cause, StopCause::EndTurn);
    assert_eq!(outcome.reply.as_deref(), Some("The tool said hello."));
    assert_eq!(*echo.calls.lock().unwrap(), vec![json!({ "text": "hello" })]);

    let requests = model.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["echo"]);
    let seen = requests[1].messages.last().unwrap().tool_result_blocks().next().unwrap().clone();
    assert_eq!((seen.call_id.as_str(), seen.content.as_str(), seen.is_error), ("c1", "hello", false));

    let roles: Vec<Role> = agent.session(session).await.unwrap().messages.iter().map(|m| m.role).collect();
    assert_eq!(roles, vec![Role::User, Role::Assistant, Role::User, Role::Assistant]);
}

#[tokio::test]
async fn unknown_tools_and_tool_errors_become_error_results() {
    let model = ScriptedModel::new(vec![
        Reply::Call("c1", "nope", json!({})),
        Reply::Call("c2", "echo", json!({})), // missing "text" → tool returns Err
        Reply::Text("Gave up."),
    ]);
    let agent = agent_with(model.clone(), vec![Arc::new(EchoTool::default())]);
    let session = agent.create_session().await.unwrap();

    let outcome = agent.submit(session, "go", &no_cancel(), &EventSink::none()).await.unwrap();
    assert_eq!(outcome.cause, StopCause::EndTurn);

    let results: Vec<_> = agent
        .session(session)
        .await
        .unwrap()
        .messages
        .iter()
        .flat_map(|m| m.tool_result_blocks().cloned().collect::<Vec<_>>())
        .collect();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.is_error));
    assert_eq!(results[0].content, "Unknown tool 'nope'.");
    assert_eq!(results[1].content, "missing 'text'");
}

#[tokio::test]
async fn stops_at_the_turn_limit_and_a_new_message_resets_it() {
    let model = ScriptedModel::new(vec![
        Reply::Call("c1", "echo", json!({ "text": "1" })),
        Reply::Call("c2", "echo", json!({ "text": "2" })),
        Reply::Text("fresh turn"),
    ]);
    let agent = Agent::builder(model.clone(), Arc::new(InMemorySessionStore::new()))
        .tool(Arc::new(EchoTool::default()))
        .max_turns(2)
        .build()
        .unwrap();
    let session = agent.create_session().await.unwrap();

    let outcome = agent.submit(session, "loop", &no_cancel(), &EventSink::none()).await.unwrap();
    assert_eq!(outcome.cause, StopCause::MaxTurns);
    assert_eq!(outcome.reply, None); // conversation ends with tool results
    assert_eq!(model.requests().len(), 2);

    let outcome = agent.submit(session, "new request", &no_cancel(), &EventSink::none()).await.unwrap();
    assert_eq!(outcome.cause, StopCause::EndTurn);
    assert_eq!(outcome.reply.as_deref(), Some("fresh turn"));
}

#[tokio::test]
async fn cancelling_during_a_tool_answers_the_open_call_then_resume_continues() {
    let model = ScriptedModel::new(vec![
        Reply::Call("c1", "hang", json!({})),
        Reply::Text("Understood, the tool was interrupted."),
    ]);
    let started = Arc::new(Notify::new());
    let agent = Arc::new(agent_with(model.clone(), vec![Arc::new(HangingTool { started: started.clone() })]));
    let session = agent.create_session().await.unwrap();

    let cancel = CancellationToken::new();
    let run = {
        let (agent, cancel) = (agent.clone(), cancel.clone());
        tokio::spawn(async move { agent.submit(session, "run it", &cancel, &EventSink::none()).await })
    };
    started.notified().await;
    cancel.cancel();
    let outcome = run.await.unwrap().unwrap();
    assert_eq!(outcome.cause, StopCause::Cancelled);

    let stored = agent.session(session).await.unwrap();
    assert!(open_tool_calls(&stored.messages).is_empty());
    let result = stored.messages.last().unwrap().tool_result_blocks().next().unwrap().clone();
    assert_eq!((result.call_id.as_str(), result.content.as_str(), result.is_error), ("c1", INTERRUPTED_TOOL_MESSAGE, true));

    let outcome = agent.resume(session, &no_cancel(), &EventSink::none()).await.unwrap();
    assert_eq!(outcome.cause, StopCause::EndTurn);
    assert_eq!(model.requests().len(), 2);
}

#[tokio::test]
async fn a_model_error_leaves_the_turn_resumable() {
    let model = ScriptedModel::new(vec![Reply::Fail, Reply::Text("second try worked")]);
    let agent = agent_with(model.clone(), vec![]);
    let session = agent.create_session().await.unwrap();

    let err = agent.submit(session, "hi", &no_cancel(), &EventSink::none()).await.unwrap_err();
    assert!(matches!(err, AgentError::Inference(InferenceError::AllProvidersFailed)));
    assert_eq!(agent.session(session).await.unwrap().messages.len(), 1);

    let outcome = agent.resume(session, &no_cancel(), &EventSink::none()).await.unwrap();
    assert_eq!(outcome.reply.as_deref(), Some("second try worked"));
}

#[tokio::test]
async fn resume_with_nothing_pending_is_idle_and_does_not_call_the_model() {
    let model = ScriptedModel::new(vec![Reply::Text("done")]);
    let agent = agent_with(model.clone(), vec![]);
    let session = agent.create_session().await.unwrap();
    agent.submit(session, "hi", &no_cancel(), &EventSink::none()).await.unwrap();

    let outcome = agent.resume(session, &no_cancel(), &EventSink::none()).await.unwrap();
    assert_eq!(outcome.cause, StopCause::Idle);
    assert_eq!(model.requests().len(), 1);
}

#[tokio::test]
async fn emits_events_in_order() {
    let model = ScriptedModel::new(vec![Reply::Call("c1", "echo", json!({ "text": "x" })), Reply::Text("ok")]);
    let agent = agent_with(model, vec![Arc::new(EchoTool::default())]);
    let session = agent.create_session().await.unwrap();
    let (events, mut rx) = EventSink::channel();

    agent.submit(session, "go", &no_cancel(), &events).await.unwrap();
    drop(events);

    let mut kinds = Vec::new();
    while let Some(event) = rx.recv().await {
        kinds.push(match event {
            AgentEvent::StepApplied { operation } => format!("step:{operation}"),
            AgentEvent::MessageAppended(m) => format!("msg:{:?}", m.role),
            AgentEvent::ToolStarted { name, .. } => format!("tool:{name}"),
            AgentEvent::ToolDenied { name, .. } => format!("denied:{name}"),
            AgentEvent::Finished(cause) => format!("finished:{cause:?}"),
        });
    }
    assert_eq!(
        kinds,
        vec![
            "msg:Assistant", "step:inference",
            "tool:echo", "msg:User", "step:tool_execution",
            "msg:Assistant", "step:inference",
            "finished:EndTurn",
        ]
    );
}

#[tokio::test]
async fn duplicate_tool_names_are_rejected() {
    let model = ScriptedModel::new(vec![]);
    let result = Agent::builder(model, Arc::new(InMemorySessionStore::new()))
        .tool(Arc::new(EchoTool::default()))
        .tool(Arc::new(EchoTool::default()))
        .build();
    assert!(matches!(result, Err(AgentError::Config(_))));
}
