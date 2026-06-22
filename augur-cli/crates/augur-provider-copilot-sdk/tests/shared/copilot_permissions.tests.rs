#[test]
fn allow_all_handler_always_approves_permission_requests() {
    let handler = augur_provider_copilot_sdk::shared::copilot_permissions::allow_all_handler();
    let request = copilot_sdk::PermissionRequest {
        kind: "tool".to_string(),
        tool_call_id: None,
        extension_data: std::collections::HashMap::new(),
    };
    let decision = handler(&request);
    assert!(decision.is_approved());
}
