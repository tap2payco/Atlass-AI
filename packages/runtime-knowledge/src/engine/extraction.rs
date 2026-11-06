use crate::pipeline::{KnowledgeStage, KnowledgeCompilationContext};

pub struct ExtractionEngine {
    // Stub for ML extraction model
}

impl ExtractionEngine {
    pub fn new() -> Self { Self {} }
}

impl<T> KnowledgeStage<T> for ExtractionEngine {
    fn execute(&self, _ctx: &mut KnowledgeCompilationContext<'_, T>) -> Result<(), String> {
        // Reads from memory_snapshot, creates RawEntity, pushes to ctx.raw_entities
        Ok(())
    }
}
