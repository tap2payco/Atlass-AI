use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryType {
    Working,
    Episodic,
    Semantic,
    Procedural,
    Architecture,
    Session,
}



#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryClass {
    Transient,
    Persistent,
    Immutable,
    Derived,
    Cached,
    Reference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryTraits {
    pub supports_search: bool,
    pub supports_versioning: bool,
    pub supports_decay: bool,
    pub supports_archival: bool,
    pub supports_reinforcement: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpirationPolicy {
    Never,
    AfterAccesses(u32),
    AfterSeconds(u64),
    WhenConsolidated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryProvenance {
    pub source_runtime: String,
    pub original_request_id: Option<String>,
    pub created_by: String, // agent role or system
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryIdentity {
    pub id: String,
    pub memory_type: MemoryType,
    pub class: MemoryClass,
    pub traits: MemoryTraits,
    pub source: String,
    pub version: u32,
    pub parent_version: Option<u32>,
    pub derived_from_version: Option<u32>,
    pub provenance: MemoryProvenance,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryRelationship {
    DerivedFrom,
    References,
    VersionOf,
    Parent,
    Child,
    SnapshotOf,
    GeneratedFrom,
    Duplicates,
    Replaces,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipEdge {
    pub target_memory_id: String,
    pub relationship: MemoryRelationship,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContent<T> {
    pub payload: T,
    pub embedding_references: Vec<String>,
    pub entities: Vec<String>,
    pub relationships: Vec<RelationshipEdge>,
    pub summaries: Vec<String>,
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub confidence: f32, // 0.0 to 1.0 (Is this probably correct?)
    pub quality: f32, // 0.0 to 1.0 (Is this actually useful?)
    pub stability: f32, // 0.0 to 1.0 (How likely is this to change?)
    pub importance: f32, // 0.0 to 1.0
    pub decay: f32, // decay rate
    pub reinforcement: f32, // cumulative reinforcement score
    pub access_count: u32,
    pub last_accessed: u64,
    pub expiration_policy: ExpirationPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord<T> {
    pub identity: MemoryIdentity,
    pub content: MemoryContent<T>,
    pub metadata: MemoryMetadata,
}
