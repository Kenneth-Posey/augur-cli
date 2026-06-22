#[test]
fn workspace_crates_are_available() {
    let openrouter_type = std::any::type_name::<augur_provider_openrouter::actors::LlmHandle>();
    let copilot_type =
        std::any::type_name::<augur_provider_copilot_sdk::actors::CopilotChatHandle>();
    assert!(!openrouter_type.is_empty());
    assert!(!copilot_type.is_empty());
    assert_eq!(augur_tui::provider().to_string(), "tui");
}
