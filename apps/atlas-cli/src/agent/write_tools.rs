//! Tools that modify files in the workspace: `write_file` and `edit_file`.
//!
//! Paths go through `Workspace::resolve_for_write`, which applies the
//! workspace's sandbox policy: nothing is written under `ReadOnly`, nor
//! outside the workspace and its writable roots, nor under the protected
//! subpaths (`.git`, `.atlas`, `.codex`, `.claude`). Whether the user is asked
//! first is the approver's job (`approval.rs`).

use std::fs;
use std::sync::Arc;

use async_trait::async_trait;
use runtime_agent::{Tool, ToolEffect};
use serde_json::{json, Value};
use shared::model::ToolSpec;

use super::tools::{blocking, looks_binary, str_arg, Workspace};

pub fn write_tools(workspace: &Workspace) -> Vec<Arc<dyn Tool>> {
    vec![Arc::new(WriteFile(workspace.clone())), Arc::new(EditFile(workspace.clone()))]
}

/// `content` may legitimately be empty, so it is read without `str_arg`.
fn text_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key).and_then(Value::as_str).ok_or_else(|| format!("missing required argument '{key}'"))
}

// ── write_file ────────────────────────────────────────────────────────────────

struct WriteFile(Workspace);

#[async_trait]
impl Tool for WriteFile {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write_file".into(),
            description: "Create a file, or replace a file's entire contents. Missing parent \
                          directories are created. Prefer edit_file for changes to existing files."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to the workspace root." },
                    "content": { "type": "string", "description": "The complete new file contents." }
                },
                "required": ["path", "content"]
            }),
        }
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::WritesWorkspace
    }

    async fn call(&self, args: Value) -> Result<String, String> {
        let ws = self.0.clone();
        let rel = str_arg(&args, "path").ok_or("missing required argument 'path'")?.to_string();
        let content = text_arg(&args, "content")?.to_string();
        blocking(move || {
            let path = ws.resolve_for_write(&rel)?;
            if path.is_dir() {
                return Err(format!("'{rel}' is a directory"));
            }
            let existed = path.exists();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("cannot create directories for '{rel}': {e}"))?;
            }
            fs::write(&path, content.as_bytes()).map_err(|e| format!("cannot write '{rel}': {e}"))?;
            Ok(format!(
                "{} {} ({} bytes).",
                if existed { "Replaced" } else { "Created" },
                ws.display(&path),
                content.len()
            ))
        })
        .await
    }
}

// ── edit_file ─────────────────────────────────────────────────────────────────

struct EditFile(Workspace);

