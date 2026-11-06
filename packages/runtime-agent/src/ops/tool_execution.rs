//! The tool runtime: runs the model's pending tool calls.
//!
//! Scheduling follows Codex's rule (read-only tools in parallel, anything that
//! changes state alone) while keeping the model's call order: consecutive
//! read-only calls form one parallel batch, and every other call runs by
//! itself, after everything before it has finished.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use shared::model::{Message, ToolCall, ToolResult, ToolSpec};
use tokio::task::JoinSet;

use crate::conversation::open_tool_calls;
use crate::error::AgentError;
use crate::events::{AgentEvent, EventSink};
use crate::operation::{Operation, OperationResult, StepContext};
use crate::tool::{truncate_output, Tool, ToolEffect, ToolLimits};

pub struct ToolExecutionOperation {
    tools: HashMap<String, Arc<dyn Tool>>,
    specs: Vec<ToolSpec>,
    limits: ToolLimits,
}

impl ToolExecutionOperation {
    /// Fails if two tools share a name.
    pub fn new(tools: Vec<Arc<dyn Tool>>, limits: ToolLimits) -> Result<Self, AgentError> {
        let mut by_name = HashMap::new();
        let mut specs = Vec::new();
        for tool in tools {
            let spec = tool.spec();
            if by_name.insert(spec.name.clone(), tool).is_some() {
                return Err(AgentError::Config(format!("duplicate tool name '{}'", spec.name)));
            }
            specs.push(spec);
        }
        Ok(Self { tools: by_name, specs, limits })
    }

    fn is_parallel_safe(&self, call: &ToolCall) -> bool {
        self.tools.get(&call.name).is_some_and(|t| t.effect() == ToolEffect::ReadOnly)
    }

    /// Run `calls` concurrently and return their results in the same order.
    async fn run_batch(&self, calls: &[ToolCall], events: &EventSink) -> Vec<ToolResult> {
        if let [call] = calls {
            return vec![self.run_one(call, events).await];
        }
        let mut set = JoinSet::new();
        for (index, call) in calls.iter().enumerate() {
            let tool = self.tools.get(&call.name).cloned();
            let call = call.clone();
            let limits = self.limits;
            events.emit(AgentEvent::ToolStarted { call_id: call.id.clone(), name: call.name.clone() });
            set.spawn(async move { (index, execute(tool, &call, limits).await) });
        }
        let mut results: Vec<Option<ToolResult>> = vec![None; calls.len()];
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((index, result)) => results[index] = Some(result),
                // A panicking tool must not take the agent down; its slot is filled below.
                Err(e) => tracing::warn!(error = %e, "tool task failed"),
            }
        }
        results
            .into_iter()
            .zip(calls)
            .map(|(result, call)| {
                result.unwrap_or_else(|| error_result(call, "The tool crashed before returning a result."))
            })
            .collect()
    }

    async fn run_one(&self, call: &ToolCall, events: &EventSink) -> ToolResult {
        events.emit(AgentEvent::ToolStarted { call_id: call.id.clone(), name: call.name.clone() });
        execute(self.tools.get(&call.name).cloned(), call, self.limits).await
    }
}

async fn execute(tool: Option<Arc<dyn Tool>>, call: &ToolCall, limits: ToolLimits) -> ToolResult {
    let Some(tool) = tool else {
        return error_result(call, &format!("Unknown tool '{}'.", call.name));
    };
    match tokio::time::timeout(limits.timeout, tool.call(call.arguments.clone())).await {
        Ok(Ok(text)) => ToolResult {
            call_id: call.id.clone(),
            content: truncate_output(&text, limits.max_output_chars),
            is_error: false,
        },
        Ok(Err(text)) => error_result(call, &truncate_output(&text, limits.max_output_chars)),
        Err(_) => error_result(
            call,
            &format!("The tool timed out after {} seconds.", limits.timeout.as_secs()),
        ),
    }
}

fn error_result(call: &ToolCall, content: &str) -> ToolResult {
    ToolResult { call_id: call.id.clone(), content: content.to_string(), is_error: true }
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
        let open: Vec<ToolCall> = open_tool_calls(&ctx.session.messages).into_iter().cloned().collect();
        if open.is_empty() {
            return Ok(OperationResult::NotApplicable);
        }

        let mut results = Vec::with_capacity(open.len());
        let mut start = 0;
        while start < open.len() {
            let end = if self.is_parallel_safe(&open[start]) {
                start + open[start..].iter().take_while(|c| self.is_parallel_safe(c)).count()
            } else {
                start + 1
            };
            results.extend(self.run_batch(&open[start..end], ctx.events).await);
            start = end;
        }

        Ok(OperationResult::continue_with(vec![Message::tool_results(results)]))
    }
}
