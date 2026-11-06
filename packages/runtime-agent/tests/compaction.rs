//! Context compaction: when it triggers, what the model sees afterwards,
//! chaining of summaries, and failure handling.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use platform_contracts::health::HealthState;
use runtime_agent::conversation::model_view;
use runtime_agent::{Agent, AgentError, CompactionSettings, EventSink, InMemorySessionStore, StopCause};
use sdk::model::ModelRuntime;
use shared::error::InferenceError;
use shared::model::{
    CircuitState, ExecutionMetadata, Message, ModelRequest, ModelResponse, ModelTier, StopReason,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

type Responder = dyn Fn(&ModelRequest, usize) -> Result<String, InferenceError> + Send + Sync;

/// Answers with `respond(request, summary_calls_so_far)`, recording every request.
struct Scripted {
    respond: Box<Responder>,
    requests: Mutex<Vec<ModelRequest>>,
}

fn is_summary_request(request: &ModelRequest) -> bool {
    request.messages.last().map(|m| m.text()).unwrap_or_default().starts_with("Summarise the conversation above")
}

impl Scripted {
    fn new(respond: impl Fn(&ModelRequest, usize) -> Result<String, InferenceError> + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self { respond: Box::new(respond), requests: Mutex::new(Vec::new()) })
    }

    fn requests(&self) -> Vec<ModelRequest> {
        self.requests.lock().unwrap().clone()
    }

    fn summary_requests(&self) -> Vec<ModelRequest> {
        self.requests().into_iter().filter(is_summary_request).collect()
    }

    fn answer_requests(&self) -> Vec<ModelRequest> {
        self.requests().into_iter().filter(|r| !is_summary_request(r)).collect()
    }
}

#[async_trait]
impl ModelRuntime for Scripted {
    async fn execute_inference(&self, request: ModelRequest) -> Result<ModelResponse, InferenceError> {
        let summaries_so_far = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request.clone());
            requests.iter().filter(|r| is_summary_request(r)).count()
        };
        let content = (self.respond)(&request, summaries_so_far)?;
        Ok(ModelResponse {
            content,
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
            capability: request.capability,
            metadata: ExecutionMetadata {
                provider_id: "t".into(),
                model_id: "t".into(),
                tier: ModelTier::Local,
                latency_ms: 0,
                tokens_used: 0,
                cache_hit: false,
                fallback_used: false,
                retry_count: 0,
                circuit_state: CircuitState::Closed,
            },
            request_id: request.request_id,
        })
    }

    async fn execute_stream(
        &self,
        _request: ModelRequest,
        _tx: mpsc::Sender<String>,
    ) -> Result<ExecutionMetadata, InferenceError> {
        unimplemented!()
    }

    async fn health_check(&self) -> HealthState {
        HealthState::Healthy
    }
}

/// ~250 tokens of text, so a couple of turns exceed a small budget.
fn long(label: &str) -> String {
    format!("{label}: {}", "lorem ipsum dolor sit amet ".repeat(37))
}

fn agent(model: Arc<Scripted>, context_tokens: usize) -> Agent {
    Agent::builder(model, Arc::new(InMemorySessionStore::new()))
        .compaction(CompactionSettings { context_tokens, ..CompactionSettings::default() })
        .build()
        .unwrap()
}

async fn ask(agent: &Agent, session: shared::id::SessionId, text: &str) -> runtime_agent::TurnOutcome {
    agent.submit(session, text, &CancellationToken::new(), &EventSink::none()).await.unwrap()
}

#[test]
fn model_view_replaces_covered_messages_with_the_latest_summary() {
    let plain = vec![Message::user("a"), Message::assistant("b")];
    assert_eq!(model_view(&plain), plain);

    let messages = vec![
        Message::user("old question"),
        Message::assistant("old answer"),
        Message::user("new question"),
        Message::summary("NOTES", 2),
        Message::assistant("new answer"),
    ];
    let view = model_view(&messages);
    assert_eq!(view.len(), 3);
    assert_eq!(view[0], Message::user("Summary of the earlier conversation:\nNOTES"));
    assert_eq!(view[1], Message::user("new question"));
    assert_eq!(view[2], Message::assistant("new answer"));
}

