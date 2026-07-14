use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::id::{AgentId, TaskId, MemoryId, ToolId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<T> {
    pub id: uuid::Uuid,
    pub timestamp: DateTime<Utc>,
    pub version: u32,
    pub payload: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AtlasEvent {
    AgentStarted {
        agent_id: AgentId,
        task_id: TaskId,
    },
    AgentCompleted {
        agent_id: AgentId,
        task_id: TaskId,
        success: bool,
    },
    TaskCreated {
        task_id: TaskId,
        description: String,
    },
    TaskCompleted {
        task_id: TaskId,
        result: String,
    },
    MemoryUpdated {
        memory_id: MemoryId,
        operation: String,
    },
    ContextResolved {
        agent_id: AgentId,
        tokens: u32,
    },
    ToolExecuted {
        tool_id: ToolId,
        agent_id: AgentId,
        success: bool,
    },
    DecisionMade {
        agent_id: AgentId,
        reasoning: String,
    },
    // Sprint 1.75: Repository Events
    RepositoryOpened {
        repository_id: String,
        path: String,
    },
    RepositoryProfileUpdated {
        repository_id: String,
        profile: serde_json::Value,
    },
    RepositoryChanged {
        repository_id: String,
        change_type: String,
    },
    SnapshotCreated {
        snapshot_id: String,
        repository_id: String,
    },
    SnapshotSuperseded {
        old_snapshot_id: String,
        new_snapshot_id: String,
    },
    SymbolAdded {
        symbol_id: String,
        module_id: String,
    },
    SymbolRemoved {
        symbol_id: String,
        module_id: String,
    },
    SymbolUpdated {
        symbol_id: String,
        module_id: String,
    },
}
