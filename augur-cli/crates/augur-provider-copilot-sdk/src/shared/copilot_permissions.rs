//! Shared Copilot SDK permission helpers.

/// Build a permission handler that approves every Copilot SDK permission request.
pub fn allow_all_handler() -> copilot_sdk::PermissionHandler {
    std::sync::Arc::new(|_req: &copilot_sdk::PermissionRequest| {
        copilot_sdk::PermissionRequestResult::approved()
    })
}
