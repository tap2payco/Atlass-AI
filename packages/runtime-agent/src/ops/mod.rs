//! Built-in operations. The default order is:
//! extra operations → tool execution → max turns → compaction (if enabled) → inference.

mod compaction;
mod inference;
mod max_turns;
mod tool_execution;

pub use compaction::{CompactionOperation, CompactionSettings};
pub use inference::{InferenceOperation, InferenceSettings};
pub use max_turns::MaxTurnsOperation;
pub use tool_execution::ToolExecutionOperation;
