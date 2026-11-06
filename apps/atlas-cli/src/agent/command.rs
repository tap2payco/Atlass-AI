//! `run_command`: run a shell command in the workspace root.
//!
//! The command runs with the user's own permissions and environment. There is
//! no sandbox or approval prompt yet (roadmap step 4), so the CLI only enables
//! this tool when explicitly asked to. On timeout the shell process is killed;
//! processes it started may keep running until step 4 adds Windows job
//! objects (and process groups elsewhere).

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use runtime_agent::{Tool, ToolEffect};
use serde_json::{json, Value};
use shared::model::ToolSpec;
use tokio::process::Command;

use super::tools::{str_arg, Workspace};

pub const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 60;
pub const MAX_COMMAND_TIMEOUT_SECS: u64 = 600;

pub struct RunCommand {
    workspace: Workspace,
}

impl RunCommand {
    pub fn new(workspace: &Workspace) -> Self {
        Self { workspace: workspace.clone() }
    }
}

fn shell(command: &str) -> Command {
    if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    }
}

#[async_trait]
impl Tool for RunCommand {
    fn spec(&self) -> ToolSpec {
        let shell_name = if cfg!(windows) { "cmd.exe" } else { "sh" };
        ToolSpec {
            name: "run_command".into(),
            description: format!(
                "Run a {shell_name} command in the workspace root and return its exit code, stdout \
                 and stderr. Use it for builds, tests and git queries. Commands cannot read input."
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command line to run." },
                    "timeout_secs": {
                        "type": "integer", "minimum": 1, "maximum": MAX_COMMAND_TIMEOUT_SECS,
                        "description": format!("Default {DEFAULT_COMMAND_TIMEOUT_SECS}.")
                    }
                },
                "required": ["command"]
            }),
        }
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::RunsProcesses
    }

    async fn call(&self, args: Value) -> Result<String, String> {
        let command = str_arg(&args, "command").ok_or("missing required argument 'command'")?;
        let timeout = args
            .get("timeout_secs")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_COMMAND_TIMEOUT_SECS)
            .clamp(1, MAX_COMMAND_TIMEOUT_SECS);

        let child = shell(command)
            .current_dir(self.workspace.root())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Dropping the future (timeout, cancel) kills the shell.
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("could not start the command: {e}"))?;

        let output = match tokio::time::timeout(Duration::from_secs(timeout), child.wait_with_output()).await {
            Ok(result) => result.map_err(|e| format!("command failed: {e}"))?,
            Err(_) => return Err(format!("command timed out after {timeout} seconds and was stopped")),
        };

        let exit = match output.status.code() {
            Some(code) => code.to_string(),
            None => "terminated by a signal".into(),
        };
        let mut text = format!("exit code: {exit}\n");
        for (label, bytes) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
            let body = String::from_utf8_lossy(bytes);
            if !body.trim().is_empty() {
                text.push_str(&format!("--- {label} ---\n{}\n", body.trim_end()));
            }
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::test_support::TempDir;

    fn tool(dir: &TempDir) -> RunCommand {
        RunCommand::new(&Workspace::new(dir.path()).unwrap())
    }

    #[tokio::test]
    async fn reports_exit_code_and_both_streams_and_runs_in_the_workspace() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("marker.txt"), "x").unwrap();
        let list = if cfg!(windows) { "dir /b" } else { "ls" };
        let out = tool(&dir)
            .call(json!({ "command": format!("{list} && echo oops 1>&2 && exit 3") }))
            .await
            .unwrap();
        assert!(out.starts_with("exit code: 3\n"), "{out}");
        assert!(out.contains("--- stdout ---") && out.contains("marker.txt"), "{out}");
        assert!(out.contains("--- stderr ---") && out.contains("oops"), "{out}");
    }

    #[tokio::test]
    async fn times_out_and_reports_it() {
        let dir = TempDir::new();
        let slow = if cfg!(windows) { "ping -n 6 127.0.0.1 >NUL" } else { "sleep 5" };
        let err = tool(&dir).call(json!({ "command": slow, "timeout_secs": 1 })).await.unwrap_err();
        assert!(err.contains("timed out after 1 seconds"), "{err}");
    }

    #[tokio::test]
    async fn requires_a_command() {
        let dir = TempDir::new();
        assert!(tool(&dir).call(json!({})).await.unwrap_err().contains("command"));
    }
}
