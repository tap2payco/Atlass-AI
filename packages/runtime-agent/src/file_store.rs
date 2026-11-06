//! A `SessionStore` on disk: one append-only JSON-lines file per session
//! (the idea of Codex's "rollout" files).
//!
//! ```text
//! <dir>/<uuid>.jsonl
//!   {"kind":"session","id":"…","created_at":1762400000,"metadata":{"workspace":"…"}}
//!   {"kind":"message","message":{"role":"user","content":[…]}}
//!   {"kind":"message","message":{"role":"assistant","content":[…]}}
//! ```
//!
//! - `append` writes all of its lines with a single write, so one call is
//!   stored completely or, if the process dies mid-write, as a cut-off last
//!   line. `load` skips a final line that does not parse, logging a warning,
//!   so a crash never makes a session unreadable.
//! - Sessions are never rewritten or deleted by this store.
//! - One process at a time per session: two processes appending to the same
//!   file concurrently are not coordinated.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shared::id::SessionId;
use shared::model::Message;

use crate::error::AgentError;
use crate::session::{Session, SessionStore};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Record {
    Session {
        id: uuid::Uuid,
        created_at: u64,
        #[serde(default)]
        metadata: BTreeMap<String, String>,
    },
    Message {
        message: Message,
    },
}

/// One stored session, as listed by `FileSessionStore::list`.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    pub id: SessionId,
    pub created_at: u64,
    /// Seconds since the Unix epoch when the file was last written.
    pub updated_at: u64,
    pub metadata: BTreeMap<String, String>,
    pub message_count: usize,
    /// First user message, shortened.
    pub title: String,
}

pub struct FileSessionStore {
    dir: PathBuf,
    /// Serialises writes from this process.
    write_lock: Mutex<()>,
}

impl FileSessionStore {
    /// Use (and create if needed) `dir` for session files.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, AgentError> {
        let dir = dir.into();
        fs::create_dir_all(&dir)
            .map_err(|e| AgentError::Store(format!("cannot create {}: {e}", dir.display())))?;
        Ok(Self { dir, write_lock: Mutex::new(()) })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, id: SessionId) -> PathBuf {
        self.dir.join(format!("{}.jsonl", id.0))
    }

    fn read(&self, path: &Path) -> Result<Session, AgentError> {
        let text = fs::read_to_string(path)
            .map_err(|e| AgentError::Store(format!("cannot read {}: {e}", path.display())))?;
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let mut session: Option<Session> = None;
        for (index, line) in lines.iter().enumerate() {
            let record = match serde_json::from_str::<Record>(line) {
                Ok(record) => record,
                Err(e) if index + 1 == lines.len() && session.is_some() => {
                    // A cut-off final write (crash mid-append). Everything before it is intact.
                    tracing::warn!(path = %path.display(), error = %e, "ignoring incomplete last line");
                    break;
                }
                Err(e) => {
                    return Err(AgentError::Store(format!(
                        "{} line {} is not valid: {e}",
                        path.display(),
                        index + 1
                    )))
                }
            };
            match (record, &mut session) {
                (Record::Session { id, created_at, metadata }, None) => {
                    session = Some(Session { id: SessionId(id), created_at, metadata, messages: Vec::new() });
                }
                (Record::Message { message }, Some(s)) => s.messages.push(message),
                _ => {
                    return Err(AgentError::Store(format!(
                        "{} line {} is out of order",
                        path.display(),
                        index + 1
                    )))
                }
            }
        }
        session.ok_or_else(|| AgentError::Store(format!("{} has no session header", path.display())))
    }

    fn write_records(&self, path: &Path, records: &[Record], create_new: bool) -> Result<(), AgentError> {
        let mut buf = String::new();
        for record in records {
            let line = serde_json::to_string(record).map_err(|e| AgentError::Store(e.to_string()))?;
            buf.push_str(&line);
            buf.push('\n');
        }
        let _guard = self.write_lock.lock().unwrap();
        let mut options = OpenOptions::new();
        if create_new {
            options.write(true).create_new(true);
        } else {
            options.append(true);
        }
        let mut file = options.open(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => AgentError::Store(format!("{} already exists", path.display())),
            _ => AgentError::Store(format!("cannot open {}: {e}", path.display())),
        })?;
        file.write_all(buf.as_bytes())
            .and_then(|_| file.flush())
            .map_err(|e| AgentError::Store(format!("cannot write {}: {e}", path.display())))
    }

    /// All readable sessions, most recently updated first. Unreadable files
    /// are skipped with a warning.
    pub fn list(&self) -> Result<Vec<SessionSummary>, AgentError> {
        let entries = fs::read_dir(&self.dir)
            .map_err(|e| AgentError::Store(format!("cannot list {}: {e}", self.dir.display())))?;
        let mut summaries = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            match self.read(&path) {
                Ok(session) => {
                    let updated_at = entry
                        .metadata()
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(session.created_at);
                    summaries.push(SessionSummary {
                        id: session.id,
                        created_at: session.created_at,
                        updated_at,
                        title: session.title(60),
                        message_count: session.messages.len(),
                        metadata: session.metadata,
                    });
                }
                Err(e) => tracing::warn!(path = %path.display(), error = %e, "skipping unreadable session"),
            }
        }
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(b.created_at.cmp(&a.created_at)));
        Ok(summaries)
    }

    /// Find a session by full id, `session_<uuid>`, or a unique prefix of the
    /// uuid (at least 4 characters).
    pub fn find(&self, query: &str) -> Result<SessionId, AgentError> {
        let q = query.trim().trim_start_matches("session_").to_ascii_lowercase();
        if q.len() < 4 {
            return Err(AgentError::Store("give at least 4 characters of the session id".into()));
        }
        let matches: Vec<SessionId> = self
            .list()?
            .into_iter()
            .map(|s| s.id)
            .filter(|id| id.0.to_string().starts_with(&q))
            .collect();
        match matches.as_slice() {
            [id] => Ok(*id),
            [] => Err(AgentError::Store(format!("no session matches '{query}'"))),
            _ => Err(AgentError::Store(format!("'{query}' matches {} sessions; give more of the id", matches.len()))),
        }
    }
}

#[async_trait]
impl SessionStore for FileSessionStore {
    async fn create(&self, session: Session) -> Result<(), AgentError> {
        let mut records = vec![Record::Session {
            id: session.id.0,
            created_at: session.created_at,
            metadata: session.metadata,
        }];
        records.extend(session.messages.into_iter().map(|message| Record::Message { message }));
        self.write_records(&self.path(session.id), &records, true)
    }

    async fn load(&self, id: SessionId) -> Result<Session, AgentError> {
        let path = self.path(id);
        if !path.exists() {
            return Err(AgentError::SessionNotFound(id));
        }
        self.read(&path)
    }

    async fn append(&self, id: SessionId, messages: Vec<Message>) -> Result<(), AgentError> {
        let path = self.path(id);
        if !path.exists() {
            return Err(AgentError::SessionNotFound(id));
        }
        let records: Vec<Record> = messages.into_iter().map(|message| Record::Message { message }).collect();
        self.write_records(&path, &records, false)
    }
}
