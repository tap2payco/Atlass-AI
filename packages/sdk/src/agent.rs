use async_trait::async_trait;
use shared::id::{AgentId, TaskId};
use shared::types::Result;

#[async_trait]
pub trait AgentRuntime: Send + Sync {
    async fn invoke(&self, agent_id: AgentId, task_id: TaskId, context: String) -> Result<String>;
    async fn get_status(&self, agent_id: AgentId) -> Result<String>;
    async fn terminate(&self, agent_id: AgentId) -> Result<()>;
}
