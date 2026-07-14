use std::fs;
use std::path::Path;

#[test]
fn test_architecture_conformance() {
    // A simple grep-like check to ensure we don't import forbidden crates
    let src_dir = Path::new("src");
    
    let forbidden_crates = vec![
        "runtime_agent",
        "runtime_prompt",
        "runtime_model",
        "runtime_context",
    ];

    for entry in walkdir::WalkDir::new(src_dir) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() && entry.path().extension().map_or(false, |ext| ext == "rs") {
            let content = fs::read_to_string(entry.path()).unwrap();
            
            for forbidden in &forbidden_crates {
                assert!(
                    !content.contains(&format!("use {}::", forbidden)),
                    "Architecture violation: {} imports forbidden crate {}",
                    entry.path().display(),
                    forbidden
                );
            }
        }
    }
}
