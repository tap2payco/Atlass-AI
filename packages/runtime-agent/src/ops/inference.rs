use std::sync::Arc;

use async_trait::async_trait;
use sdk::model::ModelRuntime;
use shared::model::{CapabilityProfile, ExecutionPolicy, ModelCapability, ModelRequest};

use crate::conversation::{awaiting_model, fit_tool_outputs, model_view};
use crate::ops::CompactionSettings;
use crate::error::AgentError;
use crate::operation::{Operation, OperationResult, StepContext, StopCause};

/// Model settings for the agent's inference calls.
#[derive(Debug, Clone)]
pub struct InferenceSettings {
    pub capability: ModelCapability,
    pub profile: CapabilityProfile,
    pub policy: ExecutionPolicy,
    pub system_prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for InferenceSettings {
    fn default() -> Self {
        Self {
            capability: ModelCapability::CodeGeneration,
            profile: CapabilityProfile::Balanced,
            policy: ExecutionPolicy::PreferLocal,
            system_prompt: "You are Atlas, an autonomous software engineering agent. \
                            Use the available tools when they help; answer directly otherwise."
                .into(),
            max_tokens: 2048,
            temperature: 0.2,
        }
    }
}

/// Calls the model when the conversation ends with a user message (a new
/// request or tool results). Continues if the model asked for tools; stops
/// with `EndTurn` otherwise. Should be the last operation.
pub struct InferenceOperation {
    model: Arc<dyn ModelRuntime>,
    settings: InferenceSettings,
    /// When set, the request is fitted into this context window.
    context: Option<CompactionSettings>,
}

impl InferenceOperation {
    pub fn new(model: Arc<dyn ModelRuntime>, settings: InferenceSettings) -> Self {
        Self { model, settings, context: None }
    }

    /// Shrink tool outputs in each request so it fits the window.
    pub fn with_context(mut self, context: Option<CompactionSettings>) -> Self {
        self.context = context;
        self
    }

    fn build_request(&self, ctx: &StepContext<'_>) -> ModelRequest {
        let mut system = self.settings.system_prompt.clone();
        for part in ctx.prompt_parts {
            system.push_str("\n\n");
            system.push_str(part);
        }
        // The model sees the compacted view: the latest summary, then newer
        // messages, with older tool outputs shrunk if they would overflow.
        let mut view = model_view(&ctx.session.messages);
        if let Some(context) = &self.context {
            let budget = context.message_budget(system.len(), ctx.tools, self.settings.max_tokens);
            view = fit_tool_outputs(view, budget);
        }
        let mut request =
            ModelRequest::conversation(self.settings.capability.clone(), view)
                .with_system_prompt(system)
                .with_tools(ctx.tools.to_vec());
        request.profile = self.settings.profile.clone();
        request.policy = self.settings.policy.clone();
        request.max_tokens = self.settings.max_tokens;
        request.temperature = self.settings.temperature;
        request.trace_id = ctx.session.id.to_string();
        request
    }
}

#[async_trait]
impl Operation for InferenceOperation {
    fn name(&self) -> &'static str {
        "inference"
    }

    async fn run(&self, ctx: &StepContext<'_>) -> Result<OperationResult, AgentError> {
        if !awaiting_model(&ctx.session.messages) {
            return Ok(OperationResult::NotApplicable);
        }

        let response = self.model.execute_inference(self.build_request(ctx)).await?;
        let wants_tools = !response.tool_calls.is_empty();
        // An empty response is still stored, so the conversation ends with an
        // assistant turn and the loop cannot call the model again forever.
        let message = response.to_message();

        Ok(if wants_tools {
            OperationResult::continue_with(vec![message])
        } else {
            OperationResult::stop_with(vec![message], StopCause::EndTurn)
        })
    }
}
