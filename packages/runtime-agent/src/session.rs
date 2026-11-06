//! Sessions and their storage.
//!
//! The state machine reloads the session from the store after every step, so
//! the store is the single source of truth for a running agent.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use shared::id::SessionId;
use shared::model::Message;

use crate::error::AgentError;

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub id: SessionId,
    /// Seconds since the Unix epoch.
    pub created_at: u64,
    /// Free-form labels set by the front end, e.g. the workspace path.
    pub metadata: BTreeMap<String, String>,
    /// Conversation, oldest first.
    pub messages: Vec<Message>,
}

impl Session {
    pub fn new() -> Self {
        Self::with_metadata(BTreeMap::new())
    }

    pub fn with_metadata(metadata: BTreeMap<String, String>) -> Self {
        Self { id: SessionId::new(), created_at: now_secs(), metadata, messages: Vec::new() }
    }

    /// The text of the first user message, shortened, for session lists.
    pub fn title(&self, max_chars: usize) -> String {
        let text = self
            .messages
            .iter()
            .find(|m| crate::conversation::is_kickoff(m))
            .map(Message::text)
            .unwrap_or_default();
        let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if one_line.chars().count() > max_chars {
            format!("{}…", one_line.chars().take(max_chars).collect::<String>())
        } else {
            one_line
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Persistence port for sessions. Implementations must make `append` atomic
/// per call: either all messages are stored or none.
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, session: Session) -> Result<(), AgentError>;
    async fn load(&self, id: SessionId) -> Result<Session, AgentError>;
    async fn append(&self, id: SessionId, messages: Vec<Message>) -> Result<(), AgentError>;
}

/// A process-local store, for tests and single-process use.
#[derive(Default)]
pub struct InMemorySessionStore {
    sessions: Mutex<HashMap<SessionId, Session>>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn create(&self, session: Session) -> Result<(), AgentError> {
        let mut sessions = self.sessions.lock().unwrap();
        if sessions.contains_key(&session.id) {
            return Err(AgentError::Store(format!("session {} already exists", session.id)));
        }
        sessions.insert(session.id, session);
        Ok(())
    }

    async fn load(&self, id: SessionId) -> Result<Session, AgentError> {
        self.sessions
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(AgentError::SessionNotFound(id))
    }

    async fn append(&self, id: SessionId, messages: Vec<Message>) -> Result<(), AgentError> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.get_mut(&id).ok_or(AgentError::SessionNotFound(id))?;
        session.messages.extend(messages);
        Ok(())
    }
}
