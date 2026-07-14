use serde::{Deserialize, Serialize};
use crate::id::WorkspaceId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEntity {
    pub id: WorkspaceId,
    pub name: String,
    pub path: String,
}
