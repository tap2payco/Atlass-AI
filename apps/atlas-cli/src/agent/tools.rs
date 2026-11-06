//! The workspace boundary and the read-only tools of the CLI agent.
//!
//! Every path is resolved inside the workspace root: `..`, absolute paths and
//! symlinks that point outside the root are rejected. Write and command tools
//! live in `write_tools.rs` and `command.rs`.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use runtime_agent::{Tool, ToolEffect};
use serde_json::{json, Value};
use shared::model::ToolSpec;

const SKIPPED_DIRS: &[&str] = &[".git", "target", "node_modules", ".venv", "__pycache__"];
const MAX_LIST_ENTRIES: usize = 500;
const DEFAULT_LIST_DEPTH: u64 = 2;
const MAX_READ_BYTES: usize = 64 * 1024;
const MAX_SEARCH_MATCHES: usize = 100;
const MAX_SEARCH_FILE_BYTES: u64 = 1024 * 1024;
const MAX_MATCH_LINE_CHARS: usize = 200;

/// The canonical workspace root shared by all tools.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub fn new(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|e| format!("cannot open workspace {}: {e}", root.display()))?;
        if !root.is_dir() {
            return Err(format!("workspace {} is not a directory", root.display()));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The root for display, without Windows' `\\?\` verbatim prefix.
    pub fn display_root(&self) -> String {
        let s = self.root.display().to_string();
        s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
    }

    /// Resolve a model-supplied relative path to an existing path inside the root.
    pub(crate) fn resolve(&self, relative: &str) -> Result<PathBuf, String> {
        let candidate = self.root.join(relative);
        let resolved = candidate
            .canonicalize()
            .map_err(|_| format!("'{relative}' does not exist in the workspace"))?;
        if !resolved.starts_with(&self.root) {
            return Err(format!("'{relative}' is outside the workspace"));
        }
        Ok(resolved)
    }

    /// Resolve a path the agent wants to create or modify. Stricter than
    /// `resolve`: `..` is not allowed at all, the path may not exist yet but
    /// its deepest existing ancestor must resolve (through symlinks) inside
    /// the root, and nothing under `.git` may be written (a write to
    /// `.git/hooks` would run code outside the agent's control).
    pub(crate) fn resolve_for_write(&self, relative: &str) -> Result<PathBuf, String> {
        if relative.trim().is_empty() {
            return Err("a path is required".into());
        }
        let candidate = self.root.join(relative);
        if candidate.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(format!("'{relative}' may not contain '..'"));
        }

        // Canonicalize the deepest part that exists, then re-attach the rest.
        let mut existing = candidate.as_path();
        let mut missing = Vec::new();
        while !existing.exists() {
            let Some(name) = existing.file_name() else {
                return Err(format!("'{relative}' is outside the workspace"));
            };
            missing.push(name.to_owned());
            existing = existing.parent().ok_or_else(|| format!("'{relative}' is outside the workspace"))?;
        }
        let mut resolved = existing
            .canonicalize()
            .map_err(|e| format!("cannot resolve '{relative}': {e}"))?;
        for name in missing.into_iter().rev() {
            resolved.push(name);
        }

        let inside = resolved
            .strip_prefix(&self.root)
            .map_err(|_| format!("'{relative}' is outside the workspace"))?;
        if inside.as_os_str().is_empty() {
            return Err(format!("'{relative}' is the workspace root, not a file"));
        }
        if inside.components().any(|c| c.as_os_str().eq_ignore_ascii_case(".git")) {
            return Err(format!("'{relative}' is inside .git, which the agent may not modify"));
        }
        Ok(resolved)
    }

    pub(crate) fn display(&self, path: &Path) -> String {
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        let s = rel.to_string_lossy().replace('\\', "/");
        if s.is_empty() { ".".into() } else { s }
    }
}

/// The read-only tools.
pub fn read_tools(workspace: &Workspace) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ListFiles(workspace.clone())),
        Arc::new(ReadFile(workspace.clone())),
        Arc::new(SearchText(workspace.clone())),
    ]
}

pub(crate) fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(|e| format!("tool task failed: {e}"))?
}

fn is_skipped(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| SKIPPED_DIRS.contains(&n))
}

/// Depth-first walk under `start`, skipping vendored/build directories and
/// never following symlinks.
fn walk(start: &Path, max_depth: u64, visit: &mut dyn FnMut(&Path, bool) -> bool) {
    fn go(dir: &Path, depth: u64, max: u64, visit: &mut dyn FnMut(&Path, bool) -> bool) -> bool {
        let Ok(entries) = fs::read_dir(dir) else { return true };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            let is_dir = kind.is_dir();
            if is_dir && is_skipped(&path) {
                continue;
            }
            if !visit(&path, is_dir) {
                return false;
            }
            if is_dir && depth + 1 < max && !go(&path, depth + 1, max, visit) {
                return false;
            }
        }
        true
    }
    go(start, 0, max_depth, visit);
}

