use augur_domain::guided_plan::HookOutcome;

#[test]
fn domain_guided_plan_verdict_suffix_extracts_rework_reason() {
    let result =
        augur_provider_copilot_sdk::guided_plan::hooks::copilot_agent::check_verdict_suffix(
            "VERDICT: REWORK(add scenario traceability)",
        );
    match result {
        Some(HookOutcome::NeedsRework(reason)) => {
            assert_eq!(reason.to_string(), "add scenario traceability");
        }
        other => panic!("expected rework verdict, got {other:?}"),
    }
}
