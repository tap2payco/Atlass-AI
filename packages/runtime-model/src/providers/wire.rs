//! Provider-neutral flattening of an Atlas conversation into chat-completion
//! turns (ADR-017).
//!
//! Chat-completion APIs (Ollama `/api/chat`, OpenAI-compatible endpoints) put
//! each tool result in its own `tool` message, while Atlas groups results in a
//! single user turn. This module does that split once, so each provider only
//! maps `ChatTurn`s to its own JSON shape.

use std::collections::HashMap;

use crate::domain::{ContentBlock, ModelRequest, Role, ToolCall};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ChatTurn {
    System(String),
    User(String),
    Assistant { text: String, tool_calls: Vec<ToolCall> },
    Tool { call_id: String, tool_name: String, content: String },
}

pub(crate) fn to_chat_turns(request: &ModelRequest) -> Vec<ChatTurn> {
    let mut turns = Vec::new();
    if let Some(sys) = &request.system_prompt {
        turns.push(ChatTurn::System(sys.clone()));
    }

    // Some APIs (Ollama) identify a tool result by tool name rather than call id.
    let names: HashMap<&str, &str> = request
        .messages
        .iter()
        .flat_map(|m| m.tool_calls())
        .map(|c| (c.id.as_str(), c.name.as_str()))
        .collect();

    for message in &request.messages {
        let mut texts: Vec<String> = Vec::new();
        let mut calls = Vec::new();
        let mut results = Vec::new();
        for block in &message.content {
            match block {
                ContentBlock::Text { text } => texts.push(text.clone()),
                ContentBlock::ToolCall(call) => calls.push(call.clone()),
                ContentBlock::ToolResult(result) => results.push(result),
                // Normally replaced by the agent's view building; sent as
                // plain text if a caller passes one through.
                ContentBlock::Summary { text, .. } => {
                    texts.push(format!("Summary of the earlier conversation:\n{text}"))
                }
            }
        }
        let text = texts.join("\n");

        match message.role {
            Role::Assistant => {
                if !text.is_empty() || !calls.is_empty() {
                    turns.push(ChatTurn::Assistant { text, tool_calls: calls });
                }
            }
            Role::User => {
                // Tool results answer the preceding assistant turn, so they go first.
                for result in results {
                    let content = if result.is_error {
                        format!("Error: {}", result.content)
                    } else {
                        result.content.clone()
                    };
                    turns.push(ChatTurn::Tool {
                        call_id: result.call_id.clone(),
                        tool_name: names
                            .get(result.call_id.as_str())
                            .map(|n| n.to_string())
                            .unwrap_or_default(),
                        content,
                    });
                }
                if !text.is_empty() {
                    turns.push(ChatTurn::User(text));
                }
            }
        }
    }
    turns
}

/// An id for a tool call whose provider did not supply one.
pub(crate) fn generated_call_id() -> String {
    format!("call_{}", uuid::Uuid::new_v4().simple())
}

/// Parse tool-call arguments that may arrive as a JSON object or as a string
/// containing JSON. Unparseable strings are kept as a JSON string so nothing is
/// lost; the tool layer reports the schema mismatch to the model.
pub(crate) fn parse_arguments(raw: serde_json::Value) -> serde_json::Value {
    match raw {
        serde_json::Value::String(s) if s.trim().is_empty() => serde_json::json!({}),
        serde_json::Value::String(s) => {
            serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s))
        }
        serde_json::Value::Null => serde_json::json!({}),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Message, ModelCapability, ToolResult};
    use serde_json::json;

    fn call(id: &str, name: &str) -> ToolCall {
        ToolCall { id: id.into(), name: name.into(), arguments: json!({ "path": "a.rs" }) }
    }

    #[test]
    fn flattens_a_tool_loop_in_order() {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![
                ContentBlock::Text { text: "Looking.".into() },
                ContentBlock::ToolCall(call("c1", "read_file")),
            ],
        };
        let results = Message::tool_results([ToolResult {
            call_id: "c1".into(),
            content: "fn main() {}".into(),
            is_error: false,
        }]);
        let req = ModelRequest::conversation(
            ModelCapability::ToolCalling,
            vec![Message::user("Open a.rs"), assistant, results],
        )
        .with_system_prompt("sys");

        assert_eq!(
            to_chat_turns(&req),
            vec![
                ChatTurn::System("sys".into()),
                ChatTurn::User("Open a.rs".into()),
                ChatTurn::Assistant { text: "Looking.".into(), tool_calls: vec![call("c1", "read_file")] },
                ChatTurn::Tool {
                    call_id: "c1".into(),
                    tool_name: "read_file".into(),
                    content: "fn main() {}".into(),
                },
            ]
        );
    }

    #[test]
    fn error_results_are_marked() {
        let req = ModelRequest::conversation(
            ModelCapability::ToolCalling,
            vec![Message::tool_results([ToolResult {
                call_id: "missing".into(),
                content: "denied".into(),
                is_error: true,
            }])],
        );
        assert_eq!(
            to_chat_turns(&req),
            vec![ChatTurn::Tool {
                call_id: "missing".into(),
                tool_name: String::new(),
                content: "Error: denied".into(),
            }]
        );
    }

    #[test]
    fn arguments_accept_objects_and_json_strings() {
        assert_eq!(parse_arguments(json!({ "a": 1 })), json!({ "a": 1 }));
        assert_eq!(parse_arguments(json!("{\"a\":1}")), json!({ "a": 1 }));
        assert_eq!(parse_arguments(json!("")), json!({}));
        assert_eq!(parse_arguments(json!(null)), json!({}));
        assert_eq!(parse_arguments(json!("not json")), json!("not json"));
    }
}
