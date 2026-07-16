use crate::domain::MemoryRecord;

pub struct SelectionEngine {
    // Stub
}

impl SelectionEngine {
    pub fn new() -> Self { Self {} }
    
    pub fn select<T>(&self, mut ranked: Vec<MemoryRecord<T>>, limit: usize) -> Vec<MemoryRecord<T>> {
        ranked.truncate(limit);
        ranked
    }
}
