//! Asking the user before the agent changes files or runs commands.

use std::collections::HashSet;
use std::fmt;
use std::io::{self, BufRead, IsTerminal, Write};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use platform_contracts::policy::ApprovalPolicy;
use runtime_agent::{ApprovalDecision, AutoApprove, ToolApprover, ToolEffect};
use serde_json::Value;
use shared::model::ToolCall;

use super::policy::needs_approval;

/// Whether the agent must ask before acting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApprovalMode {
    /// Ask before every write and every command (default).
    #[default]
    Ask,
    /// Never ask.
    Auto,
}

impl FromStr for ApprovalMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "ask" => Ok(Self::Ask),
            "auto" => Ok(Self::Auto),
            other => Err(format!("unknown approval mode '{other}' (use ask or auto)")),
        }
    }
}

impl fmt::Display for ApprovalMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ask => "ask",
            Self::Auto => "auto",
        })
    }
}

pub fn approver_for(mode: ApprovalMode, console: &Arc<ConsoleApprover>) -> Arc<dyn ToolApprover> {
    approver_for_policy(mode.approval_policy(), console)
}

/// The approver for an approval policy: asks `console` only for the calls
/// the policy says need approval (see `policy::needs_approval`).
pub fn approver_for_policy(policy: ApprovalPolicy, console: &Arc<ConsoleApprover>) -> Arc<dyn ToolApprover> {
    match policy {
        ApprovalPolicy::Never => Arc::new(AutoApprove),
        policy => Arc::new(PolicyApprover { policy, ask: console.clone() }),
    }
}

/// Lets through what `policy` does not require approval for; asks `ask` otherwise.
pub struct PolicyApprover {
    policy: ApprovalPolicy,
    ask: Arc<dyn ToolApprover>,
}

#[async_trait]
impl ToolApprover for PolicyApprover {
    async fn approve(&self, call: &ToolCall, effect: ToolEffect) -> ApprovalDecision {
        if needs_approval(self.policy, effect) {
            self.ask.approve(call, effect).await
        } else {
            ApprovalDecision::Allow
        }
    }
}

/// What the user answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Yes,
    Always,
    No { reason: String },
}

/// `y`/`yes` → Yes, `a`/`always` → Always, empty, `n`, `no` or end of input
/// → No; any other text → No, with that text as the reason for the model.
pub fn parse_answer(line: Option<&str>) -> Answer {
    let Some(line) = line else {
        return Answer::No { reason: String::new() };
    };
    let trimmed = line.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "y" | "yes" => Answer::Yes,
        "a" | "always" => Answer::Always,
        "" | "n" | "no" => Answer::No { reason: String::new() },
        _ => Answer::No { reason: trimmed.to_string() },
    }
}

/// Asks on the terminal. Remembers "always" answers per tool name for the
/// life of the CLI process.
#[derive(Default)]
pub struct ConsoleApprover {
    always: Mutex<HashSet<String>>,
}

impl ConsoleApprover {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ToolApprover for ConsoleApprover {
    async fn approve(&self, call: &ToolCall, _effect: ToolEffect) -> ApprovalDecision {
        if self.always.lock().unwrap().contains(&call.name) {
            return ApprovalDecision::Allow;
        }
        if !io::stdin().is_terminal() {
            return ApprovalDecision::Deny {
                reason: "there is no terminal to ask for approval; rerun with --approve auto to allow it".into(),
            };
        }

        let prompt = describe_for_approval(call);
        let line = tokio::task::spawn_blocking(move || {
            let mut out = io::stdout().lock();
            let _ = write!(out, "{prompt}");
            let _ = out.flush();
            drop(out);
            let mut line = String::new();
            match io::stdin().lock().read_line(&mut line) {
                Ok(0) | Err(_) => None,
                Ok(_) => Some(line),
            }
        })
        .await
        .ok()
        .flatten();

        match parse_answer(line.as_deref()) {
            Answer::Yes => ApprovalDecision::Allow,
            Answer::Always => {
                self.always.lock().unwrap().insert(call.name.clone());
                ApprovalDecision::Allow
            }
            Answer::No { reason } => ApprovalDecision::Deny { reason },
        }
    }
}

const PREVIEW_LINES: usize = 8;

fn preview(text: &str, marker: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: String = lines.iter().take(PREVIEW_LINES).map(|l| format!("    {marker} {l}\n")).collect();
    if lines.len() > PREVIEW_LINES {
        out.push_str(&format!("    … {} more lines\n", lines.len() - PREVIEW_LINES));
    }
    out
}

fn arg<'a>(call: &'a ToolCall, key: &str) -> &'a str {
    call.arguments.get(key).and_then(Value::as_str).unwrap_or("")
}

