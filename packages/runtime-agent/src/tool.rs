//! Tools the agent can call.
//!
//! A tool declares its `ToolEffect`. The tool runtime uses it to decide what
//! may run in parallel (only read-only tools), and the approval layer
//! (roadmap step 4) will use it to decide what needs confirmation.

use std::time::Duration;

use async_trait::async_trait;
use shared::model::ToolSpec;

/// What a tool can change. Ordered from least to most powerful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolEffect {
    /// Only reads. Safe to run in parallel with other read-only tools.
    ReadOnly,
    /// Creates or modifies files inside the workspace.
    WritesWorkspace,
    /// Starts processes, which may do anything the user can.
    RunsProcesses,
}

#[async_trait]
pub trait Tool: Send + Sync {
    /// Name, description and argument schema shown to the model. The name must
    /// be unique among the agent's tools.
    fn spec(&self) -> ToolSpec;

    /// What this tool can change. Defaults to the most powerful effect, so a
    /// tool that forgets to declare one is never run in parallel or treated
    /// as harmless.
    fn effect(&self) -> ToolEffect {
        ToolEffect::RunsProcesses
    }

    /// Run the tool. `Ok` text and `Err` text are both returned to the model;
    /// `Err` is marked as an error result so the model can recover.
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String>;
}

/// Limits applied to every tool call by the tool runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolLimits {
    /// A call running longer than this is abandoned and reported as an error.
    pub timeout: Duration,
    /// Longer output is cut down to its start and end, with a note saying how
    /// much was removed, so one call cannot fill the model's context.
    pub max_output_chars: usize,
}

impl Default for ToolLimits {
    fn default() -> Self {
        Self { timeout: Duration::from_secs(120), max_output_chars: 30_000 }
    }
}

/// Keep the first two thirds and the last third of `text` (by characters)
/// when it is longer than `max_chars`. The end of command output is often
/// where the error is, so it is kept too.
pub fn truncate_output(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    let head_len = max_chars * 2 / 3;
    let tail_len = max_chars - head_len;
    let head: String = text.chars().take(head_len).collect();
    let tail: String = text.chars().skip(total - tail_len).collect();
    format!("{head}\n… [{} characters omitted] …\n{tail}", total - head_len - tail_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_output_is_unchanged() {
        assert_eq!(truncate_output("hello", 10), "hello");
    }

    #[test]
    fn long_output_keeps_head_and_tail_on_char_boundaries() {
        let text = "é".repeat(50) + &"x".repeat(50);
        let out = truncate_output(&text, 30);
        assert!(out.starts_with(&"é".repeat(20)));
        assert!(out.ends_with(&"x".repeat(10)));
        assert!(out.contains("[70 characters omitted]"));
    }
}
