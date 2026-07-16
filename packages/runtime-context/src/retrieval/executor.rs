use sdk::context::provider::ContextFragments;
use platform_contracts::execution::ExecutionPlan;
use sdk::context::ContextRequest;
use super::scheduling::ProviderScheduler;

pub struct PlanExecutor;

impl PlanExecutor {
    pub async fn execute(plan: &ExecutionPlan, request: &ContextRequest) -> Vec<ContextFragments> {
        // Here, a real PlanExecutor would handle timeouts, retries, 
        // circuit breakers, and tracking execution metrics across the entire plan.
        
        // For now, it delegates to the ProviderScheduler.
        ProviderScheduler::execute_plan(plan, request).await
    }
}
