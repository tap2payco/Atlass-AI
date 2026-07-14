use serde::{Deserialize, Serialize};
use crate::id::{RepositoryId, WorkspaceId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryEntity {
    pub id: RepositoryId,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub path: String,
}
