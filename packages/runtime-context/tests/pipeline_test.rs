use sdk::context::{ContextRequest, ContextRuntime};
use sdk::context::intent::RetrievalIntent;
use sdk::context::policy::{ContextPolicy, ProviderPriority, TokenBudget};
use runtime_context::AtlasContextRuntime;

struct MockPriority;
impl ProviderPriority for MockPriority {
    fn priority(&self, _provider_name: &str) -> u32 {
        1
    }
}

// Mock policy for testing
struct MockPolicy;
impl ContextPolicy for MockPolicy {
    fn name(&self) -> &str { "MockPolicy" }
    fn scoring_weights(&self) -> sdk::context::policy::ScoringWeights {
        sdk::context::policy::ScoringWeights {
            relevance: 1.0,
            recency: 1.0,
            structural_distance: 1.0,
            execution_importance: 1.0,
            confidence: 1.0,
            token_cost: 1.0,
        }
    }
    fn optimization_strategy(&self) -> Box<dyn runtime_context::optimization::strategy::OptimizationStrategy> {
        Box::new(runtime_context::optimization::strategy::NoOpStrategy)
    }
    fn provider_priority(&self) -> Box<dyn ProviderPriority> {
        Box::new(MockPriority)
    }
    fn token_budget(&self) -> TokenBudget {
        TokenBudget {
            max_tokens: 1000,
            reserved_for_system: 100,
        }
    }
}

#[tokio::test]
async fn test_full_pipeline_resolution() {
    let runtime = AtlasContextRuntime::new();
    
    let request = ContextRequest {
        request_id: "req-123".to_string(),
        intent: RetrievalIntent::BugInvestigation,
        max_tokens: Some(100), // very low budget to force truncation
        required_entities: vec!["src/main.rs".to_string()],
    };
    
    let policy = Box::new(MockPolicy);
    
    let result = runtime.resolve_context(request, policy).await;
    assert!(result.is_ok());
    
    let package = result.unwrap();
    assert_eq!(package.request_id, "req-123");
    
    // Check that diagnostics are present due to budget constraints
    let has_truncation_warning = package.diagnostics.iter().any(|d| d.code == "CONTEXT_TRUNCATED");
    assert!(has_truncation_warning, "Should emit a truncation warning due to low token budget");
    
    // Check that missing required entity triggers a warning (since we don't really have a repository mocked that returns src/main.rs)
    let has_missing_entity = package.diagnostics.iter().any(|d| d.code == "MISSING_REQUIRED_ENTITY");
    assert!(has_missing_entity, "Should warn about missing required entity src/main.rs");
}
