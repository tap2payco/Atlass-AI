use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryPolicy {
    Never,
    Fixed { attempts: u32, delay: Duration },
    Exponential { max_attempts: u32, initial_delay: Duration, multiplier: f32 },
    Fibonacci { max_attempts: u32, initial_delay: Duration },
    Adaptive,
}
