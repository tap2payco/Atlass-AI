//! Cognitive-state operations (agent plan step 2), after
//! `docs/09-ai/cognitive-state-machine.md`.
//!
//! The doc's macro states are mapped onto one agent turn. Each state is an
//! operation whose precondition is read from the persisted conversation, like
//! everything in `conversation.rs`; nothing is held in memory, so a resumed
//! turn is in the same state it was in before. A state never appends
//! messages: while its precondition holds it adds a section to the system
//! prompt of the next model request, and it can choose the `ModelCapability`
//! that request is routed to.
//!
//! | State         | Holds when the next model call…                               |
//! |---------------|---------------------------------------------------------------|
//! | Understanding | answers a new request (the last message is the kickoff)       |
//! | Planning      | comes before the model has written any text this turn         |
//! | Execution     | follows tool results, after the model wrote a plan this turn  |
//! | Review        | follows tool results (a subtask output, maybe the last one)   |
//! | Reflection    | follows tool results in a long turn, or one stuck on errors   |
//!
//! Idle, Research, Reasoning, Learning and Memory Update are not separate
//! operations: Idle is "no pending request", Research is folded into the
//! Understanding prompt (use read tools when context is missing), Reasoning
//! is a routing choice (`ModelCapability::Reasoning`), and Learning/Memory
//! Update need a memory write path that does not exist yet.
//!
//! Off by default; enabled with `AgentBuilder::cognitive`.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use shared::model::{Message, ModelCapability, Role};

use crate::conversation::{assistant_messages_in_turn, awaiting_model, current_turn, is_kickoff};
use crate::error::AgentError;
use crate::operation::{Operation, OperationResult, StepContext};
use crate::session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CognitiveState {
    Understanding,
    Planning,
    Execution,
    Review,
    Reflection,
}

impl CognitiveState {
    pub const ALL: [CognitiveState; 5] =
        [Self::Understanding, Self::Planning, Self::Execution, Self::Review, Self::Reflection];

    pub fn name(self) -> &'static str {
        match self {
            Self::Understanding => "cognition.understanding",
            Self::Planning => "cognition.planning",
            Self::Execution => "cognition.execution",
            Self::Review => "cognition.review",
            Self::Reflection => "cognition.reflection",
        }
    }
}

/// Which states are on, when Reflection starts, and where each state's
/// model calls are routed.
#[derive(Debug, Clone)]
pub struct CognitionSettings {
    pub states: Vec<CognitiveState>,
    /// Reflection holds once the turn has this many model responses.
    pub reflect_after: usize,
    /// Reflection holds once this many tool-result rounds in the turn had an
    /// error (the doc's "review deadlock" after 3 cycles).
    pub review_deadlock: usize,
    /// Capability for a model call made in a state. States without an entry
    /// keep the agent's configured capability.
    pub routing: BTreeMap<CognitiveState, ModelCapability>,
}

impl Default for CognitionSettings {
    /// Every state on, no routing (one model for everything).
    fn default() -> Self {
        Self {
            states: CognitiveState::ALL.to_vec(),
            reflect_after: 8,
            review_deadlock: 3,
            routing: BTreeMap::new(),
        }
    }
}

impl CognitionSettings {
    /// The doc's model split: reasoning model for Understanding, Planning,
    /// Review and Reflection; code model for Execution.
    pub fn with_doc_routing(mut self) -> Self {
        use CognitiveState::*;
        self.routing = BTreeMap::from([
            (Understanding, ModelCapability::Reasoning),
            (Planning, ModelCapability::Reasoning),
            (Execution, ModelCapability::CodeGeneration),
            (Review, ModelCapability::Reasoning),
            (Reflection, ModelCapability::Reasoning),
        ]);
        self
    }

