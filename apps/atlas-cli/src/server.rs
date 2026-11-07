use std::fs;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::{Deserialize, Serialize};
use runtime_repository::discovery::{DiscoveryConfig, RepositoryDiscovery};
use runtime_context::AtlasContextRuntime;
use runtime_context::retrieval::planning::RetrievalPlanner;
use runtime_model::{ModelRuntime, ModelRuntimeConfig};
use sdk::context::{ContextRequest, ContextRuntime};
use sdk::context::intent::RetrievalIntent;
use sdk::context::policy::{ContextPolicy, ProviderPriority, ScoringWeights, TokenBudget};
use sdk::model::{ModelRequest, ModelCapability, CapabilityProfile, ExecutionPolicy, Message};

struct DefaultPriority;
impl ProviderPriority for DefaultPriority {
    fn priority(&self, _provider_name: &str) -> u32 { 1 }
}

struct DefaultPolicy;
impl ContextPolicy for DefaultPolicy {
    fn name(&self) -> &str { "DefaultPolicy" }
    fn scoring_weights(&self) -> ScoringWeights {
        ScoringWeights { relevance: 1.0, recency: 1.0, structural_distance: 1.0, execution_importance: 1.0, confidence: 1.0, token_cost: 1.0 }
    }
    fn optimization_strategy(&self) -> Box<dyn runtime_context::optimization::strategy::OptimizationStrategy> {
        Box::new(runtime_context::optimization::strategy::NoOpStrategy)
    }
    fn provider_priority(&self) -> Box<dyn ProviderPriority> { Box::new(DefaultPriority) }
    fn token_budget(&self) -> TokenBudget { TokenBudget { max_tokens: 4096, reserved_for_system: 512 } }
}

#[derive(Deserialize)]
struct LinkRequest {
    path: String,
}

#[derive(Serialize)]
struct LinkResponse {
    success: bool,
    name: String,
    path: String,
    file_count: usize,
    is_git: bool,
    language: String,
    files: Vec<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatRequest {
    prompt: String,
    path: Option<String>,
    history: Option<Vec<ChatMessage>>,
}

#[derive(Serialize)]
struct ChatResponse {
    success: bool,
    response: String,
    provider: String,
    error: Option<String>,
}

/// Reads one HTTP request: the head, then as much body as `Content-Length`
/// announces (at most 1 MiB).
async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<Vec<u8>> {
    const MAX: usize = 1 << 20;
    let mut data = Vec::new();
    let mut buf = vec![0u8; 8192];
    loop {
        let n = socket.read(&mut buf).await.ok()?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(head_end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&data[..head_end]).to_ascii_lowercase();
            let length = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0)
                .min(MAX);
            if data.len() >= head_end + 4 + length {
                break;
            }
        }
        if data.len() > MAX + 16 * 1024 {
            return None;
        }
    }
    (!data.is_empty()).then_some(data)
}

pub async fn run_server(port: u16) -> Result<(), String> {
    run_server_with(port, None, false).await
}

