use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use crate::entities::EntityCollection;

/// [v1.0.0 FROZEN]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFragment {
    pub id: String,
    pub provider: String,
    pub entity_ref: String, // or some stronger typed enum
    pub retrieval_reason: String,
    pub confidence: f32,
    pub composite_score: Option<f32>,
    pub token_cost: usize,
    pub freshness: SystemTime,
    pub provenance: Vec<String>,
    pub relationships: Vec<String>,
    pub compression_level: u8,
    // The actual raw data can either be inline or kept in the entities map.
}

/// [v1.0.0 FROZEN]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPackage {
    pub id: String,
    pub request_id: String,
    pub fragments: Vec<ContextFragment>,
    // Optional: still keep the raw entities if needed for direct resolution
    pub raw_entities: EntityCollection,
    pub budget: ContextBudget,
    pub expiration: Option<SystemTime>,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    pub total_tokens_allocated: usize,
    pub tokens_used: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationDiagnosticLevel {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationDiagnostic {
    pub level: ValidationDiagnosticLevel,
    pub code: String,
    pub message: String,
    pub fragment_id: Option<String>,
}
