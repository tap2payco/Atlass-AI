use async_trait::async_trait;
use std::sync::Arc;
use platform_contracts::execution::ExecutionContext;

pub type CoreResult<T> = Result<T, String>;

#[async_trait]
pub trait PipelineStage<I, O>: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, input: I, context: &ExecutionContext) -> CoreResult<O>;
}

pub struct SimplePipeline<T> {
    stages: Vec<Arc<dyn PipelineStage<T, T>>>,
}

impl<T: Send + Sync + 'static> SimplePipeline<T> {
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    pub fn add_stage(mut self, stage: Arc<dyn PipelineStage<T, T>>) -> Self {
        self.stages.push(stage);
        self
    }

    pub async fn run(&self, initial_input: T, context: &ExecutionContext) -> CoreResult<T> {
        let mut current = initial_input;
        for stage in &self.stages {
            current = stage.execute(current, context).await?;
        }
        Ok(current)
    }

    pub fn stages_count(&self) -> usize {
        self.stages.len()
    }
}

impl<T: Send + Sync + 'static> Default for SimplePipeline<T> {
    fn default() -> Self {
        Self::new()
    }
}
