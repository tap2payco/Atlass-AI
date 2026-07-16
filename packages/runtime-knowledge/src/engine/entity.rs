use runtime_memory::engine::snapshot::MemorySnapshot;

pub struct EntityEngine {
    // Stub
}

impl EntityEngine {
    pub fn new() -> Self { Self {} }
    
    // Extracts and normalizes entities from memory snapshots
    pub fn extract<T>(&self, _snapshot: &MemorySnapshot<T>) -> Result<(), String> {
        Ok(())
    }
}
