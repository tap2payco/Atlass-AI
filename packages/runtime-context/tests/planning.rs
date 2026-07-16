use sdk::context::ContextRequest;
use sdk::context::intent::RetrievalIntent;
use runtime_context::retrieval::planning::RetrievalPlanner;

#[test]
fn test_security_audit_plan() {
    let request = ContextRequest {
        request_id: "test_1".into(),
        intent: RetrievalIntent::SecurityAudit,
        max_tokens: None,
        required_entities: vec![],
    };

    let plan = RetrievalPlanner::build_plan(&request);

    assert_eq!(plan.version, 1);
    assert_eq!(plan.steps.len(), 2);
    
    // Assert exactly what the SecurityAudit plan should be
    assert_eq!(plan.steps[0].provider, "repository");
    assert!(plan.steps[0].is_parallel);

    assert_eq!(plan.steps[1].provider, "git");
    assert!(plan.steps[1].is_parallel);
}

#[test]
fn test_bug_investigation_plan() {
    let request = ContextRequest {
        request_id: "test_2".into(),
        intent: RetrievalIntent::BugInvestigation,
        max_tokens: None,
        required_entities: vec![],
    };

    let plan = RetrievalPlanner::build_plan(&request);

    assert_eq!(plan.steps.len(), 2);
    
    // Assert exactly what the BugInvestigation plan should be
    assert_eq!(plan.steps[0].provider, "repository");
    assert!(plan.steps[0].is_parallel);

    assert_eq!(plan.steps[1].provider, "issue");
    assert!(!plan.steps[1].is_parallel);
}
