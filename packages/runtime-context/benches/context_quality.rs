use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sdk::context::ContextRequest;
use sdk::context::intent::RetrievalIntent;
use runtime_context::retrieval::planning::RetrievalPlanner;

fn bench_planning_latency(c: &mut Criterion) {
    let request = ContextRequest {
        request_id: "bench_1".into(),
        intent: RetrievalIntent::SecurityAudit,
        max_tokens: None,
        required_entities: vec![],
    };

    c.bench_function("retrieval_planner_build_plan", |b| b.iter(|| {
        RetrievalPlanner::build_plan(black_box(&request))
    }));
}

// In a full implementation, we'd add benches for:
// - Retrieval latency (ProviderScheduler)
// - Scoring latency (ScoringEngine)
// - Optimization latency (ContextOptimization)
// - Token reduction ratio (calculated metric)
// - Validation failure rate (calculated metric)

criterion_group!(benches, bench_planning_latency);
criterion_main!(benches);
