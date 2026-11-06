use runtime_memory::api::MemoryRuntime;
use runtime_memory::domain::{
    MemoryRecord, MemoryIdentity, MemoryContent, MemoryMetadata,
    MemoryType, MemoryClass, MemoryTraits, MemoryProvenance, ExpirationPolicy,
};

fn mock_memory_record(id: &str, payload: String) -> MemoryRecord<String> {
    MemoryRecord {
        identity: MemoryIdentity {
            id: id.to_string(),
            memory_type: MemoryType::Episodic,
            class: MemoryClass::Persistent,
            traits: MemoryTraits {
                supports_search: true,
                supports_versioning: true,
                supports_decay: false,
                supports_archival: true,
                supports_reinforcement: true,
            },
            source: "unit-test".to_string(),
            version: 1,
            parent_version: None,
            derived_from_version: None,
            provenance: MemoryProvenance {
                source_runtime: "runtime-memory-test".to_string(),
                original_request_id: Some("req-1".to_string()),
                created_by: "test-runner".to_string(),
            },
            created_at: 1000,
            updated_at: 1000,
        },
        content: MemoryContent {
            payload,
            embedding_references: vec![],
            entities: vec!["entity-1".to_string()],
            relationships: vec![],
            summaries: vec!["mock summary".to_string()],
            attachments: vec![],
        },
        metadata: MemoryMetadata {
            confidence: 0.95,
            quality: 0.90,
            stability: 0.85,
            importance: 0.80,
            decay: 0.01,
            reinforcement: 1.0,
            access_count: 1,
            last_accessed: 1000,
            expiration_policy: ExpirationPolicy::Never,
        },
    }
}

#[tokio::test]
async fn test_memory_store_retrieve_delete_flow() {
    let runtime = MemoryRuntime::new();
    let record = mock_memory_record("mem-1", "user session details".to_string());

    // 1. Store
    let store_res = runtime.store(&record).await;
    assert!(store_res.is_ok());

    // 2. Retrieve
    let (retrieved, _explanation) = runtime.retrieve::<String>("session").await.unwrap();
    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].content.payload, "user session details");

    // 3. Snapshot
    let snap = runtime.snapshot::<String>("mem-1").await;
    assert!(snap.is_ok());
    assert_eq!(snap.unwrap().payload, "user session details");

    // 4. Delete
    let del_res = runtime.delete("mem-1").await;
    assert!(del_res.is_ok());

    // 5. Query post-delete
    let (post_del, _) = runtime.retrieve::<String>("session").await.unwrap();
    assert_eq!(post_del.len(), 0);
}
