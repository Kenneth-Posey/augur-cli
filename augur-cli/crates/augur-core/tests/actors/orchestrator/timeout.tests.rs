use std::fs;

#[test]
fn timeout_module_exposes_step_and_plan_timeout_handlers() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/orchestrator/timeout.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("timeout.rs must be readable");

    assert!(source.contains("pub fn step_timeout_handler"));
    assert!(source.contains("pub fn plan_timeout_handler"));
}
