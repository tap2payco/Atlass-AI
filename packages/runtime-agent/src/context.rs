//! Context providers as operations (agent plan step 7).
//!
//! A `ContextSource` gathers text about the workspace (git state, memories,
//! knowledge-graph facts). `ContextOperation` runs one source when a new
//! request arrives and adds what it found to the system prompt for every
//! model call of that turn. The result is cached per turn, keyed by the
//! session and the position of the turn's kickoff message, so the source is
//! asked once per user request, not once per tool round.
//!
//! Sources here:
//! - [`GitContext`]: branch, short status and recent commits.
//! - [`MemoryContext`]: records from `runtime-memory`'s `MemoryRuntime`.
//! - [`KnowledgeGraphContext`]: reports "not available"; `runtime-knowledge`
//!   has no query API yet.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use runtime_memory::api::MemoryRuntime;
use shared::id::SessionId;
use shared::model::Message;

use crate::conversation::is_kickoff;
use crate::error::AgentError;
use crate::operation::{Operation, OperationResult, StepContext};
use crate::session::Session;

/// What a source found for one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextContribution {
    /// A system-prompt section.
    Section(String),
    /// Nothing relevant (e.g. no memories yet).
    Empty,
    /// The source cannot be used; the reason is shown to the model in one line.
    Unavailable(String),
}

#[async_trait]
pub trait ContextSource: Send + Sync {
    /// Stable name, also the operation name.
    fn name(&self) -> &'static str;

    /// Gather context for `request`, the text of the turn's user message.
    async fn gather(&self, request: &str) -> ContextContribution;
}

type TurnKey = (SessionId, usize);

/// Runs a `ContextSource` once per turn and contributes its result as a
/// prompt part. Never appends messages.
pub struct ContextOperation {
    source: Arc<dyn ContextSource>,
    cache: Mutex<Option<(TurnKey, Vec<String>)>>,
}

impl ContextOperation {
    pub fn new(source: Arc<dyn ContextSource>) -> Self {
        Self { source, cache: Mutex::new(None) }
    }

    fn turn_key(session: &Session) -> Option<(TurnKey, &Message)> {
        let index = session.messages.iter().rposition(is_kickoff)?;
        Some(((session.id, index), &session.messages[index]))
    }

    fn render(&self, contribution: ContextContribution) -> Vec<String> {
        match contribution {
            ContextContribution::Section(text) if !text.trim().is_empty() => vec![text],
            ContextContribution::Section(_) | ContextContribution::Empty => Vec::new(),
            ContextContribution::Unavailable(reason) => {
                tracing::info!(source = self.source.name(), %reason, "context source not available");
                vec![format!("[context: {} not available: {reason}]", self.source.name())]
            }
        }
    }
}

#[async_trait]
impl Operation for ContextOperation {
    fn name(&self) -> &'static str {
        self.source.name()
    }

    async fn prepare(&self, session: &Session) -> Result<(), AgentError> {
        let Some((key, kickoff)) = Self::turn_key(session) else {
            return Ok(());
        };
        if self.cache.lock().unwrap().as_ref().is_some_and(|(k, _)| *k == key) {
            return Ok(());
        }
        let parts = self.render(self.source.gather(&kickoff.text()).await);
        *self.cache.lock().unwrap() = Some((key, parts));
        Ok(())
    }

    fn prompt_parts(&self, session: &Session) -> Vec<String> {
        let Some((key, _)) = Self::turn_key(session) else {
            return Vec::new();
        };
        match &*self.cache.lock().unwrap() {
            Some((k, parts)) if *k == key => parts.clone(),
            _ => Vec::new(),
        }
    }

    async fn run(&self, _ctx: &StepContext<'_>) -> Result<OperationResult, AgentError> {
        Ok(OperationResult::NotApplicable)
    }
}

// ── Git ───────────────────────────────────────────────────────────────────────

/// Branch, `git status --short` and recent commits of a workspace.
pub struct GitContext {
    root: PathBuf,
    max_commits: usize,
    max_status_lines: usize,
}

