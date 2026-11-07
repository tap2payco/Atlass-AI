//! Cognitive-state operations, context providers and the tool registry,
//! driven through the agent loop with a scripted model.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use platform_contracts::health::HealthState;
use runtime_agent::{
    Agent, AgentError, CognitionSettings, ContextContribution, ContextSource, EventSink, GitContext,
    InMemorySessionStore, InferenceSettings, KnowledgeGraphContext, MemoryContext, Tool,
    DEFAULT_TOOL_PRIORITY,
};
use runtime_memory::api::MemoryRuntime;
use runtime_memory::domain::*;
use sdk::model::ModelRuntime;
use serde_json::{json, Value};
use shared::error::InferenceError;
use shared::model::{
    CircuitState, ExecutionMetadata, ModelCapability, ModelRequest, ModelResponse, ModelTier, StopReason,
    ToolCall, ToolSpec,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

// ── Test doubles ──────────────────────────────────────────────────────────────

enum Reply {
    Text(&'static str),
    /// Text (e.g. a plan) plus one tool call.
    Call(&'static str, &'static str, Value),
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

    fn systems(&self) -> Vec<String> {
        self.requests().into_iter().map(|r| r.system_prompt.unwrap_or_default()).collect()
    }
}

#[async_trait]
impl ModelRuntime for ScriptedModel {
    async fn execute_inference(&self, request: ModelRequest) -> Result<ModelResponse, InferenceError> {
        self.requests.lock().unwrap().push(request.clone());
        let reply = self.replies.lock().unwrap().pop_front().expect("model called more times than scripted");
        let (content, tool_calls, stop_reason) = match reply {
            Reply::Text(t) => (t.to_string(), vec![], StopReason::EndTurn),
            Reply::Call(text, id, arguments) => (
                text.to_string(),
                vec![ToolCall { id: id.into(), name: "echo".into(), arguments }],
                StopReason::ToolUse,
            ),
        };
        Ok(ModelResponse {
            content,
            tool_calls,
            stop_reason,
            capability: request.capability,
            metadata: ExecutionMetadata {
                provider_id: "scripted".into(),
                model_id: "scripted".into(),
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
        unimplemented!("the agent does not stream")
    }

    async fn health_check(&self) -> HealthState {
        HealthState::Healthy
    }
}

/// Echoes `text`, or fails without it. Its `tag` is prefixed to the output,
/// so tests can tell which of two same-named tools ran.
struct EchoTool {
    tag: &'static str,
}

#[async_trait]
impl Tool for EchoTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: "echo".into(), description: "Echo text".into(), input_schema: json!({}) }
    }

    async fn call(&self, arguments: Value) -> Result<String, String> {
        arguments["text"].as_str().map(|t| format!("{}{t}", self.tag)).ok_or_else(|| "missing 'text'".into())
    }
}

fn echo() -> Arc<dyn Tool> {
    Arc::new(EchoTool { tag: "" })
}

/// Counts how often it is asked and returns a fixed contribution.
struct CountingSource {
    calls: AtomicUsize,
    requests: Mutex<Vec<String>>,
    contribution: ContextContribution,
}

impl CountingSource {
    fn new(contribution: ContextContribution) -> Arc<Self> {
        Arc::new(Self { calls: AtomicUsize::new(0), requests: Mutex::new(Vec::new()), contribution })
    }
}

#[async_trait]
impl ContextSource for CountingSource {
    fn name(&self) -> &'static str {
        "context.counting"
    }

    async fn gather(&self, request: &str) -> ContextContribution {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.requests.lock().unwrap().push(request.to_string());
        self.contribution.clone()
    }
}

fn store() -> Arc<InMemorySessionStore> {
    Arc::new(InMemorySessionStore::new())
}

async fn run(agent: &Agent, text: &str) -> shared::id::SessionId {
    let session = agent.create_session().await.unwrap();
    agent.submit(session, text, &CancellationToken::new(), &EventSink::none()).await.unwrap();
    session
}

// ── Default agent is unchanged ────────────────────────────────────────────────

#[tokio::test]
async fn default_agent_sends_only_the_base_system_prompt_and_capability() {
    let model = ScriptedModel::new(vec![Reply::Call("", "c1", json!({ "text": "hi" })), Reply::Text("done")]);
    let agent = Agent::builder(model.clone(), store()).tool(echo()).build().unwrap();
    run(&agent, "go").await;

    let base = InferenceSettings::default();
    for request in model.requests() {
        assert_eq!(request.system_prompt.as_deref(), Some(base.system_prompt.as_str()));
        assert_eq!(request.capability, base.capability);
    }
}

// ── Cognitive states ──────────────────────────────────────────────────────────

#[tokio::test]
async fn cognitive_states_follow_the_conversation() {
    let model = ScriptedModel::new(vec![
        Reply::Call("", "c1", json!({ "text": "look" })),       // research, no plan yet
        Reply::Call("1. echo 2. finish", "c2", json!({})),      // plan, then a failing call
        Reply::Call("", "c3", json!({ "text": "retry" })),
        Reply::Text("Done: echoed and verified."),
    ]);
    let agent = Agent::builder(model.clone(), store())
        .tool(echo())
        .cognitive(CognitionSettings::default())
        .build()
        .unwrap();
    run(&agent, "do the thing").await;

    let s = model.systems();
    assert_eq!(s.len(), 4);
    // 1st call: a new request.
    assert!(s[0].contains("## Understanding") && s[0].contains("## Planning"));
    assert!(!s[0].contains("## Review") && !s[0].contains("## Execution"));
    // 2nd call: tool output, but no plan written yet.
    assert!(!s[1].contains("## Understanding"));
    assert!(s[1].contains("## Planning") && s[1].contains("## Review"));
    assert!(!s[1].contains("## Execution"));
    // 3rd call: the plan exists and the last tool failed.
    assert!(!s[2].contains("## Planning"));
    assert!(s[2].contains("## Execution") && s[2].contains("Revision required"));
    // 4th call: success after a plan; self-check before the final answer.
    assert!(s[3].contains("## Execution") && s[3].contains("before giving your final answer"));
    assert!(!s[3].contains("## Reflection"));
    // No routing configured: the capability never changes.
    assert!(model.requests().iter().all(|r| r.capability == ModelCapability::CodeGeneration));
}

#[tokio::test]
async fn reflection_starts_in_a_long_turn_and_routing_picks_capabilities() {
    let settings = CognitionSettings { reflect_after: 2, ..CognitionSettings::default() }.with_doc_routing();
    let model = ScriptedModel::new(vec![
        Reply::Call("plan: echo twice", "c1", json!({ "text": "a" })),
        Reply::Call("", "c2", json!({ "text": "b" })),
        Reply::Text("finished"),
    ]);
    let agent = Agent::builder(model.clone(), store()).tool(echo()).cognitive(settings).build().unwrap();
    run(&agent, "echo twice").await;

    let s = model.systems();
    assert!(!s[1].contains("## Reflection"));
    assert!(s[2].contains("## Reflection") && s[2].contains("2 model responses"));
    let caps: Vec<_> = model.requests().into_iter().map(|r| r.capability).collect();
    assert_eq!(
        caps,
        [ModelCapability::Reasoning, ModelCapability::CodeGeneration, ModelCapability::Reasoning],
        "understanding → reasoning, execution → code, reflection → reasoning"
    );
}

#[tokio::test]
async fn only_enabled_states_contribute() {
    let settings = CognitionSettings {
        states: vec![runtime_agent::CognitiveState::Review],
        ..CognitionSettings::default()
    };
    let model = ScriptedModel::new(vec![Reply::Call("", "c1", json!({ "text": "x" })), Reply::Text("ok")]);
    let agent = Agent::builder(model.clone(), store()).tool(echo()).cognitive(settings).build().unwrap();
    run(&agent, "go").await;
    let s = model.systems();
    assert_eq!(s[0], InferenceSettings::default().system_prompt);
    assert!(s[1].contains("## Review") && !s[1].contains("## Planning"));
}

// ── Context providers ─────────────────────────────────────────────────────────

#[tokio::test]
async fn context_is_gathered_once_per_request_and_added_to_every_call() {
    let source = CountingSource::new(ContextContribution::Section("## Fake context\nhello".into()));
    let model = ScriptedModel::new(vec![
        Reply::Call("", "c1", json!({ "text": "x" })),
        Reply::Text("first done"),
        Reply::Text("second done"),
    ]);
    let agent = Agent::builder(model.clone(), store()).tool(echo()).context(source.clone()).build().unwrap();
    let session = run(&agent, "first request").await;
    assert_eq!(source.calls.load(Ordering::SeqCst), 1, "one gather across tool rounds");
    agent.submit(session, "second request", &CancellationToken::new(), &EventSink::none()).await.unwrap();
    assert_eq!(source.calls.load(Ordering::SeqCst), 2, "a new request gathers again");
    assert_eq!(*source.requests.lock().unwrap(), ["first request", "second request"]);
    assert!(model.systems().iter().all(|s| s.ends_with("## Fake context\nhello")));
}

#[tokio::test]
async fn unavailable_and_empty_sources_are_reported_or_skipped() {
    let empty = CountingSource::new(ContextContribution::Empty);
    let model = ScriptedModel::new(vec![Reply::Text("ok")]);
    let agent = Agent::builder(model.clone(), store())
        .context(empty.clone())
        .context(Arc::new(KnowledgeGraphContext))
        .build()
        .unwrap();
    run(&agent, "go").await;
    let system = &model.systems()[0];
    assert!(system.contains("[context: context.knowledge_graph not available: runtime-knowledge has no query API yet]"));
    assert!(!system.contains("context.counting"));
    assert_eq!(empty.calls.load(Ordering::SeqCst), 1);
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("atlas-agent-{name}-{}", shared::id::SessionId::new().0));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(status.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&status.stderr));
}

