use sdk::context::provider::{ContextProvider, ProviderCapabilities, ContextFragments};
use sdk::context::ContextRequest;
use shared::types::Result;
use async_trait::async_trait;

pub struct MemoryProvider;

#[async_trait]
impl ContextProvider for MemoryProvider {
    fn name(&self) -> &str {
        "memory"
    }

    fn capabilities(&self) -> Vec<ProviderCapabilities> {
        vec![ProviderCapabilities::Memory]
    }

    async fn retrieve(&self, _request: &ContextRequest) -> Result<ContextFragments> {
        unimplemented!("Memory Runtime is not yet implemented (L3 certification required first)")
    }
}
