use std::sync::Arc;
use sdk::context::{ContextRequest, ContextRuntime};
use sdk::context::intent::RetrievalIntent;
use sdk::context::policy::{ContextPolicy, ProviderPriority, TokenBudget, ScoringWeights, OptimizationStrategy};
use sdk::context::provider::{ContextProvider, ProviderCapabilities, ContextFragments};
use shared::context::ContextFragment;
use shared::types::Result;
use async_trait::async_trait;
use runtime_context::AtlasContextRuntime;
use runtime_context::cache::ContextCache;
use runtime_context::providers::ProviderRegistry;

struct TestPriority;
impl ProviderPriority for TestPriority {
    fn priority(&self, _provider_name: &str) -> u32 {
        1
    }
}

struct TestPolicy;
impl ContextPolicy for TestPolicy {
    fn name(&self) -> &str { "TestPolicy" }
    fn scoring_weights(&self) -> ScoringWeights {
        ScoringWeights {
            relevance: 1.0,
            recency: 1.0,
            structural_distance: 1.0,
            execution_importance: 1.0,
            confidence: 1.0,
            token_cost: 1.0,
        }
    }
    fn optimization_strategy(&self) -> Box<dyn OptimizationStrategy> {
        Box::new(runtime_context::optimization::strategy::NoOpStrategy)
    }
    fn provider_priority(&self) -> Box<dyn ProviderPriority> {
        Box::new(TestPriority)
    }
    fn token_budget(&self) -> TokenBudget {
        TokenBudget {
            max_tokens: 4096,
            reserved_for_system: 512,
        }
    }
}

struct MockCustomProvider;

#[async_trait]
impl ContextProvider for MockCustomProvider {
    fn name(&self) -> &str {
        "repository"
    }

    fn capabilities(&self) -> Vec<ProviderCapabilities> {
        vec![ProviderCapabilities::RepositoryEntities, ProviderCapabilities::Symbols]
    }

    async fn retrieve(&self, request: &ContextRequest) -> Result<ContextFragments> {
        let mut fragments = Vec::new();
        for entity in &request.required_entities {
            fragments.push(ContextFragment {
                id: uuid::Uuid::new_v4().to_string(),
                provider: self.name().to_string(),
                entity_ref: entity.clone(),
                retrieval_reason: "Custom test retrieval".to_string(),
                confidence: 0.99,
                composite_score: None,
                token_cost: 50,
                freshness: std::time::SystemTime::now(),
                provenance: vec!["MockCustomProvider".to_string()],
                relationships: vec![],
                compression_level: 0,
            });
        }

        Ok(ContextFragments { fragments })
    }
}

#[tokio::test]
async fn test_provider_registry() {
    let mut registry = ProviderRegistry::new();
    let provider = Arc::new(MockCustomProvider);
    registry.register(provider.clone());

    assert!(registry.get("repository").is_some());
    assert_eq!(registry.list().len(), 1);
    assert_eq!(registry.list()[0], "repository");
}

#[tokio::test]
async fn test_context_cache_store() {
    let cache = ContextCache::default();
    let request = ContextRequest {
        request_id: "req-1".to_string(),
        intent: RetrievalIntent::BugInvestigation,
        max_tokens: Some(1000),
        required_entities: vec!["lib.rs".to_string()],
    };

    let key = ContextCache::build_cache_key(&request);
    assert!(cache.get(&key).is_none());

    let runtime = AtlasContextRuntime::new();
    let package = runtime.resolve_context(request.clone(), Box::new(TestPolicy)).await.unwrap();

    cache.set(key.clone(), package.clone());
    assert!(cache.get(&key).is_some());
    assert_eq!(cache.len(), 1);

    cache.clear();
    assert!(cache.is_empty());
}

#[tokio::test]
async fn test_custom_provider_integration() {
    let provider = Arc::new(MockCustomProvider);
    let runtime = AtlasContextRuntime::new().with_provider(provider);

    let request = ContextRequest {
        request_id: "req-custom".to_string(),
        intent: RetrievalIntent::BugInvestigation,
        max_tokens: Some(4096),
        required_entities: vec!["src/lib.rs".to_string()],
    };

    let result = runtime.resolve_context(request, Box::new(TestPolicy)).await;
    assert!(result.is_ok());

    let package = result.unwrap();
    let found_custom_entity = package.fragments.iter().any(|f| f.entity_ref == "src/lib.rs" && f.provenance.contains(&"MockCustomProvider".to_string()));
    assert!(found_custom_entity, "Should retrieve entity from registered MockCustomProvider");
}

#[tokio::test]
async fn test_cache_hit_in_runtime() {
    let runtime = AtlasContextRuntime::new();
    let request = ContextRequest {
        request_id: "req-cache-1".to_string(),
        intent: RetrievalIntent::SecurityAudit,
        max_tokens: Some(2048),
        required_entities: vec!["auth.rs".to_string()],
    };

    // First call (miss & populates cache)
    let pkg1 = runtime.resolve_context(request.clone(), Box::new(TestPolicy)).await.unwrap();
    assert_eq!(pkg1.request_id, "req-cache-1");

    // Second call with different request_id (hit)
    let mut request2 = request.clone();
    request2.request_id = "req-cache-2".to_string();

    let pkg2 = runtime.resolve_context(request2, Box::new(TestPolicy)).await.unwrap();
    assert_eq!(pkg2.request_id, "req-cache-2");
    assert_eq!(pkg2.fragments.len(), pkg1.fragments.len());
}
