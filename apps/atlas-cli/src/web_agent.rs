//! The agent endpoints behind the web chat (`atlas-cli server`).
//!
//! - `POST /api/agent` runs one turn and streams protocol events as NDJSON.
//!   With `"access": "write"` the agent may edit files, and each edit waits
//!   for the browser's answer (an `approval_request` event).
//! - `POST /api/agent/approve` answers an approval request of a running turn.
//! - `POST /api/agent/interrupt` stops a running turn.
//! - `GET /api/sessions?path=` lists the saved conversations of a project.
//! - `GET /api/session?path=&id=` returns one saved conversation.
//!
//! Every agent endpoint refuses requests whose `Origin` is not this server,
//! so another website open in the browser cannot drive the agent.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use runtime_agent::protocol::{encode_line, Event, Op, ReviewDecision};
use runtime_agent::{FileSessionStore, SessionStore};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared::model::{ContentBlock, Role};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedSender;

use crate::agent::serve::ProtocolAgent;
use crate::agent::tools::Workspace;
use crate::agent::{sessions_dir, Access, ApprovalMode, WORKSPACE_KEY};

/// Tool output longer than this is shortened when a saved session is sent.
const MAX_HISTORY_OUTPUT: usize = 4_000;

/// The project the app opens on (`atlas ui --path`), as its display root.
static DEFAULT_PROJECT: OnceLock<String> = OnceLock::new();

pub fn set_default_project(path: &Path) -> Result<(), String> {
    let root = Workspace::new(path)?.display_root();
    let _ = DEFAULT_PROJECT.set(root);
    Ok(())
}

/// Where the app's files live: `apps/atlas-dashboard` under the current
/// directory when run from the repository, otherwise next to this crate.
pub fn dashboard_dir() -> PathBuf {
    let local = PathBuf::from("apps/atlas-dashboard");
    if local.is_dir() {
        return local;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../atlas-dashboard")
}

/// Opens `url` in the default browser; prints it if that fails.
pub fn open_in_browser(url: &str) {
    #[cfg(windows)]
    let result = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    if result.is_err() {
        println!("Open {url} in your browser.");
    }
}

/// `GET /api/info`: what the app needs at start-up.
pub async fn info(socket: &mut TcpStream) {
    let project = DEFAULT_PROJECT.get().cloned().unwrap_or_default();
    let name = Path::new(&project)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| project.clone());
    let config = runtime_model::ModelRuntimeConfig::from_env();
    respond_json(
        socket,
        "200 OK",
        &json!({
            "project": project,
            "project_name": name,
            "model": config.ollama_model_override.unwrap_or_else(|| "auto (by task)".into()),
            "context_tokens": config.ollama_num_ctx,
            "privacy_mode": config.privacy_mode,
            "version": env!("CARGO_PKG_VERSION"),
        }),
    )
    .await;
}

/// Running turns by session id, so approvals and interrupts sent on other
/// connections reach the turn's driver.
fn running() -> &'static Mutex<HashMap<String, UnboundedSender<Op>>> {
    static RUNNING: OnceLock<Mutex<HashMap<String, UnboundedSender<Op>>>> = OnceLock::new();
    RUNNING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Removes a turn from `running()` when the request ends, however it ends.
struct RunningGuard(Option<String>);

impl Drop for RunningGuard {
    fn drop(&mut self) {
        if let Some(session) = self.0.take() {
            running().lock().unwrap().remove(&session);
        }
    }
}

/// Whether a request may use the agent endpoints: same-origin requests from
/// the page this server serves, or requests with no `Origin` at all (curl,
/// scripts). A page on any other origin is refused.
pub fn origin_allowed(request_head: &str, port: u16) -> bool {
    let origin = request_head
        .lines()
        .skip(1)
        .take_while(|l| !l.is_empty())
        .find_map(|l| {
            let (name, value) = l.split_once(':')?;
            name.trim().eq_ignore_ascii_case("origin").then(|| value.trim().to_string())
        });
    match origin {
        None => true,
        Some(origin) => [format!("http://localhost:{port}"), format!("http://127.0.0.1:{port}")]
            .iter()
            .any(|allowed| origin.eq_ignore_ascii_case(allowed)),
    }
}

/// The value of `name` in a URL's query string (`+` and `%XX` decoded).
pub fn query_param(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        (key == name).then(|| percent_decode(value))
    })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

async fn respond_json(socket: &mut TcpStream, status: &str, body: &serde_json::Value) {
    let body = body.to_string();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
}

pub async fn forbidden(socket: &mut TcpStream) {
    respond_json(
        socket,
        "403 Forbidden",
        &json!({ "error": "This endpoint only accepts requests from the Atlas page this server serves." }),
    )
    .await;
}

