//! The CLI's tool-using agent: `runtime-agent` + `runtime-model` + workspace
//! tools, with terminal rendering and Ctrl+C cancellation.

pub mod approval;
pub mod command;
pub mod process;
pub mod tools;
pub mod write_tools;

#[cfg(test)]
pub(crate) mod test_support;

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use std::collections::BTreeMap;
use std::path::PathBuf;

use runtime_agent::{
    Agent, AgentError, AgentEvent, CompactionSettings, EventSink, FileSessionStore,
    InMemorySessionStore, InferenceSettings, SessionStore, SessionSummary, StopCause, Tool, TurnOutcome,
};
use runtime_model::{ModelRuntime, ModelRuntimeConfig};
use shared::error::InferenceError;
use shared::id::SessionId;
use shared::model::Role;
use tokio_util::sync::CancellationToken;

pub use approval::ApprovalMode;
use approval::{approver_for, ConsoleApprover};
use command::RunCommand;
use tools::{read_tools, Workspace};
use write_tools::write_tools;

/// Which tools the agent gets. Writes and commands are also gated by the
/// `ApprovalMode` (by default the user is asked each time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Access {
    /// list_files, read_file, search_text.
    #[default]
    Read,
    /// Read, plus write_file and edit_file inside the workspace (not `.git`).
    Write,
    /// Write, plus run_command.
    Full,
}

impl FromStr for Access {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "read" | "read-only" | "readonly" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            "full" => Ok(Self::Full),
            other => Err(format!("unknown access level '{other}' (use read, write or full)")),
        }
    }
}

impl fmt::Display for Access {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Full => "full",
        })
    }
}

fn tools_for(access: Access, workspace: &Workspace) -> Vec<Arc<dyn Tool>> {
    let mut tools = read_tools(workspace);
    if access >= Access::Write {
        tools.extend(write_tools(workspace));
    }
    if access >= Access::Full {
        tools.push(Arc::new(RunCommand::new(workspace)));
    }
    tools
}

const RETRY_HINT: &str =
    "In the REPL your message is kept: type `continue` to retry. Otherwise, run the command again.";

/// Session metadata key holding the workspace a session belongs to.
const WORKSPACE_KEY: &str = "workspace";

/// Where saved conversations live: `$ATLAS_HOME/sessions`, else the
/// per-user data directory (`%LOCALAPPDATA%\atlas\sessions` on Windows,
/// `$XDG_DATA_HOME/atlas/sessions` or `~/.local/share/atlas/sessions`
/// elsewhere). Never inside the project.
pub fn sessions_dir() -> PathBuf {
    let base = std::env::var_os("ATLAS_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            if cfg!(windows) {
                std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("atlas"))
            } else {
                std::env::var_os("XDG_DATA_HOME")
                    .map(|d| PathBuf::from(d).join("atlas"))
                    .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/atlas")))
            }
        })
        .unwrap_or_else(|| std::env::temp_dir().join("atlas"));
    base.join("sessions")
}

/// The first 8 characters of a session's uuid: enough to resume it.
pub fn short_id(id: SessionId) -> String {
    id.0.to_string().chars().take(8).collect()
}

/// One agent conversation bound to one workspace.
pub struct CliAgent {
    agent: Agent,
    session: SessionId,
    workspace: Workspace,
    access: Access,
    approval: ApprovalMode,
    max_turns: usize,
    model: Arc<dyn sdk::model::ModelRuntime>,
    store: Arc<dyn SessionStore>,
    /// Present when sessions are saved to disk; used to list and find them.
    saved: Option<Arc<FileSessionStore>>,
    /// Kept across rebuilds so "always" answers survive an access change.
    console: Arc<ConsoleApprover>,
    /// Summarise older history near this many tokens. Off when `None`.
    compaction: Option<CompactionSettings>,
    ollama_url: String,
}

impl CliAgent {
    /// Build with the model runtime configured from the environment, saving
    /// conversations under `sessions_dir()`. If that directory cannot be
    /// used, conversations are kept in memory and a warning is printed.
    pub async fn start(
        root: &Path,
        max_turns: usize,
        access: Access,
        approval: ApprovalMode,
    ) -> Result<Self, String> {
        let config = ModelRuntimeConfig::from_env();
        let ollama_url = config.ollama_base_url.clone();
        // Plan compaction around the window Ollama is asked for, unless overridden.
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
        let mut agent = Self::build(root, max_turns, access, approval, model, ollama_url, saved).await?;
        agent.set_compaction(Some(CompactionSettings { context_tokens, ..CompactionSettings::default() }))?;
        Ok(agent)
    }

    /// Turn context compaction on (with these settings) or off. The conversation is kept.
    pub fn set_compaction(&mut self, compaction: Option<CompactionSettings>) -> Result<(), String> {
        let previous = std::mem::replace(&mut self.compaction, compaction);
        self.rebuild().inspect_err(|_| self.compaction = previous.clone())
    }

