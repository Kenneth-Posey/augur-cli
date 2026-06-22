use std::fs;

#[test]
fn decision_defines_default_failure_policy() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/deterministic_orchestrator/decision.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("decision source must be readable");

    assert!(
        source.contains("DefaultFailureDecisionPolicy"),
        "decision module must define default decision policy",
    );
    assert!(
        source.contains("FailureDecisionPolicy"),
        "decision module must expose replaceable policy boundary",
    );
}
