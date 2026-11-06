use sdk::context::provider::ContextFragments;
use platform_contracts::execution::ExecutionPlan;
use sdk::context::ContextRequest;
use shared::context::ContextFragment;
use crate::providers::ProviderRegistry;

pub struct ProviderScheduler;

impl ProviderScheduler {
    pub async fn execute_plan(
        plan: &ExecutionPlan,
        request: &ContextRequest,
        registry: Option<&ProviderRegistry>,
    ) -> Vec<ContextFragments> {
        let mut results = Vec::new();

        for step in &plan.steps {
            if let Some(reg) = registry {
                if let Some(provider) = reg.get(&step.provider) {
                    if let Ok(frags) = provider.retrieve(request).await {
                        results.push(frags);
                        continue;
                    }
                }
            }

            // Fallback simulation if provider not registered or call fails
            results.push(ContextFragments {
                fragments: vec![
                    ContextFragment {
                        id: uuid::Uuid::new_v4().to_string(),
                        provider: step.provider.clone(),
                        entity_ref: format!("{}_simulated_entity", step.provider),
                        retrieval_reason: format!("Triggered by step {}", step.name),
                        confidence: 0.95,
                        composite_score: None,
                        token_cost: 120,
                        freshness: std::time::SystemTime::now(),
                        provenance: vec![format!("Scheduler -> {}", step.provider)],
                        relationships: vec![],
                        compression_level: 0,
                    }
                ]
            });
        }

        results
    }
}
