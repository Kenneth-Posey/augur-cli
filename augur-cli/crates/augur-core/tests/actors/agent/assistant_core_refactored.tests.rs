use std::path::PathBuf;

#[test]
fn assistant_core_refactored_is_retired_in_favor_of_assistant_core() {
    let retired = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/actors/agent/assistant_core_refactored.rs");
    let active =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/actors/agent/assistant_core.rs");

    assert!(!retired.exists(), "retired refactored module should be absent");
    assert!(active.exists(), "assistant_core.rs should remain the active implementation");
}