    /// Build with a given model; conversations are kept in memory only.
    pub async fn with_model(
        root: &Path,
        max_turns: usize,
        access: Access,
        approval: ApprovalMode,
        model: Arc<dyn sdk::model::ModelRuntime>,
        ollama_url: String,
    ) -> Result<Self, String> {
        Self::build(root, max_turns, access, approval, model, ollama_url, None).await
    }

    /// Build with a given model, saving conversations in `store`.
    pub async fn with_saved_sessions(
        root: &Path,
        max_turns: usize,
        access: Access,
        approval: ApprovalMode,
        model: Arc<dyn sdk::model::ModelRuntime>,
        store: Arc<FileSessionStore>,
    ) -> Result<Self, String> {
        Self::build(root, max_turns, access, approval, model, String::new(), Some(store)).await
    }

    async fn build(
        root: &Path,
        max_turns: usize,
        access: Access,
        approval: ApprovalMode,
        model: Arc<dyn sdk::model::ModelRuntime>,
        ollama_url: String,
        saved: Option<Arc<FileSessionStore>>,
    ) -> Result<Self, String> {
        let workspace = Workspace::new(root)?;
        let store: Arc<dyn SessionStore> = match &saved {
            Some(file) => file.clone(),
            None => Arc::new(InMemorySessionStore::new()),
        };
        let console = Arc::new(ConsoleApprover::new());
        let mut this = Self {
            agent: Agent::builder(model.clone(), store.clone()).build().map_err(|e| e.to_string())?,
            session: SessionId::new(),
            workspace,
            access,
            approval,
            max_turns,
            model,
            store,
            saved,
            console,
            compaction: None,
            ollama_url,
        };
        this.rebuild()?;
        this.session = this.new_session().await?;
        Ok(this)
    }

    async fn new_session(&self) -> Result<SessionId, String> {
        let metadata = BTreeMap::from([(WORKSPACE_KEY.to_string(), self.workspace.display_root())]);
        self.agent.create_session_with(metadata).await.map_err(|e| e.to_string())
    }

    pub fn session_id(&self) -> SessionId {
        self.session
    }

    /// Whether conversations are written to disk.
    pub fn is_saving(&self) -> bool {
        self.saved.is_some()
    }

    /// Saved conversations for this workspace, most recent first.
    pub fn saved_sessions(&self) -> Result<Vec<SessionSummary>, String> {
        let Some(saved) = &self.saved else {
            return Err("conversations are not being saved in this run".into());
        };
        let workspace = self.workspace.display_root();
        Ok(saved
            .list()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|s| s.metadata.get(WORKSPACE_KEY) == Some(&workspace))
            .collect())
    }

    /// Switch to a saved conversation: `last` (the most recent one for this
    /// workspace other than the current one), a full id, or an id prefix.
    pub fn switch_to(&mut self, query: &str) -> Result<SessionSummary, String> {
        let sessions = self.saved_sessions()?;
        let found = if query.eq_ignore_ascii_case("last") {
            sessions
                .iter()
                .find(|s| s.id != self.session && s.message_count > 0)
                .cloned()
                .ok_or_else(|| "there is no earlier saved conversation for this project".to_string())?
        } else {
            let id = self.saved.as_ref().expect("checked by saved_sessions").find(query).map_err(|e| e.to_string())?;
            sessions.iter().find(|s| s.id == id).cloned().ok_or_else(|| {
                "that conversation belongs to a different project folder; open it from there".to_string()
            })?
        };
        self.session = found.id;
        Ok(found)
    }

    fn rebuild(&mut self) -> Result<(), String> {
        let mut builder = Agent::builder(self.model.clone(), self.store.clone())
            .settings(settings_for(&self.workspace, self.access))
            .max_turns(self.max_turns)
            .approver(approver_for(self.approval, &self.console));
        if let Some(compaction) = &self.compaction {
            builder = builder.compaction(compaction.clone());
        }
        for tool in tools_for(self.access, &self.workspace) {
            builder = builder.tool(tool);
        }
        self.agent = builder.build().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn access(&self) -> Access {
        self.access
    }

    pub fn approval(&self) -> ApprovalMode {
        self.approval
    }

    /// Change the tool set. The conversation is kept.
    pub fn set_access(&mut self, access: Access) -> Result<(), String> {
        let previous = self.access;
        self.access = access;
        self.rebuild().inspect_err(|_| self.access = previous)
    }

    /// Change whether writes and commands need confirmation. The conversation is kept.
    pub fn set_approval(&mut self, approval: ApprovalMode) -> Result<(), String> {
        let previous = self.approval;
        self.approval = approval;
        self.rebuild().inspect_err(|_| self.approval = previous)
    }

    pub fn workspace_root(&self) -> &Path {
        self.workspace.root()
    }

    pub fn workspace_display(&self) -> String {
        self.workspace.display_root()
    }

    /// Start a fresh conversation in the same workspace.
    pub async fn reset(&mut self) -> Result<(), String> {
        self.session = self.new_session().await?;
        Ok(())
    }

    /// Send a user message and print the run as it happens.
    pub async fn ask(&self, prompt: &str) -> Result<TurnOutcome, String> {
        self.run(Some(prompt)).await
    }

    /// Continue the current turn (after the turn limit, a cancel or an error).
    pub async fn resume(&self) -> Result<TurnOutcome, String> {
        self.run(None).await
    }

    async fn run(&self, prompt: Option<&str>) -> Result<TurnOutcome, String> {
        let cancel = CancellationToken::new();
        let ctrl_c = {
            let cancel = cancel.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    cancel.cancel();
                }
            })
        };
        let (events, rx) = EventSink::channel();
        let printer = tokio::spawn(print_events(rx));

        let result = match prompt {
            Some(text) => self.agent.submit(self.session, text, &cancel, &events).await,
            None => self.agent.resume(self.session, &cancel, &events).await,
        };

        ctrl_c.abort();
        drop(events);
        let _ = printer.await;

        let outcome = result.map_err(|e| self.explain(e))?;
        print_outcome(&outcome);
        Ok(outcome)
    }

    fn explain(&self, error: AgentError) -> String {
        match error {
            AgentError::Inference(InferenceError::AllProvidersFailed) => format!(
                "No model provider answered. Start Ollama ({}) and either install qwen2.5-coder:7b \
                 or set ATLAS_OLLAMA_MODEL to an installed tool-capable model; or set \
                 ATLAS_FIREWORKS_API_KEY. {RETRY_HINT}",
                self.ollama_url
            ),
            AgentError::Inference(e) => format!("Model call failed: {e}. {RETRY_HINT}"),
            other => other.to_string(),
        }
    }
}

