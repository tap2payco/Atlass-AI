use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use shared::model::{Message, ToolResult, ToolSpec};

use crate::conversation::open_tool_calls;
use crate::error::AgentError;
use crate::events::AgentEvent;
use crate::operation::{Operation, OperationResult, StepContext};
use crate::tool::Tool;

/// Runs tool calls the model made that have no result yet, then continues so
/// the model sees the results. Calls run one at a time, in order.
pub struct ToolExecutionOperation {
    tools: HashMap<String, Arc<dyn Tool>>,
    specs: Vec<ToolSpec>,
}

impl ToolExecutionOperation {
    /// Fails if two tools share a name.
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Result<Self, AgentError> {
        let mut by_name = HashMap::new();
        let mut specs = Vec::new();
        for tool in tools {
            let spec = tool.spec();
            if by_name.insert(spec.name.clone(), tool).is_some() {
                return Err(AgentError::Config(format!("duplicate tool name '{}'", spec.name)));
            }
            specs.push(spec);
        }
        Ok(Self { tools: by_name, specs })
    }
}

#[async_trait]
impl Operation for ToolExecutionOperation {
    fn name(&self) -> &'static str {
        "tool_execution"
    }

    fn tools(&self) -> Vec<ToolSpec> {
        self.specs.clone()
    }

    async fn run(&self, ctx: &StepContext<'_>) -> Result<OperationResult, AgentError> {
        let open = open_tool_calls(&ctx.session.messages);
        if open.is_empty() {
            return Ok(OperationResult::NotApplicable);
        }

        let mut results = Vec::with_capacity(open.len());
        for call in open {
            ctx.events.emit(AgentEvent::ToolStarted {
                call_id: call.id.clone(),
                name: call.name.clone(),
            });
            let outcome = match self.tools.get(&call.name) {
                Some(tool) => tool.call(call.arguments.clone()).await,
                None => Err(format!("Unknown tool '{}'.", call.name)),
            };
            let (content, is_error) = match outcome {
                Ok(text) => (text, false),
                Err(text) => (text, true),
            };
            results.push(ToolResult { call_id: call.id.clone(), content, is_error });
        }

        Ok(OperationResult::continue_with(vec![Message::tool_results(results)]))
    }
}
