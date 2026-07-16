use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthState {
    Initializing,
    Healthy,
    Degraded,
    Recovering,
    Stopping,
    Offline,
}
