use std::fs;

#[test]
fn artifact_store_defines_resolver_and_path_guards() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/deterministic_orchestrator/artifact_store.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("artifact_store source must be readable");

    assert!(
        source.contains("StepArtifactResolver"),
        "artifact_store must define StepArtifactResolver",
    );
    assert!(
        source.contains("InvalidArtifactPath"),
        "artifact_store must guard against escaping repository root",
    );
}
