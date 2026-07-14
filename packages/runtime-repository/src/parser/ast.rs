use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasSyntaxTree {
    pub language: String,
    pub node_count: usize,
    pub syntax_errors: usize,
    pub parse_time_ms: u64,
    // Internal opaque representation of the AST nodes (e.g., JSON dumped or custom node hierarchy)
    pub root_node: serde_json::Value,
}

impl AtlasSyntaxTree {
    pub fn new(language: String, node_count: usize, syntax_errors: usize, parse_time_ms: u64, root_node: serde_json::Value) -> Self {
        Self {
            language,
            node_count,
            syntax_errors,
            parse_time_ms,
            root_node,
        }
    }
}
