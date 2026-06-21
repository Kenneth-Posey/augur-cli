use std::fs;

#[test]
fn checkpoint_module_exposes_tracker_and_threshold() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/supervisor/checkpoint.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("checkpoint.rs must be readable");

    assert!(source.contains("pub const CHECKPOINT_FILE_THRESHOLD"));
    assert!(source.contains("pub struct CheckpointTracker"));
    assert!(source.contains("pub fn record_file_change"));
}
