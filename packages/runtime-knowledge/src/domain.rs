use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const CURRENT_ONTOLOGY_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EntityTypeId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipCategory {
    Structural,
    Semantic,
    Causal,
    Temporal,
    Spatial,
    Behavioral,
    Dependency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeFact {
    pub id: String,
    pub statement: String,
    pub confidence: f32,
    pub supporting_memories: Vec<String>,
    pub ontology_version: u32,
    pub created_at: u64,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeInference {
    pub id: String,
    pub statement: String,
    pub confidence: f32,
    pub supporting_facts: Vec<String>,
    pub inference_rule: String,
    pub ontology_version: u32,
    pub created_at: u64,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipCandidate {
    pub source_id: String,
    pub target_id: String,
    pub category: RelationshipCategory,
    pub label: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRelationship {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub category: RelationshipCategory,
    pub label: String,
    pub confidence: f32,
    pub created_at: u64,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEntity {
    pub memory_id: String,
    pub extracted_name: String,
    pub context_snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EntityId {
    pub namespace: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityProvenance {
    pub source_memory_ids: Vec<String>,
    pub source_documents: Vec<String>,
    pub extraction_method: String,
    pub canonicalization_reason: String,
    pub enrichment_history: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityIdentity {
    pub id: EntityId,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub identity: EntityIdentity,
    pub entity_type: EntityTypeId,
    pub attributes: HashMap<String, String>,
    pub confidence: f32,
    pub provenance: EntityProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeSnapshot {
    pub entities: Vec<Entity>,
    pub ontology_version: u32,
    pub relationships: Vec<KnowledgeRelationship>,
    pub facts: Vec<KnowledgeFact>,
    pub inferences: Vec<KnowledgeInference>,
    pub conflicts: Vec<String>,
    pub graph_statistics: HashMap<String, u64>,
    pub provenance: String,
    pub generation: u64,
    pub parent_snapshot: Option<String>,
    pub created_from: Vec<String>, // memory snapshots or prior knowledge snapshots
    pub timestamp: u64,
}
