use std::fs;

#[test]
fn commands_include_start_and_shutdown_variants() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/deterministic_orchestrator/commands.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("commands source must be readable");

    assert!(source.contains("Start {"));
    assert!(source.contains("Shutdown"));
}
