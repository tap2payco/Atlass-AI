use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sdk::context::ContextRequest;
use sdk::context::intent::RetrievalIntent;
use sdk::context::policy::ScoringWeights;
use shared::context::{ContextFragment, ContextBudget};
use runtime_context::retrieval::planning::RetrievalPlanner;
use runtime_context::scoring::engine::ScoringEngine;
use runtime_context::assembly::engine::AssemblyEngine;

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

fn bench_scoring_engine(c: &mut Criterion) {
    let weights = ScoringWeights {
        relevance: 1.0,
        recency: 1.0,
        structural_distance: 1.0,
        execution_importance: 1.0,
        confidence: 1.0,
        token_cost: 1.0,
    };

    c.bench_function("scoring_engine_100_fragments", |b| b.iter(|| {
        let mut fragments: Vec<_> = (0..100).map(|i| ContextFragment {
            id: format!("frag-{}", i),
            provider: "repository".into(),
            entity_ref: format!("entity-{}", i),
            retrieval_reason: "bench".into(),
            confidence: 0.9,
            composite_score: None,
            token_cost: 100,
            freshness: std::time::SystemTime::now(),
            provenance: vec![],
            relationships: vec![],
            compression_level: 0,
        }).collect();

        ScoringEngine::score_fragments(black_box(&mut fragments), black_box(&weights));
    }));
}

fn bench_assembly_engine(c: &mut Criterion) {
    c.bench_function("assembly_engine_cutoff", |b| b.iter(|| {
        let fragments: Vec<_> = (0..50).map(|i| ContextFragment {
            id: format!("frag-{}", i),
            provider: "repository".into(),
            entity_ref: format!("entity-{}", i),
            retrieval_reason: "bench".into(),
            confidence: 0.9,
            composite_score: Some(1.0 - (i as f32 * 0.01)),
            token_cost: 100,
            freshness: std::time::SystemTime::now(),
            provenance: vec![],
            relationships: vec![],
            compression_level: 0,
        }).collect();

        let budget = ContextBudget {
            total_tokens_allocated: 2000,
            tokens_used: 0,
            truncated: false,
        };

        AssemblyEngine::assemble(black_box(fragments), black_box(budget), black_box("req-bench".to_string()))
    }));
}

criterion_group!(benches, bench_planning_latency, bench_scoring_engine, bench_assembly_engine);
criterion_main!(benches);
