use sdk::context::provider::{ContextProvider, ProviderCapabilities, ContextFragments};
use sdk::context::ContextRequest;
use sdk::repository::RepositoryRuntime;
use shared::types::Result;
use shared::context::ContextFragment;
use async_trait::async_trait;
use std::sync::Arc;

pub struct RepositoryProvider {
    runtime: Arc<dyn RepositoryRuntime>,
}

impl RepositoryProvider {
    pub fn new(runtime: Arc<dyn RepositoryRuntime>) -> Self {
        Self { runtime }
    }
}

#[async_trait]
impl ContextProvider for RepositoryProvider {
    fn name(&self) -> &str {
        "repository"
    }

    fn capabilities(&self) -> Vec<ProviderCapabilities> {
        vec![ProviderCapabilities::RepositoryEntities, ProviderCapabilities::Symbols]
    }

    async fn retrieve(&self, request: &ContextRequest) -> Result<ContextFragments> {
        let dummy_repo_id = shared::id::RepositoryId::new();
        let mut fragments = Vec::new();

        // Fetch symbols for each requested entity
        for entity in &request.required_entities {
            let symbols_data = match self.runtime.get_symbols(dummy_repo_id.clone(), entity).await {
                Ok(data) => data,
                Err(_) => continue, // Skip if not found
            };

            let fragment = ContextFragment {
                id: uuid::Uuid::new_v4().to_string(),
                provider: self.name().to_string(),
                entity_ref: entity.clone(),
                retrieval_reason: format!("Requested by intent: {:?}", request.intent),
                confidence: 1.0,
                composite_score: None,
                token_cost: symbols_data.len() / 4, // 1 token approx 4 chars
                freshness: std::time::SystemTime::now(),
                provenance: vec!["RepositoryRuntime SDK".to_string()],
                relationships: vec![],
                compression_level: 0,
            };
            fragments.push(fragment);
        }

        // Always include a general snapshot as a baseline
        if let Ok(snapshot) = self.runtime.get_snapshot(dummy_repo_id).await {
            fragments.push(ContextFragment {
                id: uuid::Uuid::new_v4().to_string(),
                provider: self.name().to_string(),
                entity_ref: "snapshot".to_string(),
                retrieval_reason: "Baseline repository context".to_string(),
                confidence: 0.8,
                composite_score: None,
                token_cost: snapshot.len() / 4,
                freshness: std::time::SystemTime::now(),
                provenance: vec!["RepositoryRuntime SDK".to_string()],
                relationships: vec![],
                compression_level: 0,
            });
        }

        Ok(ContextFragments {
            fragments,
        })
    }
}
