use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserCapabilities {
    pub supports_ast: bool,
    pub supports_queries: bool,
    pub supports_incremental: bool,
    pub supports_error_recovery: bool,
    pub supports_symbol_extraction: bool,
}

pub trait ParserProvider: Send + Sync {
    fn language(&self) -> &'static str;
    fn capabilities(&self) -> ParserCapabilities;
    
    // Abstracted parsing methods to be implemented in ast.rs
}