#[async_trait]
impl Tool for EditFile {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "edit_file".into(),
            description: "Replace exact text in an existing file. old_text must match the file \
                          exactly (including whitespace) and, unless replace_all is true, occur \
                          exactly once; include surrounding lines to make it unique."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to the workspace root." },
                    "old_text": { "type": "string", "description": "Text to find. Must not be empty." },
                    "new_text": { "type": "string", "description": "Replacement text. May be empty." },
                    "replace_all": { "type": "boolean", "description": "Replace every occurrence. Default false." }
                },
                "required": ["path", "old_text", "new_text"]
            }),
        }
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::WritesWorkspace
    }

    async fn call(&self, args: Value) -> Result<String, String> {
        let ws = self.0.clone();
        let rel = str_arg(&args, "path").ok_or("missing required argument 'path'")?.to_string();
        let old = text_arg(&args, "old_text")?.to_string();
        let new = text_arg(&args, "new_text")?.to_string();
        let replace_all = args.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
        if old.is_empty() {
            return Err("old_text must not be empty".into());
        }
        blocking(move || {
            let path = ws.resolve_for_write(&rel)?;
            if !path.is_file() {
                return Err(format!("'{rel}' does not exist; use write_file to create it"));
            }
            let bytes = fs::read(&path).map_err(|e| format!("cannot read '{rel}': {e}"))?;
            if looks_binary(&bytes) {
                return Err(format!("'{rel}' looks like a binary file"));
            }
            let text = String::from_utf8(bytes).map_err(|_| format!("'{rel}' is not valid UTF-8"))?;

            // Models usually write "\n"; match CRLF files too, keeping their line endings.
            let (old, new) = if !text.contains(&old) && text.contains("\r\n") && !old.contains("\r\n") {
                (old.replace('\n', "\r\n"), new.replace('\n', "\r\n"))
            } else {
                (old, new)
            };

            let count = text.matches(&old).count();
            if count == 0 {
                return Err(format!("old_text was not found in '{rel}'. Read the file and copy the text exactly."));
            }
            if count > 1 && !replace_all {
                return Err(format!(
                    "old_text occurs {count} times in '{rel}'. Include more surrounding text, or set replace_all."
                ));
            }
            let updated = if replace_all { text.replace(&old, &new) } else { text.replacen(&old, &new, 1) };
            fs::write(&path, updated.as_bytes()).map_err(|e| format!("cannot write '{rel}': {e}"))?;
            Ok(format!(
                "Replaced {} occurrence{} in {}.",
                if replace_all { count } else { 1 },
                if replace_all && count != 1 { "s" } else { "" },
                ws.display(&path)
            ))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::test_support::TempDir;
    use platform_contracts::policy::SandboxPolicy;

    fn setup() -> (TempDir, Workspace, Vec<Arc<dyn Tool>>) {
        let dir = TempDir::new();
        fs::create_dir_all(dir.path().join(".git/hooks")).unwrap();
        fs::write(dir.path().join("a.txt"), "one\ntwo\ntwo\n").unwrap();
        fs::write(dir.path().join("crlf.txt"), "alpha\r\nbeta\r\n").unwrap();
        let ws = Workspace::new(dir.path()).unwrap();
        let tools = write_tools(&ws);
        (dir, ws, tools)
    }

    fn tool<'a>(tools: &'a [Arc<dyn Tool>], name: &str) -> &'a Arc<dyn Tool> {
        tools.iter().find(|t| t.spec().name == name).unwrap()
    }

    #[tokio::test]
    async fn write_file_creates_nested_files_and_replaces_existing_ones() {
        let (dir, _ws, tools) = setup();
        let write = tool(&tools, "write_file");
        assert_eq!(
            write.call(json!({ "path": "new/dir/b.txt", "content": "hi" })).await.unwrap(),
            "Created new/dir/b.txt (2 bytes)."
        );
        assert_eq!(fs::read_to_string(dir.path().join("new/dir/b.txt")).unwrap(), "hi");
        assert_eq!(
            write.call(json!({ "path": "a.txt", "content": "" })).await.unwrap(),
            "Replaced a.txt (0 bytes)."
        );
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "");
    }

    #[tokio::test]
    async fn writes_outside_the_workspace_or_into_git_are_refused() {
        let (dir, _ws, tools) = setup();
        let write = tool(&tools, "write_file");
        let outside = dir.path().parent().unwrap().join("escaped.txt");
        for path in ["../escaped.txt", "sub/../../escaped.txt", outside.to_str().unwrap(), ".git/hooks/pre-commit", ".GIT/config", ""] {
            let err = write.call(json!({ "path": path, "content": "x" })).await.unwrap_err();
            assert!(
                err.contains("..") || err.contains("outside") || err.contains(".git") || err.contains("required"),
                "{path:?}: {err}"
            );
        }
        assert!(!outside.exists());
        assert!(!dir.path().join(".git/hooks/pre-commit").exists());
    }

    #[tokio::test]
    async fn protected_paths_and_read_only_policies_refuse_writes() {
        let (dir, ws, _tools) = setup();
        fs::create_dir_all(dir.path().join(".atlas")).unwrap();
        fs::write(dir.path().join(".atlas/policy.toml"), "mode = 'read-only'
").unwrap();
        let tools = write_tools(&ws);
        let err = tool(&tools, "write_file")
            .call(json!({ "path": ".atlas/policy.toml", "content": "mode = 'danger-full-access'" }))
            .await
            .unwrap_err();
        assert!(err.contains("inside .atlas"), "{err}");
        let err = tool(&tools, "edit_file")
            .call(json!({ "path": ".atlas/policy.toml", "old_text": "read-only", "new_text": "x" }))
            .await
            .unwrap_err();
        assert!(err.contains("inside .atlas"), "{err}");
        assert_eq!(fs::read_to_string(dir.path().join(".atlas/policy.toml")).unwrap(), "mode = 'read-only'
");

        let read_only = write_tools(&ws.with_sandbox(SandboxPolicy::ReadOnly));
        let err = tool(&read_only, "write_file").call(json!({ "path": "c.txt", "content": "x" })).await.unwrap_err();
        assert!(err.contains("read-only"), "{err}");
        assert!(!dir.path().join("c.txt").exists());
    }

    #[tokio::test]
    async fn edit_file_requires_a_unique_match_unless_replace_all() {
        let (dir, _ws, tools) = setup();
        let edit = tool(&tools, "edit_file");
        assert_eq!(
            edit.call(json!({ "path": "a.txt", "old_text": "one", "new_text": "1" })).await.unwrap(),
            "Replaced 1 occurrence in a.txt."
        );
        let err = edit.call(json!({ "path": "a.txt", "old_text": "two", "new_text": "2" })).await.unwrap_err();
        assert!(err.contains("occurs 2 times"), "{err}");
        assert_eq!(
            edit.call(json!({ "path": "a.txt", "old_text": "two", "new_text": "2", "replace_all": true }))
                .await
                .unwrap(),
            "Replaced 2 occurrences in a.txt."
        );
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "1\n2\n2\n");
        let err = edit.call(json!({ "path": "a.txt", "old_text": "zzz", "new_text": "" })).await.unwrap_err();
        assert!(err.contains("not found"), "{err}");
        let err = edit.call(json!({ "path": "missing.txt", "old_text": "a", "new_text": "b" })).await.unwrap_err();
        assert!(err.contains("write_file"), "{err}");
    }

    #[tokio::test]
    async fn edit_file_matches_lf_text_in_crlf_files_and_keeps_crlf() {
        let (dir, _ws, tools) = setup();
        let edit = tool(&tools, "edit_file");
        edit.call(json!({ "path": "crlf.txt", "old_text": "alpha\nbeta", "new_text": "alpha\ngamma" }))
            .await
            .unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("crlf.txt")).unwrap(), "alpha\r\ngamma\r\n");
    }
}
