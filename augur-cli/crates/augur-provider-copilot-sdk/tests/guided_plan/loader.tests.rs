#[test]
fn guided_plan_loader_verdict_suffix_returns_none_when_missing() {
    let result =
        augur_provider_copilot_sdk::guided_plan::hooks::copilot_agent::check_verdict_suffix(
            "analysis complete but no verdict marker present",
        );
    assert!(result.is_none());
}