#[tokio::test]
async fn small_conversations_are_not_compacted() {
    let model = Scripted::new(|_, _| Ok("answer".into()));
    let agent = agent(model.clone(), 100_000);
    let session = agent.create_session().await.unwrap();
    ask(&agent, session, &long("q1")).await;
    ask(&agent, session, &long("q2")).await;
    assert!(model.summary_requests().is_empty());
    assert_eq!(model.answer_requests()[1].messages.len(), 3);
}

#[tokio::test]
async fn compaction_summarises_earlier_turns_and_chains_summaries() {
    let model = Scripted::new(|request, summaries| {
        Ok(if is_summary_request(request) { format!("NOTES{}", summaries) } else { long("answer") })
    });
    let agent = agent(model.clone(), 600); // compacts above ~450 tokens
    let session = agent.create_session().await.unwrap();

    // Turn 1: nothing earlier to summarise.
    ask(&agent, session, &long("q1")).await;
    assert!(model.summary_requests().is_empty());

    // Turn 2: q1 + answer + q2 is over budget, so q1 and its answer are summarised.
    let outcome = ask(&agent, session, &long("q2")).await;
    assert_eq!(outcome.cause, StopCause::EndTurn);
    let summaries = model.summary_requests();
    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].tools.is_empty());
    assert!(summaries[0].messages[0].text().starts_with("q1:"), "the summary call sees the old turn");

    let seen = model.answer_requests().last().cloned().unwrap();
    assert_eq!(seen.messages.len(), 2, "summary + the new question");
    assert_eq!(seen.messages[0].text(), "Summary of the earlier conversation:\nNOTES1");
    assert!(seen.messages[1].text().starts_with("q2:"));

    // Turn 3: the new summary is written from the previous summary plus what followed.
    ask(&agent, session, &long("q3")).await;
    let summaries = model.summary_requests();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[1].messages[0].text(), "Summary of the earlier conversation:\nNOTES1");
    let seen = model.answer_requests().last().cloned().unwrap();
    assert_eq!(seen.messages[0].text(), "Summary of the earlier conversation:\nNOTES2");
    assert!(seen.messages[1].text().starts_with("q3:"));

    // Nothing is deleted from storage: every question, answer and summary is kept.
    let stored = agent.session(session).await.unwrap().messages;
    assert_eq!(stored.len(), 3 * 2 + 2);
    assert_eq!(stored.iter().filter(|m| m.as_summary().is_some()).count(), 2);
}

#[tokio::test]
async fn a_failed_summary_does_not_block_the_turn() {
    let model = Scripted::new(|request, _| {
        if is_summary_request(request) {
            Err(InferenceError::AllProvidersFailed)
        } else {
            Ok(long("answer"))
        }
    });
    let agent = agent(model.clone(), 600);
    let session = agent.create_session().await.unwrap();
    ask(&agent, session, &long("q1")).await;
    let outcome = ask(&agent, session, &long("q2")).await;

    assert_eq!(outcome.cause, StopCause::EndTurn);
    assert_eq!(model.summary_requests().len(), 1, "tried once for this request, not on every step");
    let seen = model.answer_requests().last().cloned().unwrap();
    assert_eq!(seen.messages.len(), 3, "the full, uncompacted history was sent");
}

#[test]
fn bad_settings_are_rejected() {
    let model = Scripted::new(|_, _| Ok(String::new()));
    for settings in [
        CompactionSettings { context_tokens: 0, ..CompactionSettings::default() },
        CompactionSettings { threshold: 1.5, ..CompactionSettings::default() },
    ] {
        let result = Agent::builder(model.clone(), Arc::new(InMemorySessionStore::new())).compaction(settings).build();
        assert!(matches!(result, Err(AgentError::Config(_))));
    }
}
