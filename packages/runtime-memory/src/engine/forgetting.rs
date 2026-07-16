use crate::domain::{MemoryRecord, ExpirationPolicy};
use crate::lifecycle::{MemoryState, MemoryEvent, LifecycleManager};

pub struct ForgettingEngine {
    lifecycle: LifecycleManager,
}

impl ForgettingEngine {
    pub fn new() -> Self { 
        Self {
            lifecycle: LifecycleManager::new()
        } 
    }
    
    // Progressive pipeline: Active -> Cold -> Archived -> Eligible -> Deleted
    pub fn evaluate_decay<T>(&self, state: MemoryState, record: &MemoryRecord<T>) -> (MemoryState, MemoryEvent) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let is_expired = match record.metadata.expiration_policy {
            ExpirationPolicy::AfterSeconds(secs) => now > record.identity.created_at + secs,
            ExpirationPolicy::AfterAccesses(limit) => record.metadata.access_count >= limit,
            _ => false,
        };

        if !is_expired {
            return (state.clone(), self.lifecycle.transition(&record.identity.id, Some(state.clone()), state.clone()).unwrap());
        }

        match state {
            MemoryState::Active => {
                (MemoryState::Cold, self.lifecycle.transition(&record.identity.id, Some(MemoryState::Active), MemoryState::Cold).unwrap())
            }
            MemoryState::Cold => {
                (MemoryState::Archived, self.lifecycle.transition(&record.identity.id, Some(MemoryState::Cold), MemoryState::Archived).unwrap())
            }
            MemoryState::Archived => {
                (MemoryState::Eligible, self.lifecycle.transition(&record.identity.id, Some(MemoryState::Archived), MemoryState::Eligible).unwrap())
            }
            MemoryState::Eligible => {
                (MemoryState::Deleted, self.lifecycle.transition(&record.identity.id, Some(MemoryState::Eligible), MemoryState::Deleted).unwrap())
            }
            _ => (state.clone(), self.lifecycle.transition(&record.identity.id, Some(state.clone()), state.clone()).unwrap())
        }
    }
}
