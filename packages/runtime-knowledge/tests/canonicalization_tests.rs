use runtime_knowledge::engine::canonicalization::CanonicalizationEngine;
use runtime_knowledge::domain::RawEntity;
use runtime_knowledge::pipeline::{KnowledgeStage, KnowledgeCompilationContext};
use runtime_knowledge::ontology::{OntologyRegistry, Vocabulary, OntologyVersion, TypeSchema, Taxonomy};
use runtime_memory::engine::snapshot::MemorySnapshot;
use std::collections::HashMap;

fn mock_ontology(version: u32) -> OntologyRegistry {
    let mut names = HashMap::new();
    names.insert("repo".to_string(), "Repository".to_string());
    names.insert("repository".to_string(), "Repository".to_string());
    
    // Simulate v2 alias
    if version == 2 {
        names.insert("git repository".to_string(), "Repository".to_string());
    }
    
    OntologyRegistry {
        version_info: OntologyVersion { version, created_at: 0 },
        vocabulary: Vocabulary { canonical_names: names },
        types: TypeSchema { entity_types: HashMap::new() },
        taxonomy: Taxonomy { hierarchies: HashMap::new() },
        relationship_rules: vec![],
        inference_rules: vec![],
    }
}

// Mocks for memory snapshot types
#[derive(Clone, Default)]
struct MockData;

#[test]
fn test_determinism() {
    let ontology = mock_ontology(1);
    let mem: MemorySnapshot<MockData> = MemorySnapshot::default();
    
    let engine = CanonicalizationEngine::new("atlas.application".to_string());
    
    let mut ctx1 = KnowledgeCompilationContext::new(&ontology, &mem);
    ctx1.raw_entities.push(RawEntity { memory_id: "m1".into(), extracted_name: "repo".into(), context_snippet: "a".into() });
    engine.execute(&mut ctx1).unwrap();
    let id1 = ctx1.canonical_entities[0].identity.id.clone();
    
    let mut ctx2 = KnowledgeCompilationContext::new(&ontology, &mem);
    ctx2.raw_entities.push(RawEntity { memory_id: "m2".into(), extracted_name: "repository".into(), context_snippet: "b".into() });
    engine.execute(&mut ctx2).unwrap();
    let id2 = ctx2.canonical_entities[0].identity.id.clone();
    
    // Exact same ID produced from different raw aliases
    assert_eq!(id1.hash, id2.hash);
}

#[test]
fn test_vocabulary_version() {
    let mem: MemorySnapshot<MockData> = MemorySnapshot::default();
    let engine = CanonicalizationEngine::new("atlas.application".to_string());
    
    // V1 does not know "git repository"
    let ont_v1 = mock_ontology(1);
    let mut ctx_v1 = KnowledgeCompilationContext::new(&ont_v1, &mem);
    ctx_v1.raw_entities.push(RawEntity { memory_id: "m1".into(), extracted_name: "git repository".into(), context_snippet: "a".into() });
    engine.execute(&mut ctx_v1).unwrap();
    assert!(ctx_v1.canonical_entities.is_empty()); // Dropped because unknown alias
    assert_eq!(ctx_v1.diagnostics.len(), 1); // Emitted warning
    
    // V2 knows "git repository"
    let ont_v2 = mock_ontology(2);
    let mut ctx_v2 = KnowledgeCompilationContext::new(&ont_v2, &mem);
    ctx_v2.raw_entities.push(RawEntity { memory_id: "m1".into(), extracted_name: "git repository".into(), context_snippet: "a".into() });
    engine.execute(&mut ctx_v2).unwrap();
    assert_eq!(ctx_v2.canonical_entities.len(), 1); // Successfully resolved
}

#[test]
fn test_collision_avoidance() {
    let ontology = mock_ontology(1);
    let mem: MemorySnapshot<MockData> = MemorySnapshot::default();
    
    let app_engine = CanonicalizationEngine::new("atlas.application".to_string());
    let mut ctx_app = KnowledgeCompilationContext::new(&ontology, &mem);
    ctx_app.raw_entities.push(RawEntity { memory_id: "m1".into(), extracted_name: "repo".into(), context_snippet: "a".into() });
    app_engine.execute(&mut ctx_app).unwrap();
    
    let db_engine = CanonicalizationEngine::new("atlas.database".to_string());
    let mut ctx_db = KnowledgeCompilationContext::new(&ontology, &mem);
    ctx_db.raw_entities.push(RawEntity { memory_id: "m1".into(), extracted_name: "repo".into(), context_snippet: "a".into() });
    db_engine.execute(&mut ctx_db).unwrap();
    
    // Same canonical name ("Repository") but different namespace -> Different IDs
    assert_ne!(ctx_app.canonical_entities[0].identity.id.hash, ctx_db.canonical_entities[0].identity.id.hash);
}
