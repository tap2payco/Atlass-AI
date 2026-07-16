use async_trait::async_trait;
use shared::types::Result;
use crate::context::ContextRequest;

/// [v1.0.0 FROZEN]
#[derive(Debug, Clone)]
pub enum ProviderCapabilities {
    RepositoryEntities,
    Symbols,
    Documentation,
    Embeddings,
    GitHistory,
    Issues,
    KnowledgeGraph,
    Memory,
    ExternalAPIs,
}

pub struct ContextFragments {
    // A placeholder for the actual fragments collected by a provider
    pub fragments: Vec<shared::context::ContextFragment>,
}

#[async_trait]
pub trait ContextProvider: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> Vec<ProviderCapabilities>;
    async fn retrieve(&self, request: &ContextRequest) -> Result<ContextFragments>;
}
