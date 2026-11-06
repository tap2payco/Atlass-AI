//! # Atlas Agent Runtime
//!
//! Runs an agent turn as a loop of small rules ("operations") over a
//! persisted conversation, following the Goose state-machine design
//! (see `docs/appendix/research/goose-codex-reference-study.md`).
//!
//! ```text
//! user message ─▶ [tool execution] ─▶ [max turns] ─▶ [inference] ─┐
//!                       ▲                                         │ tool calls
//!                       └─────────────────────────────────────────┘
//!                                        no tool calls ─▶ Stop(EndTurn)
//! ```
//!
//! - All turn state is derived from the stored messages, so a run can be
//!   resumed after a crash, a cancellation, a model error or the turn limit.
//! - Cancellation answers any open tool calls with an error result, keeping
//!   the conversation valid for the next model request.
//! - Extra operations (approvals, hooks, compaction) are added with
//!   `AgentBuilder::operation` and run before the built-ins.
//!
//! ```rust,no_run
//! # use std::sync::Arc;
//! # use runtime_agent::{Agent, EventSink, InMemorySessionStore};
//! # use tokio_util::sync::CancellationToken;
//! # async fn demo(model: Arc<dyn sdk::model::ModelRuntime>) -> Result<(), runtime_agent::AgentError> {
//! let agent = Agent::builder(model, Arc::new(InMemorySessionStore::new())).build()?;
//! let session = agent.create_session().await?;
//! let outcome = agent
//!     .submit(session, "Summarise src/lib.rs", &CancellationToken::new(), &EventSink::none())
//!     .await?;
//! println!("{:?}: {}", outcome.cause, outcome.reply.unwrap_or_default());
//! # Ok(()) }
//! ```

pub mod agent;
pub mod conversation;
pub mod error;
pub mod events;
pub mod machine;
pub mod operation;
pub mod ops;
pub mod session;
pub mod tool;

pub use agent::{Agent, AgentBuilder, TurnOutcome, DEFAULT_MAX_TURNS};
pub use error::AgentError;
pub use events::{AgentEvent, EventSink};
pub use machine::StateMachine;
pub use operation::{Operation, OperationResult, RunStatus, StepContext, StopCause};
pub use ops::InferenceSettings;
pub use session::{InMemorySessionStore, Session, SessionStore};
pub use tool::Tool;
