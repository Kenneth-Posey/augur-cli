use std::fs;

#[test]
fn tiers_module_contains_assignment_pipeline_symbols() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/cache/tiers.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("tiers.rs must be readable");

    assert!(source.contains("pub fn assign_tiers"));
    assert!(source.contains("fn compute_depths"));
    assert!(source.contains("fn group_by_depth"));
}
