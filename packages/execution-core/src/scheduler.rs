use async_trait::async_trait;
use platform_contracts::execution::{ExecutionContext, SchedulingPolicy};

#[async_trait]
pub trait Scheduler<T, R>: Send + Sync {
    async fn schedule(&self, task: &T, policy: &SchedulingPolicy, context: &ExecutionContext) -> Result<R, String>;
}
