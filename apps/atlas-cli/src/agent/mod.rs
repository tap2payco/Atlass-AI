//! The CLI's tool-using agent: `runtime-agent` + `runtime-model` + workspace
//! tools, with terminal rendering and Ctrl+C cancellation.

pub mod command;
pub mod tools;
pub mod write_tools;

#[cfg(test)]
pub(crate) mod test_support;

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use runtime_agent::{
    Agent, AgentError, AgentEvent, EventSink, InMemorySessionStore, InferenceSettings, StopCause,
    Tool, TurnOutcome,
};
use runtime_model::{ModelRuntime, ModelRuntimeConfig};
use shared::error::InferenceError;
use shared::id::SessionId;
use shared::model::Role;
use tokio_util::sync::CancellationToken;

use command::RunCommand;
use tools::{read_tools, Workspace};
use write_tools::write_tools;

/// Which tools the agent gets. There are no per-call approval prompts yet
/// (roadmap step 4), so anything beyond `Read` must be chosen explicitly.
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

/// One agent conversation bound to one workspace.
pub struct CliAgent {
    agent: Agent,
    session: SessionId,
    workspace: Workspace,
    access: Access,
    max_turns: usize,
    model: Arc<dyn sdk::model::ModelRuntime>,
    store: Arc<InMemorySessionStore>,
    ollama_url: String,
}

impl CliAgent {
    /// Build with the model runtime configured from the environment.
    pub async fn start(root: &Path, max_turns: usize, access: Access) -> Result<Self, String> {
        let config = ModelRuntimeConfig::from_env();
        let ollama_url = config.ollama_base_url.clone();
        let model = Arc::new(ModelRuntime::build(config).await);
        Self::with_model(root, max_turns, access, model, ollama_url).await
    }

    pub async fn with_model(
        root: &Path,
        max_turns: usize,
        access: Access,
        model: Arc<dyn sdk::model::ModelRuntime>,
        ollama_url: String,
    ) -> Result<Self, String> {
        let workspace = Workspace::new(root)?;
        let store = Arc::new(InMemorySessionStore::new());
        let agent = build_agent(&workspace, access, max_turns, model.clone(), store.clone())?;
        let session = agent.create_session().await.map_err(|e| e.to_string())?;
        Ok(Self { agent, session, workspace, access, max_turns, model, store, ollama_url })
    }

    pub fn access(&self) -> Access {
        self.access
    }

    /// Change the tool set. The conversation is kept.
    pub fn set_access(&mut self, access: Access) -> Result<(), String> {
        self.agent =
            build_agent(&self.workspace, access, self.max_turns, self.model.clone(), self.store.clone())?;
        self.access = access;
        Ok(())
    }

    pub fn workspace_root(&self) -> &Path {
        self.workspace.root()
    }

    pub fn workspace_display(&self) -> String {
        self.workspace.display_root()
    }

    /// Start a fresh conversation in the same workspace.
    pub async fn reset(&mut self) -> Result<(), String> {
        self.session = self.agent.create_session().await.map_err(|e| e.to_string())?;
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

fn build_agent(
    workspace: &Workspace,
    access: Access,
    max_turns: usize,
    model: Arc<dyn sdk::model::ModelRuntime>,
    store: Arc<InMemorySessionStore>,
) -> Result<Agent, String> {
    let mut builder = Agent::builder(model, store)
        .settings(settings_for(workspace, access))
        .max_turns(max_turns);
    for tool in tools_for(access, workspace) {
        builder = builder.tool(tool);
    }
    builder.build().map_err(|e| e.to_string())
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
        if let AgentEvent::MessageAppended(m) = event {
            if m.role != Role::Assistant {
                continue;
            }
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
        let mut agent = CliAgent::with_model(dir.path(), 5, Access::Read, Arc::new(NoModel), "x".into())
            .await
            .unwrap();
        // The model is offline, but the user message is stored before the call fails.
        assert!(agent.ask("hello").await.is_err());
        agent.set_access(Access::Full).unwrap();
        assert_eq!(agent.access(), Access::Full);
        let session = agent.agent.session(agent.session).await.unwrap();
        assert_eq!(session.messages, vec![shared::model::Message::user("hello")]);
    }
}