#[derive(Deserialize)]
struct TurnRequest {
    prompt: String,
    path: Option<String>,
    /// A saved session to continue: `last`, a full id or an id prefix.
    session: Option<String>,
    /// `read` (default) or `write`. Write access asks the browser before
    /// every edit; commands are never available from the web.
    access: Option<String>,
}

/// `POST /api/agent`: run one turn and stream its events as NDJSON, one
/// `runtime_agent::protocol::Event` per line, ending after `turn_complete`
/// (or an `error` when the request is unusable). Closing the connection
/// interrupts the turn.
pub async fn run_turn(socket: &mut TcpStream, body: &str) {
    let header = "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nCache-Control: no-cache\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n";
    if socket.write_all(header.as_bytes()).await.is_err() {
        return;
    }
    let fail = |message: String| encode_line(&Event::Error { message });

    let request: TurnRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            let _ = socket.write_all(fail(format!("Invalid JSON: {e}")).as_bytes()).await;
            return;
        }
    };
    let (access, approval) = match request.access.as_deref().unwrap_or("read") {
        "read" => (Access::Read, ApprovalMode::Auto),
        "write" => (Access::Write, ApprovalMode::Ask),
        other => {
            let _ = socket
                .write_all(fail(format!("unknown access '{other}' (use read or write)")).as_bytes())
                .await;
            return;
        }
    };
    let root = request.path.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let agent = match ProtocolAgent::from_env(&root, runtime_agent::DEFAULT_MAX_TURNS, access, approval).await {
        Ok(a) => a,
        Err(e) => {
            let _ = socket.write_all(fail(e).as_bytes()).await;
            return;
        }
    };
    let resume = match request.session.as_deref().map(|q| agent.find_session(q)).transpose() {
        Ok(id) => id,
        Err(e) => {
            let _ = socket.write_all(fail(e).as_bytes()).await;
            return;
        }
    };
    let mut driver = agent.into_driver();
    if let Some(id) = resume {
        driver = driver.with_session(id);
    }
    let (ops, mut events) = driver.spawn();
    let mut guard = RunningGuard(None);

    let mut sent = false;
    let mut connected = true;
    while let Some(event) = events.recv().await {
        if let Event::SessionConfigured { session, .. } = &event {
            // Register before the turn starts, so no approval request can
            // reach the browser before its answer has somewhere to go.
            // The bare uuid, as the browser sees it in `session_configured`
            // (`SessionId` displays as `session_<uuid>`).
            let key = session.0.to_string();
            if running().lock().unwrap().contains_key(&key) {
                let _ = socket
                    .write_all(fail("This conversation already has a turn running.".into()).as_bytes())
                    .await;
                break;
            }
            running().lock().unwrap().insert(key.clone(), ops.clone());
            guard.0 = Some(key);
            if !sent {
                sent = true;
                let _ = ops.send(Op::TurnInput { session: None, text: request.prompt.clone() });
            }
        }
        let done = matches!(event, Event::TurnComplete { .. });
        if connected && socket.write_all(encode_line(&event).as_bytes()).await.is_err() {
            connected = false;
            let _ = ops.send(Op::Interrupt);
        }
        if done {
            break;
        }
    }
    // Ends the driver; a turn that never started (bad session) ends here too.
    drop(guard);
    drop(ops);
}

#[derive(Deserialize)]
struct ApproveRequest {
    session: String,
    call_id: String,
    /// `allow` or `deny`.
    decision: ReviewDecision,
    reason: Option<String>,
}

/// `POST /api/agent/approve`.
pub async fn approve(socket: &mut TcpStream, body: &str) {
    let request: ApproveRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return respond_json(socket, "400 Bad Request", &json!({ "error": format!("Invalid JSON: {e}") })).await,
    };
    let sender = running().lock().unwrap().get(&request.session).cloned();
    let Some(sender) = sender else {
        return respond_json(socket, "404 Not Found", &json!({ "error": "No turn is running for that conversation." })).await;
    };
    let op = Op::ApprovalResponse { call_id: request.call_id, decision: request.decision, reason: request.reason };
    if sender.send(op).is_err() {
        return respond_json(socket, "410 Gone", &json!({ "error": "That turn has already ended." })).await;
    }
    respond_json(socket, "200 OK", &json!({ "ok": true })).await;
}

#[derive(Deserialize)]
struct InterruptRequest {
    session: String,
}

