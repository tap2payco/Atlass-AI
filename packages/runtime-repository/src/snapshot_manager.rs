use crate::snapshot::RepositorySnapshot;
use crate::snapshot_store::SnapshotStore;
use shared::types::Result;

pub struct SnapshotManager {
    store: Box<dyn SnapshotStore>,
}

impl SnapshotManager {
    pub fn new(store: Box<dyn SnapshotStore>) -> Self {
        Self { store }
    }

    pub fn create_snapshot(&mut self, mut new_snapshot: RepositorySnapshot) -> Result<RepositorySnapshot> {
        if let Some(latest) = self.store.latest() {
            new_snapshot.parent_snapshot_id = Some(latest.snapshot_id);
            // compute deltas here...
        }
        
        self.store.save(new_snapshot.clone());
        
        // Emit SnapshotCreated over NATS
        
        Ok(new_snapshot)
    }
}
