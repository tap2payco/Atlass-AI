use serde::{Deserialize, Serialize};
use crate::id::RepositoryId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageEntity {
    pub id: String, // e.g. "uuid" or "uuid-v4"
    pub repository_id: RepositoryId,
    pub name: String,
    pub version: Option<String>,
    pub path: String,
}