pub(crate) fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|&b| b == 0)
}

// ── list_files ────────────────────────────────────────────────────────────────

struct ListFiles(Workspace);

#[async_trait]
impl Tool for ListFiles {
    fn effect(&self) -> ToolEffect {
        ToolEffect::ReadOnly
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "list_files".into(),
            description: "List files and directories in the workspace. Directories end with '/'. \
                          Skips .git, target and node_modules."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory relative to the workspace root. Default '.'." },
                    "max_depth": { "type": "integer", "minimum": 1, "maximum": 6, "description": "Default 2." }
                }
            }),
        }
    }

    async fn call(&self, args: Value) -> Result<String, String> {
        let ws = self.0.clone();
        let rel = str_arg(&args, "path").unwrap_or(".").to_string();
        let depth = args.get("max_depth").and_then(Value::as_u64).unwrap_or(DEFAULT_LIST_DEPTH).clamp(1, 6);
        blocking(move || {
            let dir = ws.resolve(&rel)?;
            if !dir.is_dir() {
                return Err(format!("'{rel}' is not a directory"));
            }
            let mut lines = Vec::new();
            let mut truncated = false;
            walk(&dir, depth, &mut |path, is_dir| {
                if lines.len() >= MAX_LIST_ENTRIES {
                    truncated = true;
                    return false;
                }
                let name = ws.display(path);
                lines.push(if is_dir { format!("{name}/") } else { name });
                true
            });
            if lines.is_empty() {
                return Ok(format!("'{rel}' is empty."));
            }
            if truncated {
                lines.push(format!("… (stopped after {MAX_LIST_ENTRIES} entries)"));
            }
            Ok(lines.join("\n"))
        })
        .await
    }
}

// ── read_file ─────────────────────────────────────────────────────────────────

struct ReadFile(Workspace);

