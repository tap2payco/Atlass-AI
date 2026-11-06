//! Questions about the conversation that operations ask before acting.
//!
//! All agent state is derived from the persisted messages (as in Goose's
//! state machine), never held in memory, so a turn interrupted by a crash or
//! restart resumes correctly from storage.

use std::collections::HashSet;

use shared::model::{ContentBlock, Message, Role, ToolCall};

/// Whether a message is a user turn carrying text (a new request), as opposed
/// to one that only carries tool results.
pub fn is_kickoff(message: &Message) -> bool {
    message.role == Role::User
        && message.content.iter().any(|b| matches!(b, ContentBlock::Text { .. }))
}

/// The messages of the current turn: from the latest user kickoff to the end.
/// Empty if the conversation has no kickoff.
pub fn current_turn(messages: &[Message]) -> &[Message] {
    match messages.iter().rposition(is_kickoff) {
        Some(start) => &messages[start..],
        None => &[],
    }
}

/// Tool calls in the current turn that have no result yet, in call order.
pub fn open_tool_calls(messages: &[Message]) -> Vec<&ToolCall> {
    let turn = current_turn(messages);
    let answered: HashSet<&str> = turn
        .iter()
        .flat_map(|m| m.tool_result_blocks())
        .map(|r| r.call_id.as_str())
        .collect();
    turn.iter()
        .filter(|m| m.role == Role::Assistant)
        .flat_map(|m| m.tool_calls())
        .filter(|c| !answered.contains(c.id.as_str()))
        .collect()
}

/// Number of assistant messages (model responses) in the current turn.
pub fn assistant_messages_in_turn(messages: &[Message]) -> usize {
    current_turn(messages).iter().filter(|m| m.role == Role::Assistant).count()
}

/// Whether the last message is a user message awaiting a model response.
pub fn awaiting_model(messages: &[Message]) -> bool {
    messages.last().is_some_and(|m| m.role == Role::User)
}

/// The latest compaction summary: its text and how many leading messages it covers.
pub fn latest_summary(messages: &[Message]) -> Option<(&str, usize)> {
    messages.iter().rev().find_map(Message::as_summary)
}

/// The conversation as the model should see it. With no summary, all
/// messages. Otherwise the latest summary (as a user message), then every
/// message after the part it covers, leaving out summary messages.
pub fn model_view(messages: &[Message]) -> Vec<Message> {
    let Some((text, covers)) = latest_summary(messages) else {
        return messages.to_vec();
    };
    let mut view = vec![Message::user(format!("Summary of the earlier conversation:\n{text}"))];
    view.extend(
        messages[covers.min(messages.len())..]
            .iter()
            .filter(|m| m.as_summary().is_none())
            .cloned(),
    );
    view
}

/// A rough token count (about 4 characters per token), enough to decide
/// when to compact. Counts text, tool arguments, tool results and summaries.
pub fn estimate_tokens(messages: &[Message]) -> usize {
    let chars: usize = messages
        .iter()
        .flat_map(|m| &m.content)
        .map(|block| match block {
            ContentBlock::Text { text } => text.len(),
            ContentBlock::ToolCall(call) => call.name.len() + call.arguments.to_string().len(),
            ContentBlock::ToolResult(result) => result.content.len(),
            ContentBlock::Summary { text, .. } => text.len(),
        })
        .sum();
    chars / 4 + messages.len() * 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shared::model::ToolResult;

    fn call(id: &str) -> ToolCall {
        ToolCall { id: id.into(), name: "t".into(), arguments: json!({}) }
    }

    fn assistant_calls(ids: &[&str]) -> Message {
        Message {
            role: Role::Assistant,
            content: ids.iter().map(|id| ContentBlock::ToolCall(call(id))).collect(),
        }
    }

    fn result(id: &str) -> ToolResult {
        ToolResult { call_id: id.into(), content: "ok".into(), is_error: false }
    }

    #[test]
    fn open_calls_exclude_answered_ones_and_earlier_turns() {
        let messages = vec![
            Message::user("first"),
            assistant_calls(&["old"]), // never answered, but belongs to an earlier turn
            Message::user("second"),
            assistant_calls(&["a", "b"]),
            Message::tool_results([result("a")]),
        ];
        let open: Vec<_> = open_tool_calls(&messages).into_iter().map(|c| c.id.as_str()).collect();
        assert_eq!(open, vec!["b"]);
    }

    #[test]
    fn tool_result_messages_are_not_kickoffs() {
        let messages = vec![
            Message::user("go"),
            assistant_calls(&["a"]),
            Message::tool_results([result("a")]),
            Message::assistant("done"),
        ];
        assert_eq!(current_turn(&messages).len(), 4);
        assert_eq!(assistant_messages_in_turn(&messages), 2);
        assert!(!awaiting_model(&messages));
    }
}
