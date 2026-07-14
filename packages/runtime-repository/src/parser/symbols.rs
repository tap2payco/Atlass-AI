use shared::entities::SymbolEntity;
use crate::parser::ast::AtlasSyntaxTree;

pub struct SymbolExtractor;

impl SymbolExtractor {
    pub fn extract_declarations(_ast: &AtlasSyntaxTree) -> Vec<SymbolEntity> {
        // Stub for extracting declarations (Functions, Classes, Traits, Enums, etc.)
        // This will eventually query the tree-sitter AST nodes.
        vec![]
    }
}
