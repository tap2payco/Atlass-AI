use async_trait::async_trait;
use platform_contracts::execution::{ExecutionContext, SchedulingPolicy};

#[async_trait]
pub trait Scheduler<T, R>: Send + Sync {
    async fn schedule(&self, tasks: Vec<T>, policy: &SchedulingPolicy, context: &ExecutionContext) -> Result<Vec<R>, String>;
}

pub struct TaskScheduler;

impl TaskScheduler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<T: Send + 'static, R: Send + 'static> Scheduler<T, R> for TaskScheduler
where
    T: FnOnce() -> Result<R, String> + Send,
{
    async fn schedule(
        &self,
        tasks: Vec<T>,
        policy: &SchedulingPolicy,
        _context: &ExecutionContext,
    ) -> Result<Vec<R>, String> {
        match policy {
            SchedulingPolicy::Sequential => {
                let mut results = Vec::with_capacity(tasks.len());
                for task in tasks {
                    results.push(task()?);
                }
                Ok(results)
            }
            _ => {
                let mut join_set = tokio::task::JoinSet::new();
                for task in tasks {
                    join_set.spawn_blocking(task);
                }

                let mut results = Vec::new();
                while let Some(res) = join_set.join_next().await {
                    match res {
                        Ok(Ok(val)) => results.push(val),
                        Ok(Err(err)) => return Err(err),
                        Err(join_err) => return Err(join_err.to_string()),
                    }
                }
                Ok(results)
            }
        }
    }
}
