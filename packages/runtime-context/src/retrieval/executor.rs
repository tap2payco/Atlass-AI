use sdk::context::provider::ContextFragments;
use platform_contracts::execution::ExecutionPlan;
use sdk::context::ContextRequest;
use crate::providers::ProviderRegistry;
use super::scheduling::ProviderScheduler;

pub struct PlanExecutor;

impl PlanExecutor {
    pub async fn execute(
        plan: &ExecutionPlan,
        request: &ContextRequest,
        registry: Option<&ProviderRegistry>,
    ) -> Vec<ContextFragments> {
        // Delegates plan execution to the ProviderScheduler.
        ProviderScheduler::execute_plan(plan, request, registry).await
    }
}
