use async_trait::async_trait;
use std::marker::PhantomData;
use platform_contracts::execution::ExecutionContext;

// In a real generic crate, Result would be standard std::result::Result or a custom generic Result,
// but for simplicity we assume the core crate has its own error type or uses a generic approach.
pub type CoreResult<T> = Result<T, String>;

#[async_trait]
pub trait PipelineStage<I, O>: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, input: I, context: &ExecutionContext) -> CoreResult<O>;
}

pub struct Pipeline<I, O> {
    _marker: PhantomData<(I, O)>,
}
