//! `run_command`: run a shell command in the workspace root.
//!
//! The command runs without environment variables that look like secrets
//! (see `process::command_env`), with `ATLAS_SANDBOX` / `ATLAS_SANDBOX_NETWORK`
//! describing the workspace's sandbox policy. A timeout, a cancel, or the
//! command finishing ends every process it started: on Windows through a
//! kill-on-close Job Object, on Unix by killing its process group.
//!
//! On Windows, under `ReadOnly` and `WorkspaceWrite`, the command is started
//! with a restricted token (`process::restricted`); if that fails it is
//! started normally. `policy.rs` lists what is and is not enforced. The CLI
//! asks before each command unless `--approve auto` is set.

use std::future::Future;
use std::process::{Output, Stdio};
use std::time::Duration;

use async_trait::async_trait;
use platform_contracts::policy::SandboxPolicy;
use runtime_agent::{Tool, ToolEffect};
use serde_json::{json, Value};
use shared::model::ToolSpec;
use tokio::process::Command;

use super::policy::sandbox_env;
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

/// `%SystemRoot%\System32\cmd.exe`, so a `cmd.exe` in the workspace is never picked.
#[cfg(windows)]
fn cmd_path() -> std::path::PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    std::path::PathBuf::from(root).join("System32").join("cmd.exe")
}

/// The shell running `command`. On Windows the command is passed to
/// `cmd /C` verbatim (cmd does its own parsing); on Unix the shell leads a
/// new process group so the whole group can be killed.
fn shell(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut c = Command::new(cmd_path());
        c.raw_arg("/C").raw_arg(command);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        #[cfg(unix)]
        c.process_group(0);
        c
    }
}

/// The token integrity for a restricted spawn under `sandbox`, or `None`
/// to spawn normally.
#[cfg(windows)]
fn restricted_integrity(sandbox: &SandboxPolicy) -> Option<super::process::restricted::Integrity> {
    use super::process::restricted::Integrity;
    match sandbox {
        SandboxPolicy::ReadOnly => Some(Integrity::Low),
        SandboxPolicy::WorkspaceWrite { .. } => Some(Integrity::Unchanged),
        SandboxPolicy::DangerFullAccess => None,
    }
}

/// `"<cmd.exe>" /C <command>`, the command verbatim.
#[cfg(windows)]
fn cmd_line(command: &str) -> std::ffi::OsString {
    let mut line = std::ffi::OsString::from("\"");
    line.push(cmd_path().as_os_str());
    line.push("\" /C ");
    line.push(command);
    line
}

/// Start `command` with a restricted token, or `None` if that is not wanted
/// or did not work (the caller then starts it normally).
#[cfg(windows)]
fn spawn_restricted(
    command: &str,
    workspace: &Workspace,
    sandbox: &SandboxPolicy,
) -> Option<super::process::restricted::RestrictedChild> {
    use super::process::{command_env, restricted::RestrictedChild};
    let integrity = restricted_integrity(sandbox)?;
    let (env, removed) = command_env(&sandbox_env(sandbox));
    if !removed.is_empty() {
        tracing::debug!(?removed, "removed secret-looking variables from the command environment");
    }
    match RestrictedChild::spawn(&cmd_path(), &cmd_line(command), workspace.root(), &env, integrity) {
        Ok(child) => {
            tracing::debug!(?integrity, contained = child.is_contained(), "started the command with a restricted token");
            Some(child)
        }
        Err(e) => {
            tracing::warn!("could not start the command with a restricted token ({e}); starting it without one");
            None
        }
    }
}

