use shared::context::{ContextPackage, ContextFragment, ContextBudget};
use shared::entities::EntityCollection;

pub struct AssemblyEngine;

impl AssemblyEngine {
    pub fn assemble(fragments: Vec<ContextFragment>, budget: ContextBudget, request_id: String) -> ContextPackage {
        ContextPackage {
            id: uuid::Uuid::new_v4().to_string(),
            request_id,
            fragments,
            raw_entities: EntityCollection {
                packages: vec![],
                modules: vec![],
                symbols: vec![],
                relationships: vec![],
            },
            budget,
            expiration: Some(std::time::SystemTime::now() + std::time::Duration::from_secs(3600)),
            diagnostics: vec![],
        }
    }
}