/// `POST /api/agent/interrupt`.
pub async fn interrupt(socket: &mut TcpStream, body: &str) {
    let request: InterruptRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return respond_json(socket, "400 Bad Request", &json!({ "error": format!("Invalid JSON: {e}") })).await,
    };
    let sender = running().lock().unwrap().get(&request.session).cloned();
    match sender {
        Some(sender) => {
            let _ = sender.send(Op::Interrupt);
            respond_json(socket, "200 OK", &json!({ "ok": true })).await;
        }
        None => respond_json(socket, "404 Not Found", &json!({ "error": "No turn is running for that conversation." })).await,
    }
}

fn workspace_key(path: Option<String>) -> Result<String, String> {
    let root = path.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    Ok(Workspace::new(Path::new(&root))?.display_root())
}

#[derive(Serialize)]
struct SessionRow {
    id: String,
    title: String,
    created_at: u64,
    updated_at: u64,
    message_count: usize,
}

/// `GET /api/sessions?path=`: this project's saved conversations, newest first.
pub async fn list_sessions(socket: &mut TcpStream, url: &str) {
    let workspace = match workspace_key(query_param(url, "path")) {
        Ok(w) => w,
        Err(e) => return respond_json(socket, "400 Bad Request", &json!({ "error": e })).await,
    };
    let store = match FileSessionStore::open(sessions_dir()) {
        Ok(s) => s,
        Err(e) => return respond_json(socket, "500 Internal Server Error", &json!({ "error": e.to_string() })).await,
    };
    let mut rows: Vec<SessionRow> = match store.list() {
        Ok(list) => list
            .into_iter()
            .filter(|s| s.metadata.get(WORKSPACE_KEY) == Some(&workspace) && s.message_count > 0)
            .map(|s| SessionRow {
                id: s.id.to_string(),
                title: s.title,
                created_at: s.created_at,
                updated_at: s.updated_at,
                message_count: s.message_count,
            })
            .collect(),
        Err(e) => return respond_json(socket, "500 Internal Server Error", &json!({ "error": e.to_string() })).await,
    };
    rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    respond_json(socket, "200 OK", &json!({ "workspace": workspace, "sessions": rows })).await;
}

/// `GET /api/session?path=&id=`: one saved conversation, as display items.
pub async fn get_session(socket: &mut TcpStream, url: &str) {
    let workspace = match workspace_key(query_param(url, "path")) {
        Ok(w) => w,
        Err(e) => return respond_json(socket, "400 Bad Request", &json!({ "error": e })).await,
    };
    let Some(query) = query_param(url, "id") else {
        return respond_json(socket, "400 Bad Request", &json!({ "error": "missing id" })).await;
    };
    let store = match FileSessionStore::open(sessions_dir()) {
        Ok(s) => s,
        Err(e) => return respond_json(socket, "500 Internal Server Error", &json!({ "error": e.to_string() })).await,
    };
    let session = match store.find(&query) {
        Ok(id) => store.load(id).await,
        Err(e) => Err(e),
    };
    let session = match session {
        Ok(s) => s,
        Err(e) => return respond_json(socket, "404 Not Found", &json!({ "error": e.to_string() })).await,
    };
    if session.metadata.get(WORKSPACE_KEY) != Some(&workspace) {
        return respond_json(
            socket,
            "403 Forbidden",
            &json!({ "error": "That conversation belongs to a different project folder." }),
        )
        .await;
    }
    respond_json(socket, "200 OK", &json!({ "id": session.id.to_string(), "items": history_items(&session.messages) }))
        .await;
}

// ── Terminals ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OpenTerminal {
    path: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
}

/// `POST /api/terminal`: start a shell in the project folder.
pub async fn terminal_open(socket: &mut TcpStream, body: &str) {
    let request: OpenTerminal = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return respond_json(socket, "400 Bad Request", &json!({ "error": format!("Invalid JSON: {e}") })).await,
    };
    let root = request
        .path
        .or_else(|| DEFAULT_PROJECT.get().cloned())
        .unwrap_or_else(|| ".".into());
    let cwd = match Workspace::new(Path::new(&root)) {
        Ok(w) => PathBuf::from(w.display_root()),
        Err(e) => return respond_json(socket, "400 Bad Request", &json!({ "error": e })).await,
    };
    match crate::terminal::open(&cwd, request.cols.unwrap_or(100), request.rows.unwrap_or(30)) {
        Ok(t) => respond_json(socket, "200 OK", &json!({ "id": t.id, "shell": t.shell, "cwd": cwd })).await,
        Err(e) => respond_json(socket, "500 Internal Server Error", &json!({ "error": e })).await,
    }
}