/// The prompt shown before a call runs.
pub fn describe_for_approval(call: &ToolCall) -> String {
    let body = match call.name.as_str() {
        "run_command" => format!("  run: {}\n", arg(call, "command")),
        "edit_file" => format!(
            "  edit {}{}\n{}{}",
            arg(call, "path"),
            if call.arguments.get("replace_all").and_then(Value::as_bool) == Some(true) {
                " (all occurrences)"
            } else {
                ""
            },
            preview(arg(call, "old_text"), "-"),
            preview(arg(call, "new_text"), "+"),
        ),
        "write_file" => {
            let content = arg(call, "content");
            format!("  write {} ({} bytes)\n{}", arg(call, "path"), content.len(), preview(content, "+"))
        }
        _ => format!("  {} {}\n", call.name, call.arguments),
    };
    format!("\n  ? The agent wants to:\n{body}  Allow? [y]es / [n]o / [a]lways for {} (or type what to do instead): ", call.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn answers_are_parsed_safely() {
        assert_eq!(parse_answer(Some("y\n")), Answer::Yes);
        assert_eq!(parse_answer(Some(" YES ")), Answer::Yes);
        assert_eq!(parse_answer(Some("a")), Answer::Always);
        assert_eq!(parse_answer(Some("\n")), Answer::No { reason: String::new() });
        assert_eq!(parse_answer(None), Answer::No { reason: String::new() });
        assert_eq!(
            parse_answer(Some("use cargo test instead\n")),
            Answer::No { reason: "use cargo test instead".into() }
        );
    }

    #[test]
    fn prompts_show_what_will_happen() {
        let cmd = ToolCall { id: "1".into(), name: "run_command".into(), arguments: json!({ "command": "cargo test" }) };
        assert!(describe_for_approval(&cmd).contains("run: cargo test"));

        let edit = ToolCall {
            id: "2".into(),
            name: "edit_file".into(),
            arguments: json!({ "path": "calc.py", "old_text": "a - b", "new_text": "a + b" }),
        };
        let text = describe_for_approval(&edit);
        assert!(text.contains("edit calc.py") && text.contains("- a - b") && text.contains("+ a + b"), "{text}");

        let long = "x\n".repeat(20);
        let write = ToolCall {
            id: "3".into(),
            name: "write_file".into(),
            arguments: json!({ "path": "big.txt", "content": long }),
        };
        assert!(describe_for_approval(&write).contains("… 12 more lines"));
    }

    struct Refuse;

    #[async_trait]
    impl ToolApprover for Refuse {
        async fn approve(&self, _: &ToolCall, _: ToolEffect) -> ApprovalDecision {
            ApprovalDecision::Deny { reason: "asked".into() }
        }
    }

    #[tokio::test]
    async fn policy_approver_only_asks_when_the_policy_requires_it() {
        let call = ToolCall { id: "1".into(), name: "edit_file".into(), arguments: json!({}) };
        let on_request = PolicyApprover { policy: ApprovalPolicy::OnRequest, ask: Arc::new(Refuse) };
        assert_eq!(on_request.approve(&call, ToolEffect::WritesWorkspace).await, ApprovalDecision::Allow);
        assert!(matches!(on_request.approve(&call, ToolEffect::RunsProcesses).await, ApprovalDecision::Deny { .. }));
        let untrusted = PolicyApprover { policy: ApprovalPolicy::Untrusted, ask: Arc::new(Refuse) };
        assert!(matches!(untrusted.approve(&call, ToolEffect::WritesWorkspace).await, ApprovalDecision::Deny { .. }));
        assert_eq!(untrusted.approve(&call, ToolEffect::ReadOnly).await, ApprovalDecision::Allow);
    }

    #[test]
    fn modes_parse() {
        assert_eq!("ASK".parse::<ApprovalMode>().unwrap(), ApprovalMode::Ask);
        assert_eq!("auto".parse::<ApprovalMode>().unwrap(), ApprovalMode::Auto);
        assert!("sometimes".parse::<ApprovalMode>().is_err());
    }
}
