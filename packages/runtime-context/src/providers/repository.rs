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
        // As a thin adapter, we translate the ContextRequest into specific SDK calls
        // For demonstration, we just fetch a snapshot and wrap it in a fragment.
        
        let dummy_repo_id = shared::id::RepositoryId::new("local");
        let snapshot = self.runtime.get_snapshot(dummy_repo_id).await?;

        let fragment = ContextFragment {
            id: uuid::Uuid::new_v4().to_string(),
            provider: self.name().to_string(),
            entity_ref: "snapshot".to_string(),
            retrieval_reason: format!("Requested by intent: {:?}", request.intent),
            confidence: 1.0,
            composite_score: None,
            token_cost: snapshot.len(), // rough estimate
            freshness: std::time::SystemTime::now(),
            provenance: vec!["RepositoryRuntime SDK".to_string()],
            relationships: vec![],
            compression_level: 0,
        };

        Ok(ContextFragments {
            fragments: vec![fragment],
        })
    }
}
