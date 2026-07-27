use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub request_id: String,
    pub correlation_id: String,
    pub trace_id: String,
    pub timeout: Option<Duration>,
    pub retry_count: u32,
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchedulingPolicy {
    Sequential,
    Parallel,
    Priority,
    RoundRobin,
    Weighted,
    Deadline,
    Adaptive,
}

pub struct ValidationReport<D> {
    pub errors: Vec<D>,
    pub warnings: Vec<D>,
    pub suggestions: Vec<D>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub step_id: String,
    pub name: String,
    pub provider: String,
    pub is_parallel: bool,
    pub timeout_ms: Option<u64>,
    pub required_capabilities: Vec<String>,
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStrategy {
    Sequential,
    Parallel,
    BestEffort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub version: u16,
    pub id: String,
    pub steps: Vec<ExecutionStep>,
    pub strategy: ExecutionStrategy,
    pub timeout_ms: Option<u64>,
}
