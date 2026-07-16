use async_trait::async_trait;
use platform_contracts::execution::ExecutionContext;

#[async_trait]
pub trait Executor<T, R>: Send + Sync {
    async fn execute(&self, plan: &T, context: &ExecutionContext) -> Result<R, String>;
}
