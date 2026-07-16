use serde::{Deserialize, Serialize};
use crate::health::HealthState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub capability: String,
    pub version: String,
    pub provider: String, // runtime name
    pub priority: u8,
    pub health: HealthState,
}
