pub mod policy;
pub mod provider;
pub mod intent;

use async_trait::async_trait;
use shared::context::ContextPackage;
use shared::types::Result;
use self::policy::ContextPolicy;
use self::intent::RetrievalIntent;

#[derive(Debug, Clone)]
pub struct ContextRequest {
    pub request_id: String,
    pub intent: RetrievalIntent,
    pub max_tokens: Option<usize>,
    pub required_entities: Vec<String>,
}

#[async_trait]
pub trait ContextRuntime: Send + Sync {
    /// Resolves the optimal context package for a given request.
    /// Operates via the full orchestration engine (Planning, Parallel Retrieval, Scoring, Assembly, Optimization, Validation).
    async fn resolve_context(&self, request: ContextRequest, policy: Box<dyn ContextPolicy>) -> Result<ContextPackage>;
}
