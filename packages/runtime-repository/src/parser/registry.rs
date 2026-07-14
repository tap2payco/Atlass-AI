use std::collections::HashMap;
use crate::parser::provider::ParserProvider;

pub struct ParserRegistry {
    providers: HashMap<String, Box<dyn ParserProvider>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register(&mut self, ext: &str, provider: Box<dyn ParserProvider>) {
        self.providers.insert(ext.to_string(), provider);
    }

    pub fn get_provider(&self, ext: &str) -> Option<&Box<dyn ParserProvider>> {
        self.providers.get(ext)
    }
}
