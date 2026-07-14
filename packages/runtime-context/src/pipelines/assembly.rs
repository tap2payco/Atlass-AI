use shared::entities::EntityCollection;
use shared::context::{ContextPackage, ContextBudget};
use sdk::context::ContextRequest;

pub struct ContextAssembly;

impl ContextAssembly {
    pub fn assemble_package(request: &ContextRequest, ranked: EntityCollection) -> ContextPackage {
        // Build a coherent context package while preserving provenance.
        ContextPackage {
            id: uuid::Uuid::new_v4().to_string(),
            request_id: request.request_id.clone(),
            provenance: vec!["RepositoryRuntime".to_string()],
            entities: ranked,
            memory_references: vec![],
            retrieved_documents: vec![],
            budget: ContextBudget {
                total_tokens_allocated: request.max_tokens.unwrap_or(8192),
                tokens_used: 0,
                truncated: false,
            },
            confidence_score: 1.0,
            expiration: None,
        }
    }
}
