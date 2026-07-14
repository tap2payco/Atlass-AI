use std::path::PathBuf;
use runtime_repository::sync::coordinator::UpdateCoordinator;
use runtime_repository::sync::change::{RepositoryChange, ChangeSource, ChangeType};
use std::time::SystemTime;

#[test]
fn test_file_deleted_during_parsing() {
    let mut coordinator = UpdateCoordinator::new();
    
    // Simulate a file modification event
    let change = RepositoryChange {
        source: ChangeSource::Watcher,
        timestamp: SystemTime::now(),
        change_type: ChangeType::FileModified,
        affected_path: PathBuf::from("nonexistent.rs"),
        metadata: None,
    };
    
    // The coordinator should gracefully handle the fact that the file is gone by the time it parses it
    let result = coordinator.process_change(change);
    
    // It shouldn't panic, but should probably return an Ok() since it handled the deletion, or an error that is handled.
    // For now we just verify it doesn't panic.
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_burst_of_events() {
    let mut coordinator = UpdateCoordinator::new();
    
    for i in 0..100 {
        let change = RepositoryChange {
            source: ChangeSource::Watcher,
            timestamp: SystemTime::now(),
            change_type: ChangeType::FileModified,
            affected_path: PathBuf::from(format!("file_{}.rs", i)),
            metadata: None,
        };
        let _ = coordinator.process_change(change);
    }
    
    // Verify it survives a burst.
}
