use augur_provider_copilot_sdk::actors::copilot::commands::CopilotChatCmd;

#[test]
fn mirrored_surface_smoke_commands() {
    let type_name = core::any::type_name::<CopilotChatCmd>();
    assert!(type_name.contains("CopilotChatCmd"));
}
