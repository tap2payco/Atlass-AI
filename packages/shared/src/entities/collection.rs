use serde::{Deserialize, Serialize};
use crate::entities::package::PackageEntity;
use crate::entities::module::ModuleEntity;
use crate::entities::symbol::SymbolEntity;
use crate::entities::relationship::RelationshipEntity;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntityCollection {
    pub packages: Vec<PackageEntity>,
    pub modules: Vec<ModuleEntity>,
    pub symbols: Vec<SymbolEntity>,
    pub relationships: Vec<RelationshipEntity>,
}
