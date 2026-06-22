use std::fs;

#[test]
fn supervisor_handle_module_exposes_command_and_subscription_surface() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/supervisor/handle.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("handle.rs must be readable");

    assert!(source.contains("pub struct SupervisorHandle"));
    assert!(source.contains("pub async fn start_plan"));
    assert!(source.contains("pub fn subscribe_events"));
}
