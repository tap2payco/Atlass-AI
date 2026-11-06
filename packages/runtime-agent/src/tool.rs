//! Tools the agent can call.
//!
//! This is the minimal contract the agent loop needs. Approval, sandboxing and
//! parallel execution (roadmap steps 3–4 in the reference study) will wrap it
//! rather than change it.

use async_trait::async_trait;
use shared::model::ToolSpec;

#[async_trait]
pub trait Tool: Send + Sync {
    /// Name, description and argument schema shown to the model. The name must
    /// be unique among the agent's tools.
    fn spec(&self) -> ToolSpec;

    /// Run the tool. `Ok` text and `Err` text are both returned to the model;
    /// `Err` is marked as an error result so the model can recover.
    async fn call(&self, arguments: serde_json::Value) -> Result<String, String>;
}
