use async_trait::async_trait;
use platform_contracts::execution::ExecutionContext;
use crate::pipeline::CoreResult;

#[async_trait]
pub trait Middleware<I, O>: Send + Sync {
    async fn handle(&self, input: I, context: &ExecutionContext, next: &dyn Next<I, O>) -> CoreResult<O>;
}

#[async_trait]
pub trait Next<I, O>: Send + Sync {
    async fn run(&self, input: I, context: &ExecutionContext) -> CoreResult<O>;
}
