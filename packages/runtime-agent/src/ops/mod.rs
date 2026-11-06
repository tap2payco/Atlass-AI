//! Built-in operations. The default order is:
//! extra operations → tool execution → max turns → inference.

mod inference;
mod max_turns;
mod tool_execution;

pub use inference::{InferenceOperation, InferenceSettings};
pub use max_turns::MaxTurnsOperation;
pub use tool_execution::ToolExecutionOperation;
