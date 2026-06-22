use std::fs;

#[test]
fn deterministic_orchestrator_actor_exposes_spawn() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/deterministic_orchestrator/deterministic_orchestrator_actor.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("deterministic_orchestrator_actor source must be readable");

    assert!(
        source.contains("pub fn spawn"),
        "deterministic_orchestrator_actor must expose spawn entry point",
    );
}
