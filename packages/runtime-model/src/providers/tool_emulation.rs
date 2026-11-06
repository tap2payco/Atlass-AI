//! Tool-call emulation for models that cannot take tools natively
//! (ADR-017, following Goose's "toolshim" approach).
//!
//! When a request offers tools but the model has no native tool support, the
//! tools and a strict JSON call format are written into the system prompt, the
//! earlier tool calls and results are replayed as plain text, and the model's
//! reply is parsed back into `ToolCall`s:
//!
//! ~~~text
//! ```json
//! {"name": "read_file", "arguments": {"path": "src/main.rs"}}
//! ```
//! ~~~
//!
//! A fenced block (or a bare JSON object or array of `{"name", "arguments"}`)
//! becomes `ModelResponse.tool_calls`; any prose around it stays as `content`.
//! Everything here is pure, so each provider only decides *when* to emulate.

use std::collections::HashSet;

use serde_json::Value;

use crate::domain::{ModelRequest, ToolCall, ToolSpec};
use crate::providers::wire::{self, ChatTurn};

/// Write the tool list and the call format the model must use.
pub(crate) fn render_tool_prompt(tools: &[ToolSpec]) -> String {
    let mut out = String::from(
        "# Tools\n\n\
         You can call the tools listed below. To call a tool, reply with a fenced JSON block \
         in exactly this format, and write nothing after the block:\n\n\
         ```json\n\
         {\"name\": \"<tool name>\", \"arguments\": {<arguments matching the tool's schema>}}\n\
         ```\n\n\
         To call several tools at once, put a JSON array of these objects in one block. \
         You may write a short sentence before the block. Each tool result comes back in \
         the next user message, starting with \"Tool result\" or \"Tool error\". \
         Use only the tool names listed here. If you do not need a tool, answer normally \
         and do not write a JSON block.\n\n\
         ## Available tools\n",
    );
    for tool in tools {
        let schema = serde_json::to_string(&tool.input_schema).unwrap_or_else(|_| "{}".into());
        out.push_str(&format!(
            "\n### {}\n{}\nArguments (JSON Schema): {}\n",
            tool.name,
            tool.description.trim(),
            schema
        ));
    }
    out
}

/// Render a request for a model without native tools: the tool prompt joins the
/// system prompt, earlier calls become fenced JSON in the assistant text, and
/// results become user text. Adjacent user turns are merged so the
/// conversation keeps alternating.
pub(crate) fn to_emulated_turns(request: &ModelRequest) -> Vec<ChatTurn> {
    let tool_prompt = render_tool_prompt(&request.tools);
    let mut turns: Vec<ChatTurn> = Vec::new();
    let mut has_system = false;

    for turn in wire::to_chat_turns(request) {
        let mapped = match turn {
            ChatTurn::System(sys) => {
                has_system = true;
                ChatTurn::System(format!("{sys}\n\n{tool_prompt}"))
            }
            ChatTurn::Assistant { text, tool_calls } => {
                ChatTurn::Assistant { text: render_assistant_calls(&text, &tool_calls), tool_calls: Vec::new() }
            }
            ChatTurn::Tool { call_id, tool_name, content } => {
                ChatTurn::User(render_tool_result(&call_id, &tool_name, &content))
            }
            user @ ChatTurn::User(_) => user,
        };
        match (turns.last_mut(), mapped) {
            (Some(ChatTurn::User(prev)), ChatTurn::User(next)) => {
                prev.push_str("\n\n");
                prev.push_str(&next);
            }
            (_, mapped) => turns.push(mapped),
        }
    }
    if !has_system {
        turns.insert(0, ChatTurn::System(tool_prompt));
    }
    turns
}

fn render_assistant_calls(text: &str, calls: &[ToolCall]) -> String {
    if calls.is_empty() {
        return text.to_string();
    }
    // Written by hand so `name` comes first whatever map order serde_json uses.
    let objects: Vec<String> = calls
        .iter()
        .map(|c| {
            format!(
                "{{\"name\":{},\"arguments\":{}}}",
                Value::String(c.name.clone()),
                c.arguments
            )
        })
        .collect();
    let block = if objects.len() == 1 {
        objects[0].clone()
    } else {
        format!("[{}]", objects.join(","))
    };
    let fenced = format!("```json\n{block}\n```");
    if text.trim().is_empty() {
        fenced
    } else {
        format!("{}\n\n{fenced}", text.trim_end())
    }
}

