pub use sdk::context::policy::OptimizationStrategy;
use shared::context::ContextPackage;

pub struct NoOpStrategy;

impl OptimizationStrategy for NoOpStrategy {
    fn name(&self) -> &str {
        "NoOpStrategy"
    }
}

pub struct ContextOptimization;

impl ContextOptimization {
    pub fn optimize(package: &mut ContextPackage, _strategy: &dyn OptimizationStrategy) {
        let max_tokens = package.budget.total_tokens_allocated;
        let mut current_tokens = 0;
        let mut kept_fragments = Vec::new();
        let mut truncated = package.budget.truncated;

        // Since they are pre-sorted by scoring engine, we just take top N until budget is hit
        for fragment in package.fragments.drain(..) {
            if current_tokens + fragment.token_cost <= max_tokens {
                current_tokens += fragment.token_cost;
                kept_fragments.push(fragment);
            } else {
                truncated = true;
                // Once we hit the limit, we discard the rest (simplest optimization level)
                break;
            }
        }

        package.fragments = kept_fragments;
        package.budget.tokens_used = current_tokens;
        package.budget.truncated = truncated;
    }
}
