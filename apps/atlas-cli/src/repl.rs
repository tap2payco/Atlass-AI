use std::io::{self, Write};
use std::path::{Path, PathBuf};
use runtime_repository::discovery::{DiscoveryConfig, RepositoryDiscovery};
use crate::agent::{short_id, Access, ApprovalMode, CliAgent};
use crate::commands::{boot, status, agent_run, query};

/// The chat agent for the linked project, created on first use and replaced
/// when another project is linked.
async fn chat_agent<'a>(
    slot: &'a mut Option<CliAgent>,
    project: &Path,
    access: Access,
    approval: ApprovalMode,
) -> Result<&'a CliAgent, String> {
    let stale = slot
        .as_ref()
        .is_none_or(|a| project.canonicalize().ok().as_deref() != Some(a.workspace_root()));
    if stale {
        *slot = Some(CliAgent::start(project, runtime_agent::DEFAULT_MAX_TURNS, access, approval).await?);
    }
    Ok(slot.as_ref().expect("agent was just created"))
}

fn describe_access(access: Access, approval: ApprovalMode) -> String {
    let asking = match approval {
        ApprovalMode::Ask => "asking you first",
        ApprovalMode::Auto => "without asking",
    };
    match access {
        Access::Read => "read — the agent can only look at files".to_string(),
        Access::Write => format!("write — the agent can also create and edit files (not .git), {asking}"),
        Access::Full => format!("full — the agent can also edit files and run shell commands, {asking}"),
    }
}

