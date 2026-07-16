use async_trait::async_trait;
use shared::types::Result;

#[async_trait]
pub trait PipelineStage<I, O>: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, input: I) -> Result<O>;
}

pub struct Pipeline<I, O> {
    // A pipeline could store a series of boxed stages, but with varying intermediate types, 
    // it's typically easier to compose them structurally or via a macro. 
    // For now, this is a placeholder marker that will be expanded.
    _marker: std::marker::PhantomData<(I, O)>,
}
