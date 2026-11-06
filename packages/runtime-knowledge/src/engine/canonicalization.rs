use crate::pipeline::{KnowledgeStage, KnowledgeCompilationContext};
use crate::domain::{Entity, EntityIdentity, EntityId, EntityProvenance, EntityTypeId, RawEntity};
use crate::events::{KnowledgeEvent, EntityMergedEvent};
use crate::diagnostics::{KnowledgeDiagnostic, DiagnosticSeverity, CanonicalizationDiagnostic};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

pub struct CanonicalizationEngine {
    pub namespace: String,
}

impl CanonicalizationEngine {
    pub fn new(namespace: String) -> Self { Self { namespace } }
    
    fn generate_hash(namespace: &str, entity_type: &str, canonical_name: &str) -> String {
        let mut hasher = DefaultHasher::new();
        namespace.hash(&mut hasher);
        entity_type.hash(&mut hasher);
        canonical_name.hash(&mut hasher);
        format!("ENT-{:X}", hasher.finish())
    }
}

impl<T> KnowledgeStage<T> for CanonicalizationEngine {
    fn execute(&self, ctx: &mut KnowledgeCompilationContext<'_, T>) -> Result<(), String> {
        let mut canonical_map: HashMap<String, Vec<RawEntity>> = HashMap::new();
        
        // 1. Resolve aliases exactly against Vocabulary
        for raw in &ctx.raw_entities {
            if let Some(canonical) = ctx.ontology.vocabulary.canonical_names.get(&raw.extracted_name) {
                canonical_map.entry(canonical.clone()).or_default().push(raw.clone());
            } else {
                ctx.diagnostics.push(KnowledgeDiagnostic::Canonicalization {
                    severity: DiagnosticSeverity::Warning,
                    diagnostic: CanonicalizationDiagnostic::MissingCanonicalName(raw.extracted_name.clone()),
                    context: format!("Raw entity {} not found in Vocabulary", raw.extracted_name),
                });
            }
        }
        
        // 2. Merge and instantiate canonical entities
        for (canonical_name, raw_list) in canonical_map {
            // Retrieve type from ontology (stub: we assume 'Repository' type for this example)
            let entity_type = EntityTypeId("Repository".to_string());
            
            let id = Self::generate_hash(&self.namespace, &entity_type.0, &canonical_name);
            let entity_id = EntityId { namespace: self.namespace.clone(), hash: id.clone() };
            
            let mut source_memories = Vec::new();
            let mut source_docs = Vec::new();
            
            for raw in &raw_list {
                source_memories.push(raw.memory_id.clone());
                source_docs.push(raw.context_snippet.clone());
            }
            
            let entity = Entity {
                identity: EntityIdentity {
                    id: entity_id.clone(),
                    canonical_name: canonical_name.clone(),
                    aliases: vec![], // Populate from registry if needed
                    version: 1,
                },
                entity_type,
                attributes: HashMap::new(),
                confidence: 1.0,
                provenance: EntityProvenance {
                    source_memory_ids: source_memories,
                    source_documents: source_docs,
                    extraction_method: "Exact Vocabulary Match".to_string(),
                    canonicalization_reason: format!("Merged {} raw entities", raw_list.len()),
                    enrichment_history: vec![],
                },
            };
            
            // 3. Emit merge event if multiple raw entities merged
            if raw_list.len() > 1 {
                ctx.events.push(KnowledgeEvent::EntityMerged(EntityMergedEvent {
                    primary: entity_id,
                    merged: vec![], // Stub for previous IDs if they had any
                    reason: "Vocabulary alias match".to_string(),
                }));
            }
            
            ctx.canonical_entities.push(entity);
        }
        
        Ok(())
    }
}
