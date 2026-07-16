use shared::context::ContextPackage;

pub struct ContextCache;

impl ContextCache {
    pub fn get(_id: &str) -> Option<ContextPackage> {
        unimplemented!()
    }
    
    pub fn set(_package: &ContextPackage) {
        unimplemented!()
    }
}
