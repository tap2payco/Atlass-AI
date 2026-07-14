use shared::entities::SymbolType;

pub fn normalize_kind(language: &str, raw_kind: &str) -> SymbolType {
    match language {
        "rust" => match raw_kind {
            "function_item" => SymbolType::Function,
            "struct_item" => SymbolType::Class, // Mapped to Class conceptually
            "impl_item" => SymbolType::Module, // Or grouping
            "trait_item" => SymbolType::Trait,
            "enum_item" => SymbolType::Enum,
            _ => SymbolType::Unknown,
        },
        "python" => match raw_kind {
            "function_definition" => SymbolType::Function,
            "class_definition" => SymbolType::Class,
            _ => SymbolType::Unknown,
        },
        _ => SymbolType::Unknown,
    }
}
