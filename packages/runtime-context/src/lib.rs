pub mod pipelines;

use sdk::context::{ContextRuntime, ContextRequest};
use shared::context::ContextPackage;
use shared::types::Result;
use sdk::repository::RepositoryRuntime;
use async_trait::async_trait;
use std::sync::Arc;

pub struct AtlasContextRuntime {
    repository: Arc<dyn RepositoryRuntime>,
}

impl AtlasContextRuntime {
    pub fn new(repository: Arc<dyn RepositoryRuntime>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl ContextRuntime for AtlasContextRuntime {
    async fn resolve_context(&self, request: ContextRequest) -> Result<ContextPackage> {
        // Pipeline Stage 1: Retrieval
        let candidates = pipelines::retrieval::ContextRetrieval::retrieve_candidates(&request, &*self.repository).await?;
        
        // Pipeline Stage 2: Ranking
        let ranked = pipelines::ranking::ContextRanking::rank_candidates(candidates);
        
        // Pipeline Stage 3: Assembly
        let assembled = pipelines::assembly::ContextAssembly::assemble_package(&request, ranked);
        
        // Pipeline Stage 4: Optimization
        let optimized = pipelines::optimization::ContextOptimization::optimize_package(assembled);
        
        Ok(optimized)
    }
}