fn settings_for(workspace: &Workspace, access: Access) -> InferenceSettings {
    let abilities = match access {
        Access::Read => {
            "You can inspect it with list_files, read_file and search_text. \
             You cannot modify files or run commands in this session."
        }
        Access::Write => {
            "You can inspect it with list_files, read_file and search_text, and change it \
             with edit_file (preferred for existing files) and write_file. You cannot run commands \
             in this session. Read a file before editing it."
        }
        Access::Full => {
            "You can inspect it with list_files, read_file and search_text, change it with \
             edit_file (preferred for existing files) and write_file, and run builds, tests and \
             other shell commands with run_command. Read a file before editing it, and run the \
             relevant tests after changing code."
        }
    };
    InferenceSettings {
        system_prompt: format!(
            "You are Atlas, a software engineering agent working in the project at {}.\n\
             {abilities} All paths are relative to the project root.\n\
             The user may be asked to approve each change or command. If a call is declined, \
             follow the reason they give and do not repeat the same call.\n\
             Look at the code before answering questions about it, cite file paths with line \
             numbers, and say plainly what you changed. Be concise.",
            workspace.display_root()
        ),
        ..InferenceSettings::default()
    }
}

/// A one-line summary of a tool call's main argument, for the terminal.
fn describe_call(call: &shared::model::ToolCall) -> String {
    let arg = ["command", "path", "query"]
        .iter()
        .find_map(|key| call.arguments.get(*key).and_then(|v| v.as_str()));
    match arg {
        Some(value) => {
            let short: String = value.chars().take(80).collect();
            let ellipsis = if value.chars().count() > 80 { "…" } else { "" };
            format!("{} {short}{ellipsis}", call.name)
        }
        None => call.name.clone(),
    }
}

async fn print_events(mut rx: tokio::sync::mpsc::UnboundedReceiver<AgentEvent>) {
    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::MessageAppended(m) if m.role == Role::Assistant => {
                // Text the model wrote alongside tool calls ("Let me look at…").
                let text = m.text();
                let calls: Vec<_> = m.tool_calls().collect();
                if !calls.is_empty() && !text.trim().is_empty() {
                    println!("  {}", text.trim());
                }
                for call in calls {
                    println!("  ⚙ {}", describe_call(call));
                }
            }
            AgentEvent::ToolDenied { name, .. } => println!("  ✗ {name} declined"),
            AgentEvent::MessageAppended(m) => {
                if let Some((_, covers)) = m.as_summary() {
                    println!("  ⟳ summarised the {covers} earlier messages to stay within the context window");
                }
            }
            _ => {}
        }
    }
}

