use std::fs;

#[test]
fn background_dispatch_defines_dispatcher_boundary() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/deterministic_orchestrator/background_dispatch.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("background_dispatch source must be readable");

    assert!(
        source.contains("DeterministicAgentDispatcher"),
        "background_dispatch must expose deterministic dispatcher boundary",
    );
    assert!(
        source.contains("BackgroundAgentRuntime"),
        "background_dispatch must define runtime abstraction trait",
    );
}
