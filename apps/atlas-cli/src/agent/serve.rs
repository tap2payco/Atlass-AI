//! The agent as a protocol server (roadmap step 5): `atlas agent
//! --serve-stdio` speaks JSON lines (`runtime_agent::protocol`) on
//! stdin/stdout, and the web server streams the same events over HTTP.
//!
//! One `Op` per input line, one `Event` per output line. Lines that are not
//! valid ops get an `error` event; end of input lets the running turn finish
//! and then exits. Nothing else is written to stdout.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use runtime_agent::protocol::{decode_op, encode_line, AgentDriver, ApprovalBroker, Event, DEFAULT_APPROVAL_TIMEOUT};
use runtime_agent::{
    Agent, AutoApprove, CompactionSettings, FileSessionStore, InMemorySessionStore, SessionStore, ToolApprover,
};
use runtime_model::{ModelRuntime, ModelRuntimeConfig};
use shared::id::SessionId;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

use super::tools::Workspace;
use super::{sessions_dir, settings_for, tools_for, Access, ApprovalMode, WORKSPACE_KEY};

/// Everything needed to drive an agent for one workspace.
pub struct ProtocolAgent {
    pub agent: Agent,
    pub broker: Option<ApprovalBroker>,
    /// Labels for new sessions (the workspace path).
    pub metadata: BTreeMap<String, String>,
    pub store: Option<Arc<FileSessionStore>>,
    pub workspace: String,
}

impl ProtocolAgent {
    /// Build the agent with the CLI's tools for `access`. With `Ask`,
    /// writes and commands are approved by the protocol client.
    pub fn build(
        root: &Path,
        max_turns: usize,
        access: Access,
        approval: ApprovalMode,
        model: Arc<dyn sdk::model::ModelRuntime>,
        saved: Option<Arc<FileSessionStore>>,
        compaction: Option<CompactionSettings>,
    ) -> Result<Self, String> {
        let workspace = Workspace::new(root)?;
        let store: Arc<dyn SessionStore> = match &saved {
            Some(file) => file.clone(),
            None => Arc::new(InMemorySessionStore::new()),
        };
        let broker = match approval {
            ApprovalMode::Ask => Some(ApprovalBroker::new(DEFAULT_APPROVAL_TIMEOUT)),
            ApprovalMode::Auto => None,
        };
        let approver: Arc<dyn ToolApprover> = match &broker {
            Some(broker) => broker.approver(),
            None => Arc::new(AutoApprove),
        };
        let mut builder = Agent::builder(model, store)
            .settings(settings_for(&workspace, access))
            .max_turns(max_turns)
            .approver(approver);
        if let Some(compaction) = compaction {
            builder = builder.compaction(compaction);
        }
        for tool in tools_for(access, &workspace) {
            builder = builder.tool(tool);
        }
        let agent = builder.build().map_err(|e| e.to_string())?;
        let metadata = BTreeMap::from([(WORKSPACE_KEY.to_string(), workspace.display_root())]);
        Ok(Self { agent, broker, metadata, store: saved, workspace: workspace.display_root() })
    }

    /// Build with the model configured from the environment and sessions
    /// saved under `sessions_dir()` (in memory if that fails). Warnings go
    /// to stderr so stdout stays protocol-only.
    pub async fn from_env(root: &Path, max_turns: usize, access: Access, approval: ApprovalMode) -> Result<Self, String> {
        let config = ModelRuntimeConfig::from_env();
        let context_tokens = std::env::var("ATLAS_CONTEXT_TOKENS")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|n| *n > 0)
            .or(config.ollama_num_ctx.map(|n| n as usize))
            .unwrap_or(runtime_model::config::DEFAULT_OLLAMA_NUM_CTX as usize);
        let model = Arc::new(ModelRuntime::build(config).await);
        let saved = match FileSessionStore::open(sessions_dir()) {
            Ok(store) => Some(Arc::new(store)),
            Err(e) => {
                eprintln!("warning: conversations will not be saved ({e})");
                None
            }
        };
        let compaction = Some(CompactionSettings { context_tokens, ..CompactionSettings::default() });
        Self::build(root, max_turns, access, approval, model, saved, compaction)
    }

    /// Resolve `last` or an id (prefix) to a saved session of this workspace.
    pub fn find_session(&self, query: &str) -> Result<SessionId, String> {
        let Some(store) = &self.store else {
            return Err("conversations are not being saved in this run".into());
        };
        let mine: Vec<_> = store
            .list()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|s| s.metadata.get(WORKSPACE_KEY) == Some(&self.workspace))
            .collect();
        if query.eq_ignore_ascii_case("last") {
            return mine
                .iter()
                .find(|s| s.message_count > 0)
                .map(|s| s.id)
                .ok_or_else(|| "there is no saved conversation for this project".to_string());
        }
        let id = store.find(query).map_err(|e| e.to_string())?;
        if mine.iter().any(|s| s.id == id) {
            Ok(id)
        } else {
            Err("that conversation belongs to a different project folder; open it from there".into())
        }
    }

    pub fn into_driver(self) -> AgentDriver {
        let mut driver = AgentDriver::new(self.agent).with_session_metadata(self.metadata);
        if let Some(broker) = self.broker {
            driver = driver.with_approvals(broker);
        }
        driver
    }
}