/// Wait up to `timeout` seconds for `output`, then format it for the model.
/// On timeout `output` is dropped, which must end the command's processes.
async fn finish(timeout: u64, output: impl Future<Output = std::io::Result<Output>>) -> Result<String, String> {
    let output = match tokio::time::timeout(Duration::from_secs(timeout), output).await {
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
        let sandbox = self.workspace.sandbox();

        #[cfg(windows)]
        if let Some(child) = spawn_restricted(command, &self.workspace, sandbox) {
            // Dropping the child (timeout, cancel) closes its job, ending everything in it.
            return finish(timeout, child.wait_with_output()).await;
        }

        let mut cmd = shell(command);
        let removed = scrub_secrets(&mut cmd);
        if !removed.is_empty() {
            tracing::debug!(?removed, "removed secret-looking variables from the command environment");
        }
        let child = cmd
            .envs(sandbox_env(sandbox))
            .current_dir(self.workspace.root())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Dropping the future (timeout, cancel) kills the shell…
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("could not start the command: {e}"))?;
        // …and dropping this guard kills everything the shell started.
        let tree = ProcessTree::contain(&child);
        let result = finish(timeout, child.wait_with_output()).await;
        drop(tree);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::test_support::TempDir;

    fn tool(dir: &TempDir) -> RunCommand {
        RunCommand::new(&Workspace::new(dir.path()).unwrap())
    }

    fn tool_with(dir: &TempDir, sandbox: SandboxPolicy) -> RunCommand {
        RunCommand::new(&Workspace::new(dir.path()).unwrap().with_sandbox(sandbox))
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
        // Both spawn paths on Windows: restricted (default) and plain.
        let outs = [
            tool(&dir).call(json!({ "command": echo })).await.unwrap(),
            tool_with(&dir, SandboxPolicy::DangerFullAccess).call(json!({ "command": echo })).await.unwrap(),
        ];
        std::env::remove_var(&name);
        std::env::remove_var(&plain);
        for out in outs {
            assert!(!out.contains("s3cr3t-value"), "{out}");
            assert!(out.contains("plain-value"), "{out}");
        }
    }

    #[tokio::test]
    async fn commands_see_the_sandbox_policy_and_network_flag() {
        let dir = TempDir::new();
        let echo = if cfg!(windows) {
            "echo [%ATLAS_SANDBOX%] [%ATLAS_SANDBOX_NETWORK%]"
        } else {
            "echo [$ATLAS_SANDBOX] [$ATLAS_SANDBOX_NETWORK]"
        };
        let out = tool(&dir).call(json!({ "command": echo })).await.unwrap();
        assert!(out.contains("[workspace-write] [0]"), "{out}");
        let out = tool_with(&dir, SandboxPolicy::DangerFullAccess).call(json!({ "command": echo })).await.unwrap();
        assert!(out.contains("[danger-full-access] [1]"), "{out}");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn quoted_arguments_reach_cmd_verbatim() {
        let dir = TempDir::new();
        for sandbox in [SandboxPolicy::workspace_write(), SandboxPolicy::DangerFullAccess] {
            let out = tool_with(&dir, sandbox).call(json!({ "command": r#"echo "a b""# })).await.unwrap();
            assert!(out.contains("--- stdout ---\n\"a b\""), "{out}");
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn a_timeout_also_kills_processes_the_command_started() {
        let dir = TempDir::new();
        // A background grandchild that would write late.txt after ~3 s, while the
        // command itself keeps running past the 1 s timeout. Checked on both spawn paths.
        let command = r#"start "" /b cmd /c "ping -n 4 127.0.0.1 >NUL & echo late> late.txt" & ping -n 30 127.0.0.1 >NUL"#;
        for sandbox in [SandboxPolicy::workspace_write(), SandboxPolicy::DangerFullAccess] {
            let err = tool_with(&dir, sandbox).call(json!({ "command": command, "timeout_secs": 1 })).await.unwrap_err();
            assert!(err.contains("timed out"), "{err}");
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert!(!dir.path().join("late.txt").exists(), "the background process outlived the command");
    }

    /// Whether this machine lets us start a process with a restricted token.
    /// Policy (or an unusual account) can refuse; the tests then only check
    /// that commands still run, through the plain fallback.
    #[cfg(windows)]
    async fn restricted_spawn_works(dir: &TempDir) -> bool {
        use crate::agent::process::restricted::{Integrity, RestrictedChild};
        match RestrictedChild::spawn(&cmd_path(), &cmd_line("exit 0"), dir.path(), &[], Integrity::Unchanged) {
            Ok(child) => child.wait_with_output().await.is_ok(),
            Err(e) => {
                eprintln!("restricted spawn unavailable, skipping the restricted checks: {e}");
                false
            }
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn restricted_token_drops_privileges_or_degrades_to_a_plain_spawn() {
        let dir = TempDir::new();
        let out = tool(&dir).call(json!({ "command": r"%SystemRoot%\System32\whoami.exe /priv" })).await.unwrap();
        assert!(out.starts_with("exit code: 0"), "{out}");
        if !restricted_spawn_works(&dir).await {
            return;
        }
        let privileges: Vec<&str> =
            out.split_whitespace().filter(|w| w.starts_with("Se") && w.ends_with("Privilege")).collect();
        assert_eq!(privileges, ["SeChangeNotifyPrivilege"], "{out}");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn read_only_commands_run_at_low_integrity_and_cannot_write_the_workspace() {
        let dir = TempDir::new();
        let read_only = tool_with(&dir, SandboxPolicy::ReadOnly);
        let out = read_only.call(json!({ "command": r"%SystemRoot%\System32\whoami.exe /groups" })).await.unwrap();
        assert!(out.starts_with("exit code: 0"), "{out}");
        if !restricted_spawn_works(&dir).await {
            return;
        }
        assert!(out.contains("Low Mandatory Level"), "{out}");
        let out = read_only.call(json!({ "command": "echo x> blocked.txt" })).await.unwrap();
        assert!(!out.starts_with("exit code: 0"), "{out}");
        assert!(!dir.path().join("blocked.txt").exists());
        // The same command is allowed under WorkspaceWrite.
        tool(&dir).call(json!({ "command": "echo x> allowed.txt" })).await.unwrap();
        assert!(dir.path().join("allowed.txt").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_timeout_kills_the_whole_process_group() {
        let dir = TempDir::new();
        let command = "(sleep 3; echo late > late.txt) & sleep 30";
        let err = tool(&dir).call(json!({ "command": command, "timeout_secs": 1 })).await.unwrap_err();
        assert!(err.contains("timed out"), "{err}");
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert!(!dir.path().join("late.txt").exists(), "the background process outlived the command");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn background_processes_do_not_outlive_a_finished_command() {
        let dir = TempDir::new();
        let command = "(sleep 2; echo late > late.txt) >/dev/null 2>&1 &";
        let out = tool(&dir).call(json!({ "command": command })).await.unwrap();
        assert!(out.starts_with("exit code: 0"), "{out}");
        tokio::time::sleep(Duration::from_secs(4)).await;
        assert!(!dir.path().join("late.txt").exists(), "the background process outlived the command");
    }
}