    /// Whether `state`'s precondition holds for the next model call.
    pub fn holds(&self, state: CognitiveState, messages: &[Message]) -> bool {
        if !awaiting_model(messages) {
            return false;
        }
        match state {
            CognitiveState::Understanding => messages.last().is_some_and(is_kickoff),
            CognitiveState::Planning => !turn_has_assistant_text(messages),
            CognitiveState::Execution => last_is_tool_results(messages) && turn_has_assistant_text(messages),
            CognitiveState::Review => last_is_tool_results(messages),
            CognitiveState::Reflection => {
                last_is_tool_results(messages)
                    && (assistant_messages_in_turn(messages) >= self.reflect_after
                        || error_rounds_in_turn(messages) >= self.review_deadlock)
            }
        }
    }

    /// Enabled states whose precondition holds, in `CognitiveState::ALL` order.
    pub fn active(&self, messages: &[Message]) -> Vec<CognitiveState> {
        CognitiveState::ALL
            .into_iter()
            .filter(|s| self.states.contains(s) && self.holds(*s, messages))
            .collect()
    }

    /// The state that picks the model for the next call: the first active one
    /// in the order Understanding, Planning, Reflection, Execution, Review.
    pub fn dominant(&self, messages: &[Message]) -> Option<CognitiveState> {
        use CognitiveState::*;
        let active = self.active(messages);
        [Understanding, Planning, Reflection, Execution, Review].into_iter().find(|s| active.contains(s))
    }

    /// The capability the next model call should use, if routing says so.
    pub fn route(&self, messages: &[Message]) -> Option<ModelCapability> {
        self.dominant(messages).and_then(|s| self.routing.get(&s).cloned())
    }

    /// The system-prompt section `state` adds.
    pub fn prompt(&self, state: CognitiveState, messages: &[Message]) -> String {
        match state {
            CognitiveState::Understanding => "## Understanding\n\
                Before acting, work out what the user is asking for and which part of the \
                project it concerns. If you lack the context to be sure, read the relevant \
                files first instead of guessing."
                .into(),
            CognitiveState::Planning => "## Planning\n\
                Unless the request is trivial, start your reply with a short numbered plan \
                of the steps you will take, then begin the first step."
                .into(),
            CognitiveState::Execution => "## Execution\n\
                Carry out your plan one step at a time. Say which step you are on, and \
                change the plan out loud if a result shows it was wrong."
                .into(),
            CognitiveState::Review => {
                if latest_results_have_error(messages) {
                    "## Review\n\
                     A tool call just failed. Revision required: read the error, fix the \
                     cause and retry, or explain why the step cannot be done."
                        .into()
                } else {
                    "## Review\n\
                     Check the tool output above. If the request is not done yet, continue. \
                     If it is, verify the result against what the user asked before giving \
                     your final answer, and say what you checked."
                        .into()
                }
            }
            CognitiveState::Reflection => format!(
                "## Reflection\n\
                 This turn has taken {} model responses and {} failed tool rounds. Stop and \
                 reflect: is the plan still right, are you repeating an approach that keeps \
                 failing? Either change approach or give your final answer now, stating what \
                 was verified and what is left.",
                assistant_messages_in_turn(messages),
                error_rounds_in_turn(messages)
            ),
        }
    }
}

/// The last message carries tool results (not a new request).
pub fn last_is_tool_results(messages: &[Message]) -> bool {
    messages.last().is_some_and(|m| {
        m.role == Role::User && !is_kickoff(m) && m.tool_result_blocks().next().is_some()
    })
}

/// Whether the model has written any text in the current turn.
pub fn turn_has_assistant_text(messages: &[Message]) -> bool {
    current_turn(messages)
        .iter()
        .any(|m| m.role == Role::Assistant && !m.text().trim().is_empty())
}

/// Tool-result messages in the current turn with at least one error.
pub fn error_rounds_in_turn(messages: &[Message]) -> usize {
    current_turn(messages)
        .iter()
        .filter(|m| m.tool_result_blocks().any(|r| r.is_error))
        .count()
}

fn latest_results_have_error(messages: &[Message]) -> bool {
    messages.last().is_some_and(|m| m.tool_result_blocks().any(|r| r.is_error))
}

/// One cognitive state as an operation: contributes its prompt section
/// while its precondition holds; never appends messages itself.
pub struct CognitiveOperation {
    state: CognitiveState,
    settings: Arc<CognitionSettings>,
}

