use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryState {
    Observed,
    Candidate,
    Consolidation,
    Validation,
    Classification,
    Active,
    Cold,
    Archived,
    Eligible,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryEvent {
    MemoryObserved { memory_id: String, timestamp: u64 },
    MemoryCandidate { memory_id: String, timestamp: u64 },
    MemoryValidated { memory_id: String, timestamp: u64 },
    MemoryClassified { memory_id: String, timestamp: u64 },
    MemoryStored { memory_id: String, timestamp: u64 },
    MemoryIndexed { memory_id: String, timestamp: u64 },
    MemoryRetrieved { memory_id: String, timestamp: u64 },
    MemoryReinforced { memory_id: String, delta_reinforcement: f32, timestamp: u64 },
    MemoryArchived { memory_id: String, timestamp: u64 },
    MemoryEligibleForDeletion { memory_id: String, timestamp: u64 },
    MemoryForgotten { memory_id: String, timestamp: u64 },
    MemoryDeleted { memory_id: String, timestamp: u64 },
    MemoryConsolidated { source_ids: Vec<String>, new_memory_id: String, timestamp: u64 },
}

pub struct LifecycleManager {
    // Manages transitions and emits events to RuntimeBus
}

impl LifecycleManager {
    pub fn new() -> Self {
        Self {}
    }

    pub fn transition(&self, memory_id: &str, current_state: Option<MemoryState>, next: MemoryState) -> Result<MemoryEvent, String> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        
        // Validation rules
        if next == MemoryState::Deleted {
            if let Some(ref current) = current_state {
                if *current != MemoryState::Eligible && *current != MemoryState::Archived {
                    return Err(format!("Memory {} cannot be deleted without being archived first.", memory_id));
                }
            } else {
                return Err(format!("Memory {} cannot be deleted without being archived first.", memory_id));
            }
        }
        
        let event = match next {
            MemoryState::Observed => MemoryEvent::MemoryObserved { memory_id: memory_id.to_string(), timestamp: now },
            MemoryState::Candidate => MemoryEvent::MemoryCandidate { memory_id: memory_id.to_string(), timestamp: now },
            MemoryState::Validation => MemoryEvent::MemoryValidated { memory_id: memory_id.to_string(), timestamp: now },
            MemoryState::Classification => MemoryEvent::MemoryClassified { memory_id: memory_id.to_string(), timestamp: now },
            MemoryState::Active => MemoryEvent::MemoryStored { memory_id: memory_id.to_string(), timestamp: now }, // Simplification
            MemoryState::Archived => MemoryEvent::MemoryArchived { memory_id: memory_id.to_string(), timestamp: now },
            MemoryState::Eligible => MemoryEvent::MemoryEligibleForDeletion { memory_id: memory_id.to_string(), timestamp: now },
            MemoryState::Deleted => MemoryEvent::MemoryDeleted { memory_id: memory_id.to_string(), timestamp: now },
            _ => MemoryEvent::MemoryObserved { memory_id: memory_id.to_string(), timestamp: now }, // Fallback stub
        };
        
        Ok(event)
    }
}
