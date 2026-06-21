use std::fs;

#[test]
fn assistant_core_exposes_turn_processing_symbols() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/agent/assistant_core.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("assistant_core.rs must be readable");

    assert!(source.contains("pub async fn process_turn"));
    assert!(source.contains("async fn consume_stream"));
}
