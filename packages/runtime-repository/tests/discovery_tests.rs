use runtime_repository::{RepositoryDiscovery, DiscoveryConfig};
use std::path::PathBuf;

#[test]
fn test_git_detection() {
    let config = DiscoveryConfig {
        root_path: PathBuf::from("../../"),
        max_depth: Some(1),
        respect_gitignore: true,
    };
    
    let discovery = RepositoryDiscovery::new(config);
    let result = discovery.discover().expect("Failed to discover");
    
    assert!(result.is_git, "Should detect root as git repo (if it is one)");
}

#[test]
fn test_file_enumeration() {
    let config = DiscoveryConfig {
        root_path: PathBuf::from("./"),
        max_depth: None,
        respect_gitignore: true,
    };
    
    let discovery = RepositoryDiscovery::new(config);
    let result = discovery.discover().expect("Failed to discover");
    
    assert!(!result.files.is_empty(), "Should find files in its own directory");
}
