use sdk::context::provider::{ContextProvider, ProviderCapabilities, ContextFragments};
use sdk::context::ContextRequest;
use shared::types::Result;
use async_trait::async_trait;

pub struct DocumentationProvider;

#[async_trait]
impl ContextProvider for DocumentationProvider {
    fn name(&self) -> &str {
        "documentation"
    }

    fn capabilities(&self) -> Vec<ProviderCapabilities> {
        vec![ProviderCapabilities::Documentation]
    }

    async fn retrieve(&self, _request: &ContextRequest) -> Result<ContextFragments> {
        unimplemented!("Documentation Provider is not yet implemented")
    }
}
