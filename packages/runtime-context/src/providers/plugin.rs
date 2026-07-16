use sdk::context::provider::{ContextProvider, ProviderCapabilities, ContextFragments};
use sdk::context::ContextRequest;
use shared::types::Result;
use async_trait::async_trait;

pub struct PluginProvider;

#[async_trait]
impl ContextProvider for PluginProvider {
    fn name(&self) -> &str {
        "plugin"
    }

    fn capabilities(&self) -> Vec<ProviderCapabilities> {
        vec![]
    }

    async fn retrieve(&self, _request: &ContextRequest) -> Result<ContextFragments> {
        unimplemented!("Plugin Runtime is not yet implemented")
    }
}
