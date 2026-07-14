use async_trait::async_trait;
use shared::context::ContextPackage;
use shared::types::Result;

pub struct ContextRequest {
    pub request_id: String,
    pub intent: String,
    pub max_tokens: Option<usize>,
    pub required_entities: Vec<String>,
}

#[async_trait]
pub trait ContextRuntime: Send + Sync {
    /// Resolves the optimal context package for a given request.
    /// Operates entirely via the 4-stage pipeline (Retrieval, Ranking, Assembly, Optimization).
    async fn resolve_context(&self, request: ContextRequest) -> Result<ContextPackage>;
}
