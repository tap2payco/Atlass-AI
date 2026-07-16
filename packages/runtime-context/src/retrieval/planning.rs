use sdk::context::ContextRequest;
use sdk::context::intent::RetrievalIntent;
use platform_contracts::execution::{ExecutionPlan, ExecutionStep, ExecutionStrategy};

pub struct RetrievalPlanner;

impl RetrievalPlanner {
    pub fn build_plan(request: &ContextRequest) -> ExecutionPlan {
        let mut steps = Vec::new();

        match request.intent {
            RetrievalIntent::SecurityAudit => {
                steps.push(ExecutionStep {
                    name: "fetch_repo_symbols".into(),
                    provider: "repository".into(),
                    required_capabilities: vec!["RepositoryEntities".into(), "Symbols".into()],
                    priority: 1,
                    is_parallel: true,
                });
                steps.push(ExecutionStep {
                    name: "fetch_git_history".into(),
                    provider: "git".into(),
                    required_capabilities: vec!["GitHistory".into()],
                    priority: 1,
                    is_parallel: true,
                });
            },
            RetrievalIntent::BugInvestigation => {
                steps.push(ExecutionStep {
                    name: "fetch_repo_entities".into(),
                    provider: "repository".into(),
                    required_capabilities: vec!["RepositoryEntities".into()],
                    priority: 1,
                    is_parallel: true,
                });
                steps.push(ExecutionStep {
                    name: "fetch_issues".into(),
                    provider: "issue".into(),
                    required_capabilities: vec!["Issues".into()],
                    priority: 2,
                    is_parallel: false,
                });
            },
            _ => {
                // Default fallback plan
                steps.push(ExecutionStep {
                    name: "fetch_repo_fallback".into(),
                    provider: "repository".into(),
                    required_capabilities: vec!["RepositoryEntities".into()],
                    priority: 1,
                    is_parallel: false,
                });
            }
        }

        ExecutionPlan {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            steps,
            strategy: ExecutionStrategy::BestEffort,
            timeout_ms: Some(5000),
        }
    }
}
