use std::fs;

#[test]
fn loader_defines_canonical_and_local_paths() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/deterministic_orchestrator/loader.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("loader source must be readable");

    assert!(source.contains("CANONICAL_PLAN_EXECUTION_PATH"));
    assert!(source.contains("LOCAL_PLAN_EXECUTION_PATH"));
}
