use serde::{Deserialize, Serialize};
use crate::entities::EntityCollection;
use crate::id::WorkspaceId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPackage {
    pub id: String,
    pub request_id: String,
    pub provenance: Vec<String>, // Tracks where every item came from
    pub entities: EntityCollection, // Sub-selection of relevant entities
    pub memory_references: Vec<String>, // IDs of retrieved memories
    pub retrieved_documents: Vec<String>, // IDs or paths of retrieved docs
    pub budget: ContextBudget,
    pub confidence_score: f32,
    pub expiration: Option<std::time::SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    pub total_tokens_allocated: usize,
    pub tokens_used: usize,
    pub truncated: bool,
}
