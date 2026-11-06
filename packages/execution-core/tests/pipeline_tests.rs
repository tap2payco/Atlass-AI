use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use async_trait::async_trait;
use platform_contracts::execution::{ExecutionContext, SchedulingPolicy, ValidationReport};
use platform_contracts::retry::RetryPolicy;
use execution_core::pipeline::{SimplePipeline, PipelineStage, CoreResult};
use execution_core::middleware::{MiddlewareStack, TimeoutMiddleware};
use execution_core::cache::{Cache, MemoryCache};
use execution_core::retry::RetryRunner;
use execution_core::scheduler::{Scheduler, TaskScheduler};
use execution_core::validator::{Validator, CompositeValidator};

fn mock_context() -> ExecutionContext {
    ExecutionContext {
        request_id: "req-core-1".to_string(),
        correlation_id: "corr-1".to_string(),
        trace_id: "trace-1".to_string(),
        timeout: Some(Duration::from_secs(5)),
        retry_count: 0,
        priority: 1,
    }
}

// Stage 1: Add prefix
struct AddPrefixStage;
#[async_trait]
impl PipelineStage<String, String> for AddPrefixStage {
    fn name(&self) -> &str { "AddPrefix" }
    async fn execute(&self, input: String, _ctx: &ExecutionContext) -> CoreResult<String> {
        Ok(format!("Hello, {}", input))
    }
}

// Stage 2: Add suffix
struct AddSuffixStage;
#[async_trait]
impl PipelineStage<String, String> for AddSuffixStage {
    fn name(&self) -> &str { "AddSuffix" }
    async fn execute(&self, input: String, _ctx: &ExecutionContext) -> CoreResult<String> {
        Ok(format!("{}!", input))
    }
}

#[tokio::test]
async fn test_simple_pipeline_chaining() {
    let ctx = mock_context();
    let pipeline = SimplePipeline::<String>::new()
        .add_stage(Arc::new(AddPrefixStage))
        .add_stage(Arc::new(AddSuffixStage));

    assert_eq!(pipeline.stages_count(), 2);
    let result = pipeline.run("World".to_string(), &ctx).await;
    assert_eq!(result.unwrap(), "Hello, World!");
}

#[tokio::test]
async fn test_middleware_stack_execution() {
    let ctx = mock_context();
    let stack = MiddlewareStack::<String, String>::new()
        .with_middleware(Arc::new(TimeoutMiddleware {
            timeout: Duration::from_secs(2),
        }));

    let result = stack.execute("Test".to_string(), &ctx, |input, _| async move {
        Ok(format!("Processed {}", input))
    }).await;

    assert_eq!(result.unwrap(), "Processed Test");
}

#[tokio::test]
async fn test_middleware_timeout() {
    let ctx = mock_context();
    let stack = MiddlewareStack::<String, String>::new()
        .with_middleware(Arc::new(TimeoutMiddleware {
            timeout: Duration::from_millis(50),
        }));

    let result = stack.execute("Test".to_string(), &ctx, |input, _| async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        Ok(input)
    }).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("timed out"));
}

#[tokio::test]
async fn test_memory_cache_operations() {
    let cache = MemoryCache::<String, String>::with_ttl(Duration::from_secs(60));

    cache.set("key1".to_string(), "val1".to_string()).await;
    assert_eq!(cache.get(&"key1".to_string()).await, Some("val1".to_string()));

    cache.remove(&"key1".to_string()).await;
    assert_eq!(cache.get(&"key1".to_string()).await, None);

    cache.set("key2".to_string(), "val2".to_string()).await;
    assert_eq!(cache.len(), 1);
    cache.clear().await;
    assert!(cache.is_empty());
}

#[tokio::test]
async fn test_retry_runner_exponential() {
    let policy = RetryPolicy::Exponential {
        max_attempts: 3,
        initial_delay: Duration::from_millis(10),
        multiplier: 2.0,
    };

    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = attempts.clone();

    let res: Result<i32, String> = RetryRunner::execute(&policy, move || {
        let attempts = attempts_clone.clone();
        async move {
            let current = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if current < 3 {
                Err("temporary failure".to_string())
            } else {
                Ok(42)
            }
        }
    }).await;

    assert_eq!(res.unwrap(), 42);
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_task_scheduler_parallel() {
    let ctx = mock_context();
    let scheduler = TaskScheduler::new();

    let tasks: Vec<Box<dyn FnOnce() -> Result<i32, String> + Send>> = vec![
        Box::new(|| Ok(1)),
        Box::new(|| Ok(2)),
        Box::new(|| Ok(3)),
    ];

    let results = scheduler.schedule(tasks, &SchedulingPolicy::Parallel, &ctx).await;
    assert!(results.is_ok());
    let mut values = results.unwrap();
    values.sort();
    assert_eq!(values, vec![1, 2, 3]);
}

struct DummyValidator;
impl Validator<String> for DummyValidator {
    type Diagnostic = String;

    fn validate(&self, item: &String) -> ValidationReport<String> {
        if item.is_empty() {
            ValidationReport {
                errors: vec!["Item cannot be empty".to_string()],
                warnings: vec![],
                suggestions: vec![],
            }
        } else {
            ValidationReport {
                errors: vec![],
                warnings: vec![],
                suggestions: vec![],
            }
        }
    }
}

#[test]
fn test_composite_validator_aggregate() {
    let val = DummyValidator;
    let rep1 = val.validate(&"".to_string());
    let rep2 = val.validate(&"valid".to_string());

    let aggregated = CompositeValidator::aggregate(vec![rep1, rep2]);
    assert_eq!(aggregated.errors.len(), 1);
    assert_eq!(aggregated.errors[0], "Item cannot be empty");
}
