//! Context compaction (after Goose's `CompactionOperation`).
//!
//! When a new user request arrives and the conversation the model would see
//! is over `threshold × context_tokens`, the model is asked to summarise
//! everything before that request. The summary is appended as a
//! `ContentBlock::Summary` covering those messages; from then on the model
//! sees the summary followed by the newer messages (`model_view`). Stored
//! history is append-only, so nothing is lost on disk, and summaries chain:
//! a later summary is written from the earlier one plus what followed.
//!
//! Summaries are only written at the start of a request, never in the middle
//! of a tool loop. Within a loop, `InferenceOperation` instead shrinks older
//! tool outputs in what it sends (`fit_tool_outputs`). If the summary call fails, the turn continues uncompacted and
//! compaction is not retried for that request.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sdk::model::ModelRuntime;
use shared::model::{Message, ModelCapability, ModelRequest, ToolSpec};

use crate::conversation::{
    awaiting_model, estimate_tokens, fit_tool_outputs, is_kickoff, latest_summary, model_view,
};
use crate::error::AgentError;
use crate::operation::{Operation, OperationResult, StepContext};

#[derive(Debug, Clone, PartialEq)]
pub struct CompactionSettings {
    /// The model's context window, in tokens.
    pub context_tokens: usize,
    /// Compact once the estimated request size passes this fraction of the window.
    pub threshold: f32,
    /// Capability used for the summary call.
    pub capability: ModelCapability,
    pub max_summary_tokens: u32,
}

impl CompactionSettings {
    /// Tokens left for messages once the system prompt, the tool specs and
    /// room for a `reply_tokens` reply are taken out of the window.
    pub fn message_budget(&self, system_chars: usize, tools: &[ToolSpec], reply_tokens: u32) -> usize {
        let tools_chars: usize = tools
            .iter()
            .map(|t| t.name.len() + t.description.len() + t.input_schema.to_string().len())
            .sum();
        self.context_tokens
            .saturating_sub((system_chars + tools_chars) / 4)
            .saturating_sub(reply_tokens as usize)
    }
}

impl Default for CompactionSettings {
    fn default() -> Self {
        Self {
            context_tokens: 8_192,
            threshold: 0.75,
            capability: ModelCapability::Summarization,
            max_summary_tokens: 1_024,
        }
    }
}

const SUMMARIZER_PROMPT: &str = "You write handover notes for a software engineering agent whose \
    conversation history is being shortened. The agent will continue the work using only your notes \
    and the newest messages.";

const SUMMARY_REQUEST: &str = "Summarise the conversation above for the agent that will continue it. \
    Include: what the user asked for and any constraints or preferences they stated; decisions made; \
    files read, created or changed (with paths); commands run and their outcomes; and anything still \
    open or unresolved. Be specific and concise. Write only the notes.";

pub struct CompactionOperation {
    model: Arc<dyn ModelRuntime>,
    settings: CompactionSettings,
    /// Index of the request whose compaction failed, so it is not retried
    /// on every step of that request's tool loop.
    failed_for: Mutex<Option<usize>>,
}

impl CompactionOperation {
    pub fn new(model: Arc<dyn ModelRuntime>, settings: CompactionSettings) -> Self {
        Self { model, settings, failed_for: Mutex::new(None) }
    }

    fn over_budget(&self, view: &[Message], system_parts: &[String]) -> bool {
        let system: usize = system_parts.iter().map(|p| p.len() / 4).sum();
        let estimate = estimate_tokens(view) + system;
        estimate as f32 > self.settings.context_tokens as f32 * self.settings.threshold
    }
}

#[async_trait]
impl Operation for CompactionOperation {
    fn name(&self) -> &'static str {
        "compaction"
    }

    async fn run(&self, ctx: &StepContext<'_>) -> Result<OperationResult, AgentError> {
        let messages = &ctx.session.messages;
        if !awaiting_model(messages) {
            return Ok(OperationResult::NotApplicable);
        }
        let Some(kickoff) = messages.iter().rposition(is_kickoff) else {
            return Ok(OperationResult::NotApplicable);
        };
        let covered = latest_summary(messages).map(|(_, covers)| covers).unwrap_or(0);
        if kickoff <= covered || *self.failed_for.lock().unwrap() == Some(kickoff) {
            return Ok(OperationResult::NotApplicable);
        }
        if !self.over_budget(&model_view(messages), ctx.prompt_parts) {
            return Ok(OperationResult::NotApplicable);
        }

        // Summarise everything before the current request, as the model sees it
        // (which includes any earlier summary), with old tool outputs shrunk
        // so the summary request itself fits the window.
        let budget = self
            .settings
            .message_budget(SUMMARIZER_PROMPT.len() + SUMMARY_REQUEST.len(), &[], self.settings.max_summary_tokens);
        let mut to_summarise = fit_tool_outputs(model_view(&messages[..kickoff]), budget);
        to_summarise.push(Message::user(SUMMARY_REQUEST));
        let mut request = ModelRequest::conversation(self.settings.capability.clone(), to_summarise)
            .with_system_prompt(SUMMARIZER_PROMPT);
        request.max_tokens = self.settings.max_summary_tokens;
        request.temperature = 0.1;
        request.trace_id = ctx.session.id.to_string();

        let summary = match self.model.execute_inference(request).await {
            Ok(response) if !response.content.trim().is_empty() => response.content.trim().to_string(),
            Ok(_) => {
                tracing::warn!("compaction returned an empty summary; continuing without it");
                *self.failed_for.lock().unwrap() = Some(kickoff);
                return Ok(OperationResult::NotApplicable);
            }
            Err(e) => {
                tracing::warn!(error = %e, "compaction failed; continuing without it");
                *self.failed_for.lock().unwrap() = Some(kickoff);
                return Ok(OperationResult::NotApplicable);
            }
        };

        Ok(OperationResult::continue_with(vec![Message::summary(summary, kickoff)]))
    }
}