fn render_tool_result(call_id: &str, tool_name: &str, content: &str) -> String {
    let name = if tool_name.is_empty() { "unknown tool" } else { tool_name };
    match content.strip_prefix("Error: ") {
        Some(err) => format!("Tool error from `{name}` (call {call_id}):\n{err}"),
        None => format!("Tool result from `{name}` (call {call_id}):\n{content}"),
    }
}

/// Split a model reply into prose and emulated tool calls.
///
/// Fenced code blocks are tried first, then bare JSON anywhere in the reply.
/// A fenced block is taken as a call whatever tool it names (the agent
/// reports an unknown tool back to the model); bare JSON must name one of
/// `known_tools`, so an ordinary JSON answer is not mistaken for a call.
/// Text inside a leading `<think>…</think>` section is never parsed.
pub(crate) fn parse_reply(text: &str, known_tools: &HashSet<&str>) -> (String, Vec<ToolCall>) {
    let scan_from = match (text.find("<think>"), text.find("</think>")) {
        (Some(open), Some(close)) if open < close => close + "</think>".len(),
        _ => 0,
    };

    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut calls: Vec<ToolCall> = Vec::new();

    for (start, end, body) in fenced_blocks(text, scan_from) {
        if let Ok(value) = serde_json::from_str::<Value>(body.trim()) {
            if let Some(found) = calls_from_value(&value, None) {
                spans.push((start, end));
                calls.extend(found);
            }
        }
    }

    if calls.is_empty() {
        let mut i = scan_from;
        while i < text.len() {
            let c = text.as_bytes()[i];
            if c == b'{' || c == b'[' {
                let mut stream = serde_json::Deserializer::from_str(&text[i..]).into_iter::<Value>();
                if let Some(Ok(value)) = stream.next() {
                    let end = i + stream.byte_offset();
                    if let Some(found) = calls_from_value(&value, Some(known_tools)) {
                        spans.push((i, end));
                        calls.extend(found);
                    }
                    i = end;
                    continue;
                }
            }
            i += 1;
        }
    }

    if calls.is_empty() {
        return (text.to_string(), calls);
    }

    let mut prose = String::new();
    let mut cursor = 0;
    for (start, end) in spans {
        prose.push_str(&text[cursor..start]);
        cursor = end;
    }
    prose.push_str(&text[cursor..]);
    (tidy(&prose), calls)
}

/// `(start, end, body)` of each ``` fenced block at or after `from`.
fn fenced_blocks(text: &str, from: usize) -> Vec<(usize, usize, &str)> {
    let mut out = Vec::new();
    let mut pos = from;
    while let Some(rel) = text[pos..].find("```") {
        let start = pos + rel;
        let after_ticks = start + 3;
        // The info string (`json`, `tool_call`, …) runs to the end of the line.
        let Some(nl) = text[after_ticks..].find('\n') else { break };
        let body_start = after_ticks + nl + 1;
        let Some(close_rel) = text[body_start..].find("```") else { break };
        let body_end = body_start + close_rel;
        let end = body_end + 3;
        out.push((start, end, &text[body_start..body_end]));
        pos = end;
    }
    out
}

/// A call object, an array of call objects, or `{"tool_calls": [...]}`.
/// With `known`, every call must name a known tool.
fn calls_from_value(value: &Value, known: Option<&HashSet<&str>>) -> Option<Vec<ToolCall>> {
    match value {
        Value::Array(items) if !items.is_empty() => {
            items.iter().map(|v| call_from_object(v, known)).collect()
        }
        Value::Object(map) => match map.get("tool_calls") {
            Some(inner @ Value::Array(_)) => calls_from_value(inner, known),
            _ => call_from_object(value, known).map(|c| vec![c]),
        },
        _ => None,
    }
}

fn call_from_object(value: &Value, known: Option<&HashSet<&str>>) -> Option<ToolCall> {
    let map = value.as_object()?;
    // `{"function": {"name", "arguments"}}`, as some models copy the OpenAI shape.
    if let Some(inner @ Value::Object(_)) = map.get("function") {
        if !map.contains_key("name") {
            return call_from_object(inner, known);
        }
    }
    let name = map.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    if let Some(known) = known {
        if !known.contains(name) {
            return None;
        }
    }
    let arguments = ["arguments", "parameters", "input", "args"]
        .iter()
        .find_map(|k| map.get(*k).cloned())
        .unwrap_or(Value::Null);
    Some(ToolCall {
        id: wire::generated_call_id(),
        name: name.to_string(),
        arguments: wire::parse_arguments(arguments),
    })
}