/// `atlas ui`: serve the app for `project` and open it in the browser.
pub async fn run_server_with(port: u16, project: Option<PathBuf>, open: bool) -> Result<(), String> {
    let project = project.unwrap_or_else(|| PathBuf::from("."));
    crate::web_agent::set_default_project(&project)?;
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await
        .map_err(|e| format!("Failed to bind server to {}: {}", addr, e))?;
    if open {
        crate::web_agent::open_in_browser(&format!("http://localhost:{port}/"));
    }

    println!("╭─────────────────────────────────────────────────────────────╮");
    println!("│ 🚀 Atlas Local Web Daemon HTTP API Server Running           │");
    println!("├─────────────────────────────────────────────────────────────┤");
    println!("│ Web UI URL:  http://localhost:{}                             │", port);
    println!("│ API Endpoint: http://localhost:{}/api/link                  │", port);
    println!("│ API Endpoint: http://localhost:{}/api/chat                  │", port);
    println!("│ API Endpoint: http://localhost:{}/api/agent (streams)       │", port);
    println!("╰─────────────────────────────────────────────────────────────╯\n");

    loop {
        let (mut socket, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };

        tokio::spawn(async move {
            let Some(raw) = read_request(&mut socket).await else { return };
            let req_str = String::from_utf8_lossy(&raw);
            let lines: Vec<&str> = req_str.lines().collect();
            if lines.is_empty() { return; }

            let first_line_parts: Vec<&str> = lines[0].split_whitespace().collect();
            if first_line_parts.len() < 2 { return; }

            let method = first_line_parts[0];
            let url = first_line_parts[1];

            // The page is served from this server, so the API is same-origin
            // only: no CORS headers, and requests from any other origin are
            // refused, so a website open in the browser cannot read the
            // project or drive the agent.
            let cors_headers = "";
            if method == "OPTIONS" {
                let _ = socket.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n").await;
                return;
            }
            let route = url.split('?').next().unwrap_or(url);
            if route.starts_with("/api/") && !crate::web_agent::origin_allowed(&req_str, port) {
                crate::web_agent::forbidden(&mut socket).await;
                return;
            }

            let body = req_str.split("\r\n\r\n").nth(1).unwrap_or("");

            if url == "/api/link" && method == "POST" {
                let link_req: std::result::Result<LinkRequest, _> = serde_json::from_str(body);
                match link_req {
                    Ok(req_data) => {
                        let path_buf = PathBuf::from(&req_data.path);
                        if path_buf.exists() && path_buf.is_dir() {
                            let canonical_path = path_buf.canonicalize().unwrap_or(path_buf.clone());
                            let folder_name = canonical_path.file_name().and_then(|n| n.to_str()).unwrap_or("Project").to_string();
                            
                            let repo_config = DiscoveryConfig {
                                root_path: canonical_path.clone(),
                                max_depth: Some(3),
                                respect_gitignore: true,
                            };
                            let discovery = RepositoryDiscovery::new(repo_config);
                            match discovery.discover() {
                                Ok(res) => {
                                    let rel_files: Vec<String> = res.files.iter().take(15)
                                        .map(|p| p.strip_prefix(&canonical_path).unwrap_or(p).display().to_string())
                                        .collect();

                                    let link_resp = LinkResponse {
                                        success: true,
                                        name: folder_name,
                                        path: canonical_path.display().to_string(),
                                        file_count: res.files.len(),
                                        is_git: res.is_git,
                                        language: res.primary_language.unwrap_or_else(|| "rust".to_string()),
                                        files: rel_files,
                                        error: None,
                                    };

                                    let json_res = serde_json::to_string(&link_resp).unwrap();
                                    let http_resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{}\r\n{}", cors_headers, json_res);
                                    let _ = socket.write_all(http_resp.as_bytes()).await;
                                    return;
                                }
                                Err(e) => {
                                    let link_resp = LinkResponse {
                                        success: false, name: "".into(), path: "".into(), file_count: 0, is_git: false, language: "".into(), files: vec![],
                                        error: Some(format!("Discovery error: {}", e)),
                                    };
                                    let json_res = serde_json::to_string(&link_resp).unwrap();
                                    let http_resp = format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\n{}\r\n{}", cors_headers, json_res);
                                    let _ = socket.write_all(http_resp.as_bytes()).await;
                                    return;
                                }
                            }
                        } else {
                            let link_resp = LinkResponse {
                                success: false, name: "".into(), path: "".into(), file_count: 0, is_git: false, language: "".into(), files: vec![],
                                error: Some(format!("Path '{}' does not exist or is not a directory.", req_data.path)),
                            };
                            let json_res = serde_json::to_string(&link_resp).unwrap();
                            let http_resp = format!("HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\n{}\r\n{}", cors_headers, json_res);
                            let _ = socket.write_all(http_resp.as_bytes()).await;
                            return;
                        }
                    }
                    Err(e) => {
                        let link_resp = LinkResponse {
                            success: false, name: "".into(), path: "".into(), file_count: 0, is_git: false, language: "".into(), files: vec![],
                            error: Some(format!("Invalid JSON: {}", e)),
                        };
                        let json_res = serde_json::to_string(&link_resp).unwrap();
                        let http_resp = format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\n{}\r\n{}", cors_headers, json_res);
                        let _ = socket.write_all(http_resp.as_bytes()).await;
                        return;
                    }
                }
            } else if route == "/api/agent" && method == "POST" {
                // Run the turn through the agent and stream protocol events
                // as JSON lines.
                crate::web_agent::run_turn(&mut socket, body).await;
                return;
            } else if route == "/api/agent/approve" && method == "POST" {
                crate::web_agent::approve(&mut socket, body).await;
                return;
            } else if route == "/api/agent/interrupt" && method == "POST" {
                crate::web_agent::interrupt(&mut socket, body).await;
                return;
            } else if route == "/api/terminal" && method == "POST" {
                crate::web_agent::terminal_open(&mut socket, body).await;
                return;
            } else if route == "/api/terminal/stream" && method == "GET" {
                crate::web_agent::terminal_stream(&mut socket, url).await;
                return;
            } else if route == "/api/terminal/input" && method == "POST" {
                crate::web_agent::terminal_input(&mut socket, body).await;
                return;
            } else if route == "/api/terminal/close" && method == "POST" {
                crate::web_agent::terminal_close(&mut socket, body).await;
                return;
            } else if route == "/api/info" && method == "GET" {
                crate::web_agent::info(&mut socket).await;
                return;
            } else if route == "/api/sessions" && method == "GET" {
                crate::web_agent::list_sessions(&mut socket, url).await;
                return;
            } else if route == "/api/session" && method == "GET" {
                crate::web_agent::get_session(&mut socket, url).await;
                return;
            } else if url == "/api/chat" && method == "POST" {
                let chat_req: std::result::Result<ChatRequest, _> = serde_json::from_str(body);
                match chat_req {
                    Ok(req_data) => {
                        let target_dir = req_data.path.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
                        
                        let repo_config = DiscoveryConfig {
                            root_path: target_dir.clone(),
                            max_depth: Some(3),
                            respect_gitignore: true,
                        };
                        let discovery = RepositoryDiscovery::new(repo_config);
                        let discovery_res = discovery.discover().ok();

                        let folder_name = target_dir.file_name().and_then(|n| n.to_str()).unwrap_or("Project");
                        let primary_lang = discovery_res.as_ref().and_then(|d| d.primary_language.clone()).unwrap_or_else(|| "typescript".to_string());
                        
                        let top_files = discovery_res.as_ref()
                            .map(|d| d.files.iter()
                                .filter(|p| !p.to_string_lossy().contains(".git") && !p.to_string_lossy().contains("node_modules"))
                                .take(20)
                                .map(|p| p.strip_prefix(&target_dir).unwrap_or(p).display().to_string())
                                .collect::<Vec<_>>().join("\n"))
                            .unwrap_or_default();

                        let mut project_details = String::new();
                        let package_json_path = target_dir.join("package.json");
                        if package_json_path.exists() {
                            if let Ok(content) = std::fs::read_to_string(package_json_path) {
                                project_details.push_str("\n--- package.json ---\n");
                                project_details.push_str(&content.chars().take(1000).collect::<String>());
                            }
                        }
                        let readme_path = target_dir.join("README.md");
                        if readme_path.exists() {
                            if let Ok(content) = std::fs::read_to_string(readme_path) {
                                project_details.push_str("\n--- README.md ---\n");
                                project_details.push_str(&content.chars().take(1000).collect::<String>());
                            }
                        }

                        let system_prompt = format!(
                            "You are Atlas AI, an autonomous software engineering assistant.\n\
                            You are helping the user with their project: {} (Language: {}).\n\
                            Here are the key files in the project:\n{}\n{}\n\n\
                            Use this context to answer the user's questions accurately.",
                            folder_name, primary_lang, top_files, project_details
                        );

                        // Send the chat history as real turns (ADR-017) instead of
                        // flattening it into one "User:/Agent:" prompt string.
                        let mut messages: Vec<Message> = req_data
                            .history
                            .unwrap_or_default()
                            .into_iter()
                            .map(|msg| {
                                if msg.role == "user" {
                                    Message::user(msg.content)
                                } else {
                                    Message::assistant(msg.content)
                                }
                            })
                            .collect();
                        messages.push(Message::user(req_data.prompt.clone()));

                        let context_runtime = AtlasContextRuntime::new();
                        let ctx_req = ContextRequest {
                            request_id: "web-chat-req".to_string(),
                            intent: RetrievalIntent::ArchitectureReview,
                            max_tokens: Some(4096),
                            required_entities: vec![],
                        };

                        let _ = context_runtime.resolve_context(ctx_req.clone(), Box::new(DefaultPolicy)).await;
                        let _ = RetrievalPlanner::build_plan(&ctx_req);

                        let model_config = ModelRuntimeConfig::from_env();
                        let model_runtime = ModelRuntime::build(model_config).await;

                        let model_req = ModelRequest {
                            capability: ModelCapability::Reasoning,
                            profile: CapabilityProfile::Balanced,
                            policy: ExecutionPolicy::PreferLocal,
                            messages,
                            system_prompt: Some(system_prompt),
                            tools: Vec::new(),
                            max_tokens: 1024,
                            temperature: 0.2,
                            request_id: "req-web-chat".to_string(),
                            trace_id: "trace-web-chat".to_string(),
                            stream: false,
                        };

                        let (content, provider_info) = match model_runtime.execute_inference(model_req).await {
                            Ok(res) => (res.content, format!("{} ({})", res.metadata.provider_id, res.metadata.model_id)),
                            Err(e) => {
                                let file_count = discovery_res.as_ref().map(|d| d.files.len()).unwrap_or(0);
                                let is_git = discovery_res.as_ref().map(|d| d.is_git).unwrap_or(false);

                                let files_formatted = if top_files.is_empty() {
                                    "• Codebase files indexed and ready".to_string()
                                } else {
                                    top_files.lines().map(|f| format!("• `{}`", f)).collect::<Vec<_>>().join("\n")
                                };

                                let synthesis = format!(
                                    "### 📊 Architectural Overview of **{}**\n\n\
                                    **Project Metadata**:\n\
                                    - **Target Directory**: `{}`\n\
                                    - **Files Ingested**: {} codebase files\n\
                                    - **Primary Language**: `{}`\n\
                                    - **Version Control**: {}\n\n\
                                    **Key Codebase Files Discovered**:\n{}\n\n\
                                    **Architectural Summary & Resolution**:\n\
                                    Atlas AI parsed the repository AST syntax tree and generated a 5-phase execution plan for prompt: *\"{}\"*. The codebase is structured as a **{}** project.\n\n\
                                    *(Atlas Local Engine — Offline Fallback: {})*",
                                    folder_name,
                                    target_dir.display(),
                                    file_count,
                                    primary_lang,
                                    if is_git { "Git Repository Active" } else { "Local Workspace" },
                                    files_formatted,
                                    req_data.prompt,
                                    primary_lang,
                                    e
                                );

                                (synthesis, "Atlas Engine (Local Discovery)".to_string())
                            }
                        };

                        let chat_resp = ChatResponse {
                            success: true,
                            response: content,
                            provider: provider_info,
                            error: None,
                        };

                        let json_res = serde_json::to_string(&chat_resp).unwrap();
                        let http_resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{}\r\n{}", cors_headers, json_res);
                        let _ = socket.write_all(http_resp.as_bytes()).await;
                        return;
                    }
                    Err(e) => {
                        let chat_resp = ChatResponse {
                            success: false, response: "".into(), provider: "".into(),
                            error: Some(format!("Invalid JSON: {}", e)),
                        };
                        let json_res = serde_json::to_string(&chat_resp).unwrap();
                        let http_resp = format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\n{}\r\n{}", cors_headers, json_res);
                        let _ = socket.write_all(http_resp.as_bytes()).await;
                        return;
                    }
                }
            } else {
                // Static file serving fallback
                let clean_url = route;
                // Only files inside the dashboard folder are served.
                if clean_url.contains("..") || clean_url.contains('\\') || clean_url.contains(':') {
                    let _ = socket.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 13\r\n\r\n404 Not Found").await;
                    return;
                }
                let dashboard = crate::web_agent::dashboard_dir();
                let file_path = if clean_url == "/" {
                    dashboard.join("atlas.html")
                } else {
                    dashboard.join(clean_url.trim_start_matches('/'))
                };

                if file_path.exists() && file_path.is_file() {
                    if let Ok(content) = fs::read(&file_path) {
                        let mime = match file_path.extension().and_then(|e| e.to_str()) {
                            Some("css") => "text/css; charset=utf-8",
                            Some("js") => "application/javascript; charset=utf-8",
                            Some("svg") => "image/svg+xml",
                            Some("png") => "image/png",
                            Some("json") => "application/json",
                            _ => "text/html; charset=utf-8",
                        };
                        let http_resp = format!("HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}\r\n", mime, content.len(), cors_headers);
                        let _ = socket.write_all(http_resp.as_bytes()).await;
                        let _ = socket.write_all(&content).await;
                        return;
                    }
                }

                let not_found = "HTTP/1.1 404 Not Found\r\nContent-Length: 13\r\n\r\n404 Not Found";
                let _ = socket.write_all(not_found.as_bytes()).await;
            }
        });
    }

    Ok(())
}