pub async fn run_repl() -> Result<(), String> {
    let mut active_project_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut history: Vec<(String, String)> = Vec::new();
    let mut chat: Option<CliAgent> = None;
    let mut access = Access::Read;
    let mut approval = ApprovalMode::Ask;

    println!("╭─────────────────────────────────────────────────────────────────────────────╮");
    println!("│ 🤖 Atlas AI Platform — Interactive Terminal Desktop Agent (Claude Code Style)│");
    println!("├─────────────────────────────────────────────────────────────────────────────┤");
    println!("│ Linked Project: {} ", active_project_path.display());
    println!("│ Type 'link <path>' to link any local project folder on your computer.       │");
    println!("│ Type 'help' for commands, 'clear' to reset screen, 'exit' or 'quit' to exit.│");
    println!("╰─────────────────────────────────────────────────────────────────────────────╯\n");

    loop {
        let folder_name = active_project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project");

        print!("atlas [{}] ❯ ", folder_name);
        io::stdout().flush().map_err(|e| e.to_string())?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }

        let line = input.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts[0] {
            "exit" | "quit" => {
                println!("\n╭─ Goodbye ───────────────────────────────────────────────────╮");
                println!("│ Exiting Atlas Shell. Happy engineering!                     │");
                println!("╰─────────────────────────────────────────────────────────────╯");
                break;
            }
            "clear" => {
                print!("\x1B[2J\x1B[1;1H");
                io::stdout().flush().map_err(|e| e.to_string())?;
                continue;
            }
            "help" => {
                println!("\n╭─ 📖 Commands Manual ────────────────────────────────────────╮");
                println!("│ link <path>        - Link & index target project folder     │");
                println!("│ boot               - Boot all platform runtimes             │");
                println!("│ status             - Check platform health & telemetry      │");
                println!("│ <any text>         - Ask the agent (reads files; Ctrl+C stops)│");
                println!("│ continue           - Let the agent keep going after a stop  │");
                println!("│ new                - Start a fresh agent conversation       │");
                println!("│ access <level>     - Agent tools: read | write | full       │");
                println!("│ approve <mode>     - ask before writes/commands, or auto    │");
                println!("│ sessions           - List saved conversations for the project│");
                println!("│ resume <id|last>   - Continue a saved conversation          │");
                println!("│ run <prompt>       - Run 5-phase agent task on linked project│");
                println!("│ query <domain> <q> - Query memory or context                    │");
                println!("│ clear              - Clear terminal output                  │");
                println!("│ exit / quit        - Exit interactive shell                 │");
                println!("╰─────────────────────────────────────────────────────────────╯");
            }
            "link" => {
                if parts.len() < 2 {
                    println!("Usage: link <project_folder_path>");
                } else {
                    let target_str = parts[1..].join(" ");
                    let target_path = PathBuf::from(&target_str);
                    if target_path.exists() && target_path.is_dir() {
                        active_project_path = target_path.canonicalize().unwrap_or(target_path.clone());
                        println!("\n╭─ ⚙️ Tool Execution: Link Project ───────────────────────────╮");
                        println!("│ Linked Path: {} ", active_project_path.display());
                        
                        // Run Discovery scan
                        let repo_config = DiscoveryConfig {
                            root_path: active_project_path.clone(),
                            max_depth: Some(3),
                            respect_gitignore: true,
                        };
                        let discovery = RepositoryDiscovery::new(repo_config);
                        match discovery.discover() {
                            Ok(res) => {
                                println!("│ Files Discovered: {} | Git: {} | Language: {}", 
                                    res.files.len(), 
                                    res.is_git, 
                                    res.primary_language.as_deref().unwrap_or("unknown")
                                );
                                println!("│ Status: 🟢 Project Indexed & Ready");
                            }
                            Err(e) => {
                                println!("│ Warning: Discovery error: {}", e);
                            }
                        }
                        println!("╰─────────────────────────────────────────────────────────────╯");
                    } else {
                        println!("Error: Path '{}' does not exist or is not a directory.", target_str);
                    }
                }
            }
            "boot" => {
                println!("\n╭─ ⚙️ Tool Execution: Boot Runtimes ───────────────────────────╮");
                if let Err(e) = boot::execute_boot().await {
                    println!("│ Boot failed: {}", e);
                } else {
                    println!("│ Status: 🟢 All Platform Runtimes Booted Topologically");
                }
                println!("╰─────────────────────────────────────────────────────────────╯");
            }
            "status" => {
                println!("\n╭─ ⚙️ Tool Execution: Platform Telemetry ─────────────────────╮");
                if let Err(e) = status::execute_status().await {
                    println!("│ Status check failed: {}", e);
                }
                println!("╰─────────────────────────────────────────────────────────────╯");
            }
            "run" => {
                let prompt = parts[1..].join(" ");
                if prompt.is_empty() {
                    println!("Usage: run <prompt>");
                } else {
                    if let Err(e) = agent_run::execute_agent_run_on_path(&prompt, Some(active_project_path.clone())).await {
                        println!("Agent run error: {}", e);
                    } else {
                        history.push((prompt, "Completed agent trace".to_string()));
                    }
                }
            }
            "query" => {
                if parts.len() < 3 {
                    println!("Usage: query <domain> <query_str>");
                } else {
                    println!("\n╭─ ⚙️ Tool Execution: Query Engine ───────────────────────────╮");
                    if let Err(e) = query::execute_query(parts[1], parts[2]).await {
                        println!("│ Query failed: {}", e);
                    }
                    println!("╰─────────────────────────────────────────────────────────────╯");
                }
            }
            "access" => match parts.get(1) {
                None => println!("Agent access: {}", describe_access(access, approval)),
                Some(level) => match level.parse::<Access>() {
                    Err(e) => println!("{e}"),
                    Ok(level) => {
                        let applied = match chat.as_mut() {
                            Some(agent) => agent.set_access(level),
                            None => Ok(()),
                        };
                        match applied {
                            Ok(()) => {
                                access = level;
                                println!("Agent access: {}", describe_access(access, approval));
                            }
                            Err(e) => println!("Agent error: {}", e),
                        }
                    }
                },
            },
            "approve" => match parts.get(1) {
                None => println!("Approval mode: {approval}"),
                Some(mode) => match mode.parse::<ApprovalMode>() {
                    Err(e) => println!("{e}"),
                    Ok(mode) => {
                        let applied = match chat.as_mut() {
                            Some(agent) => agent.set_approval(mode),
                            None => Ok(()),
                        };
                        match applied {
                            Ok(()) => {
                                approval = mode;
                                println!("Agent access: {}", describe_access(access, approval));
                            }
                            Err(e) => println!("Agent error: {}", e),
                        }
                    }
                },
            },
            "sessions" => match chat_agent(&mut chat, &active_project_path, access, approval).await {
                Err(e) => println!("Agent error: {}", e),
                Ok(agent) => match agent.saved_sessions() {
                    Err(e) => println!("{e}"),
                    Ok(list) if list.is_empty() => println!("No saved conversations for this project yet."),
                    Ok(list) => {
                        let current = agent.session_id();
                        for s in list.iter().take(20) {
                            let marker = if s.id == current { "*" } else { " " };
                            let title = if s.title.is_empty() { "(empty)" } else { s.title.as_str() };
                            println!("{marker} {}  {:>3} msgs  {}", short_id(s.id), s.message_count, title);
                        }
                        println!("Resume one with: resume <id>   (* = current)");
                    }
                },
            },
            "resume" => {
                let query = parts.get(1).copied().unwrap_or("last");
                let switched = match chat_agent(&mut chat, &active_project_path, access, approval).await {
                    Err(e) => Err(e),
                    Ok(_) => chat.as_mut().expect("agent exists").switch_to(query),
                };
                match switched {
                    Ok(s) => println!(
                        "Resumed {} ({} messages): {}. Type `continue` to finish an interrupted turn, or ask something new.",
                        short_id(s.id),
                        s.message_count,
                        s.title
                    ),
                    Err(e) => println!("{e}"),
                }
            }
            "new" => match chat.as_mut() {
                Some(agent) => match agent.reset().await {
                    Ok(()) => println!("Started a new agent conversation."),
                    Err(e) => println!("Agent error: {}", e),
                },
                None => println!("Started a new agent conversation."),
            },
            "continue" => match chat.as_ref() {
                Some(agent) => {
                    if let Err(e) = agent.resume().await {
                        println!("Agent error: {}", e);
                    }
                }
                None => println!("Nothing to continue yet. Ask the agent something first."),
            },
            _ => {
                // Conversational prompt: one ongoing agent conversation per linked project.
                let user_prompt = line.to_string();
                let result = match chat_agent(&mut chat, &active_project_path, access, approval).await {
                    Ok(agent) => agent.ask(&user_prompt).await,
                    Err(e) => Err(e),
                };
                match result {
                    Ok(outcome) => history.push((user_prompt, outcome.reply.unwrap_or_default())),
                    Err(e) => println!("Agent error: {}", e),
                }
            }
        }
        println!();
    }

    Ok(())
}