/// `GET /api/terminal/stream?id=`: the terminal's output so far, then live
/// output, as raw bytes until the shell exits.
pub async fn terminal_stream(socket: &mut TcpStream, url: &str) {
    let Some(terminal) = query_param(url, "id").and_then(|id| crate::terminal::get(&id)) else {
        return respond_json(socket, "404 Not Found", &json!({ "error": "No such terminal." })).await;
    };
    let header = "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nCache-Control: no-cache\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n";
    if socket.write_all(header.as_bytes()).await.is_err() {
        return;
    }
    let (backlog, mut live) = terminal.subscribe();
    if socket.write_all(&backlog).await.is_err() {
        return;
    }
    loop {
        match live.recv().await {
            Ok(chunk) if chunk.is_empty() => break,
            Ok(chunk) => {
                if socket.write_all(&chunk).await.is_err() {
                    break;
                }
            }
            // Fell behind: carry on with live output.
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
        if terminal.has_exited() && live.is_empty() {
            break;
        }
    }
}

#[derive(Deserialize)]
struct TerminalInput {
    id: String,
    data: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
}

/// `POST /api/terminal/input`: keystrokes (`data`) and/or a new size.
pub async fn terminal_input(socket: &mut TcpStream, body: &str) {
    let request: TerminalInput = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return respond_json(socket, "400 Bad Request", &json!({ "error": format!("Invalid JSON: {e}") })).await,
    };
    let Some(terminal) = crate::terminal::get(&request.id) else {
        return respond_json(socket, "404 Not Found", &json!({ "error": "No such terminal." })).await;
    };
    if let (Some(cols), Some(rows)) = (request.cols, request.rows) {
        terminal.resize(cols, rows);
    }
    if let Some(data) = request.data {
        if let Err(e) = terminal.write(data.as_bytes()) {
            return respond_json(socket, "410 Gone", &json!({ "error": format!("The terminal has closed: {e}") })).await;
        }
    }
    respond_json(socket, "200 OK", &json!({ "ok": true })).await;
}

/// `POST /api/terminal/close`.
pub async fn terminal_close(socket: &mut TcpStream, body: &str) {
    let id = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("id").and_then(|i| i.as_str()).map(String::from))
        .unwrap_or_default();
    let closed = crate::terminal::close(&id);
    respond_json(socket, if closed { "200 OK" } else { "404 Not Found" }, &json!({ "ok": closed })).await;
}

/// Flattens stored messages into what the page draws: user and agent text,
/// tool calls with their results, and compaction summaries.
fn history_items(messages: &[shared::model::Message]) -> Vec<serde_json::Value> {
    let mut items = Vec::new();
    for message in messages {
        for block in &message.content {
            match block {
                ContentBlock::Text { text } if !text.trim().is_empty() => items.push(json!({
                    "kind": if message.role == Role::User { "user" } else { "agent" },
                    "text": text,
                })),
                ContentBlock::ToolCall(call) => items.push(json!({
                    "kind": "tool_call",
                    "call_id": call.id,
                    "tool": call.name,
                    "arguments": call.arguments,
                })),
                ContentBlock::ToolResult(result) => {
                    let mut output: String = result.content.chars().take(MAX_HISTORY_OUTPUT).collect();
                    if output.len() < result.content.len() {
                        output.push_str("\n… (shortened)");
                    }
                    items.push(json!({
                        "kind": "tool_result",
                        "call_id": result.call_id,
                        "output": output,
                        "is_error": result.is_error,
                    }))
                }
                ContentBlock::Summary { covers, .. } => items.push(json!({ "kind": "compacted", "covers": covers })),
                _ => {}
            }
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_this_servers_origin_or_no_origin_is_allowed() {
        let head = |origin: Option<&str>| {
            let mut h = String::from("POST /api/agent HTTP/1.1\r\nHost: localhost:8080\r\n");
            if let Some(o) = origin {
                h.push_str(&format!("Origin: {o}\r\n"));
            }
            h.push_str("\r\n");
            h
        };
        assert!(origin_allowed(&head(None), 8080));
        assert!(origin_allowed(&head(Some("http://localhost:8080")), 8080));
        assert!(origin_allowed(&head(Some("http://127.0.0.1:8080")), 8080));
        assert!(!origin_allowed(&head(Some("https://evil.example")), 8080));
        assert!(!origin_allowed(&head(Some("http://localhost:9999")), 8080));
        assert!(!origin_allowed(&head(Some("null")), 8080));
    }

    #[test]
    fn query_params_are_decoded() {
        let url = "/api/sessions?path=D%3A%5CProgramming%5CPersonal+projects&id=abc";
        assert_eq!(query_param(url, "path").as_deref(), Some("D:\\Programming\\Personal projects"));
        assert_eq!(query_param(url, "id").as_deref(), Some("abc"));
        assert_eq!(query_param(url, "missing"), None);
        assert_eq!(query_param("/x?bad=%zz", "bad").as_deref(), Some("%zz"));
    }
}
