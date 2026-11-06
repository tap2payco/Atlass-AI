//! Built-in operations. The default order is:
//! context sources → cognitive states (if enabled) → extra operations →
//! tool execution → max turns → compaction (if enabled) → inference.
//! Context and cognitive operations only add prompt parts; they never apply.

mod compaction;
mod inference;
mod max_turns;
mod tool_execution;

pub use compaction::{CompactionOperation, CompactionSettings};
pub use inference::{InferenceOperation, InferenceSettings};
pub use max_turns::MaxTurnsOperation;
pub use tool_execution::ToolExecutionOperation;
