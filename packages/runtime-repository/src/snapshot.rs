use serde::{Deserialize, Serialize};
use shared::entities::*;
use crate::metadata::RepositoryProfile;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositorySnapshot {
    pub snapshot_id: String,
    pub parent_snapshot_id: Option<String>,
    pub repository_id: String,
    pub git_revision: Option<String>,
    pub timestamp: SystemTime,
    pub generation_version: String,
    pub generation_reason: String,
    pub profile: RepositoryProfile,
    pub entities: EntityCollection,
    pub added_entities: Vec<String>,
    pub removed_entities: Vec<String>,
    pub updated_entities: Vec<String>,
}

