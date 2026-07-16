use crate::engine::policy::ConsolidationPolicy;
use crate::domain::MemoryRecord;
use crate::lifecycle::{MemoryState, MemoryEvent, LifecycleManager};

pub struct ConsolidationDecision {
    pub destination: MemoryState,
    pub confidence: f32,
    pub reason: String,
    pub policy: ConsolidationPolicy,
    pub explanation: String,
}

pub struct ConsolidationEngine {
    lifecycle: LifecycleManager,
}

impl ConsolidationEngine {
    pub fn new() -> Self { 
        Self {
            lifecycle: LifecycleManager::new()
        }
    }

    // Pipeline: Working -> Candidate -> Consolidation -> Validation -> Classification -> Active
    pub fn decide<T>(&self, policy: ConsolidationPolicy, state: MemoryState, _record: &MemoryRecord<T>) -> ConsolidationDecision {
        let destination = match state {
            MemoryState::Active => MemoryState::Candidate,
            MemoryState::Candidate => MemoryState::Consolidation,
            MemoryState::Consolidation => MemoryState::Validation,
            MemoryState::Validation => MemoryState::Classification,
            MemoryState::Classification => MemoryState::Active,
            _ => state.clone(),
        };

        ConsolidationDecision {
            destination,
            confidence: 0.95,
            reason: "Standard lifecycle progression".to_string(),
            policy,
            explanation: "Progressing through consolidation pipeline".to_string(),
        }
    }

    pub fn apply<T>(&self, decision: ConsolidationDecision, record: &MemoryRecord<T>, current_state: MemoryState) -> (MemoryState, MemoryEvent) {
        let event = self.lifecycle.transition(&record.identity.id, Some(current_state.clone()), decision.destination.clone()).unwrap();
        (decision.destination, event)
    }
}
