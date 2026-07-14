use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymbolType {
    Function,
    Class,
    Method,
    Interface,
    Enum,
    Trait,
    Constant,
    Module,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolEntity {
    pub id: String,
    pub module_id: String,
    pub name: String,
    pub symbol_type: SymbolType,
    pub signature: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
}