#[async_trait]
impl Tool for ReadFile {
    fn effect(&self) -> ToolEffect {
        ToolEffect::ReadOnly
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file".into(),
            description: format!(
                "Read a UTF-8 text file from the workspace, with line numbers. \
                 Output stops after {} KB; use start_line to read further.",
                MAX_READ_BYTES / 1024
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to the workspace root." },
                    "start_line": { "type": "integer", "minimum": 1, "description": "First line to return. Default 1." }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Value) -> Result<String, String> {
        let ws = self.0.clone();
        let rel = str_arg(&args, "path").ok_or("missing required argument 'path'")?.to_string();
        let start = args.get("start_line").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
        blocking(move || {
            let path = ws.resolve(&rel)?;
            if !path.is_file() {
                return Err(format!("'{rel}' is not a file"));
            }
            let bytes = fs::read(&path).map_err(|e| format!("cannot read '{rel}': {e}"))?;
            if looks_binary(&bytes) {
                return Err(format!("'{rel}' looks like a binary file"));
            }
            let text = String::from_utf8_lossy(&bytes);
            let mut out = String::new();
            for (i, line) in text.lines().enumerate().skip(start - 1) {
                if out.len() + line.len() > MAX_READ_BYTES {
                    out.push_str(&format!("… (truncated; continue with start_line={})\n", i + 1));
                    return Ok(out);
                }
                out.push_str(&format!("{:>5}  {line}\n", i + 1));
            }
            if out.is_empty() {
                return Ok(format!("'{rel}' has no lines at or after line {start}."));
            }
            Ok(out)
        })
        .await
    }
}

// ── search_text ───────────────────────────────────────────────────────────────

struct SearchText(Workspace);

#[async_trait]
impl Tool for SearchText {
    fn effect(&self) -> ToolEffect {
        ToolEffect::ReadOnly
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "search_text".into(),
            description: format!(
                "Case-insensitive plain-text search across workspace files. \
                 Returns up to {MAX_SEARCH_MATCHES} matches as 'path:line: text'."
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Text to find (not a regex)." },
                    "path": { "type": "string", "description": "Directory or file to search. Default '.'." }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Value) -> Result<String, String> {
        let ws = self.0.clone();
        let query = str_arg(&args, "query").ok_or("missing required argument 'query'")?.to_lowercase();
        let rel = str_arg(&args, "path").unwrap_or(".").to_string();
        blocking(move || {
            let start = ws.resolve(&rel)?;
            let mut matches = Vec::new();
            let mut search_file = |path: &Path| -> bool {
                let Ok(meta) = fs::metadata(path) else { return true };
                if meta.len() > MAX_SEARCH_FILE_BYTES {
                    return true;
                }
                let Ok(bytes) = fs::read(path) else { return true };
                if looks_binary(&bytes) {
                    return true;
                }
                for (i, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
                    if line.to_lowercase().contains(&query) {
                        let shown: String = line.trim().chars().take(MAX_MATCH_LINE_CHARS).collect();
                        matches.push(format!("{}:{}: {shown}", ws.display(path), i + 1));
                        if matches.len() >= MAX_SEARCH_MATCHES {
                            return false;
                        }
                    }
                }
                true
            };
            if start.is_file() {
                search_file(&start);
            } else {
                walk(&start, u64::MAX, &mut |path, is_dir| is_dir || search_file(path));
            }
            if matches.is_empty() {
                return Ok("No matches.".into());
            }
            if matches.len() >= MAX_SEARCH_MATCHES {
                matches.push(format!("… (stopped after {MAX_SEARCH_MATCHES} matches)"));
            }
            Ok(matches.join("\n"))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::test_support::TempDir;

    fn fixture() -> (TempDir, Workspace) {
        let dir = TempDir::new();
        fs::create_dir_all(dir.path().join("src/nested")).unwrap();
        fs::create_dir_all(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "pub fn alpha() {}\npub fn Beta() {}\n").unwrap();
        fs::write(dir.path().join("src/nested/deep.rs"), "// alpha again\n").unwrap();
        fs::write(dir.path().join("target/junk.rs"), "alpha in build output\n").unwrap();
        fs::write(dir.path().join("blob.bin"), [0u8, 1, 2, 3]).unwrap();
        let ws = Workspace::new(dir.path()).unwrap();
        (dir, ws)
    }

    fn tool<'a>(tools: &'a [Arc<dyn Tool>], name: &str) -> &'a Arc<dyn Tool> {
        tools.iter().find(|t| t.spec().name == name).unwrap()
    }

    #[tokio::test]
    async fn list_files_respects_depth_and_skips_build_dirs() {
        let (_dir, ws) = fixture();
        let tools = read_tools(&ws);
        let out = tool(&tools, "list_files").call(json!({})).await.unwrap();
        assert_eq!(out, "blob.bin\nsrc/\nsrc/lib.rs\nsrc/nested/");
        let deep = tool(&tools, "list_files").call(json!({ "path": "src", "max_depth": 3 })).await.unwrap();
        assert!(deep.contains("src/nested/deep.rs"));
    }

    #[tokio::test]
    async fn read_file_numbers_lines_and_supports_offsets() {
        let (_dir, ws) = fixture();
        let tools = read_tools(&ws);
        let read = tool(&tools, "read_file");
        assert_eq!(
            read.call(json!({ "path": "src/lib.rs" })).await.unwrap(),
            "    1  pub fn alpha() {}\n    2  pub fn Beta() {}\n"
        );
        assert_eq!(
            read.call(json!({ "path": "src/lib.rs", "start_line": 2 })).await.unwrap(),
            "    2  pub fn Beta() {}\n"
        );
        assert!(read.call(json!({ "path": "blob.bin" })).await.unwrap_err().contains("binary"));
        assert!(read.call(json!({})).await.unwrap_err().contains("path"));
    }

    #[tokio::test]
    async fn search_is_case_insensitive_and_skips_build_output() {
        let (_dir, ws) = fixture();
        let tools = read_tools(&ws);
        let search = tool(&tools, "search_text");
        assert_eq!(
            search.call(json!({ "query": "ALPHA" })).await.unwrap(),
            "src/lib.rs:1: pub fn alpha() {}\nsrc/nested/deep.rs:1: // alpha again"
        );
        assert_eq!(search.call(json!({ "query": "beta" })).await.unwrap(), "src/lib.rs:2: pub fn Beta() {}");
        assert_eq!(search.call(json!({ "query": "zzz" })).await.unwrap(), "No matches.");
    }

    #[tokio::test]
    async fn paths_outside_the_workspace_are_rejected() {
        let (dir, ws) = fixture();
        let outside = dir.path().parent().unwrap().to_string_lossy().to_string();
        let tools = read_tools(&ws);
        let read = tool(&tools, "read_file");
        for path in ["../x", "..", outside.as_str()] {
            let err = tool(&tools, "list_files").call(json!({ "path": path })).await.unwrap_err();
            assert!(err.contains("outside the workspace") || err.contains("does not exist"), "{path}: {err}");
        }
        assert!(read.call(json!({ "path": "../../etc/passwd" })).await.is_err());
    }
}