impl CognitiveOperation {
    pub fn new(state: CognitiveState, settings: Arc<CognitionSettings>) -> Self {
        Self { state, settings }
    }

    /// One operation per enabled state.
    pub fn all(settings: Arc<CognitionSettings>) -> Vec<Arc<dyn Operation>> {
        CognitiveState::ALL
            .into_iter()
            .filter(|s| settings.states.contains(s))
            .map(|s| Arc::new(Self::new(s, settings.clone())) as Arc<dyn Operation>)
            .collect()
    }
}

#[async_trait]
impl Operation for CognitiveOperation {
    fn name(&self) -> &'static str {
        self.state.name()
    }

    fn prompt_parts(&self, session: &Session) -> Vec<String> {
        if self.settings.holds(self.state, &session.messages) {
            vec![self.settings.prompt(self.state, &session.messages)]
        } else {
            Vec::new()
        }
    }

    async fn run(&self, _ctx: &StepContext<'_>) -> Result<OperationResult, AgentError> {
        Ok(OperationResult::NotApplicable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shared::model::{ContentBlock, ToolCall, ToolResult};
    use CognitiveState::*;

    fn call(id: &str) -> Message {
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolCall(ToolCall { id: id.into(), name: "t".into(), arguments: json!({}) })],
        }
    }

    fn result(id: &str, is_error: bool) -> Message {
        Message::tool_results([ToolResult { call_id: id.into(), content: "out".into(), is_error }])
    }

    #[test]
    fn kickoff_is_understanding_and_planning() {
        let s = CognitionSettings::default();
        let messages = vec![Message::user("fix the bug")];
        assert_eq!(s.active(&messages), [Understanding, Planning]);
        assert_eq!(s.dominant(&messages), Some(Understanding));
    }

    #[test]
    fn research_without_a_plan_keeps_planning_but_not_understanding() {
        let s = CognitionSettings::default();
        let messages = vec![Message::user("fix"), call("a"), result("a", false)];
        assert_eq!(s.active(&messages), [Planning, Review]);
    }

    #[test]
    fn tool_results_after_a_plan_are_execution_and_review() {
        let s = CognitionSettings::default();
        let mut plan = call("a");
        plan.content.insert(0, ContentBlock::Text { text: "1. read 2. edit".into() });
        let messages = vec![Message::user("fix"), plan, result("a", false)];
        assert_eq!(s.active(&messages), [Execution, Review]);
        assert!(s.prompt(Review, &messages).contains("final answer"));
    }

    #[test]
    fn failed_rounds_lead_to_revision_then_reflection() {
        let s = CognitionSettings::default();
        let mut messages = vec![Message::user("fix")];
        for id in ["a", "b"] {
            messages.push(call(id));
            messages.push(result(id, true));
        }
        assert!(s.prompt(Review, &messages).contains("Revision required"));
        assert!(!s.holds(Reflection, &messages));
        messages.push(call("c"));
        messages.push(result("c", true));
        assert!(s.holds(Reflection, &messages));
        assert_eq!(s.dominant(&messages), Some(Planning), "no plan text was ever written");
    }

    #[test]
    fn nothing_holds_when_the_model_is_not_awaited() {
        let s = CognitionSettings::default();
        let messages = vec![Message::user("hi"), Message::assistant("hello")];
        assert!(s.active(&messages).is_empty());
        assert_eq!(s.route(&messages), None);
    }

    #[test]
    fn routing_follows_the_dominant_state() {
        let s = CognitionSettings::default().with_doc_routing();
        assert_eq!(s.route(&[Message::user("go")]), Some(ModelCapability::Reasoning));
        let mut plan = call("a");
        plan.content.insert(0, ContentBlock::Text { text: "plan".into() });
        let messages = vec![Message::user("go"), plan, result("a", false)];
        assert_eq!(s.route(&messages), Some(ModelCapability::CodeGeneration));
        assert_eq!(CognitionSettings::default().route(&messages), None);
    }
}
