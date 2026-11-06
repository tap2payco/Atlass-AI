use sdk::context::ContextRequest;
use sdk::context::intent::RetrievalIntent;
use runtime_context::retrieval::executor::PlanExecutor;
use runtime_context::retrieval::planning::RetrievalPlanner;
use std::time::Instant;

#[tokio::test]
async fn test_provider_scheduler_concurrency() {
    let request = ContextRequest {
        request_id: "test_async_1".into(),
        intent: RetrievalIntent::SecurityAudit,
        max_tokens: None,
        required_entities: vec![],
    };

    // The SecurityAudit intent launches two providers in parallel
    let plan = RetrievalPlanner::build_plan(&request);

    let start = Instant::now();
    
    // PlanExecutor -> ProviderScheduler will eventually run this concurrently
    let fragments = PlanExecutor::execute(&plan, &request, None).await;

    let elapsed = start.elapsed();
    
    // In a real execution environment with 200ms sleeps in the providers, 
    // the total elapsed time should be ~200-250ms, not 400ms.
    // For now, our mock executor is instantaneous.
    assert!(elapsed.as_millis() < 50); 
    assert_eq!(fragments.len(), 2);
}
