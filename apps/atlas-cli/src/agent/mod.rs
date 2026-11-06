//! The CLI's tool-using agent: `runtime-agent` + `runtime-model` + read-only
//! workspace tools, with terminal rendering and Ctrl+C cancellation.

pub mod tools;

use std::path::Path;
use std::sync::Arc;

use runtime_agent::{
    Agent, AgentError, AgentEvent, EventSink, InMemorySessionStore, InferenceSettings, StopCause,
    TurnOutcome,
};
use runtime_model::{ModelRuntime, ModelRuntimeConfig};
use shared::error::InferenceError;
use shared::id::SessionId;
use shared::model::Role;
use tokio_util::sync::CancellationToken;

use tools::{workspace_tools, Workspace};

/// One agent conversation bound to one workspace.
pub struct CliAgent {
    agent: Agent,
    session: SessionId,
    workspace: Workspace,
    ollama_url: String,
}

impl CliAgent {
    /// Build with the model runtime configured from the environment.
    pub async fn start(root: &Path, max_turns: usize) -> Result<Self, String> {
        let config = ModelRuntimeConfig::from_env();
        let ollama_url = config.ollama_base_url.clone();
        let model = Arc::new(ModelRuntime::build(config).await);
        Self::with_model(root, max_turns, model, ollama_url).await
    }

    pub async fn with_model(
        root: &Path,
        max_turns: usize,
        model: Arc<dyn sdk::model::ModelRuntime>,
        ollama_url: String,
    ) -> Result<Self, String> {
        let workspace = Workspace::new(root)?;
        let mut builder = Agent::builder(model, Arc::new(InMemorySessionStore::new()))
            .settings(settings_for(&workspace))
            .max_turns(max_turns);
        for tool in workspace_tools(&workspace) {
            builder = builder.tool(tool);
        }
        let agent = builder.build().map_err(|e| e.to_string())?;
        let session = agent.create_session().await.map_err(|e| e.to_string())?;
        Ok(Self { agent, session, workspace, ollama_url })
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
                 ATLAS_FIREWORKS_API_KEY. Your message is kept; type `continue` to retry.",
                self.ollama_url
            ),
            AgentError::Inference(e) => {
                format!("Model call failed: {e}. Your message is kept; type `continue` to retry.")
            }
            other => other.to_string(),
        }
    }
}

fn settings_for(workspace: &Workspace) -> InferenceSettings {
    InferenceSettings {
        system_prompt: format!(
            "You are Atlas, a software engineering agent working in the project at {}.\n\
             You can inspect it with the list_files, read_file and search_text tools; \
             all paths are relative to the project root. You cannot modify files yet.\n\
             Look at the code before answering questions about it, and cite file paths \
             with line numbers. Be concise.",
            workspace.display_root()
        ),
        ..InferenceSettings::default()
    }
}

async fn print_events(mut rx: tokio::sync::mpsc::UnboundedReceiver<AgentEvent>) {
    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::ToolStarted { name, .. } => println!("  ⚙ {name}"),
            AgentEvent::MessageAppended(m) if m.role == Role::Assistant => {
                // Text the model wrote alongside tool calls ("Let me look at…").
                let text = m.text();
                if m.tool_calls().next().is_some() && !text.trim().is_empty() {
                    println!("  {}", text.trim());
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
