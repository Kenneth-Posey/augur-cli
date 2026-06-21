use std::fs;

#[test]
fn ingestion_module_exposes_submission_and_scheduler_surfaces() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/orchestrator/ingestion.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("ingestion.rs must be readable");

    assert!(source.contains("pub fn submit_execution_plan"));
    assert!(source.contains("pub fn drive_scheduler_tick"));
    assert!(source.contains("pub fn handle_step_terminal"));
}
