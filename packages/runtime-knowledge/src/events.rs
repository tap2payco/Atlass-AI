use serde::{Deserialize, Serialize};
use crate::domain::EntityId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMergedEvent {
    pub primary: EntityId,
    pub merged: Vec<EntityId>, // Using EntityId representation even if they were raw initially
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KnowledgeEvent {
    EntityMerged(EntityMergedEvent),
    // Future events
}
