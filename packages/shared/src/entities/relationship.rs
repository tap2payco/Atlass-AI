use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipType {
    Calls,
    Implements,
    Inherits,
    Imports,
    Exports,
    DependsOn,
    Contains,
    References,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipEntity {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relationship_type: RelationshipType,
    pub metadata: Option<serde_json::Value>,
}
