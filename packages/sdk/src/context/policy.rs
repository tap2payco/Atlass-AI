#[derive(Debug, Clone)]
pub struct ScoringWeights {
    pub relevance: f32,
    pub recency: f32,
    pub structural_distance: f32,
    pub execution_importance: f32,
    pub confidence: f32,
    pub token_cost: f32,
}

#[derive(Debug, Clone)]
pub struct TokenBudget {
    pub max_tokens: usize,
    pub reserved_for_system: usize,
}

pub trait OptimizationStrategy: Send + Sync {
    fn name(&self) -> &str;
}

pub trait ProviderPriority: Send + Sync {
    fn priority(&self, provider_name: &str) -> u32;
}

pub trait ContextPolicy: Send + Sync {
    fn name(&self) -> &str;
    fn scoring_weights(&self) -> ScoringWeights;
    fn optimization_strategy(&self) -> Box<dyn OptimizationStrategy>;
    fn provider_priority(&self) -> Box<dyn ProviderPriority>;
    fn token_budget(&self) -> TokenBudget;
}
