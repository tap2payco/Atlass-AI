use runtime_memory::api::MemoryRuntime;
use runtime_memory::storage::file_adapter::FileMemoryStore;
use runtime_memory::storage::repository::MemoryRepository;
use runtime_memory::domain::{
    MemoryRecord, MemoryIdentity, MemoryContent, MemoryMetadata,
    MemoryType, MemoryClass, MemoryTraits, MemoryProvenance, ExpirationPolicy,
};

fn mock_record(id: &str, text: &str) -> MemoryRecord<String> {
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
                source_runtime: "test-runtime".to_string(),
                original_request_id: None,
                created_by: "test".to_string(),
            },
            created_at: 100,
            updated_at: 100,
        },
        content: MemoryContent {
            payload: text.to_string(),
            embedding_references: vec![],
            entities: vec![],
            relationships: vec![],
            summaries: vec![],
            attachments: vec![],
        },
        metadata: MemoryMetadata {
            confidence: 1.0,
            quality: 1.0,
            stability: 1.0,
            importance: 1.0,
            decay: 0.0,
            reinforcement: 1.0,
            access_count: 0,
            last_accessed: 100,
            expiration_policy: ExpirationPolicy::Never,
        },
    }
}

#[tokio::test]
async fn test_file_memory_store_persistence() {
    let temp_dir = std::env::temp_dir().join("atlas_test_memory_store");
    let _ = std::fs::remove_dir_all(&temp_dir);

    // 1. First runtime instance writes to disk
    {
        let store = FileMemoryStore::new(temp_dir.clone());
        let repo = MemoryRepository::with_file_store(store);
        let runtime = MemoryRuntime::with_repository(repo);

        let rec = mock_record("disk-mem-1", "Persistent Cognitive State");
        runtime.store(&rec).await.unwrap();
    }

    // 2. Second runtime instance reads from disk after restart
    {
        let store = FileMemoryStore::new(temp_dir.clone());
        let repo = MemoryRepository::with_file_store(store);
        let runtime = MemoryRuntime::with_repository(repo);

        let (records, _) = runtime.retrieve::<String>("disk-mem-1").await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content.payload, "Persistent Cognitive State");

        // Delete from disk
        runtime.delete("disk-mem-1").await.unwrap();
    }

    // 3. Verify deletion persisted
    {
        let store = FileMemoryStore::new(temp_dir.clone());
        let repo = MemoryRepository::with_file_store(store);
        let runtime = MemoryRuntime::with_repository(repo);

        let (records, _) = runtime.retrieve::<String>("disk-mem-1").await.unwrap();
        assert_eq!(records.len(), 0);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}
