//! `run_command`: run a shell command in the workspace root.
//!
//! The command runs with the user's own permissions, minus environment
//! variables that look like secrets (see `process::scrub_secrets`). On
//! Windows it runs in a kill-on-close Job Object, so a timeout, a cancel, or
//! the command finishing ends every process it started. There is no file or
//! network sandbox yet; the CLI asks before each command unless `--approve
//! auto` is set.

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use runtime_agent::{Tool, ToolEffect};
use serde_json::{json, Value};
use shared::model::ToolSpec;
use tokio::process::Command;

use super::process::{scrub_secrets, ProcessTree};
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
                 and stderr. Use it for builds, tests and git queries. Commands cannot read input, \
                 cannot leave background processes running, and do not see environment variables \
                 that hold keys, tokens, secrets or passwords."
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

        let mut cmd = shell(command);
        let removed = scrub_secrets(&mut cmd);
        if !removed.is_empty() {
            tracing::debug!(?removed, "removed secret-looking variables from the command environment");
        }
        let child = cmd
            .current_dir(self.workspace.root())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Dropping the future (timeout, cancel) kills the shell…
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("could not start the command: {e}"))?;
        // …and dropping this guard kills everything the shell started (Windows).
        let tree = ProcessTree::contain(&child);

        let waited = tokio::time::timeout(Duration::from_secs(timeout), child.wait_with_output()).await;
        drop(tree);
        let output = match waited {
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

    #[tokio::test]
    async fn secret_looking_variables_are_hidden_from_commands() {
        let dir = TempDir::new();
        let name = format!("ATLAS_TEST_{}_TOKEN", std::process::id());
        let plain = format!("ATLAS_TEST_{}_VISIBLE", std::process::id());
        std::env::set_var(&name, "s3cr3t-value");
        std::env::set_var(&plain, "plain-value");
        let echo = if cfg!(windows) {
            format!("echo [%{name}%] [%{plain}%]")
        } else {
            format!("echo [${name}] [${plain}]")
        };
        let out = tool(&dir).call(json!({ "command": echo })).await.unwrap();
        std::env::remove_var(&name);
        std::env::remove_var(&plain);
        assert!(!out.contains("s3cr3t-value"), "{out}");
        assert!(out.contains("plain-value"), "{out}");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn a_timeout_also_kills_processes_the_command_started() {
        let dir = TempDir::new();
        // A background grandchild that would write late.txt after ~3 s, while the
        // command itself keeps running past the 1 s timeout.
        let command = r#"start "" /b cmd /c "ping -n 4 127.0.0.1 >NUL & echo late> late.txt" & ping -n 30 127.0.0.1 >NUL"#;
        let err = tool(&dir).call(json!({ "command": command, "timeout_secs": 1 })).await.unwrap_err();
        assert!(err.contains("timed out"), "{err}");
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert!(!dir.path().join("late.txt").exists(), "the background process outlived the command");
    }
}
