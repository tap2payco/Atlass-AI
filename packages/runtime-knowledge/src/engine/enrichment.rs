use crate::pipeline::{KnowledgeStage, KnowledgeCompilationContext};
use crate::domain::{Entity, EntityTypeId};

pub struct EnrichmentRule {
    pub condition_type: EntityTypeId,
    pub attribute_key: String,
    pub attribute_value: String,
}

pub struct EnrichmentEngine {
    rules: Vec<EnrichmentRule>,
}

impl EnrichmentEngine {
    pub fn new() -> Self { 
        Self { 
            // Stub known rules
            rules: vec![
                EnrichmentRule {
                    condition_type: EntityTypeId("ProgrammingLanguage".to_string()),
                    attribute_key: "category".to_string(),
                    attribute_value: "Language".to_string(),
                }
            ]
        } 
    }
}

impl<T> KnowledgeStage<T> for EnrichmentEngine {
    fn execute(&self, ctx: &mut KnowledgeCompilationContext<'_, T>) -> Result<(), String> {
        for entity in &mut ctx.canonical_entities {
            for rule in &self.rules {
                if entity.entity_type == rule.condition_type {
                    entity.attributes.insert(rule.attribute_key.clone(), rule.attribute_value.clone());
                    entity.provenance.enrichment_history.push(format!("Applied EnrichmentRule for {}", rule.condition_type.0));
                }
            }
        }
        Ok(())
    }
}
