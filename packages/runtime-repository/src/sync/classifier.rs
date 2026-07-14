use crate::sync::change::{ChangeSource, ChangeType, RepositoryChange};
use std::path::Path;
use std::time::SystemTime;

pub struct ChangeClassifier;

impl ChangeClassifier {
    /// Maps a raw file modification event into a specific `ChangeType`.
    pub fn classify_modification(path: &Path) -> RepositoryChange {
        let change_type = if path.ends_with("Cargo.toml") || path.ends_with("package.json") {
            ChangeType::ConfigurationChanged
        } else if path.ends_with("Cargo.lock") || path.ends_with("pnpm-lock.yaml") {
            ChangeType::DependencyChanged
        } else if path.starts_with(".git/") {
            ChangeType::GitBranchChanged
        } else {
            ChangeType::FileModified
        };

        RepositoryChange {
            source: ChangeSource::Watcher,
            timestamp: SystemTime::now(),
            change_type,
            affected_path: path.to_path_buf(),
            metadata: None,
        }
    }
}
