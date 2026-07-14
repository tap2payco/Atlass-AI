use async_trait::async_trait;
use shared::id::TaskId;
use shared::types::Result;

#[async_trait]
pub trait ExecutionRuntime: Send + Sync {
    async fn schedule_task(&self, task_id: TaskId, payload: String) -> Result<()>;
    async fn get_task_status(&self, task_id: TaskId) -> Result<String>;
    async fn cancel_task(&self, task_id: TaskId) -> Result<()>;
}