impl GitContext {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), max_commits: 5, max_status_lines: 20 }
    }

    pub fn with_limits(mut self, max_commits: usize, max_status_lines: usize) -> Self {
        self.max_commits = max_commits;
        self.max_status_lines = max_status_lines;
        self
    }

    fn git(root: &PathBuf, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            // Read-only: never take the index lock for a status refresh.
            .env("GIT_OPTIONAL_LOCKS", "0")
            .output()
            .map_err(|e| format!("git could not be run: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(stderr.lines().next().unwrap_or("git failed").trim().to_string());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
    }

    fn collect(root: PathBuf, max_commits: usize, max_status_lines: usize) -> ContextContribution {
        if let Err(e) = Self::git(&root, &["rev-parse", "--is-inside-work-tree"]) {
            return ContextContribution::Unavailable(e);
        }
        let branch = Self::git(&root, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_else(|_| "(unknown)".into());
        let status = Self::git(&root, &["status", "--short"]).unwrap_or_default();
        let commits = Self::git(&root, &["log", "--oneline", "--no-decorate", &format!("-n{max_commits}")])
            .unwrap_or_default();

        let mut text = format!("## Workspace git state\nBranch: {branch}\n");
        let lines: Vec<&str> = status.lines().collect();
        if lines.is_empty() {
            text.push_str("Status: clean\n");
        } else {
            text.push_str(&format!("Status ({} changed):\n", lines.len()));
            for line in lines.iter().take(max_status_lines) {
                text.push_str(line);
                text.push('\n');
            }
            if lines.len() > max_status_lines {
                text.push_str(&format!("… and {} more\n", lines.len() - max_status_lines));
            }
        }
        if commits.is_empty() {
            text.push_str("Recent commits: none\n");
        } else {
            text.push_str("Recent commits:\n");
            text.push_str(&commits);
            text.push('\n');
        }
        ContextContribution::Section(text.trim_end().to_string())
    }
}

#[async_trait]
impl ContextSource for GitContext {
    fn name(&self) -> &'static str {
        "context.git"
    }

    async fn gather(&self, _request: &str) -> ContextContribution {
        let (root, commits, status) = (self.root.clone(), self.max_commits, self.max_status_lines);
        tokio::task::spawn_blocking(move || Self::collect(root, commits, status))
            .await
            .unwrap_or_else(|e| ContextContribution::Unavailable(format!("git task failed: {e}")))
    }
}

// ── Memory ────────────────────────────────────────────────────────────────────

/// Memories (text payloads) from `runtime-memory`, most important first.
pub struct MemoryContext {
    runtime: Arc<MemoryRuntime>,
    limit: usize,
}

impl MemoryContext {
    pub fn new(runtime: Arc<MemoryRuntime>) -> Self {
        Self { runtime, limit: 5 }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

#[async_trait]
impl ContextSource for MemoryContext {
    fn name(&self) -> &'static str {
        "context.memory"
    }

    async fn gather(&self, request: &str) -> ContextContribution {
        let mut records = match self.runtime.retrieve::<String>(request).await {
            Ok((records, _)) => records,
            Err(e) => return ContextContribution::Unavailable(e),
        };
        if records.is_empty() {
            return ContextContribution::Empty;
        }
        records.sort_by(|a, b| b.metadata.importance.total_cmp(&a.metadata.importance));
        let mut text = String::from("## Relevant memories\n");
        for record in records.iter().take(self.limit) {
            let body = record.content.summaries.first().unwrap_or(&record.content.payload);
            let body: String = body.chars().take(400).collect();
            text.push_str(&format!("- {}\n", body.replace('\n', " ")));
        }
        ContextContribution::Section(text.trim_end().to_string())
    }
}

// ── Knowledge graph ───────────────────────────────────────────────────────────

/// Placeholder until `runtime-knowledge` can answer queries: always reports
/// that the knowledge graph is not available.
pub struct KnowledgeGraphContext;

#[async_trait]
impl ContextSource for KnowledgeGraphContext {
    fn name(&self) -> &'static str {
        "context.knowledge_graph"
    }

    async fn gather(&self, _request: &str) -> ContextContribution {
        ContextContribution::Unavailable("runtime-knowledge has no query API yet".into())
    }
}
