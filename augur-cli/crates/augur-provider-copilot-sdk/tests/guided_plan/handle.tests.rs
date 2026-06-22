#[test]
fn guided_plan_handle_timeout_constant_is_five_minutes() {
    use augur_provider_copilot_sdk::guided_plan::hooks::copilot_agent::AGENT_HOOK_TIMEOUT;
    assert_eq!(AGENT_HOOK_TIMEOUT, std::time::Duration::from_secs(300));
}
