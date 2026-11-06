pub mod policy;
pub mod retrieval;
pub mod scoring;
pub mod assembly;
pub mod optimization;
pub mod validation;
pub mod cache;
pub mod providers;

use std::sync::Arc;
use sdk::context::{ContextRuntime, ContextRequest};
use sdk::context::policy::ContextPolicy;
use sdk::context::provider::ContextProvider;
use shared::context::{ContextPackage, ContextBudget};
use shared::types::Result;
use async_trait::async_trait;

use self::retrieval::planning::RetrievalPlanner;
use self::retrieval::executor::PlanExecutor;
use self::scoring::engine::ScoringEngine;
use self::assembly::engine::AssemblyEngine;
use self::optimization::strategy::ContextOptimization;
use self::validation::layer::ValidationLayer;
use self::cache::ContextCache;
use self::providers::ProviderRegistry;

pub struct AtlasContextRuntime {
    registry: ProviderRegistry,
    cache: ContextCache,
}

impl AtlasContextRuntime {
    pub fn new() -> Self {
        Self {
            registry: ProviderRegistry::new(),
            cache: ContextCache::default(),
        }
    }

    pub fn with_provider(mut self, provider: Arc<dyn ContextProvider>) -> Self {
        self.registry.register(provider);
        self
    }

    pub fn register_provider(&mut self, provider: Arc<dyn ContextProvider>) {
        self.registry.register(provider);
    }

    pub fn cache(&self) -> &ContextCache {
        &self.cache
    }
}

impl Default for AtlasContextRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContextRuntime for AtlasContextRuntime {
    async fn resolve_context(&self, request: ContextRequest, policy: Box<dyn ContextPolicy>) -> Result<ContextPackage> {
        // 0. Cache Lookup
        let cache_key = ContextCache::build_cache_key(&request);
        if let Some(mut cached_package) = self.cache.get(&cache_key) {
            cached_package.request_id = request.request_id;
            return Ok(cached_package);
        }

        // 1. Policy Resolution & Retrieval Planning
        let plan = RetrievalPlanner::build_plan(&request);

        // 2. Plan Execution (Provider Scheduling)
        let multi_fragments = PlanExecutor::execute(&plan, &request, Some(&self.registry)).await;
        
        // Flatten fragments
        let mut fragments = Vec::new();
        for mut f in multi_fragments {
            fragments.append(&mut f.fragments);
        }

        // 3. Scoring Engine
        ScoringEngine::score_fragments(&mut fragments, &policy.scoring_weights());

        // 4. Assembly Engine
        let budget = ContextBudget {
            total_tokens_allocated: request.max_tokens.unwrap_or(8192),
            tokens_used: 0,
            truncated: false,
        };
        let mut package = AssemblyEngine::assemble(fragments, budget, request.request_id.clone());

        // 5. Optimization Strategy
        ContextOptimization::optimize(&mut package, policy.optimization_strategy().as_ref());

        // 6. Validation
        let diagnostics = ValidationLayer::validate(&package, &request.required_entities);
        package.diagnostics = diagnostics;

        // 7. Store in Cache
        self.cache.set(cache_key, package.clone());

        Ok(package)
    }
}
