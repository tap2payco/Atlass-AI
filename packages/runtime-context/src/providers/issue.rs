use sdk::context::provider::{ContextProvider, ProviderCapabilities, ContextFragments};
use sdk::context::ContextRequest;
use shared::types::Result;
use async_trait::async_trait;

pub struct IssueProvider;

#[async_trait]
impl ContextProvider for IssueProvider {
    fn name(&self) -> &str {
        "issue"
    }

    fn capabilities(&self) -> Vec<ProviderCapabilities> {
        vec![ProviderCapabilities::Issues]
    }

    async fn retrieve(&self, _request: &ContextRequest) -> Result<ContextFragments> {
        unimplemented!("Issue Provider is not yet implemented")
    }
}
