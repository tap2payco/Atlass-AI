use shared::context::{ContextPackage, ContextFragment, ContextBudget};
use shared::entities::EntityCollection;

pub struct AssemblyEngine;

impl AssemblyEngine {
    pub fn assemble(fragments: Vec<ContextFragment>, mut budget: ContextBudget, request_id: String) -> ContextPackage {
        let mut assembled_fragments = Vec::new();
        
        for fragment in fragments {
            // Check if adding this fragment would exceed the budget
            if budget.tokens_used + fragment.token_cost <= budget.total_tokens_allocated {
                budget.tokens_used += fragment.token_cost;
                assembled_fragments.push(fragment);
            } else {
                budget.truncated = true;
                // Since fragments are already sorted by priority/score, we stop here
                // Note: In an advanced version, we might try to fit smaller fragments
                // but for L2 strict priority cutoff is preferred to avoid context fragmentation.
                break;
            }
        }

        ContextPackage {
            id: uuid::Uuid::new_v4().to_string(),
            request_id,
            fragments: assembled_fragments,
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