fn print_outcome(outcome: &TurnOutcome) {
    match outcome.cause {
        StopCause::EndTurn => {
            let reply = outcome.reply.as_deref().unwrap_or("").trim();
            println!("\n{}", if reply.is_empty() { "(the model returned an empty reply)" } else { reply });
        }
        StopCause::MaxTurns => {
            println!("\nStopped at the turn limit. Type `continue` to let the agent keep going.")
        }
        StopCause::Cancelled => println!("\nCancelled."),
        StopCause::Idle => println!("\nNothing to continue."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_support::TempDir;

    struct NoModel;

    #[async_trait::async_trait]
    impl sdk::model::ModelRuntime for NoModel {
        async fn execute_inference(
            &self,
            _request: shared::model::ModelRequest,
        ) -> Result<shared::model::ModelResponse, InferenceError> {
            Err(InferenceError::AllProvidersFailed)
        }

        async fn execute_stream(
            &self,
            _request: shared::model::ModelRequest,
            _tx: tokio::sync::mpsc::Sender<String>,
        ) -> Result<shared::model::ExecutionMetadata, InferenceError> {
            Err(InferenceError::AllProvidersFailed)
        }

        async fn health_check(&self) -> platform_contracts::health::HealthState {
            platform_contracts::health::HealthState::Offline
        }
    }

    fn names(access: Access, ws: &Workspace) -> Vec<String> {
        tools_for(access, ws).iter().map(|t| t.spec().name).collect()
    }

    #[test]
    fn each_access_level_adds_tools() {
        let dir = TempDir::new();
        let ws = Workspace::new(dir.path()).unwrap();
        assert_eq!(names(Access::Read, &ws), ["list_files", "read_file", "search_text"]);
        assert_eq!(
            names(Access::Write, &ws),
            ["list_files", "read_file", "search_text", "write_file", "edit_file"]
        );
        assert_eq!(names(Access::Full, &ws).last().map(String::as_str), Some("run_command"));
    }

    #[test]
    fn access_parses_and_orders() {
        assert_eq!("FULL".parse::<Access>().unwrap(), Access::Full);
        assert_eq!("read-only".parse::<Access>().unwrap(), Access::Read);
        assert!("admin".parse::<Access>().is_err());
        assert!(Access::Read < Access::Write && Access::Write < Access::Full);
    }

    #[tokio::test]
    async fn changing_access_keeps_the_conversation() {
        let dir = TempDir::new();
        let mut agent = CliAgent::with_model(dir.path(), 5, Access::Read, ApprovalMode::Auto, Arc::new(NoModel), "x".into())
            .await
            .unwrap();
        // The model is offline, but the user message is stored before the call fails.
        assert!(agent.ask("hello").await.is_err());
        agent.set_access(Access::Full).unwrap();
        assert_eq!(agent.access(), Access::Full);
        let session = agent.agent.session(agent.session).await.unwrap();
        assert_eq!(session.messages, vec![shared::model::Message::user("hello")]);
    }

    #[tokio::test]
    async fn saved_sessions_are_per_workspace_and_resumable() {
        let sessions = TempDir::new();
        let project_a = TempDir::new();
        let project_b = TempDir::new();
        let store = Arc::new(FileSessionStore::open(sessions.path()).unwrap());
        async fn open(root: &TempDir, store: &Arc<FileSessionStore>) -> CliAgent {
            CliAgent::with_saved_sessions(root.path(), 5, Access::Read, ApprovalMode::Auto, Arc::new(NoModel), store.clone())
                .await
                .unwrap()
        }

        // Earlier run in project A: the model is offline, but the question is saved.
        let earlier = open(&project_a, &store).await;
        assert!(earlier.ask("first question").await.is_err());
        let earlier_id = earlier.session_id();
        drop(earlier);

        // A run in project B does not see A's conversation.
        let mut other = open(&project_b, &store).await;
        assert!(other.switch_to("last").is_err());
        let err = other.switch_to(&short_id(earlier_id)).unwrap_err();
        assert!(err.contains("different project"), "{err}");

        // A new run in A resumes it with `last`.
        let mut later = open(&project_a, &store).await;
        assert_ne!(later.session_id(), earlier_id);
        let found = later.switch_to("last").unwrap();
        assert_eq!(found.id, earlier_id);
        assert_eq!(found.title, "first question");
        assert_eq!(later.session_id(), earlier_id);
        let titles: Vec<_> = later.saved_sessions().unwrap().into_iter().map(|s| s.title).collect();
        assert!(titles.contains(&"first question".to_string()));
    }

    #[test]
    fn sessions_dir_honours_atlas_home() {
        // Only checks the override; the platform default depends on the machine.
        let dir = TempDir::new();
        std::env::set_var("ATLAS_HOME", dir.path());
        let resolved = sessions_dir();
        std::env::remove_var("ATLAS_HOME");
        assert_eq!(resolved, dir.path().join("sessions"));
    }
}
