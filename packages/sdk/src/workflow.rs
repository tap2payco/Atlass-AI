use async_trait::async_trait;
use shared::id::WorkflowId;
use shared::types::Result;

#[async_trait]
pub trait WorkflowRuntime: Send + Sync {
    async fn start_workflow(&self, id: WorkflowId, payload: String) -> Result<()>;
    async fn get_status(&self, id: WorkflowId) -> Result<String>;
}
