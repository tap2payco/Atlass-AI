pub mod policy;
pub mod retrieval;
pub mod scoring;
pub mod assembly;
pub mod optimization;
pub mod validation;
pub mod cache;
pub mod providers;

use sdk::context::{ContextRuntime, ContextRequest};
use sdk::context::policy::ContextPolicy;
use shared::context::{ContextPackage, ContextBudget};
use shared::types::Result;
use async_trait::async_trait;

use self::retrieval::planning::RetrievalPlanner;
use self::retrieval::executor::PlanExecutor;
use self::scoring::engine::ScoringEngine;
use self::assembly::engine::AssemblyEngine;
use self::optimization::strategy::ContextOptimization;
use self::validation::layer::ValidationLayer;

pub struct AtlasContextRuntime {
    // A real implementation would hold a ProviderRegistry to inject into the scheduler
}

impl AtlasContextRuntime {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ContextRuntime for AtlasContextRuntime {
    async fn resolve_context(&self, request: ContextRequest, policy: Box<dyn ContextPolicy>) -> Result<ContextPackage> {
        // 1. Policy Resolution & Retrieval Planning
        let plan = RetrievalPlanner::build_plan(&request);

        // 2. Plan Execution (Provider Scheduling)
        let multi_fragments = PlanExecutor::execute(&plan, &request).await;
        
        // Flatten fragments
        let mut fragments = Vec::new();
        for mut f in multi_fragments {
            fragments.append(&mut f.fragments);
        }

        // 3. Scoring Engine
        ScoringEngine::score_fragments(&mut fragments, &policy.scoring_weights());

        // 4. Assembly Engine
        let mut budget = ContextBudget {
            total_tokens_allocated: request.max_tokens.unwrap_or(8192),
            tokens_used: 0,
            truncated: false,
        };
        let mut package = AssemblyEngine::assemble(fragments, budget, request.request_id.clone());

        // 5. Optimization Strategy
        ContextOptimization::optimize(&mut package, policy.optimization_strategy().as_ref());

        // 6. Validation
        let diagnostics = ValidationLayer::validate(&package);
        package.diagnostics = diagnostics;

        // 7. Cache Lookup/Update could go here

        Ok(package)
    }
}
