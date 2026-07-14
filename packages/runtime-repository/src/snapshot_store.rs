use crate::snapshot::RepositorySnapshot;

pub trait SnapshotStore: Send + Sync {
    fn save(&mut self, snapshot: RepositorySnapshot);
    fn load(&self, id: &str) -> Option<RepositorySnapshot>;
    fn latest(&self) -> Option<RepositorySnapshot>;
}

pub struct InMemorySnapshotStore {
    snapshots: std::collections::HashMap<String, RepositorySnapshot>,
    latest_id: Option<String>,
}

impl InMemorySnapshotStore {
    pub fn new() -> Self {
        Self {
            snapshots: std::collections::HashMap::new(),
            latest_id: None,
        }
    }
}

impl SnapshotStore for InMemorySnapshotStore {
    fn save(&mut self, snapshot: RepositorySnapshot) {
        self.latest_id = Some(snapshot.snapshot_id.clone());
        self.snapshots.insert(snapshot.snapshot_id.clone(), snapshot);
    }

    fn load(&self, id: &str) -> Option<RepositorySnapshot> {
        self.snapshots.get(id).cloned()
    }

    fn latest(&self) -> Option<RepositorySnapshot> {
        self.latest_id.as_ref().and_then(|id| self.load(id))
    }
}
