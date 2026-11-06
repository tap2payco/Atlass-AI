//! How atlas-cli's `--access` and `--approve` options map onto the
//! `platform-contracts` policies, and what each policy actually enforces.
//!
//! | `--access` | sandbox policy                                  | tools                       |
//! |------------|-------------------------------------------------|-----------------------------|
//! | `read`     | `ReadOnly`                                      | list/read/search            |
//! | `write`    | `WorkspaceWrite { [], network_access: false }`  | + write_file, edit_file     |
//! | `full`     | `WorkspaceWrite { [], network_access: false }`  | + run_command               |
//!
//! | `--approve` | approval policy | asks before                         |
//! |-------------|-----------------|-------------------------------------|
//! | `ask`       | `Untrusted`     | every write and every command       |
//! | `auto`      | `Never`         | nothing                             |
//!
//! Enforced today:
//! - **File tools** (`write_file`, `edit_file`): writes are refused under
//!   `ReadOnly`, outside the workspace and `writable_roots`, and under the
//!   protected subpaths (`.git`, `.atlas`, `.codex`, `.claude`).
//! - **Commands, Windows**: run in a kill-on-close Job Object (assigned
//!   while suspended, so nothing escapes) with a restricted token
//!   (`DISABLE_MAX_PRIVILEGE`: every privilege but SeChangeNotify removed).
//!   Under `ReadOnly` the token is also lowered to Low integrity, so the
//!   command cannot write the user's ordinary files. If the restricted
//!   spawn fails, the command falls back to a plain spawn (still in a job).
//! - **Commands, Unix**: run in their own process group, which is killed
//!   on timeout, cancel and completion.
//! - Secret-looking environment variables are removed from commands.
//!
//! Not enforced yet:
//! - **Network**: `network_access: false` is only advertised to the command
//!   as `ATLAS_SANDBOX_NETWORK=0` (and `ATLAS_SANDBOX=<mode>`). Nothing blocks
//!   sockets (no WFP filters on Windows, no seccomp/namespace on Linux).
//! - **Command file writes under `WorkspaceWrite`**: a command can write
//!   anything the user can, including `.git`. Protected subpaths only bind
//!   the file tools. (Codex adds workspace ACLs and a sandbox user for this.)
//! - `OnRequest` / `OnFailure` have no sandbox-denial signal to escalate on,
//!   so both allow file-tool writes (which the checks above contain) and
//!   ask before every command.

use platform_contracts::policy::{ApprovalPolicy, SandboxPolicy};
use runtime_agent::ToolEffect;

use super::approval::ApprovalMode;
use super::Access;

impl Access {
    /// The sandbox this access level runs tools under.
    pub fn sandbox_policy(self) -> SandboxPolicy {
        match self {
            Access::Read => SandboxPolicy::ReadOnly,
            Access::Write | Access::Full => SandboxPolicy::workspace_write(),
        }
    }

    /// Whether `run_command` is offered at all.
    pub fn allows_commands(self) -> bool {
        self >= Access::Full
    }
}

impl ApprovalMode {
    pub fn approval_policy(self) -> ApprovalPolicy {
        match self {
            ApprovalMode::Ask => ApprovalPolicy::Untrusted,
            ApprovalMode::Auto => ApprovalPolicy::Never,
        }
    }
}

/// Whether a call with `effect` must be put to the user under `policy`.
pub fn needs_approval(policy: ApprovalPolicy, effect: ToolEffect) -> bool {
    match policy {
        ApprovalPolicy::Untrusted => effect != ToolEffect::ReadOnly,
        // File-tool writes are contained by the workspace checks; commands are not.
        ApprovalPolicy::OnRequest | ApprovalPolicy::OnFailure => effect == ToolEffect::RunsProcesses,
        ApprovalPolicy::Never => false,
    }
}

/// Environment variables that tell a command which sandbox it runs in.
pub fn sandbox_env(sandbox: &SandboxPolicy) -> [(&'static str, &'static str); 2] {
    [
        ("ATLAS_SANDBOX", sandbox.mode_name()),
        ("ATLAS_SANDBOX_NETWORK", if sandbox.has_network_access() { "1" } else { "0" }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_maps_to_sandbox_policies() {
        assert_eq!(Access::Read.sandbox_policy(), SandboxPolicy::ReadOnly);
        assert_eq!(Access::Write.sandbox_policy(), SandboxPolicy::workspace_write());
        assert_eq!(Access::Full.sandbox_policy(), SandboxPolicy::workspace_write());
        assert!(!Access::Full.sandbox_policy().has_network_access());
        assert!(!Access::Write.allows_commands() && Access::Full.allows_commands());
        // The CLI's defaults stay read-only and asking.
        assert_eq!(Access::default().sandbox_policy(), SandboxPolicy::ReadOnly);
        assert_eq!(ApprovalMode::default().approval_policy(), ApprovalPolicy::Untrusted);
    }

    #[test]
    fn approval_modes_map_to_policies_and_keep_behaviour() {
        assert_eq!(ApprovalMode::Ask.approval_policy(), ApprovalPolicy::Untrusted);
        assert_eq!(ApprovalMode::Auto.approval_policy(), ApprovalPolicy::Never);

        use ToolEffect::*;
        let ask = |p| [ReadOnly, WritesWorkspace, RunsProcesses].map(|e| needs_approval(p, e));
        assert_eq!(ask(ApprovalPolicy::Untrusted), [false, true, true]);
        assert_eq!(ask(ApprovalPolicy::OnRequest), [false, false, true]);
        assert_eq!(ask(ApprovalPolicy::OnFailure), [false, false, true]);
        assert_eq!(ask(ApprovalPolicy::Never), [false, false, false]);
    }

    #[test]
    fn sandbox_env_advertises_mode_and_network() {
        assert_eq!(
            sandbox_env(&SandboxPolicy::workspace_write()),
            [("ATLAS_SANDBOX", "workspace-write"), ("ATLAS_SANDBOX_NETWORK", "0")]
        );
        assert_eq!(sandbox_env(&SandboxPolicy::DangerFullAccess)[1], ("ATLAS_SANDBOX_NETWORK", "1"));
    }
}
