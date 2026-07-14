use async_trait::async_trait;
use shared::id::ToolId;
use shared::types::Result;

#[async_trait]
pub trait ToolRuntime: Send + Sync {
    async fn execute(&self, tool_id: ToolId, args: &str) -> Result<String>;
    async fn get_schema(&self, tool_id: ToolId) -> Result<String>;
}
