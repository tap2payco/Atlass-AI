//! Sessions and their storage.
//!
//! The state machine reloads the session from the store after every step, so
//! the store is the single source of truth for a running agent.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use shared::id::SessionId;
use shared::model::Message;

use crate::error::AgentError;

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub id: SessionId,
    /// Conversation, oldest first.
    pub messages: Vec<Message>,
}

impl Session {
    pub fn new() -> Self {
        Self { id: SessionId::new(), messages: Vec::new() }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
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