fn tidy(prose: &str) -> String {
    let lines: Vec<&str> = prose.lines().map(str::trim_end).collect();
    let mut out: Vec<&str> = Vec::new();
    for line in lines {
        if line.is_empty() && out.last().map_or(true, |l| l.is_empty()) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// Ollama's reply to tools offered to a model without tool support, e.g.
/// `registry.ollama.ai/library/gemma2:2b does not support tools`.
pub(crate) fn is_tools_unsupported_error(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("does not support tools") || lower.contains("does not support tool")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ContentBlock, Message, ModelCapability, Role, ToolResult};
    use serde_json::json;

    fn known() -> HashSet<&'static str> {
        ["read_file", "list_dir"].into_iter().collect()
    }

    #[test]
    fn parses_a_fenced_call_and_keeps_the_prose() {
        let reply = "I'll read it.\n\n```json\n{\"name\": \"read_file\", \"arguments\": {\"path\": \"a.rs\"}}\n```\n";
        let (prose, calls) = parse_reply(reply, &known());
        assert_eq!(prose, "I'll read it.");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments, json!({ "path": "a.rs" }));
        assert!(calls[0].id.starts_with("call_"));
    }

    #[test]
    fn parses_bare_json_object() {
        let (prose, calls) =
            parse_reply("{\"name\": \"list_dir\", \"arguments\": {\"dir\": \".\"}}", &known());
        assert_eq!(prose, "");
        assert_eq!(calls[0].name, "list_dir");
        assert_eq!(calls[0].arguments, json!({ "dir": "." }));
    }

    #[test]
    fn parses_several_calls_from_an_array_and_from_several_blocks() {
        let array = "```json\n[{\"name\":\"read_file\",\"arguments\":{\"path\":\"a\"}},\
                     {\"name\":\"read_file\",\"arguments\":{\"path\":\"b\"}}]\n```";
        let (_, calls) = parse_reply(array, &known());
        assert_eq!(calls.len(), 2);
        assert_ne!(calls[0].id, calls[1].id);
        assert_eq!(calls[1].arguments, json!({ "path": "b" }));

        let blocks = "First:\n```json\n{\"name\":\"list_dir\",\"arguments\":{}}\n```\n\
                      Then:\n```\n{\"name\":\"read_file\",\"arguments\":{\"path\":\"x\"}}\n```";
        let (prose, calls) = parse_reply(blocks, &known());
        assert_eq!(calls.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["list_dir", "read_file"]);
        assert_eq!(prose, "First:\n\nThen:");

        let bare = "{\"name\":\"list_dir\",\"arguments\":{}}\n{\"name\":\"read_file\",\"arguments\":{}}";
        assert_eq!(parse_reply(bare, &known()).1.len(), 2);
    }

    #[test]
    fn invalid_json_is_left_as_text() {
        let reply = "```json\n{\"name\": \"read_file\", \"arguments\": {\"path\": }\n```";
        let (prose, calls) = parse_reply(reply, &known());
        assert!(calls.is_empty());
        assert_eq!(prose, reply);
    }

    #[test]
    fn plain_text_and_unrelated_json_are_not_calls() {
        let (prose, calls) = parse_reply("The answer is 42.", &known());
        assert!(calls.is_empty());
        assert_eq!(prose, "The answer is 42.");

        // Bare JSON naming no known tool is an answer, not a call.
        let data = "Here is the user: {\"name\": \"Ada\", \"age\": 36}";
        assert!(parse_reply(data, &known()).1.is_empty());
    }

    #[test]
    fn arguments_given_as_a_string_are_decoded() {
        let reply = "```json\n{\"name\": \"read_file\", \"arguments\": \"{\\\"path\\\": \\\"a.rs\\\"}\"}\n```";
        let (_, calls) = parse_reply(reply, &known());
        assert_eq!(calls[0].arguments, json!({ "path": "a.rs" }));

        let (_, calls) = parse_reply("{\"name\": \"list_dir\"}", &known());
        assert_eq!(calls[0].arguments, json!({}));
    }

    #[test]
    fn accepts_common_variants() {
        let wrapped = "{\"tool_calls\": [{\"function\": {\"name\": \"list_dir\", \"arguments\": {}}}]}";
        assert_eq!(parse_reply(wrapped, &known()).1[0].name, "list_dir");
        let params = "{\"name\": \"read_file\", \"parameters\": {\"path\": \"p\"}}";
        assert_eq!(parse_reply(params, &known()).1[0].arguments, json!({ "path": "p" }));
    }

    #[test]
    fn json_inside_think_is_ignored() {
        let reply = "<think>maybe {\"name\":\"read_file\",\"arguments\":{}}</think>\nNo tool needed.";
        let (prose, calls) = parse_reply(reply, &known());
        assert!(calls.is_empty());
        assert_eq!(prose, reply);
    }

    #[test]
    fn tool_prompt_lists_tools_and_format() {
        let prompt = render_tool_prompt(&[ToolSpec {
            name: "read_file".into(),
            description: "Read a file.".into(),
            input_schema: json!({ "type": "object", "properties": { "path": { "type": "string" } } }),
        }]);
        assert!(prompt.contains("### read_file\nRead a file.\n"));
        assert!(prompt.contains("\"properties\":{\"path\":{\"type\":\"string\"}}"));
        assert!(prompt.contains("```json\n{\"name\": \"<tool name>\", \"arguments\""));
        assert!(prompt.contains("JSON array"));
    }

    #[test]
    fn emulated_turns_replay_calls_and_results_as_text() {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![
                ContentBlock::Text { text: "Looking.".into() },
                ContentBlock::ToolCall(ToolCall {
                    id: "c1".into(),
                    name: "read_file".into(),
                    arguments: json!({ "path": "a.rs" }),
                }),
            ],
        };
        let mut results = Message::tool_results([ToolResult {
            call_id: "c1".into(),
            content: "denied".into(),
            is_error: true,
        }]);
        results.content.push(ContentBlock::Text { text: "Try again.".into() });
        let req = ModelRequest::conversation(
            ModelCapability::ToolCalling,
            vec![Message::user("Open a.rs"), assistant, results],
        )
        .with_system_prompt("sys")
        .with_tools(vec![ToolSpec {
            name: "read_file".into(),
            description: "Read a file".into(),
            input_schema: json!({ "type": "object" }),
        }]);

        let turns = to_emulated_turns(&req);
        assert_eq!(turns.len(), 4);
        let ChatTurn::System(sys) = &turns[0] else { panic!("{turns:?}") };
        assert!(sys.starts_with("sys\n\n# Tools"));
        assert_eq!(turns[1], ChatTurn::User("Open a.rs".into()));
        assert_eq!(
            turns[2],
            ChatTurn::Assistant {
                text: "Looking.\n\n```json\n{\"name\":\"read_file\",\"arguments\":{\"path\":\"a.rs\"}}\n```"
                    .into(),
                tool_calls: vec![],
            }
        );
        assert_eq!(
            turns[3],
            ChatTurn::User("Tool error from `read_file` (call c1):\ndenied\n\nTry again.".into())
        );

        // What we replay must parse back into the same call.
        let ChatTurn::Assistant { text, .. } = &turns[2] else { unreachable!() };
        let (_, calls) = parse_reply(text, &known());
        assert_eq!(calls[0].arguments, json!({ "path": "a.rs" }));
    }

    #[test]
    fn emulated_turns_add_a_system_prompt_when_missing() {
        let req = ModelRequest::conversation(ModelCapability::ToolCalling, vec![Message::user("hi")])
            .with_tools(vec![ToolSpec {
                name: "list_dir".into(),
                description: "List".into(),
                input_schema: json!({}),
            }]);
        let turns = to_emulated_turns(&req);
        assert!(matches!(&turns[0], ChatTurn::System(s) if s.starts_with("# Tools")));
        assert_eq!(turns[1], ChatTurn::User("hi".into()));
    }

    #[test]
    fn recognises_ollama_unsupported_tools_error() {
        assert!(is_tools_unsupported_error(
            "HTTP 400 Bad Request: {\"error\":\"registry.ollama.ai/library/gemma2:2b does not support tools\"}"
        ));
        assert!(!is_tools_unsupported_error("HTTP 404: model \"x\" not found"));
    }
}
