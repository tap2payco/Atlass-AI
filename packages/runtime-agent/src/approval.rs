//! Tool approval (roadmap step 4, after Goose's `ToolApproval` and Codex's
//! `AskForApproval`).
//!
//! Before a tool call that can change something (`ToolEffect` other than
//! `ReadOnly`) runs, the tool runtime asks the agent's `ToolApprover`. A
//! denied call is not run; the model receives an error result saying so and
//! can choose another approach. Read-only calls are never asked about.

use async_trait::async_trait;
use shared::model::ToolCall;

use crate::tool::ToolEffect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    /// `reason` is shown to the model, e.g. what the user wants instead.
    Deny { reason: String },
}

#[async_trait]
pub trait ToolApprover: Send + Sync {
    /// Decide whether `call` may run. May wait for a person to answer.
    async fn approve(&self, call: &ToolCall, effect: ToolEffect) -> ApprovalDecision;
}

/// Allows everything. The default, matching behaviour before approvals
/// existed; interactive front ends should install their own approver.
pub struct AutoApprove;

#[async_trait]
impl ToolApprover for AutoApprove {
    async fn approve(&self, _call: &ToolCall, _effect: ToolEffect) -> ApprovalDecision {
        ApprovalDecision::Allow
    }
}

/// The text the model sees when a call is denied.
pub fn denied_message(reason: &str) -> String {
    if reason.trim().is_empty() {
        "The user declined to run this tool call.".to_string()
    } else {
        format!("The user declined to run this tool call: {}", reason.trim())
    }
}
