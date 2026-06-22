use augur_graph_builder::module_walker;
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn test_walk_all_crates_empty_paths() {
    let result = module_walker::walk_all_crates(&HashMap::new(), &[]);
    assert!(result.is_empty());
}

#[test]
fn test_walk_all_crates_nonexistent() {
    let mut paths = HashMap::new();
    paths.insert("test-crate".to_string(), PathBuf::from("/nonexistent/path"));
    let result = module_walker::walk_all_crates(&paths, &[]);
    assert!(result.is_empty());
}
