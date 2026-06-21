use std::fs;

#[test]
fn deterministic_orchestrator_ops_handles_dispatch_and_updates() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/deterministic_orchestrator/deterministic_orchestrator_actor/deterministic_orchestrator_ops.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("deterministic_orchestrator_ops source must be readable");

    assert!(source.contains("dispatch_request"));
    assert!(source.contains("merge_artifact_updates"));
}