/// `atlas agent --serve-stdio`.
pub async fn serve_stdio(
    root: &Path,
    max_turns: usize,
    access: Access,
    approval: ApprovalMode,
    resume: Option<&str>,
) -> Result<(), String> {
    let agent = ProtocolAgent::from_env(root, max_turns, access, approval).await?;
    let resume = resume.map(|q| agent.find_session(q)).transpose()?;
    let mut driver = agent.into_driver();
    if let Some(id) = resume {
        driver = driver.with_session(id);
    }
    let stdin = tokio::io::BufReader::new(tokio::io::stdin());
    serve_lines(driver, stdin, tokio::io::stdout()).await
}

/// Run `driver` over a JSON-lines transport until `input` ends and the
/// driver has sent its last event.
pub async fn serve_lines<R, W>(driver: AgentDriver, input: R, mut output: W) -> Result<(), String>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let (ops, mut events) = driver.spawn();
    let mut ops = Some(ops);
    let mut lines = input.lines();
    loop {
        tokio::select! {
            line = lines.next_line(), if ops.is_some() => match line {
                Ok(Some(line)) if line.trim().is_empty() => {}
                Ok(Some(line)) => match decode_op(&line) {
                    Ok(op) => {
                        let _ = ops.as_ref().expect("guarded").send(op);
                    }
                    Err(message) => write_event(&mut output, &Event::Error { message }).await?,
                },
                // End of input (or an unreadable stream): let the turn finish.
                Ok(None) | Err(_) => ops = None,
            },
            event = events.recv() => match event {
                Some(event) => write_event(&mut output, &event).await?,
                None => return Ok(()),
            },
        }
    }
}

async fn write_event<W: AsyncWrite + Unpin>(output: &mut W, event: &Event) -> Result<(), String> {
    output.write_all(encode_line(event).as_bytes()).await.map_err(|e| e.to_string())?;
    output.flush().await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::test_support::TempDir;
    use runtime_agent::protocol::{Op, TurnCause};
    use shared::error::InferenceError;
    use shared::model::{CircuitState, ExecutionMetadata, ModelRequest, ModelResponse, ModelTier, StopReason};
    use tokio::io::{AsyncReadExt, BufReader};

    /// Answers every request with the same text.
    struct FixedReply;

    #[async_trait::async_trait]
    impl sdk::model::ModelRuntime for FixedReply {
        async fn execute_inference(&self, request: ModelRequest) -> Result<ModelResponse, InferenceError> {
            Ok(ModelResponse {
                content: "hello from the agent".into(),
                tool_calls: vec![],
                stop_reason: StopReason::EndTurn,
                capability: request.capability,
                metadata: ExecutionMetadata {
                    provider_id: "fixed".into(),
                    model_id: "fixed".into(),
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
            _tx: tokio::sync::mpsc::Sender<String>,
        ) -> Result<ExecutionMetadata, InferenceError> {
            Err(InferenceError::AllProvidersFailed)
        }

        async fn health_check(&self) -> platform_contracts::health::HealthState {
            platform_contracts::health::HealthState::Healthy
        }
    }

    async fn run_lines(input: String) -> Vec<Event> {
        let dir = TempDir::new();
        let agent = ProtocolAgent::build(dir.path(), 5, Access::Read, ApprovalMode::Ask, Arc::new(FixedReply), None, None)
            .unwrap();
        let (mut client, server) = tokio::io::duplex(64 * 1024);
        let (server_read, server_write) = tokio::io::split(server);
        let serve = tokio::spawn(serve_lines(agent.into_driver(), BufReader::new(server_read), server_write));
        client.write_all(input.as_bytes()).await.unwrap();
        client.shutdown().await.unwrap(); // end of input
        let mut out = String::new();
        client.read_to_string(&mut out).await.unwrap();
        serve.await.unwrap().unwrap();
        assert!(out.ends_with('\n'), "{out:?}");
        out.lines().map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}"))).collect()
    }

    #[tokio::test]
    async fn stdio_mode_frames_one_event_per_line() {
        let input = format!(
            "{}\n   \nnot json\n{}",
            r#"{"type":"turn_input","text":"hi"}"#,
            encode_line(&Op::Interrupt), // no turn running by then, or a harmless cancel
        );
        let events = run_lines(input).await;
        // Parse errors are written by the transport, so they may come before the driver's first event.
        let driver_events: Vec<_> = events.iter().filter(|e| !matches!(e, Event::Error { .. })).collect();
        assert!(matches!(driver_events[0], Event::SessionConfigured { resumed: false, .. }), "{events:?}");
        assert!(events.iter().any(|e| matches!(e, Event::Error { message } if message.starts_with("invalid op"))));
        assert!(matches!(events.last(), Some(Event::TurnComplete { .. })), "{events:?}");
    }

    #[tokio::test]
    async fn stdio_mode_runs_a_turn_to_completion_after_end_of_input() {
        let events = run_lines(encode_line(&Op::TurnInput { session: None, text: "hi".into() })).await;
        assert_eq!(events.len(), 3, "{events:?}");
        assert_eq!(events[1], Event::AgentMessage { text: "hello from the agent".into() });
        assert!(matches!(
            &events[2],
            Event::TurnComplete { cause: TurnCause::EndTurn, reply: Some(r), .. } if r == "hello from the agent"
        ));
    }
}
