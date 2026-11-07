use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use atlas_cli::{agent::{short_id, Access, ApprovalMode, CliAgent}, commands, repl};

#[derive(Parser)]
#[command(name = "atlas")]
#[command(about = "Atlas Platform Unified CLI Daemon & Agent Runner")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Boot,
    Status,
    Run {
        #[arg(short, long)]
        prompt: String,
    },
    /// Ask the tool-using agent to look at or work on a project (Ctrl+C cancels).
    Agent {
        /// The request, e.g. "Where is the cache key computed?"
        #[arg(required_unless_present = "serve_stdio", num_args = 1..)]
        prompt: Vec<String>,
        /// Project directory. Defaults to the current directory.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Maximum model responses before the agent stops.
        #[arg(long, default_value_t = runtime_agent::DEFAULT_MAX_TURNS)]
        max_turns: usize,
        /// Tools the agent may use: read (inspect only), write (also edit
        /// files, not .git) or full (also run shell commands).
        #[arg(long, default_value = "read")]
        access: Access,
        /// ask: confirm every write and command (default). auto: never ask.
        #[arg(long, default_value = "ask")]
        approve: ApprovalMode,
        /// Continue a saved conversation for this project: `last`, or the
        /// session id (the first 8 characters are enough).
        #[arg(long)]
        resume: Option<String>,
        /// Instead of one prompt, speak the agent protocol as JSON lines on
        /// stdin/stdout (ops in, events out), e.g. for atlas-dashboard.
        #[arg(long)]
        serve_stdio: bool,
    },
    Query {
        #[arg(short, long, default_value = "memory")]
        domain: String,
        #[arg(short, long)]
        query: String,
    },
    Server {
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
    /// Open the Atlas app in your browser: the same agent and saved
    /// conversations as `atlas agent`, with approvals and diffs in the page.
    Ui {
        /// Project directory. Defaults to the current directory.
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
        /// Start the server without opening a browser.
        #[arg(long)]
        no_open: bool,
    },
    Repl,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let res = match &cli.command {
        Some(Commands::Boot) => commands::boot::execute_boot().await,
        Some(Commands::Status) => commands::status::execute_status().await,
        Some(Commands::Run { prompt }) => commands::agent_run::execute_agent_run(prompt).await,
        Some(Commands::Agent { path, max_turns, access, approve, resume, serve_stdio: true, .. }) => {
            match path.clone().map(Ok).unwrap_or_else(|| std::env::current_dir().map_err(|e| e.to_string())) {
                Ok(root) => {
                    atlas_cli::agent::serve::serve_stdio(&root, *max_turns, *access, *approve, resume.as_deref()).await
                }
                Err(e) => Err(e),
            }
        }
        Some(Commands::Agent { prompt, path, max_turns, access, approve, resume, .. }) => {
            run_agent_once(&prompt.join(" "), path.clone(), *max_turns, *access, *approve, resume.as_deref()).await
        }
        Some(Commands::Query { domain, query }) => commands::query::execute_query(domain, query).await,
        Some(Commands::Server { port }) => atlas_cli::server::run_server(*port).await,
        Some(Commands::Ui { path, port, no_open }) => {
            atlas_cli::server::run_server_with(*port, path.clone(), !*no_open).await
        }
        Some(Commands::Repl) | None => repl::run_repl().await,
    };

    match res {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

async fn run_agent_once(
    prompt: &str,
    path: Option<PathBuf>,
    max_turns: usize,
    access: Access,
    approve: ApprovalMode,
    resume: Option<&str>,
) -> Result<(), String> {
    let root = match path {
        Some(p) => p,
        None => std::env::current_dir().map_err(|e| e.to_string())?,
    };
    let mut agent = CliAgent::start(&root, max_turns, access, approve).await?;
    if let Some(query) = resume {
        let found = agent.switch_to(query)?;
        println!("resuming {} ({} messages): {}", short_id(found.id), found.message_count, found.title);
    }
    println!(
        "atlas agent · {} · access: {} · approve: {} · session {}",
        agent.workspace_display(),
        agent.access(),
        agent.approval(),
        short_id(agent.session_id())
    );
    let outcome = agent.ask(prompt).await;
    if agent.is_saving() {
        println!("\n(saved; continue with --resume {})", short_id(agent.session_id()));
    }
    match outcome?.cause {
        runtime_agent::StopCause::EndTurn => Ok(()),
        cause => Err(format!("agent stopped: {cause:?}")),
    }
}
