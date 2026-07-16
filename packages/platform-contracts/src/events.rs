use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeEvent {
    RuntimeRegistered { runtime_id: String, name: String },
    RuntimeStarted { runtime_id: String },
    RuntimeStopped { runtime_id: String },
    CapabilityRegistered { capability: String, provider: String },
    CapabilityRemoved { capability: String, provider: String },
    HealthChanged { runtime_id: String, previous: crate::health::HealthState, current: crate::health::HealthState },
}
