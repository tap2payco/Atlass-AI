use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::domain::{EntityTypeId, RelationshipCategory};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyVersion {
    pub version: u32,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vocabulary {
    // Maps aliases/raw strings to canonical names
    pub canonical_names: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeSchema {
    // Defines what something is
    pub entity_types: HashMap<EntityTypeId, String>, // id -> description
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Taxonomy {
    // Defines hierarchy (child -> parent)
    pub hierarchies: HashMap<EntityTypeId, Vec<EntityTypeId>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipRule {
    pub id: String,
    pub version: u32,
    pub source_type: EntityTypeId,
    pub target_type: EntityTypeId,
    pub valid_categories: Vec<RelationshipCategory>,
    pub introduced: u32,
    pub deprecated: Option<u32>,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRule {
    pub id: String,
    pub version: u32,
    pub condition_logic: String, // Stub for DSL or code rule
    pub derivation_logic: String,
    pub introduced: u32,
    pub deprecated: Option<u32>,
    pub replacement: Option<String>,
}

pub struct OntologyRegistry {
    pub version_info: OntologyVersion,
    pub vocabulary: Vocabulary,
    pub types: TypeSchema,
    pub taxonomy: Taxonomy,
    pub relationship_rules: Vec<RelationshipRule>,
    pub inference_rules: Vec<InferenceRule>,
}

impl OntologyRegistry {
    pub fn new() -> Self {
        // Stub for dynamic loading
        Self {
            version_info: OntologyVersion { version: 1, created_at: 0 },
            vocabulary: Vocabulary { canonical_names: HashMap::new() },
            types: TypeSchema { entity_types: HashMap::new() },
            taxonomy: Taxonomy { hierarchies: HashMap::new() },
            relationship_rules: vec![],
            inference_rules: vec![],
        }
    }

    // Dependencies: Vocabulary -> Types -> Taxonomy -> RelationshipRules -> InferenceRules
    // The registry acts as the single source of truth for the entire pipeline.
    
    pub fn validate_dependency_graph(&self) -> Result<(), String> {
        // Check that Taxonomy only references defined Types
        for (child, parents) in &self.taxonomy.hierarchies {
            if !self.types.entity_types.contains_key(child) {
                return Err(format!("Taxonomy references undefined child type: {:?}", child));
            }
            for parent in parents {
                if !self.types.entity_types.contains_key(parent) {
                    return Err(format!("Taxonomy references undefined parent type: {:?}", parent));
                }
            }
        }
        
        // Check that Relationship Rules only reference defined Types
        for rule in &self.relationship_rules {
            if !self.types.entity_types.contains_key(&rule.source_type) {
                return Err(format!("Relationship Rule references undefined source type: {:?}", rule.source_type));
            }
            if !self.types.entity_types.contains_key(&rule.target_type) {
                return Err(format!("Relationship Rule references undefined target type: {:?}", rule.target_type));
            }
        }
        
        Ok(())
    }
}