#[tokio::test]
async fn git_context_reports_branch_status_and_commits() {
    let dir = temp_dir("git");
    git(&dir, &["init", "-q", "-b", "trunk"]);
    std::fs::write(dir.join("a.txt"), "one").unwrap();
    git(&dir, &["add", "a.txt"]);
    git(&dir, &["commit", "-q", "-m", "first commit"]);
    std::fs::write(dir.join("a.txt"), "two").unwrap();
    std::fs::write(dir.join("new.txt"), "new").unwrap();

    let ContextContribution::Section(text) = GitContext::new(&dir).gather("anything").await else {
        panic!("expected a section");
    };
    assert!(text.starts_with("## Workspace git state"), "{text}");
    assert!(text.contains("Branch: trunk"), "{text}");
    assert!(text.contains("Status (2 changed):") && text.contains(" M a.txt") && text.contains("?? new.txt"), "{text}");
    assert!(text.contains("first commit"), "{text}");

    // Limits apply to the status list.
    let ContextContribution::Section(short) = GitContext::new(&dir).with_limits(1, 1).gather("").await else {
        panic!("expected a section");
    };
    assert!(short.contains("… and 1 more"), "{short}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn git_context_outside_a_repository_is_unavailable() {
    let dir = temp_dir("nogit");
    // Stop git from finding a repository in a parent of the temp dir.
    std::env::set_var("GIT_CEILING_DIRECTORIES", dir.parent().unwrap());
    let result = GitContext::new(&dir).gather("").await;
    assert!(matches!(result, ContextContribution::Unavailable(_)), "{result:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn memory(id: &str, text: &str, importance: f32) -> MemoryRecord<String> {
    MemoryRecord {
        identity: MemoryIdentity {
            id: id.into(),
            memory_type: MemoryType::Semantic,
            class: MemoryClass::Persistent,
            traits: MemoryTraits {
                supports_search: true,
                supports_versioning: false,
                supports_decay: false,
                supports_archival: false,
                supports_reinforcement: false,
            },
            source: "test".into(),
            version: 1,
            parent_version: None,
            derived_from_version: None,
            provenance: MemoryProvenance {
                source_runtime: "test".into(),
                original_request_id: None,
                created_by: "test".into(),
            },
            created_at: 0,
            updated_at: 0,
        },
        content: MemoryContent {
            payload: text.into(),
            embedding_references: vec![],
            entities: vec![],
            relationships: vec![],
            summaries: vec![],
            attachments: vec![],
        },
        metadata: MemoryMetadata {
            confidence: 1.0,
            quality: 1.0,
            stability: 1.0,
            importance,
            decay: 0.0,
            reinforcement: 0.0,
            access_count: 0,
            last_accessed: 0,
            expiration_policy: ExpirationPolicy::Never,
        },
    }
}

#[tokio::test]
async fn memory_context_lists_memories_by_importance() {
    let runtime = Arc::new(MemoryRuntime::new());
    assert_eq!(MemoryContext::new(runtime.clone()).gather("q").await, ContextContribution::Empty);

    runtime.store(&memory("m1", "minor fact", 0.1)).await.unwrap();
    runtime.store(&memory("m2", "use the migration tool", 0.9)).await.unwrap();
    let ContextContribution::Section(text) = MemoryContext::new(runtime.clone()).gather("q").await else {
        panic!("expected a section");
    };
    assert_eq!(text, "## Relevant memories\n- use the migration tool\n- minor fact");
    let ContextContribution::Section(one) = MemoryContext::new(runtime).with_limit(1).gather("q").await else {
        panic!("expected a section");
    };
    assert!(!one.contains("minor fact"));
}

// ── Tool registry ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn higher_priority_provider_overrides_a_builtin_tool() {
    let model = ScriptedModel::new(vec![Reply::Call("", "c1", json!({ "text": "x" })), Reply::Text("ok")]);
    let agent = Agent::builder(model.clone(), store())
        .tool(echo())
        .tool_from(Arc::new(EchoTool { tag: "mcp:" }), "mcp", DEFAULT_TOOL_PRIORITY + 1)
        .tool_from(Arc::new(EchoTool { tag: "low:" }), "other", 1)
        .build()
        .unwrap();
    let session = run(&agent, "go").await;

    let requests = model.requests();
    assert_eq!(requests[0].tools.len(), 1, "one spec per tool name");
    let messages = agent.session(session).await.unwrap().messages;
    let result = messages.iter().flat_map(|m| m.tool_result_blocks()).next().unwrap();
    assert_eq!(result.content, "mcp:x");
}

#[tokio::test]
async fn equal_priority_providers_are_rejected() {
    let model = ScriptedModel::new(vec![]);
    let result = Agent::builder(model, store()).tool(echo()).tool_from(echo(), "mcp", DEFAULT_TOOL_PRIORITY).build();
    assert!(matches!(result, Err(AgentError::Config(_))));
}
