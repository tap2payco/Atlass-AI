use crate::pipeline::{KnowledgeStage, KnowledgeCompilationContext};

pub struct ExtractionEngine {
    // Stub for ML extraction model
}

impl ExtractionEngine {
    pub fn new() -> Self { Self {} }
}

impl KnowledgeStage for ExtractionEngine {
    fn execute<T>(&self, ctx: &mut KnowledgeCompilationContext<'_, T>) -> Result<(), String> {
        // Reads from memory_snapshot, creates RawEntity, pushes to ctx.raw_entities
        Ok(())
    }
}
