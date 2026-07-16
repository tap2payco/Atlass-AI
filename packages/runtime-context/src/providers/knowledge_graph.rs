use sdk::context::provider::{ContextProvider, ProviderCapabilities, ContextFragments};
use sdk::context::ContextRequest;
use shared::types::Result;
use async_trait::async_trait;

pub struct KnowledgeGraphProvider;

#[async_trait]
impl ContextProvider for KnowledgeGraphProvider {
    fn name(&self) -> &str {
        "knowledge_graph"
    }

    fn capabilities(&self) -> Vec<ProviderCapabilities> {
        vec![ProviderCapabilities::KnowledgeGraph]
    }

    async fn retrieve(&self, _request: &ContextRequest) -> Result<ContextFragments> {
        unimplemented!("Knowledge Graph Runtime is not yet implemented")
    }
}
