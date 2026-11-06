use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use atlas_cli::{agent::CliAgent, commands, repl};

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
    /// Ask the tool-using agent about a project (read-only tools; Ctrl+C cancels).
    Agent {
        /// The request, e.g. "Where is the cache key computed?"
        #[arg(required = true, num_args = 1..)]
        prompt: Vec<String>,
        /// Project directory. Defaults to the current directory.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Maximum model responses before the agent stops.
        #[arg(long, default_value_t = runtime_agent::DEFAULT_MAX_TURNS)]
        max_turns: usize,
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
    Repl,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let res = match &cli.command {
        Some(Commands::Boot) => commands::boot::execute_boot().await,
        Some(Commands::Status) => commands::status::execute_status().await,
        Some(Commands::Run { prompt }) => commands::agent_run::execute_agent_run(prompt).await,
        Some(Commands::Agent { prompt, path, max_turns }) => {
            run_agent_once(&prompt.join(" "), path.clone(), *max_turns).await
        }
        Some(Commands::Query { domain, query }) => commands::query::execute_query(domain, query).await,
        Some(Commands::Server { port }) => atlas_cli::server::run_server(*port).await,
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

async fn run_agent_once(prompt: &str, path: Option<PathBuf>, max_turns: usize) -> Result<(), String> {
    let root = match path {
        Some(p) => p,
        None => std::env::current_dir().map_err(|e| e.to_string())?,
    };
    let agent = CliAgent::start(&root, max_turns).await?;
    println!("atlas agent · {}", agent.workspace_display());
    let outcome = agent.ask(prompt).await?;
    match outcome.cause {
        runtime_agent::StopCause::EndTurn => Ok(()),
        cause => Err(format!("agent stopped: {cause:?}")),
    }
}
