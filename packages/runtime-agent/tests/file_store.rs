//! FileSessionStore: persistence across store instances (process restarts),
//! crash tolerance, listing and lookup.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use platform_contracts::health::HealthState;
use runtime_agent::{Agent, AgentError, EventSink, FileSessionStore, Session, SessionStore, StopCause};
use sdk::model::ModelRuntime;
use shared::error::InferenceError;
use shared::id::SessionId;
use shared::model::{
    CircuitState, ExecutionMetadata, Message, ModelRequest, ModelResponse, ModelTier, Role, StopReason,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "atlas-file-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn file_of(dir: &TempDir, id: SessionId) -> PathBuf {
    dir.path().join(format!("{}.jsonl", id.0))
}

#[tokio::test]
async fn sessions_survive_a_new_store_instance() {
    let dir = TempDir::new();
    let metadata = BTreeMap::from([("workspace".to_string(), "D:/proj".to_string())]);
    let session = Session::with_metadata(metadata.clone());
    let id = session.id;
    {
        let store = FileSessionStore::open(dir.path()).unwrap();
        store.create(session).await.unwrap();
        store.append(id, vec![Message::user("hello")]).await.unwrap();
        store.append(id, vec![Message::assistant("hi"), Message::user("again")]).await.unwrap();
    }
    let reopened = FileSessionStore::open(dir.path()).unwrap();
    let loaded = reopened.load(id).await.unwrap();
    assert_eq!(loaded.metadata, metadata);
    assert_eq!(
        loaded.messages,
        vec![Message::user("hello"), Message::assistant("hi"), Message::user("again")]
    );
}

#[tokio::test]
async fn a_cut_off_last_line_is_ignored_but_a_corrupt_middle_line_is_an_error() {
    let dir = TempDir::new();
    let store = FileSessionStore::open(dir.path()).unwrap();
    let session = Session::new();
    let id = session.id;
    store.create(session).await.unwrap();
    store.append(id, vec![Message::user("kept")]).await.unwrap();

    // Simulate a crash in the middle of writing the next line.
    let path = file_of(&dir, id);
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("{\"kind\":\"message\",\"message\":{\"role\":\"assis");
    fs::write(&path, &text).unwrap();
    assert_eq!(store.load(id).await.unwrap().messages, vec![Message::user("kept")]);

    // A damaged line that is not the last one is reported, not silently dropped.
    text.push_str("\n{\"kind\":\"message\",\"message\":{\"role\":\"user\",\"content\":[]}}\n");
    fs::write(&path, &text).unwrap();
    assert!(matches!(store.load(id).await, Err(AgentError::Store(_))));
}

#[tokio::test]
async fn unknown_ids_and_duplicates_are_rejected() {
    let dir = TempDir::new();
    let store = FileSessionStore::open(dir.path()).unwrap();
    let missing = SessionId::new();
    assert!(matches!(store.load(missing).await, Err(AgentError::SessionNotFound(_))));
    assert!(matches!(store.append(missing, vec![]).await, Err(AgentError::SessionNotFound(_))));

    let session = Session::new();
    store.create(session.clone()).await.unwrap();
    assert!(matches!(store.create(session).await, Err(AgentError::Store(_))));
}

#[tokio::test]
async fn list_and_find_by_prefix() {
    let dir = TempDir::new();
    let store = FileSessionStore::open(dir.path()).unwrap();
    let a = Session::new();
    let b = Session::new();
    let (a_id, b_id) = (a.id, b.id);
    store.create(a).await.unwrap();
    store.create(b).await.unwrap();
    store
        .append(a_id, vec![Message::user("Fix   the\nlogin bug please, it has been broken for a long long time now")])
        .await
        .unwrap();
    fs::write(dir.path().join("notes.txt"), "not a session").unwrap();
    fs::write(dir.path().join("broken.jsonl"), "garbage").unwrap();

    let list = store.list().unwrap();
    assert_eq!(list.len(), 2, "non-session and unreadable files are skipped");
    let a_summary = list.iter().find(|s| s.id == a_id).unwrap();
    assert_eq!(a_summary.message_count, 1);
    assert!(a_summary.title.starts_with("Fix the login bug please"), "{}", a_summary.title);
    assert!(a_summary.title.ends_with('…'));

    let prefix: String = a_id.0.to_string().chars().take(8).collect();
    assert_eq!(store.find(&prefix).unwrap(), a_id);
    assert_eq!(store.find(&format!("session_{}", b_id.0)).unwrap(), b_id);
    assert!(store.find("abc").is_err(), "too short");
    assert!(store.find("ffffffff-not-there").is_err());
}

// ── An agent resumed by a new process ─────────────────────────────────────────

/// Fails the first call (as if the model were offline), then answers.
#[derive(Default)]
struct FailOnce {
    calls: Mutex<usize>,
    last_request: Mutex<Option<ModelRequest>>,
}

#[async_trait]
impl ModelRuntime for FailOnce {
    async fn execute_inference(&self, request: ModelRequest) -> Result<ModelResponse, InferenceError> {
        *self.last_request.lock().unwrap() = Some(request.clone());
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        if *calls == 1 {
            return Err(InferenceError::AllProvidersFailed);
        }
        Ok(ModelResponse {
            content: "answer".into(),
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
            capability: request.capability,
            metadata: ExecutionMetadata {
                provider_id: "t".into(),
                model_id: "t".into(),
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

#[tokio::test]
async fn a_turn_interrupted_in_one_process_is_resumed_by_another() {
    let dir = TempDir::new();
    let model = Arc::new(FailOnce::default());

    // "Process 1": the model call fails; the user's message is already on disk.
    let id = {
        let agent = Agent::builder(model.clone(), Arc::new(FileSessionStore::open(dir.path()).unwrap()))
            .build()
            .unwrap();
        let id = agent.create_session().await.unwrap();
        let err = agent.submit(id, "question", &CancellationToken::new(), &EventSink::none()).await;
        assert!(err.is_err());
        id
    };

    // "Process 2": a fresh store and agent over the same directory.
    let agent = Agent::builder(model.clone(), Arc::new(FileSessionStore::open(dir.path()).unwrap()))
        .build()
        .unwrap();
    let outcome = agent.resume(id, &CancellationToken::new(), &EventSink::none()).await.unwrap();
    assert_eq!(outcome.cause, StopCause::EndTurn);
    assert_eq!(outcome.reply.as_deref(), Some("answer"));
    let seen = model.last_request.lock().unwrap().clone().unwrap();
    assert_eq!(seen.messages, vec![Message::user("question")]);

    let stored = FileSessionStore::open(dir.path()).unwrap().load(id).await.unwrap();
    let roles: Vec<Role> = stored.messages.iter().map(|m| m.role).collect();
    assert_eq!(roles, vec![Role::User, Role::Assistant]);
}
