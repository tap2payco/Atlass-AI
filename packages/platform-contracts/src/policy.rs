//! Approval and sandbox policies for agents that change files or run
//! commands, modelled on Codex CLI's `AskForApproval` and `SandboxPolicy`.
//!
//! These types only *describe* a policy. What a given front end actually
//! enforces is documented next to the code that applies it (for atlas-cli,
//! `apps/atlas-cli/src/agent/policy.rs`).

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

/// When the user must approve an action before it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalPolicy {
    /// Ask before every action that is not read-only.
    Untrusted,
    /// Run what the sandbox contains without asking; ask for anything the
    /// sandbox cannot contain (or when the agent asks to go outside it).
    #[default]
    OnRequest,
    /// Run everything in the sandbox without asking; ask only to retry an
    /// action outside the sandbox after the sandbox blocked it.
    OnFailure,
    /// Never ask. Whatever the sandbox refuses is reported to the model.
    Never,
}

/// What actions may touch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum SandboxPolicy {
    /// Read anything, write nothing, no network.
    ReadOnly,
    /// Write inside the workspace and `writable_roots`, except the
    /// [`PROTECTED_SUBPATHS`], which stay read-only.
    WorkspaceWrite {
        /// Extra directories that may be written, besides the workspace root.
        #[serde(default)]
        writable_roots: Vec<PathBuf>,
        /// Whether commands may use the network.
        #[serde(default)]
        network_access: bool,
    },
    /// No restrictions. Only for environments that are already isolated.
    DangerFullAccess,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self::workspace_write()
    }
}

/// Directory names that stay read-only even inside a writable root, at any
/// depth (matched case-insensitively). Writing them could run code outside
/// the agent's control (`.git/hooks`) or change the agent's own
/// configuration and permissions (`.atlas`, and other agents' config dirs).
pub const PROTECTED_SUBPATHS: &[&str] = &[".git", ".atlas", ".codex", ".claude"];

impl SandboxPolicy {
    /// `WorkspaceWrite` with no extra roots and no network.
    pub fn workspace_write() -> Self {
        Self::WorkspaceWrite { writable_roots: Vec::new(), network_access: false }
    }

    /// The name used in config files and `ATLAS_SANDBOX`.
    pub fn mode_name(&self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite { .. } => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }

    /// Whether anything may be written at all.
    pub fn allows_writes(&self) -> bool {
        !matches!(self, Self::ReadOnly)
    }

    pub fn has_network_access(&self) -> bool {
        match self {
            Self::ReadOnly => false,
            Self::WorkspaceWrite { network_access, .. } => *network_access,
            Self::DangerFullAccess => true,
        }
    }

    /// The roots that may be written for a workspace at `workspace`: the
    /// workspace itself, then `writable_roots`. Empty under `ReadOnly`;
    /// under `DangerFullAccess` callers should not restrict at all.
    pub fn writable_roots(&self, workspace: &Path) -> Vec<PathBuf> {
        match self {
            Self::ReadOnly => Vec::new(),
            Self::WorkspaceWrite { writable_roots, .. } => {
                std::iter::once(workspace.to_path_buf()).chain(writable_roots.iter().cloned()).collect()
            }
            Self::DangerFullAccess => vec![workspace.to_path_buf()],
        }
    }

    /// The protected directory `relative` (a path inside a writable root)
    /// falls under, if any. Nothing is protected under `DangerFullAccess`.
    pub fn protected_component(&self, relative: &Path) -> Option<&'static str> {
        if matches!(self, Self::DangerFullAccess) {
            return None;
        }
        relative.components().find_map(|c| match c {
            Component::Normal(name) => {
                PROTECTED_SUBPATHS.iter().copied().find(|p| name.eq_ignore_ascii_case(p))
            }
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_workspace_write_and_on_request() {
        assert_eq!(ApprovalPolicy::default(), ApprovalPolicy::OnRequest);
        assert_eq!(
            SandboxPolicy::default(),
            SandboxPolicy::WorkspaceWrite { writable_roots: vec![], network_access: false }
        );
    }

    #[test]
    fn serde_uses_kebab_case_and_fills_defaults() {
        assert_eq!(serde_json::to_string(&ApprovalPolicy::OnFailure).unwrap(), r#""on-failure""#);
        assert_eq!(serde_json::from_str::<ApprovalPolicy>(r#""untrusted""#).unwrap(), ApprovalPolicy::Untrusted);
        assert_eq!(serde_json::from_str::<ApprovalPolicy>(r#""never""#).unwrap(), ApprovalPolicy::Never);

        assert_eq!(serde_json::to_string(&SandboxPolicy::ReadOnly).unwrap(), r#"{"mode":"read-only"}"#);
        let parsed: SandboxPolicy = serde_json::from_str(r#"{"mode":"workspace-write"}"#).unwrap();
        assert_eq!(parsed, SandboxPolicy::workspace_write());
        let full = SandboxPolicy::WorkspaceWrite { writable_roots: vec!["/tmp/x".into()], network_access: true };
        let round: SandboxPolicy = serde_json::from_str(&serde_json::to_string(&full).unwrap()).unwrap();
        assert_eq!(round, full);
        assert_eq!(
            serde_json::from_str::<SandboxPolicy>(r#"{"mode":"danger-full-access"}"#).unwrap(),
            SandboxPolicy::DangerFullAccess
        );
        assert!(serde_json::from_str::<SandboxPolicy>(r#"{"mode":"yolo"}"#).is_err());
    }

    #[test]
    fn protected_subpaths_are_found_at_any_depth() {
        let ws = SandboxPolicy::workspace_write();
        assert_eq!(ws.protected_component(Path::new(".git/hooks/pre-commit")), Some(".git"));
        assert_eq!(ws.protected_component(Path::new("vendor/lib/.GIT/config")), Some(".git"));
        assert_eq!(ws.protected_component(Path::new(".atlas/policy.toml")), Some(".atlas"));
        assert_eq!(ws.protected_component(Path::new("src/.gitignore")), None);
        assert_eq!(ws.protected_component(Path::new("src/main.rs")), None);
        assert_eq!(SandboxPolicy::DangerFullAccess.protected_component(Path::new(".git/config")), None);
    }

    #[test]
    fn writable_roots_and_network() {
        let root = Path::new("/w");
        assert!(SandboxPolicy::ReadOnly.writable_roots(root).is_empty());
        assert!(!SandboxPolicy::ReadOnly.allows_writes());
        let ws = SandboxPolicy::WorkspaceWrite { writable_roots: vec!["/extra".into()], network_access: false };
        assert_eq!(ws.writable_roots(root), [PathBuf::from("/w"), PathBuf::from("/extra")]);
        assert!(!ws.has_network_access());
        assert!(SandboxPolicy::DangerFullAccess.has_network_access());
        assert_eq!(ws.mode_name(), "workspace-write");
    }
}
